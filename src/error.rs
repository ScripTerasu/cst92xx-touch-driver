/// Errors emitted by the CST9217 driver.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone)]
pub enum Error<E> {
    /// The connected device did not expose a supported chip identifier.
    UnexpectedChipId,

    InvalidFirmware,
    InvalidCheckCode,
    InvalidChipType(u16),
    /// A low-level I2C error.
    I2C(E),
    /// No new data is available.
    NotReady,
}
