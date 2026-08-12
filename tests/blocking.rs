use core::convert::Infallible;
use embedded_hal::delay::DelayNs;

use cst92xx::{CST92xx, Error, RunMode, registers};

struct DummyDelay;

impl DelayNs for DummyDelay {
    fn delay_ns(&mut self, _ns: u32) {}
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

#[derive(Clone, Copy, Debug)]
enum Expected<'a> {
    Write {
        address: u8,
        data: &'a [u8],
    },
    WriteRead {
        address: u8,
        write: &'a [u8],
        read: &'a [u8],
    },
}

struct FakeI2c<'a> {
    expected: &'a [Expected<'a>],
    position: usize,
}

impl<'a> FakeI2c<'a> {
    fn new(expected: &'a [Expected<'a>]) -> Self {
        Self {
            expected,
            position: 0,
        }
    }

    fn next(&mut self) -> &Expected<'a> {
        let expected = self.expected.get(self.position).unwrap_or_else(|| {
            panic!(
                "received more I2C operations than expected (overran at index {})",
                self.position
            )
        });
        self.position += 1;
        expected
    }

    fn assert_done(&self) {
        assert_eq!(
            self.position,
            self.expected.len(),
            "not all expected I2C operations were consumed"
        );
    }
}

fn slice_eq(expected: &[u8], actual: &[u8]) -> bool {
    expected.len() == actual.len()
        && expected
            .iter()
            .zip(actual.iter())
            .all(|(expected_byte, actual_byte)| expected_byte == actual_byte)
}

impl embedded_hal::i2c::ErrorType for FakeI2c<'_> {
    type Error = Infallible;
}

impl embedded_hal::i2c::I2c for FakeI2c<'_> {
    fn read(&mut self, _address: u8, _read: &mut [u8]) -> Result<(), Self::Error> {
        unreachable!("read() is not invoked by the driver");
    }

    fn write(&mut self, address: u8, write: &[u8]) -> Result<(), Self::Error> {
        match self.next() {
            Expected::Write {
                address: expected_address,
                data,
            } if *expected_address == address && slice_eq(data, write) => Ok(()),
            other => panic!("unexpected write operation: {other:?}"),
        }
    }

    fn write_read(
        &mut self,
        address: u8,
        write: &[u8],
        read: &mut [u8],
    ) -> Result<(), Self::Error> {
        match self.next() {
            Expected::WriteRead {
                address: expected_address,
                write: expected_write,
                read: expected_read,
            } if *expected_address == address && slice_eq(expected_write, write) => {
                assert_eq!(
                    read.len(),
                    expected_read.len(),
                    "read buffer length mismatch"
                );
                read.copy_from_slice(expected_read);
                Ok(())
            }
            other => panic!("unexpected write_read operation: {other:?}"),
        }
    }

    fn transaction(
        &mut self,
        _address: u8,
        _operations: &mut [embedded_hal::i2c::Operation<'_>],
    ) -> Result<(), Self::Error> {
        unreachable!("transaction() is not invoked by the driver");
    }
}

#[test]
fn touches_empty_report_returns_no_points() {
    let expectations = [Expected::WriteRead {
        address: registers::CST92XX_SLAVE_ADDRESS,
        write: &REG_READ_BYTES,
        read: &READ_REPORT_EMPTY,
    }];

    let mut driver = CST92xx::new(FakeI2c::new(&expectations), DummyDelay);
    let touches = driver.touches().unwrap();
    assert!(touches.iter().all(|point| point.is_none()));

    let (i2c, _, _) = driver.into_inner();
    i2c.assert_done();
}

#[test]
fn touches_parses_single_point() {
    let expectations = [
        Expected::WriteRead {
            address: registers::CST92XX_SLAVE_ADDRESS,
            write: &REG_READ_BYTES,
            read: &READ_REPORT_POINT,
        },
        Expected::Write {
            address: registers::CST92XX_SLAVE_ADDRESS,
            data: &ACK_COMMAND,
        },
    ];

    let mut driver = CST92xx::new(FakeI2c::new(&expectations), DummyDelay);
    let touches = driver.touches().unwrap();
    let point = touches[0].unwrap();
    assert_eq!(point.track_id, 1);
    assert_eq!(point.x, ((0x0Au16) << 4) | 0x05);
    assert_eq!(point.y, ((0x14u16) << 4) | 0x07);
    assert!(touches[1].is_none());

    let (i2c, _, _) = driver.into_inner();
    i2c.assert_done();
}

