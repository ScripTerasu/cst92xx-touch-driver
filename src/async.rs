use embedded_hal::digital::OutputPin;
use embedded_hal_async::delay::DelayNs;

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

/// Async CST92xx controller driver backed by `embedded-hal-async`.
///
/// This struct owns an async I²C bus instance and an async `DelayNs`
/// provider, and exposes the same flow SensorLib uses (init, mode switching,
/// touch polling) without blocking the executor. Bring your own delay impl
/// (e.g. `embassy_time::Delay` if you already depend on embassy-time) —
/// this crate no longer hardcodes one.
pub struct CST92xx<I2C, DELAY, RST = NoResetPin> {
    i2c: I2C,
    delay: DELAY,
    rst: RST,
    config: TouchConfig,
    chip_info: ChipInfo,
}

impl<I2C, E, DELAY> CST92xx<I2C, DELAY, NoResetPin>
where
    I2C: embedded_hal_async::i2c::I2c<Error = E>,
    DELAY: DelayNs,
{
    /// Create a new async driver without a dedicated reset pin.
    ///
    /// `i2c` must provide exclusive ownership of the bus and implement
    /// `embedded_hal_async::i2c::I2c` for the 7-bit slave address the
    /// CST92xx controller listens on. `delay` is used for reset/mode timing.
    /// Use `.with_reset()` to attach a real `RST` line if one is wired up.
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
    I2C: embedded_hal_async::i2c::I2c<Error = E>,
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
    pub fn into_inner(self) -> (I2C, DELAY, RST) {
        (self.i2c, self.delay, self.rst)
    }

    /// Initialize the controller (reset + attribute read) and ensure a supported chip is present.
    ///
    /// Mirrors `TouchDrvCST92xx::initImpl()`, which just calls `getAttribute()` — the
    /// reset pulse itself happens inside `get_attribute()`, matching SensorLib.
    pub async fn init(&mut self) -> Result<(), Error<E>> {
        self.get_attribute().await?;

        #[cfg(feature = "defmt")]
        defmt::debug!("Touch type:{}", self.model_name());
        Ok(())
    }

    /// Pulse the reset pin (if any) and wait for the controller to come back up.
    ///
    /// With the default `NoResetPin` this is just the settle delay; with a real `RST`
    /// pin attached via `.with_reset()`, the pin is pulsed low first. Timing is from
    /// the CST9217 datasheet (section 10.5, "上电/复位"): `TRST` (reset pulse width) is
    /// 0.1 ms typical, and `TRON` (chip reinitialization time after reset) is 100 ms
    /// typical — the 10 ms low pulse comfortably clears `TRST`, and the 100 ms settle
    /// after releasing it matches `TRON`.
    pub async fn reset(&mut self) {
        let _ = self.rst.set_low();
        self.delay.delay_ms(10).await;
        let _ = self.rst.set_high();
        self.delay.delay_ms(100).await;
    }

    /// Read controller metadata (checkcode, resolution, chip/version) and validate the chip.
    ///
    /// Runs the same reset + attribute reads as SensorLib's `getAttribute()` (`0xD1/0xD2`)
    /// and caches the result, retrievable via `chip_info()`.
    pub async fn get_attribute(&mut self) -> Result<(), Error<E>> {
        self.reset().await;

        let mut buffer = [0u8; 8];
        // Enter command mode: this is the same register as `REG_DEBUG_MODE`.
        self.write(&REG_DEBUG_MODE.to_be_bytes()).await?;
        self.delay.delay_ms(10).await;

        self.write_read(&REG_CHECK_CODE.to_be_bytes(), &mut buffer[..4])
            .await?;
        let check_code = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);

        #[cfg(feature = "defmt")]
        defmt::info!("Chip checkcode: {=u32:#010X}", check_code);

        self.write_read(&REG_RESOLUTION.to_be_bytes(), &mut buffer[..4])
            .await?;
        self.chip_info.resolution_x = u16::from_le_bytes([buffer[0], buffer[1]]);
        self.chip_info.resolution_y = u16::from_le_bytes([buffer[2], buffer[3]]);

        #[cfg(feature = "defmt")]
        defmt::info!(
            "Chip resolution X={=u16} Y={=u16}",
            self.chip_info.resolution_x,
            self.chip_info.resolution_y
        );

        self.write_read(&REG_CHIP_TYPE.to_be_bytes(), &mut buffer[..4])
            .await?;
        self.chip_info.chip_type =
            (u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) >> 16) as u16;
        self.chip_info.project_id = u32::from(u16::from_le_bytes([buffer[0], buffer[1]]));

        #[cfg(feature = "defmt")]
        defmt::info!(
            "Chip type={=u16:#06X}, Project ID={=u32:#010X}",
            self.chip_info.chip_type,
            self.chip_info.project_id
        );

        self.write_read(&REG_FW_VERSION.to_be_bytes(), &mut buffer[..8])
            .await?;
        self.chip_info.fw_version =
            u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);
        self.chip_info.checksum = u32::from_le_bytes([buffer[4], buffer[5], buffer[6], buffer[7]]);

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

    /// Chip metadata discovered by the last successful `get_attribute()`/`init()` call.
    pub fn chip_info(&self) -> ChipInfo {
        self.chip_info
    }

    /// Request the controller to enter sleep, mirroring SensorLib's `sleep()`.
    ///
    /// This helper switches into `DebugInfo` mode before issuing `REG_SLEEP_MODE`, matching
    /// SensorLib's behaviour so the controller observes the full command handshake.
    pub async fn sleep(&mut self) -> Result<(), Error<E>> {
        self.set_mode(RunMode::DebugInfo).await?;
        self.write(&REG_SLEEP_MODE.to_be_bytes()).await?;
        Ok(())
    }

    /// Return the model string derived from the cached chip ID.
    ///
    /// Returns `"UNKNOWN"` until `get_attribute()` has populated `chip_info()`.
    pub fn model_name(&self) -> &'static str {
        self.chip_info.model_name()
    }

    /// Switch to a controller run mode (normal, debug, factory, etc.).
    ///
    /// Runs the `0xD1/0x00` handshake SensorLib uses, writes the mode register,
    /// and verifies the controller reported the expected mode back via `REG_0x0002`.
    /// Most modes just write a constant, but `Factory` retries until the special register
    /// reports readiness.
    pub async fn set_mode(&mut self, mode: RunMode) -> Result<(), Error<E>> {
        let mut ready = false;
        let mut read_buffer = [0u8; 4];
        let handshake = REG_MODE_HANDSHAKE.to_be_bytes();
        let status_reg = REG_MODE_STATUS.to_be_bytes();
        let mut last_error = None;

        for _ in 0..3 {
            if let Err(e) = self.write(&handshake).await {
                last_error = Some(e);
                self.delay.delay_ms(200).await;
                continue;
            }
            if let Err(e) = self.write(&handshake).await {
                last_error = Some(e);
                self.delay.delay_ms(200).await;
                continue;
            }
            if let Err(e) = self.write_read(&status_reg, &mut read_buffer).await {
                last_error = Some(e);
                self.delay.delay_ms(200).await;
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
            ModeBytes::NeedsFactoryHandshake => self.prepare_factory_mode(&mut read_buffer).await?,
        };

        let mode_cmd = mode_bytes[1];
        self.write(&mode_bytes).await?;
        let mut status = [0u8; 2];
        self.write_read(&status_reg, &mut status).await?;
        if status[1] != mode_cmd {
            #[cfg(feature = "defmt")]
            defmt::error!(
                "set_mode: read 0x0002 responded with 0x{:02X}, expected 0x{:02X}",
                status[1],
                mode_cmd
            );
            return Err(Error::NotReady);
        }
        self.delay.delay_ms(10).await;
        Ok(())
    }

    /// Helper that polls the factory register until the controller is ready for factory mode commands.
    ///
    /// SensorLib performs repeated write/read cycles to `REG_FACTORY_MODE`/`0x0009` until
    /// the controller returns `0x14`. We mirror that loop and return the factory command bytes on success.
    async fn prepare_factory_mode(
        &mut self,
        read_buffer: &mut [u8; 4],
    ) -> Result<[u8; 2], Error<E>> {
        let mut last_error = None;
        for _ in 0..10 {
            if let Err(e) = self.write(&REG_FACTORY_MODE.to_be_bytes()).await {
                last_error = Some(e);
                self.delay.delay_ms(1).await;
                #[cfg(feature = "defmt")]
                defmt::debug!("factory mode write failed");
                continue;
            }
            self.delay.delay_ms(10).await;
            if let Err(e) = self
                .write_read(&REG_FACTORY_STATUS.to_be_bytes(), &mut read_buffer[..1])
                .await
            {
                last_error = Some(e);
                self.delay.delay_ms(1).await;
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

    /// Write raw bytes (register + payload) to the controller.
    ///
    /// Uses the fixed slave address `CST92XX_SLAVE_ADDRESS` so callers can always pass
    /// register+payload bytes directly.
    async fn write(&mut self, write: &[u8]) -> Result<(), Error<E>> {
        self.i2c
            .write(CST92XX_SLAVE_ADDRESS, write)
            .await
            .map_err(Error::I2C)
    }

    /// Write bytes and then read a response without leaving command mode.
    ///
    /// Performs a single `write_read` transaction that keeps the bus busy until the controller responds.
    async fn write_read(&mut self, write: &[u8], read: &mut [u8]) -> Result<(), Error<E>> {
        self.i2c
            .write_read(CST92XX_SLAVE_ADDRESS, write, read)
            .await
            .map_err(Error::I2C)
    }

    /// Read the latest touch report from `REG_READ` and translate it into `Point`s.
    ///
    /// Performs the same optimized SensorLib path that transfers 15 bytes instead of 30,
    /// sends the acknowledgment, and filters out inactive slots before returning the touch array.
    pub async fn touches(&mut self) -> Result<[Option<Point>; MAX_FINGER_NUM], Error<E>> {
        let mut buffer = [0u8; MAX_FINGER_NUM * 5 + 5];
        let reg_bytes = REG_READ.to_be_bytes();

        self.write_read(&reg_bytes, &mut buffer).await?;

        if !buffer.iter().any(|&x| x != 0) {
            return Ok([None; MAX_FINGER_NUM]);
        }

        let mut write_buffer = [0u8; 3];
        write_buffer[0] = reg_bytes[0];
        write_buffer[1] = reg_bytes[1];
        write_buffer[2] = CST92XX_ACK;
        self.write(&write_buffer).await?;

        let panel_resolution = (self.chip_info.resolution_x, self.chip_info.resolution_y);
        Ok(protocol::decode_touch_report(
            &buffer,
            &self.config,
            panel_resolution,
        ))
    }
}
