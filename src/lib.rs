#![no_std]

//! Basic driver for the CST92xx touch screen that supports both blocking and async I2C access.

#[cfg(all(feature = "async", feature = "blocking"))]
compile_error!(
    "features `async` and `blocking` both export a `CST92xx` type at the crate root and are \
    mutually exclusive; enable exactly one (`default-features = false, features = [\"blocking\"]` \
    for the sync driver, or just `features = [\"async\"]`, which is already the default)."
);

pub mod error;
pub mod info;
pub mod mode;
pub mod registers;
pub mod reset_pin;
pub mod types;

pub use error::Error;
pub use info::{ChipInfo, Point};
pub use mode::RunMode;
pub use reset_pin::NoResetPin;
pub use types::{DisplayMapping, Orientation, TouchConfig};

#[cfg(feature = "async")]
mod r#async;
#[cfg(feature = "blocking")]
mod blocking;

#[cfg(feature = "async")]
pub use r#async::CST92xx;
#[cfg(feature = "blocking")]
pub use blocking::CST92xx;
