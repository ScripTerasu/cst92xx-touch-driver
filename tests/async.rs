use embedded_hal::i2c::ErrorKind;
use embedded_hal_mock::eh1::delay::NoopDelay;
use embedded_hal_mock::eh1::i2c::{Mock as I2cMock, Transaction as I2cTransaction};
use futures::executor::block_on;

use cst92xx::{CST92xx, Error, RunMode, registers};

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
const REG_FACTORY_MODE_BYTES: [u8; 2] = registers::REG_FACTORY_MODE.to_be_bytes();
const REG_FACTORY_STATUS_BYTES: [u8; 2] = registers::REG_FACTORY_STATUS.to_be_bytes();
const REG_FACTORY_READY_BYTES: [u8; 2] = registers::REG_FACTORY_READY.to_be_bytes();

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
// read_buffer[1] == REG_MODE_HANDSHAKE's low byte (0x1E): the handshake succeeded.
const MODE_HANDSHAKE_READY: [u8; 4] = [0, 0x1E, 0, 0];
// Factory status byte other than 0x14: not ready yet.
const FACTORY_STATUS_NOT_READY: [u8; 1] = [0x00];
// Factory status byte 0x14: ready.
const FACTORY_STATUS_READY: [u8; 1] = [0x14];
// status[1] == REG_FACTORY_READY's low byte (0x19): the factory command was accepted.
const FACTORY_MODE_CONFIRMED: [u8; 2] = [0, 0x19];

const ADDR: u8 = registers::CST92XX_SLAVE_ADDRESS;

#[test]
fn touches_empty_report_returns_no_points_async() {
    let expectations = [I2cTransaction::write_read(
        ADDR,
        REG_READ_BYTES.to_vec(),
        READ_REPORT_EMPTY.to_vec(),
    )];
    let mut i2c = I2cMock::new(&expectations);

    let mut driver = CST92xx::new(i2c.clone(), NoopDelay::new());
    let touches = block_on(async { driver.touches().await.unwrap() });
    assert!(touches.iter().all(|point| point.is_none()));

    i2c.done();
}

#[test]
fn touches_parses_single_point_async() {
    let expectations = [
        I2cTransaction::write_read(ADDR, REG_READ_BYTES.to_vec(), READ_REPORT_POINT.to_vec()),
        I2cTransaction::write(ADDR, ACK_COMMAND.to_vec()),
    ];
    let mut i2c = I2cMock::new(&expectations);

    let mut driver = CST92xx::new(i2c.clone(), NoopDelay::new());
    let touches = block_on(async { driver.touches().await.unwrap() });
    let point = touches[0].unwrap();
    assert_eq!(point.track_id, 1);
    assert_eq!(point.x, ((0x0Au16) << 4) | 0x05);
    assert_eq!(point.y, ((0x14u16) << 4) | 0x07);
    assert!(touches[1].is_none());

    i2c.done();
}

fn attribute_expectations(fw_version_and_checksum: &'static [u8; 8]) -> [I2cTransaction; 5] {
    [
        I2cTransaction::write(ADDR, REG_DEBUG_MODE_BYTES.to_vec()),
        I2cTransaction::write_read(
            ADDR,
            REG_CHECK_CODE_BYTES.to_vec(),
            CHECK_CODE_VALID.to_vec(),
        ),
        I2cTransaction::write_read(
            ADDR,
            REG_RESOLUTION_BYTES.to_vec(),
            RESOLUTION_VALID.to_vec(),
        ),
        I2cTransaction::write_read(ADDR, REG_CHIP_TYPE_BYTES.to_vec(), CHIP_TYPE_VALID.to_vec()),
        I2cTransaction::write_read(
            ADDR,
            REG_FW_VERSION_BYTES.to_vec(),
            fw_version_and_checksum.to_vec(),
        ),
    ]
}

#[test]
fn get_attribute_populates_chip_info_on_success_async() {
    let expectations = attribute_expectations(&FW_VERSION_VALID);
    let mut i2c = I2cMock::new(&expectations);

    let mut driver = CST92xx::new(i2c.clone(), NoopDelay::new());
    block_on(async { driver.get_attribute().await.unwrap() });

    let info = driver.chip_info();
    assert_eq!(info.chip_type, registers::CST9217_CHIP_ID);
    assert_eq!(info.resolution_x, 240);
    assert_eq!(info.resolution_y, 320);
    assert_eq!(info.project_id, 0x1234);
    assert_eq!(info.fw_version, 0x0102_0304);
    assert_eq!(info.checksum, 0xAABB_CCDD);
    assert_eq!(driver.model_name(), "CST9217");

    i2c.done();
}

#[test]
fn get_attribute_rejects_missing_firmware_async() {
    let expectations = attribute_expectations(&FW_VERSION_NO_FIRMWARE);
    let mut i2c = I2cMock::new(&expectations);

    let mut driver = CST92xx::new(i2c.clone(), NoopDelay::new());
    let result = block_on(async { driver.get_attribute().await });
    assert!(matches!(result, Err(Error::InvalidFirmware)));

    i2c.done();
}

