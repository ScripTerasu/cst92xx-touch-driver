//! Shared `CST92xx` driver body for both the `async` and `blocking` features.
//!
//! `async` and `blocking` are mutually exclusive (enforced in `lib.rs`), so of the
//! two `maybe_async!`/`maybe_await!` definitions below, only the one matching the
//! active feature is ever compiled. Each rewrites the driver body written once
//! here — written in blocking style, as plain `fn`s with no `.await` — into the
//! async or sync version. That means an orchestration fix (retry counts, delay
//! ordering, error propagation, like the one in `set_mode()`'s handshake loop)
//! only has to be made in one place instead of drifting between two
//! hand-mirrored files, which is what happened with the reset pin before
//! `protocol.rs` split out the pure decode/validation logic. This module covers
//! the I/O orchestration `protocol.rs` deliberately leaves out.

#[cfg(feature = "blocking")]
use embedded_hal::delay::DelayNs;
#[cfg(feature = "blocking")]
use embedded_hal::i2c::I2c;
#[cfg(feature = "async")]
use embedded_hal_async::delay::DelayNs;
#[cfg(feature = "async")]
use embedded_hal_async::i2c::I2c;

use embedded_hal::digital::OutputPin;

use crate::error::Error;
use crate::info::{ChipInfo, Point};
use crate::mode::RunMode;
use crate::protocol::{self, AttributeError, ModeBytes};
use crate::registers::{
    CST92XX_ACK, CST92XX_SLAVE_ADDRESS, MAX_FINGER_NUM, REG_CHECK_CODE, REG_CHIP_TYPE,
    REG_DEBUG_MODE, REG_FACTORY_MODE, REG_FACTORY_READY, REG_FACTORY_STATUS, REG_FW_VERSION,
    REG_MODE_HANDSHAKE, REG_MODE_STATUS, REG_READ, REG_RESOLUTION, REG_SLEEP_MODE,
};
use crate::reset_pin::NoResetPin;
use crate::types::TouchConfig;