fn attribute_expectations(fw_version_and_checksum: &'static [u8; 8]) -> [Expected<'static>; 5] {
    [
        Expected::Write {
            address: registers::CST92XX_SLAVE_ADDRESS,
            data: &REG_DEBUG_MODE_BYTES,
        },
        Expected::WriteRead {
            address: registers::CST92XX_SLAVE_ADDRESS,
            write: &REG_CHECK_CODE_BYTES,
            read: &CHECK_CODE_VALID,
        },
        Expected::WriteRead {
            address: registers::CST92XX_SLAVE_ADDRESS,
            write: &REG_RESOLUTION_BYTES,
            read: &RESOLUTION_VALID,
        },
        Expected::WriteRead {
            address: registers::CST92XX_SLAVE_ADDRESS,
            write: &REG_CHIP_TYPE_BYTES,
            read: &CHIP_TYPE_VALID,
        },
        Expected::WriteRead {
            address: registers::CST92XX_SLAVE_ADDRESS,
            write: &REG_FW_VERSION_BYTES,
            read: fw_version_and_checksum,
        },
    ]
}

#[test]
fn get_attribute_populates_chip_info_on_success() {
    let expectations = attribute_expectations(&FW_VERSION_VALID);

    let mut driver = CST92xx::new(FakeI2c::new(&expectations), DummyDelay);
    driver.get_attribute().unwrap();

    let info = driver.chip_info();
    assert_eq!(info.chip_type, registers::CST9217_CHIP_ID);
    assert_eq!(info.resolution_x, 240);
    assert_eq!(info.resolution_y, 320);
    assert_eq!(info.project_id, 0x1234);
    assert_eq!(info.fw_version, 0x0102_0304);
    assert_eq!(info.checksum, 0xAABB_CCDD);
    assert_eq!(driver.model_name(), "CST9217");

    let (i2c, _, _) = driver.into_inner();
    i2c.assert_done();
}

#[test]
fn get_attribute_rejects_missing_firmware() {
    let expectations = attribute_expectations(&FW_VERSION_NO_FIRMWARE);

    let mut driver = CST92xx::new(FakeI2c::new(&expectations), DummyDelay);
    let result = driver.get_attribute();
    assert!(matches!(result, Err(Error::InvalidFirmware)));
}

#[test]
fn get_attribute_rejects_bad_checkcode() {
    let mut expectations = attribute_expectations(&FW_VERSION_VALID);
    expectations[1] = Expected::WriteRead {
        address: registers::CST92XX_SLAVE_ADDRESS,
        write: &REG_CHECK_CODE_BYTES,
        read: &CHECK_CODE_INVALID,
    };

    let mut driver = CST92xx::new(FakeI2c::new(&expectations), DummyDelay);
    let result = driver.get_attribute();
    assert!(matches!(result, Err(Error::InvalidCheckCode)));
}

#[test]
fn get_attribute_rejects_unknown_chip_type() {
    let mut expectations = attribute_expectations(&FW_VERSION_VALID);
    expectations[3] = Expected::WriteRead {
        address: registers::CST92XX_SLAVE_ADDRESS,
        write: &REG_CHIP_TYPE_BYTES,
        read: &CHIP_TYPE_UNKNOWN,
    };

    let mut driver = CST92xx::new(FakeI2c::new(&expectations), DummyDelay);
    let result = driver.get_attribute();
    assert!(matches!(result, Err(Error::InvalidChipType(0x1234))));
}

#[test]
fn set_mode_returns_not_ready_when_handshake_never_acks() {
    let handshake_round = [
        Expected::Write {
            address: registers::CST92XX_SLAVE_ADDRESS,
            data: &REG_MODE_HANDSHAKE_BYTES,
        },
        Expected::Write {
            address: registers::CST92XX_SLAVE_ADDRESS,
            data: &REG_MODE_HANDSHAKE_BYTES,
        },
        Expected::WriteRead {
            address: registers::CST92XX_SLAVE_ADDRESS,
            write: &REG_MODE_STATUS_BYTES,
            read: &MODE_STATUS_NOT_READY,
        },
    ];
    let expectations = [
        handshake_round[0],
        handshake_round[1],
        handshake_round[2],
        handshake_round[0],
        handshake_round[1],
        handshake_round[2],
        handshake_round[0],
        handshake_round[1],
        handshake_round[2],
    ];

    let mut driver = CST92xx::new(FakeI2c::new(&expectations), DummyDelay);
    let result = driver.set_mode(RunMode::Normal);
    assert!(matches!(result, Err(Error::NotReady)));

    let (i2c, _, _) = driver.into_inner();
    i2c.assert_done();
}
