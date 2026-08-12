use core::convert::Infallible;
use embedded_hal::digital::{ErrorType, OutputPin};

/// Default reset pin used when no hardware `RST` line is attached.
///
/// Both drivers use this as their `RST` type parameter until you call
/// `.with_reset()`. `set_low`/`set_high` are no-ops, so `reset()` still runs
/// its settle delay but never actually toggles a pin — equivalent to relying
/// on power-on reset alone.
#[derive(Debug, Default)]
pub struct NoResetPin;

impl ErrorType for NoResetPin {
    type Error = Infallible;
}

impl OutputPin for NoResetPin {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    fn set_high(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
