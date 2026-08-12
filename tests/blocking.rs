use core::convert::Infallible;
use embedded_hal::delay::DelayNs;

use cst92xx::{CST92xx, registers};

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

    let (i2c, _) = driver.into_inner();
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

    let (i2c, _) = driver.into_inner();
    i2c.assert_done();
}
