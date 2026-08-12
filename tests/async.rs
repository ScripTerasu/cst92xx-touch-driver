use core::convert::Infallible;

use embedded_hal::i2c::Operation;
use embedded_hal_async::delay::DelayNs;
use embedded_hal_async::i2c::{ErrorType, I2c};
use futures::executor::block_on;

use cst92xx::{CST92xx, Error, RunMode, registers};

struct DummyDelay;

impl DelayNs for DummyDelay {
    async fn delay_ns(&mut self, _ns: u32) {}
}

struct DummyI2c<'a> {
    expected: &'a [ExpectedOperation<'a>],
    position: usize,
}

#[derive(Clone, Copy, Debug)]
enum ExpectedOperation<'a> {
    Write { address: u8, data: &'a [u8] },
    Read { address: u8, data: &'a [u8] },
}

impl<'a> DummyI2c<'a> {
    fn new(expected: &'a [ExpectedOperation<'a>]) -> Self {
        Self {
            expected,
            position: 0,
        }
    }

    fn next(&mut self) -> &'a ExpectedOperation<'a> {
        let op = self.expected.get(self.position).unwrap_or_else(|| {
            panic!(
                "received more I2C operations than expected (overran at index {})",
                self.position
            )
        });
        self.position += 1;
        op
    }
}

impl<'a> Drop for DummyI2c<'a> {
    fn drop(&mut self) {
        assert_eq!(
            self.position,
            self.expected.len(),
            "not all expected I2C operations were consumed"
        );
    }
}

const READ_LEN: usize = registers::MAX_FINGER_NUM * 5 + 5;
const REG_READ_BYTES: [u8; 2] = registers::REG_READ.to_be_bytes();
const READ_REPORT_EMPTY: [u8; READ_LEN] = [0u8; READ_LEN];
const READ_REPORT_POINT: [u8; READ_LEN] = [
    0x16,
    0x0A,
    0x14,
    0x57,
    0,
    1,
    registers::CST92XX_ACK,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
];
const ACK_COMMAND: [u8; 3] = [REG_READ_BYTES[0], REG_READ_BYTES[1], registers::CST92XX_ACK];

const REG_DEBUG_MODE_BYTES: [u8; 2] = registers::REG_DEBUG_MODE.to_be_bytes();
const REG_CHECK_CODE_BYTES: [u8; 2] = registers::REG_CHECK_CODE.to_be_bytes();
const REG_RESOLUTION_BYTES: [u8; 2] = registers::REG_RESOLUTION.to_be_bytes();
const REG_CHIP_TYPE_BYTES: [u8; 2] = registers::REG_CHIP_TYPE.to_be_bytes();
const REG_FW_VERSION_BYTES: [u8; 2] = registers::REG_FW_VERSION.to_be_bytes();
const REG_MODE_HANDSHAKE_BYTES: [u8; 2] = registers::REG_MODE_HANDSHAKE.to_be_bytes();
const REG_MODE_STATUS_BYTES: [u8; 2] = registers::REG_MODE_STATUS.to_be_bytes();

// checkcode = 0xCACA_0000, little-endian.
const CHECK_CODE_VALID: [u8; 4] = [0x00, 0x00, 0xCA, 0xCA];
// checkcode = 0x1234_0000 (high 16 bits don't match the expected 0xCACA marker).
const CHECK_CODE_INVALID: [u8; 4] = [0x00, 0x00, 0x34, 0x12];
// resolution_x = 240, resolution_y = 320, both little-endian u16.
const RESOLUTION_VALID: [u8; 4] = [0xF0, 0x00, 0x40, 0x01];
// project_id = 0x1234, chip_type = CST9217_CHIP_ID (0x9217), little-endian.
const CHIP_TYPE_VALID: [u8; 4] = [0x34, 0x12, 0x17, 0x92];
// project_id = 0x0000, chip_type = 0x1234 (not CST9217/CST9220).
const CHIP_TYPE_UNKNOWN: [u8; 4] = [0x00, 0x00, 0x34, 0x12];
// fw_version = 0x0102_0304, checksum = 0xAABB_CCDD, little-endian.
const FW_VERSION_VALID: [u8; 8] = [0x04, 0x03, 0x02, 0x01, 0xDD, 0xCC, 0xBB, 0xAA];
// fw_version = 0xA5A5_A5A5, the "chip has no firmware" sentinel.
const FW_VERSION_NO_FIRMWARE: [u8; 8] = [0xA5, 0xA5, 0xA5, 0xA5, 0, 0, 0, 0];
// Anything other than the handshake's own low byte (0x1E) counts as "not ready yet".
const MODE_STATUS_NOT_READY: [u8; 4] = [0, 0, 0, 0];