/// Turns a `fn`/`pub fn` item written below into `async fn`/`pub async fn`.
#[cfg(feature = "async")]
macro_rules! maybe_async {
    ($(#[$attr:meta])* pub fn $($rest:tt)*) => { $(#[$attr])* pub async fn $($rest)* };
    ($(#[$attr:meta])* fn $($rest:tt)*) => { $(#[$attr])* async fn $($rest)* };
}
/// Leaves a `fn`/`pub fn` item written below untouched.
#[cfg(feature = "blocking")]
macro_rules! maybe_async {
    ($(#[$attr:meta])* pub fn $($rest:tt)*) => { $(#[$attr])* pub fn $($rest)* };
    ($(#[$attr:meta])* fn $($rest:tt)*) => { $(#[$attr])* fn $($rest)* };
}

/// Appends `.await` to `$e`.
#[cfg(feature = "async")]
macro_rules! maybe_await {
    ($e:expr) => {
        $e.await
    };
}
/// Evaluates `$e` as-is, with no `.await`.
#[cfg(feature = "blocking")]
macro_rules! maybe_await {
    ($e:expr) => {
        $e
    };
}

/// CST92xx controller driver, generic over blocking `embedded-hal` or
/// `embedded-hal-async` I²C depending on which of the `async`/`blocking`
/// features is enabled (see the crate-level docs).
pub struct CST92xx<I2C, DELAY, RST = NoResetPin> {
    i2c: I2C,
    delay: DELAY,
    rst: RST,
    config: TouchConfig,
    chip_info: ChipInfo,
}

impl<I2C, E, DELAY> CST92xx<I2C, DELAY, NoResetPin>
where
    I2C: I2c<Error = E>,
    DELAY: DelayNs,
{
    /// Create a new driver without a dedicated reset pin.
    ///
    /// `i2c` must provide exclusive ownership of the bus and implement the 7-bit
    /// slave address the CST92xx controller listens on. `delay` is used for
    /// reset/mode timing. Use `.with_reset()` to attach a real `RST` line if one
    /// is wired up.
    pub fn new(i2c: I2C, delay: DELAY) -> Self {
        Self {
            i2c,
            delay,
            rst: NoResetPin,
            config: TouchConfig::default(),
            chip_info: ChipInfo::default(),
        }
    }
}

impl<I2C, E, DELAY, RST> CST92xx<I2C, DELAY, RST>
where
    I2C: I2c<Error = E>,
    DELAY: DelayNs,
    RST: OutputPin,
{
    /// Attach a hardware reset pin, replacing the no-op default.
    pub fn with_reset<RST2: OutputPin>(self, rst: RST2) -> CST92xx<I2C, DELAY, RST2> {
        CST92xx {
            i2c: self.i2c,
            delay: self.delay,
            rst,
            config: self.config,
            chip_info: self.chip_info,
        }
    }

    /// Override the touch coordinate transform (orientation/display mapping).
    pub fn with_config(mut self, config: TouchConfig) -> Self {
        self.config = config;
        self
    }

    /// Take ownership of the I2C bus, delay provider, and reset pin.
    ///
    /// Useful when you want to re-use the bus for other devices after the driver is dropped.
    pub fn into_inner(self) -> (I2C, DELAY, RST) {
        (self.i2c, self.delay, self.rst)
    }

    maybe_async! {
        /// Initialize the controller (reset + attribute read) and ensure a supported chip is present.
        ///
        /// Mirrors `TouchDrvCST92xx::initImpl()`, which just calls `getAttribute()` — the
        /// reset pulse itself happens inside `get_attribute()`, matching SensorLib.
        pub fn init(&mut self) -> Result<(), Error<E>> {
            maybe_await!(self.get_attribute())?;

            #[cfg(feature = "defmt")]
            defmt::debug!("Touch type:{}", self.model_name());
            Ok(())
        }
    }

    maybe_async! {
        /// Pulse the reset pin (if any) and wait for the controller to come back up.
        ///
        /// With the default `NoResetPin` this is just the settle delay; with a real `RST`
        /// pin attached via `.with_reset()`, the pin is pulsed low first. Timing is from
        /// the CST9217 datasheet (section 10.5, "上电/复位"): `TRST` (reset pulse width) is
        /// 0.1 ms typical, and `TRON` (chip reinitialization time after reset) is 100 ms
        /// typical — the 10 ms low pulse comfortably clears `TRST`, and the 100 ms settle
        /// after releasing it matches `TRON`.
        pub fn reset(&mut self) {
            let _ = self.rst.set_low();
            maybe_await!(self.delay.delay_ms(10));
            let _ = self.rst.set_high();
            maybe_await!(self.delay.delay_ms(100));
        }
    }

    maybe_async! {
        /// Read controller metadata (checkcode, resolution, chip/version) and validate the chip.
        ///
        /// Runs the same reset + attribute reads as SensorLib's `getAttribute()` (`0xD1/0xD2`)
        /// and caches the result, retrievable via `chip_info()`.
        pub fn get_attribute(&mut self) -> Result<(), Error<E>> {
            maybe_await!(self.reset());

            let mut buffer = [0u8; 8];
            // Enter command mode: this is the same register as `REG_DEBUG_MODE`.
            maybe_await!(self.write(&REG_DEBUG_MODE.to_be_bytes()))?;
            maybe_await!(self.delay.delay_ms(10));

            maybe_await!(self.write_read(&REG_CHECK_CODE.to_be_bytes(), &mut buffer[..4]))?;
            let check_code = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);

            #[cfg(feature = "defmt")]
            defmt::info!("Chip checkcode: {=u32:#010X}", check_code);

            maybe_await!(self.write_read(&REG_RESOLUTION.to_be_bytes(), &mut buffer[..4]))?;
            self.chip_info.resolution_x = u16::from_le_bytes([buffer[0], buffer[1]]);
            self.chip_info.resolution_y = u16::from_le_bytes([buffer[2], buffer[3]]);

            #[cfg(feature = "defmt")]
            defmt::info!(
                "Chip resolution X={=u16} Y={=u16}",
                self.chip_info.resolution_x,
                self.chip_info.resolution_y
            );

            maybe_await!(self.write_read(&REG_CHIP_TYPE.to_be_bytes(), &mut buffer[..4]))?;
            self.chip_info.chip_type =
                (u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) >> 16) as u16;
            self.chip_info.project_id = u32::from(u16::from_le_bytes([buffer[0], buffer[1]]));

            #[cfg(feature = "defmt")]
            defmt::info!(
                "Chip type={=u16:#06X}, Project ID={=u32:#010X}",
                self.chip_info.chip_type,
                self.chip_info.project_id
            );

            maybe_await!(self.write_read(&REG_FW_VERSION.to_be_bytes(), &mut buffer[..8]))?;
            self.chip_info.fw_version =
                u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);
            self.chip_info.checksum =
                u32::from_le_bytes([buffer[4], buffer[5], buffer[6], buffer[7]]);

            #[cfg(feature = "defmt")]
            defmt::info!(
                "Chip IC version={=u32:#010X}, checksum={=u32:#010X}",
                self.chip_info.fw_version,
                self.chip_info.checksum
            );

            protocol::validate_chip_info(check_code, &self.chip_info).map_err(|err| match err {
                AttributeError::Firmware => {
                    #[cfg(feature = "defmt")]
                    defmt::error!("Chip doesn't have firmware.");
                    Error::InvalidFirmware
                }
                AttributeError::CheckCode => {
                    #[cfg(feature = "defmt")]
                    defmt::error!("Firmware info read error.");
                    Error::InvalidCheckCode
                }
                AttributeError::ChipType(chip_type) => {
                    #[cfg(feature = "defmt")]
                    defmt::error!("Unsupported chip type: {=u16:#06X}", chip_type);
                    Error::InvalidChipType(chip_type)
                }
            })
        }
    }

    /// Chip metadata discovered by the last successful `get_attribute()`/`init()` call.
    pub fn chip_info(&self) -> ChipInfo {
        self.chip_info
    }

    maybe_async! {
        /// Request the controller to enter sleep, mirroring SensorLib's `sleep()`.
        ///
        /// This helper switches into `DebugInfo` mode before issuing `REG_SLEEP_MODE`, matching
        /// SensorLib's behaviour so the controller observes the full command handshake.
        pub fn sleep(&mut self) -> Result<(), Error<E>> {
            maybe_await!(self.set_mode(RunMode::DebugInfo))?;
            maybe_await!(self.write(&REG_SLEEP_MODE.to_be_bytes()))?;
            Ok(())
        }
    }

    /// Return the model string derived from the cached chip ID.
    ///
    /// Returns `"UNKNOWN"` until `get_attribute()` has populated `chip_info()`.
    pub fn model_name(&self) -> &'static str {
        self.chip_info.model_name()
    }

    maybe_async! {
        /// Switch to a controller run mode (normal, debug, factory, etc.).
        ///
        /// Runs the `0xD1/0x00` handshake SensorLib uses, writes the mode register,
        /// and verifies the controller reported the expected mode back via `REG_0x0002`.
        /// Most modes just write a constant, but `Factory` retries until the special register
        /// reports readiness.
        pub fn set_mode(&mut self, mode: RunMode) -> Result<(), Error<E>> {
            let mut ready = false;
            let mut read_buffer = [0u8; 4];
            let handshake = REG_MODE_HANDSHAKE.to_be_bytes();
            let status_reg = REG_MODE_STATUS.to_be_bytes();
            let mut last_error = None;

            for _ in 0..3 {
                if let Err(e) = maybe_await!(self.write(&handshake)) {
                    last_error = Some(e);
                    maybe_await!(self.delay.delay_ms(200));
                    continue;
                }
                if let Err(e) = maybe_await!(self.write(&handshake)) {
                    last_error = Some(e);
                    maybe_await!(self.delay.delay_ms(200));
                    continue;
                }
                if let Err(e) = maybe_await!(self.write_read(&status_reg, &mut read_buffer)) {
                    last_error = Some(e);
                    maybe_await!(self.delay.delay_ms(200));
                    continue;
                }
                if read_buffer[1] == handshake[1] {
                    ready = true;
                    break;
                }
            }

            if !ready {
                #[cfg(feature = "defmt")]
                defmt::debug!("mode handshake failed");
                // If every retry failed on an I2C error, surface that instead of a
                // generic NotReady — it's the difference between "the bus is broken"
                // and "the chip just never confirmed the handshake in time".
                return Err(last_error.unwrap_or(Error::NotReady));
            }

            #[cfg(feature = "defmt")]
            defmt::debug!("set_mode -> {:?}", mode);

            let mode_bytes = match protocol::mode_bytes(mode) {
                ModeBytes::Fixed(bytes) => bytes,
                ModeBytes::NeedsFactoryHandshake => {
                    maybe_await!(self.prepare_factory_mode(&mut read_buffer))?
                }
            };

            let mode_cmd = mode_bytes[1];
            maybe_await!(self.write(&mode_bytes))?;
            let mut status = [0u8; 2];
            maybe_await!(self.write_read(&status_reg, &mut status))?;
            if status[1] != mode_cmd {
                #[cfg(feature = "defmt")]
                defmt::error!(
                    "set_mode: read 0x0002 responded with 0x{:02X}, expected 0x{:02X}",
                    status[1],
                    mode_cmd
                );
                return Err(Error::NotReady);
            }
            maybe_await!(self.delay.delay_ms(10));
            Ok(())
        }
    }

    maybe_async! {
        /// Poll the factory register until the controller is ready for factory mode commands.
        ///
        /// SensorLib performs repeated write/read cycles to `REG_FACTORY_MODE`/`0x0009` until
        /// the controller returns `0x14`. We mirror that loop and return the factory command bytes on success.
        fn prepare_factory_mode(
            &mut self,
            read_buffer: &mut [u8; 4],
        ) -> Result<[u8; 2], Error<E>> {
            let mut last_error = None;
            for _ in 0..10 {
                if let Err(e) = maybe_await!(self.write(&REG_FACTORY_MODE.to_be_bytes())) {
                    last_error = Some(e);
                    maybe_await!(self.delay.delay_ms(1));
                    #[cfg(feature = "defmt")]
                    defmt::debug!("factory mode write failed");
                    continue;
                }
                maybe_await!(self.delay.delay_ms(10));
                if let Err(e) = maybe_await!(
                    self.write_read(&REG_FACTORY_STATUS.to_be_bytes(), &mut read_buffer[..1])
                ) {
                    last_error = Some(e);
                    maybe_await!(self.delay.delay_ms(1));
                    #[cfg(feature = "defmt")]
                    defmt::debug!("factory mode status read failed");
                    continue;
                }
                if read_buffer[0] == 0x14 {
                    #[cfg(feature = "defmt")]
                    defmt::debug!("factory mode ready");
                    return Ok(REG_FACTORY_READY.to_be_bytes());
                }
            }
            // Same reasoning as set_mode()'s handshake loop: prefer the real I2C
            // error over a generic NotReady when that's why we never saw 0x14.
            Err(last_error.unwrap_or(Error::NotReady))
        }
    }

    maybe_async! {
        /// Write raw bytes (register + payload) to the controller.
        ///
        /// Uses the fixed slave address `CST92XX_SLAVE_ADDRESS` so callers can always pass
        /// register+payload bytes directly.
        fn write(&mut self, write: &[u8]) -> Result<(), Error<E>> {
            maybe_await!(self.i2c.write(CST92XX_SLAVE_ADDRESS, write)).map_err(Error::I2C)
        }
    }

    maybe_async! {
        /// Write bytes and then read a response without leaving command mode.
        ///
        /// Performs a single `write_read` transaction that keeps the bus busy until the controller responds.
        fn write_read(&mut self, write: &[u8], read: &mut [u8]) -> Result<(), Error<E>> {
            maybe_await!(self.i2c.write_read(CST92XX_SLAVE_ADDRESS, write, read))
                .map_err(Error::I2C)
        }
    }

    maybe_async! {
        /// Read the latest touch report from `REG_READ` and translate it into `Point`s.
        ///
        /// Performs the same optimized SensorLib path that transfers 15 bytes instead of 30,
        /// sends the acknowledgment, and filters out inactive slots before returning the touch array.
        pub fn touches(&mut self) -> Result<[Option<Point>; MAX_FINGER_NUM], Error<E>> {
            let mut buffer = [0u8; MAX_FINGER_NUM * 5 + 5];
            let reg_bytes = REG_READ.to_be_bytes();

            maybe_await!(self.write_read(&reg_bytes, &mut buffer))?;

            if !buffer.iter().any(|&x| x != 0) {
                return Ok([None; MAX_FINGER_NUM]);
            }

            let mut write_buffer = [0u8; 3];
            write_buffer[0] = reg_bytes[0];
            write_buffer[1] = reg_bytes[1];
            write_buffer[2] = CST92XX_ACK;
            maybe_await!(self.write(&write_buffer))?;

            let panel_resolution = (self.chip_info.resolution_x, self.chip_info.resolution_y);
            Ok(protocol::decode_touch_report(
                &buffer,
                &self.config,
                panel_resolution,
            ))
        }
    }
}