#[test]
fn get_attribute_rejects_bad_checkcode_async() {
    let mut expectations = attribute_expectations(&FW_VERSION_VALID);
    expectations[1] = I2cTransaction::write_read(
        ADDR,
        REG_CHECK_CODE_BYTES.to_vec(),
        CHECK_CODE_INVALID.to_vec(),
    );
    let mut i2c = I2cMock::new(&expectations);

    let mut driver = CST92xx::new(i2c.clone(), NoopDelay::new());
    let result = block_on(async { driver.get_attribute().await });
    assert!(matches!(result, Err(Error::InvalidCheckCode)));

    i2c.done();
}

#[test]
fn get_attribute_rejects_unknown_chip_type_async() {
    let mut expectations = attribute_expectations(&FW_VERSION_VALID);
    expectations[3] = I2cTransaction::write_read(
        ADDR,
        REG_CHIP_TYPE_BYTES.to_vec(),
        CHIP_TYPE_UNKNOWN.to_vec(),
    );
    let mut i2c = I2cMock::new(&expectations);

    let mut driver = CST92xx::new(i2c.clone(), NoopDelay::new());
    let result = block_on(async { driver.get_attribute().await });
    assert!(matches!(result, Err(Error::InvalidChipType(0x1234))));

    i2c.done();
}

#[test]
fn set_mode_returns_not_ready_when_handshake_never_acks_async() {
    let handshake_round = [
        I2cTransaction::write(ADDR, REG_MODE_HANDSHAKE_BYTES.to_vec()),
        I2cTransaction::write(ADDR, REG_MODE_HANDSHAKE_BYTES.to_vec()),
        I2cTransaction::write_read(
            ADDR,
            REG_MODE_STATUS_BYTES.to_vec(),
            MODE_STATUS_NOT_READY.to_vec(),
        ),
    ];
    let expectations: Vec<_> = handshake_round.iter().cloned().cycle().take(9).collect();
    let mut i2c = I2cMock::new(&expectations);

    let mut driver = CST92xx::new(i2c.clone(), NoopDelay::new());
    let result = block_on(async { driver.set_mode(RunMode::Normal).await });
    assert!(matches!(result, Err(Error::NotReady)));

    i2c.done();
}

#[test]
fn set_mode_factory_succeeds_after_polling_retries_async() {
    let expectations = [
        // Outer mode handshake succeeds on the first attempt.
        I2cTransaction::write(ADDR, REG_MODE_HANDSHAKE_BYTES.to_vec()),
        I2cTransaction::write(ADDR, REG_MODE_HANDSHAKE_BYTES.to_vec()),
        I2cTransaction::write_read(
            ADDR,
            REG_MODE_STATUS_BYTES.to_vec(),
            MODE_HANDSHAKE_READY.to_vec(),
        ),
        // prepare_factory_mode: not ready on the first poll...
        I2cTransaction::write(ADDR, REG_FACTORY_MODE_BYTES.to_vec()),
        I2cTransaction::write_read(
            ADDR,
            REG_FACTORY_STATUS_BYTES.to_vec(),
            FACTORY_STATUS_NOT_READY.to_vec(),
        ),
        // ...ready on the second.
        I2cTransaction::write(ADDR, REG_FACTORY_MODE_BYTES.to_vec()),
        I2cTransaction::write_read(
            ADDR,
            REG_FACTORY_STATUS_BYTES.to_vec(),
            FACTORY_STATUS_READY.to_vec(),
        ),
        // set_mode writes the factory-ready command and confirms it.
        I2cTransaction::write(ADDR, REG_FACTORY_READY_BYTES.to_vec()),
        I2cTransaction::write_read(
            ADDR,
            REG_MODE_STATUS_BYTES.to_vec(),
            FACTORY_MODE_CONFIRMED.to_vec(),
        ),
    ];
    let mut i2c = I2cMock::new(&expectations);

    let mut driver = CST92xx::new(i2c.clone(), NoopDelay::new());
    block_on(async { driver.set_mode(RunMode::Factory).await.unwrap() });

    i2c.done();
}

#[test]
fn set_mode_factory_propagates_i2c_error_when_polling_never_succeeds_async() {
    let mut expectations = vec![
        // Outer mode handshake succeeds on the first attempt.
        I2cTransaction::write(ADDR, REG_MODE_HANDSHAKE_BYTES.to_vec()),
        I2cTransaction::write(ADDR, REG_MODE_HANDSHAKE_BYTES.to_vec()),
        I2cTransaction::write_read(
            ADDR,
            REG_MODE_STATUS_BYTES.to_vec(),
            MODE_HANDSHAKE_READY.to_vec(),
        ),
    ];
    // prepare_factory_mode retries 10 times; every write to REG_FACTORY_MODE fails.
    for _ in 0..10 {
        expectations.push(
            I2cTransaction::write(ADDR, REG_FACTORY_MODE_BYTES.to_vec())
                .with_error(ErrorKind::Other),
        );
    }
    let mut i2c = I2cMock::new(&expectations);

    let mut driver = CST92xx::new(i2c.clone(), NoopDelay::new());
    let result = block_on(async { driver.set_mode(RunMode::Factory).await });
    assert!(matches!(result, Err(Error::I2C(ErrorKind::Other))));

    i2c.done();
}
