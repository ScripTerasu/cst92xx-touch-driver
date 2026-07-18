use core::convert::Infallible;

use embedded_hal::i2c::Operation;
use embedded_hal_async::i2c::{ErrorType, I2c};
use futures::executor::block_on;

use cst92xx::{CST92xx, registers};

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

    let mut driver = CST92xx::new(DummyI2c::new(&expectations));
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

    let mut driver = CST92xx::new(DummyI2c::new(&expectations));
    let touches = block_on(async { driver.touches().await.unwrap() });
    let point = touches[0].unwrap();
    assert_eq!(point.track_id, 1);
    assert_eq!(point.x, ((0x0Au16) << 4) | 0x05);
    assert_eq!(point.y, ((0x14u16) << 4) | 0x07);
    assert!(touches[1].is_none());
}
