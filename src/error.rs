/// Errors emitted by the CST9217 driver.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone)]
pub enum Error<E> {
    /// `get_attribute()` read a firmware version of `0xA5A5A5A5`, which
    /// SensorLib treats as "chip has no firmware flashed".
    InvalidFirmware,
    /// `get_attribute()`'s checkcode didn't match the expected `0xCACA____`
    /// pattern, indicating a garbled or unsupported attribute read.
    InvalidCheckCode,
    /// `get_attribute()` read a chip type that isn't [`crate::registers::CST9217_CHIP_ID`]
    /// or [`crate::registers::CST9220_CHIP_ID`]. Carries the chip type that was read.
    InvalidChipType(u16),
    /// A low-level I2C error.
    I2C(E),
    /// No new data is available.
    NotReady,
}