impl<'a> ErrorType for DummyI2c<'a> {
    type Error = Infallible;
}

impl<'a> I2c for DummyI2c<'a> {
    async fn transaction(
        &mut self,
        address: u8,
        operations: &mut [Operation<'_>],
    ) -> Result<(), Self::Error> {
        for operation in operations {
            match operation {
                Operation::Write(write) => match self.next() {
                    ExpectedOperation::Write {
                        address: expected_address,
                        data,
                    } if *expected_address == address && data == write => {
                        // Write matches expectation
                    }
                    other => panic!("unexpected write operation: {other:?}"),
                },
                Operation::Read(read) => match self.next() {
                    ExpectedOperation::Read {
                        address: expected_address,
                        data,
                    } if *expected_address == address => {
                        assert_eq!(read.len(), data.len(), "read length mismatch");
                        read.copy_from_slice(data);
                    }
                    other => panic!("unexpected read operation: {other:?}"),
                },
            }
        }
        Ok(())
    }
}

#[test]
fn touches_empty_report_returns_no_points_async() {
    let expectations = [
        ExpectedOperation::Write {
            address: registers::CST92XX_SLAVE_ADDRESS,
            data: &REG_READ_BYTES,
        },
        ExpectedOperation::Read {
            address: registers::CST92XX_SLAVE_ADDRESS,
            data: &READ_REPORT_EMPTY,
        },
    ];

    let mut driver = CST92xx::new(DummyI2c::new(&expectations), DummyDelay);
    let touches = block_on(async { driver.touches().await.unwrap() });
    assert!(touches.iter().all(|point| point.is_none()));
}

#[test]
fn touches_parses_single_point_async() {
    let expectations = [
        ExpectedOperation::Write {
            address: registers::CST92XX_SLAVE_ADDRESS,
            data: &REG_READ_BYTES,
        },
        ExpectedOperation::Read {
            address: registers::CST92XX_SLAVE_ADDRESS,
            data: &READ_REPORT_POINT,
        },
        ExpectedOperation::Write {
            address: registers::CST92XX_SLAVE_ADDRESS,
            data: &ACK_COMMAND,
        },
    ];

    let mut driver = CST92xx::new(DummyI2c::new(&expectations), DummyDelay);
    let touches = block_on(async { driver.touches().await.unwrap() });
    let point = touches[0].unwrap();
    assert_eq!(point.track_id, 1);
    assert_eq!(point.x, ((0x0Au16) << 4) | 0x05);
    assert_eq!(point.y, ((0x14u16) << 4) | 0x07);
    assert!(touches[1].is_none());
}

// Each `write_read` call decomposes into a Write + Read pair once it goes
// through `transaction()`, which is all `DummyI2c` implements.
fn attribute_expectations(
    fw_version_and_checksum: &'static [u8; 8],
) -> [ExpectedOperation<'static>; 9] {
    [
        ExpectedOperation::Write {
            address: registers::CST92XX_SLAVE_ADDRESS,
            data: &REG_DEBUG_MODE_BYTES,
        },
        ExpectedOperation::Write {
            address: registers::CST92XX_SLAVE_ADDRESS,
            data: &REG_CHECK_CODE_BYTES,
        },
        ExpectedOperation::Read {
            address: registers::CST92XX_SLAVE_ADDRESS,
            data: &CHECK_CODE_VALID,
        },
        ExpectedOperation::Write {
            address: registers::CST92XX_SLAVE_ADDRESS,
            data: &REG_RESOLUTION_BYTES,
        },
        ExpectedOperation::Read {
            address: registers::CST92XX_SLAVE_ADDRESS,
            data: &RESOLUTION_VALID,
        },
        ExpectedOperation::Write {
            address: registers::CST92XX_SLAVE_ADDRESS,
            data: &REG_CHIP_TYPE_BYTES,
        },
        ExpectedOperation::Read {
            address: registers::CST92XX_SLAVE_ADDRESS,
            data: &CHIP_TYPE_VALID,
        },
        ExpectedOperation::Write {
            address: registers::CST92XX_SLAVE_ADDRESS,
            data: &REG_FW_VERSION_BYTES,
        },
        ExpectedOperation::Read {
            address: registers::CST92XX_SLAVE_ADDRESS,
            data: fw_version_and_checksum,
        },
    ]
}

#[test]
fn get_attribute_populates_chip_info_on_success_async() {
    let expectations = attribute_expectations(&FW_VERSION_VALID);

    let mut driver = CST92xx::new(DummyI2c::new(&expectations), DummyDelay);
    block_on(async { driver.get_attribute().await.unwrap() });

    let info = driver.chip_info();
    assert_eq!(info.chip_type, registers::CST9217_CHIP_ID);
    assert_eq!(info.resolution_x, 240);
    assert_eq!(info.resolution_y, 320);
    assert_eq!(info.project_id, 0x1234);
    assert_eq!(info.fw_version, 0x0102_0304);
    assert_eq!(info.checksum, 0xAABB_CCDD);
    assert_eq!(driver.model_name(), "CST9217");
}

#[test]
fn get_attribute_rejects_missing_firmware_async() {
    let expectations = attribute_expectations(&FW_VERSION_NO_FIRMWARE);

    let mut driver = CST92xx::new(DummyI2c::new(&expectations), DummyDelay);
    let result = block_on(async { driver.get_attribute().await });
    assert!(matches!(result, Err(Error::InvalidFirmware)));
}

#[test]
fn get_attribute_rejects_bad_checkcode_async() {
    let mut expectations = attribute_expectations(&FW_VERSION_VALID);
    expectations[2] = ExpectedOperation::Read {
        address: registers::CST92XX_SLAVE_ADDRESS,
        data: &CHECK_CODE_INVALID,
    };

    let mut driver = CST92xx::new(DummyI2c::new(&expectations), DummyDelay);
    let result = block_on(async { driver.get_attribute().await });
    assert!(matches!(result, Err(Error::InvalidCheckCode)));
}

#[test]
fn get_attribute_rejects_unknown_chip_type_async() {
    let mut expectations = attribute_expectations(&FW_VERSION_VALID);
    expectations[6] = ExpectedOperation::Read {
        address: registers::CST92XX_SLAVE_ADDRESS,
        data: &CHIP_TYPE_UNKNOWN,
    };

    let mut driver = CST92xx::new(DummyI2c::new(&expectations), DummyDelay);
    let result = block_on(async { driver.get_attribute().await });
    assert!(matches!(result, Err(Error::InvalidChipType(0x1234))));
}

#[test]
fn set_mode_returns_not_ready_when_handshake_never_acks_async() {
    let handshake_round = [
        ExpectedOperation::Write {
            address: registers::CST92XX_SLAVE_ADDRESS,
            data: &REG_MODE_HANDSHAKE_BYTES,
        },
        ExpectedOperation::Write {
            address: registers::CST92XX_SLAVE_ADDRESS,
            data: &REG_MODE_HANDSHAKE_BYTES,
        },
        ExpectedOperation::Write {
            address: registers::CST92XX_SLAVE_ADDRESS,
            data: &REG_MODE_STATUS_BYTES,
        },
        ExpectedOperation::Read {
            address: registers::CST92XX_SLAVE_ADDRESS,
            data: &MODE_STATUS_NOT_READY,
        },
    ];
    let expectations = [
        handshake_round[0],
        handshake_round[1],
        handshake_round[2],
        handshake_round[3],
        handshake_round[0],
        handshake_round[1],
        handshake_round[2],
        handshake_round[3],
        handshake_round[0],
        handshake_round[1],
        handshake_round[2],
        handshake_round[3],
    ];

    let mut driver = CST92xx::new(DummyI2c::new(&expectations), DummyDelay);
    let result = block_on(async { driver.set_mode(RunMode::Normal).await });
    assert!(matches!(result, Err(Error::NotReady)));
}
