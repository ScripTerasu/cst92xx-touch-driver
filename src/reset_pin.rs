use core::convert::Infallible;
use embedded_hal::digital::{ErrorType, OutputPin};

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
