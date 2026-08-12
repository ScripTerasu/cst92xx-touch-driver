#![no_std]

//! A `no_std` driver for the CST92xx family of capacitive touch controllers
//! (CST9217, CST9220), ported from [SensorLib's `TouchDrvCST92xx`][sensorlib]
//! C++ driver to idiomatic `embedded-hal`.
//!
//! The crate exposes one `CST92xx` type backed by either `embedded-hal-async`
//! or blocking `embedded-hal` I²C, selected via Cargo features:
//!
//! | Feature | Default | Effect |
//! | --- | --- | --- |
//! | `async` | yes | `CST92xx::new(i2c)` over `embedded_hal_async::i2c::I2c`, plus an optional reset pin via `.with_reset()`. |
//! | `blocking` | no | `CST92xx::new(i2c, delay)` over `embedded_hal::i2c::I2c` + `DelayNs`, plus an optional reset pin. |
//! | `defmt` | no | Derives `defmt::Format` on the public types for logging. |
//!
//! `async` and `blocking` both export a `CST92xx` type at the crate root and
//! are mutually exclusive — enabling both is a compile error. Use
//! `default-features = false, features = ["blocking"]` to switch to the sync
//! driver.
//!
//! ```rust,ignore
//! use cst92xx::{CST92xx, RunMode};
//!
//! let mut driver = CST92xx::new(i2c); // add `.with_reset(rst_pin)` if wired up
//! driver.init().await?;
//!
//! for point in driver.touches().await?.iter().flatten() {
//!     // point.track_id, point.x, point.y
//! }
//! ```
//!
//! See the [repository README](https://github.com/ScripTerasu/cst92xx-touch-driver)
//! for complete async and blocking examples, wiring notes for the reference
//! hardware, and the list of `RunMode` variants that are unverified against
//! real hardware.
//!
//! [sensorlib]: https://github.com/lewisxhe/SensorLib/blob/baa3e0b83c256b74d9870a95d96d55595946926c/src/touch/TouchDrvCST92xx.cpp

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

#[cfg(any(feature = "async", feature = "blocking"))]
mod protocol;

#[cfg(feature = "async")]
mod r#async;
#[cfg(feature = "blocking")]
mod blocking;

#[cfg(feature = "async")]
pub use r#async::CST92xx;
#[cfg(feature = "blocking")]
pub use blocking::CST92xx;
