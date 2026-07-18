#![no_std]

//! Basic driver for the CST92xx touch screen that supports both blocking and async I2C access.

pub mod r#async;
pub mod blocking;
pub mod error;
pub mod mode;
pub mod registers;
pub mod types;

pub use r#async::CST92xx;
pub use blocking::BlockingCST92xx;
pub use error::Error;
pub use mode::RunMode;
pub use types::Point;
