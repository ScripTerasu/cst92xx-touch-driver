use embedded_hal::delay::DelayNs;
use embedded_hal::digital::OutputPin;
use embedded_hal::i2c::I2c;

use crate::error::Error;
use crate::info::{ChipInfo, Point};
use crate::mode::RunMode;
use crate::registers::{
    CST92XX_ACK, CST92XX_SLAVE_ADDRESS, CST9217_CHIP_ID, CST9220_CHIP_ID, MAX_FINGER_NUM,
    REG_BASE_LINE_MODE, REG_CHECK_CODE, REG_CHIP_TYPE, REG_DEBUG_MODE, REG_DIFF_MODE,
    REG_FACTORY_HIGH_DRV, REG_FACTORY_LOW_DRV, REG_FACTORY_MODE, REG_FACTORY_READY,
    REG_FACTORY_SHORT, REG_FACTORY_STATUS, REG_FW_VERSION, REG_LOW_POWER_MODE, REG_MODE_HANDSHAKE,
    REG_MODE_STATUS, REG_NORMAL_MODE, REG_RAW_MODE, REG_READ, REG_RESOLUTION, REG_SLEEP_MODE,
    REG_UPDATE_FIRMWARE,
};
use crate::reset_pin::NoResetPin;
use crate::types::TouchConfig;

/// Blocking CST92xx controller driver backed by synchronous `embedded-hal`.
///
/// Owns an I²C bus, a `DelayNs` provider, and (optionally) a reset pin so you
/// can call `.touches()` from contexts without an async executor.
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
    /// Create a new blocking CST92xx driver without a dedicated reset pin.
    ///
    /// `i2c` should implement `embedded-hal::i2c::I2c` for the controller's 7-bit address,
    /// and `delay` is used for reset/mode timing. Use `.with_reset()` to attach a real
    /// `RST` line if one is wired up.
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

    /// Initialize the controller (reset + attribute read) and ensure the chip is supported.
    ///
    /// Mirrors `TouchDrvCST92xx::initImpl()`, which just calls `getAttribute()` — the
    /// reset pulse itself happens inside `get_attribute()`, matching SensorLib.
    pub fn init(&mut self) -> Result<(), Error<E>> {
        self.get_attribute()?;

        #[cfg(feature = "defmt")]
        defmt::debug!("Touch type:{}", self.model_name());
        Ok(())
    }

    /// Pulse the reset pin (if any) and wait for the controller to come back up.
    ///
    /// With the default `NoResetPin` this is just the settle delay SensorLib waits after
    /// resetting; with a real `RST` pin attached via `.with_reset()`, the pin is pulsed
    /// low first. The datasheet does not document the minimum low-pulse width, so this
    /// timing may need tuning for your hardware.
    pub fn reset(&mut self) {
        let _ = self.rst.set_low();
        self.delay.delay_ms(10_u32);
        let _ = self.rst.set_high();
        self.delay.delay_ms(30_u32);
    }

    /// Read controller metadata (checkcode, resolution, chip/version) and validate the chip.
    ///
    /// Runs the same reset + attribute reads as SensorLib's `getAttribute()` (`0xD1/0xD2`)
    /// and caches the result, retrievable via `chip_info()`.
    pub fn get_attribute(&mut self) -> Result<(), Error<E>> {
        self.reset();

        let mut buffer = [0u8; 8];
        // Enter command mode: this is the same register as `REG_DEBUG_MODE`.
        self.write(&REG_DEBUG_MODE.to_be_bytes())?;
        self.delay.delay_ms(10_u32);

        self.write_read(&REG_CHECK_CODE.to_be_bytes(), &mut buffer[..4])?;
        let check_code = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);

        #[cfg(feature = "defmt")]
        defmt::info!("Chip checkcode: {=u32:#010X}", check_code);

        self.write_read(&REG_RESOLUTION.to_be_bytes(), &mut buffer[..4])?;
        self.chip_info.resolution_x = u16::from_le_bytes([buffer[0], buffer[1]]);
        self.chip_info.resolution_y = u16::from_le_bytes([buffer[2], buffer[3]]);

        #[cfg(feature = "defmt")]
        defmt::info!(
            "Chip resolution X={=u16} Y={=u16}",
            self.chip_info.resolution_x,
            self.chip_info.resolution_y
        );

        self.write_read(&REG_CHIP_TYPE.to_be_bytes(), &mut buffer[..4])?;
        self.chip_info.chip_type =
            (u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) >> 16) as u16;
        self.chip_info.project_id = u32::from(u16::from_le_bytes([buffer[0], buffer[1]]));

        #[cfg(feature = "defmt")]
        defmt::info!(
            "Chip type={=u16:#06X}, Project ID={=u32:#010X}",
            self.chip_info.chip_type,
            self.chip_info.project_id
        );

        self.write_read(&REG_FW_VERSION.to_be_bytes(), &mut buffer[..8])?;
        self.chip_info.fw_version =
            u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);
        self.chip_info.checksum = u32::from_le_bytes([buffer[4], buffer[5], buffer[6], buffer[7]]);

        #[cfg(feature = "defmt")]
        defmt::info!(
            "Chip IC version={=u32:#010X}, checksum={=u32:#010X}",
            self.chip_info.fw_version,
            self.chip_info.checksum
        );

        if self.chip_info.fw_version == 0xA5A5_A5A5 {
            #[cfg(feature = "defmt")]
            defmt::error!("Chip doesn't have firmware.");

            return Err(Error::InvalidFirmware);
        }

        if (check_code & 0xFFFF_0000) != 0xCACA_0000 {
            #[cfg(feature = "defmt")]
            defmt::error!("Firmware info read error.");

            return Err(Error::InvalidCheckCode);
        }

        if self.chip_info.chip_type != CST9217_CHIP_ID
            && self.chip_info.chip_type != CST9220_CHIP_ID
        {
            #[cfg(feature = "defmt")]
            defmt::error!(
                "Unsupported chip type: {=u16:#06X}",
                self.chip_info.chip_type
            );

            return Err(Error::InvalidChipType(self.chip_info.chip_type));
        }

        Ok(())
    }

    /// Chip metadata discovered by the last successful `get_attribute()`/`init()` call.
    pub fn chip_info(&self) -> ChipInfo {
        self.chip_info
    }

    /// Request the controller to enter sleep, mirroring SensorLib's `sleep()`.
    ///
    /// Switches into `DebugInfo` first and then writes `REG_SLEEP_MODE`, matching the firmware's handshake.
    pub fn sleep(&mut self) -> Result<(), Error<E>> {
        self.set_mode(RunMode::DebugInfo)?;
        self.write(&REG_SLEEP_MODE.to_be_bytes())?;
        Ok(())
    }

    /// Return a human-friendly model string derived from the cached chip ID.
    ///
    /// Returns `"UNKNOWN"` until `get_attribute` has populated `chip_info()`.
    pub fn model_name(&self) -> &'static str {
        self.chip_info.model_name()
    }

    /// Switch to a controller run mode (normal, debug, factory, etc.).
    ///
    /// Reuses the same `0xD1/0x00` handshake that SensorLib requires before writing the mode register.
    pub fn set_mode(&mut self, mode: RunMode) -> Result<(), Error<E>> {
        let mut ready = false;
        let mut read_buffer = [0u8; 4];
        let handshake = REG_MODE_HANDSHAKE.to_be_bytes();
        let status_reg = REG_MODE_STATUS.to_be_bytes();

        for _ in 0..3 {
            if self.write(&handshake).is_err() {
                self.delay.delay_ms(200_u32);
                continue;
            }
            if self.write(&handshake).is_err() {
                self.delay.delay_ms(200_u32);
                continue;
            }
            if self.write_read(&status_reg, &mut read_buffer).is_err() {
                self.delay.delay_ms(200_u32);
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
            return Err(Error::NotReady);
        }

        #[cfg(feature = "defmt")]
        defmt::debug!("set_mode -> {:?}", mode);

        let mode_bytes = match mode {
            RunMode::Normal => REG_NORMAL_MODE.to_be_bytes(),
            RunMode::LowPower => REG_LOW_POWER_MODE.to_be_bytes(),
            RunMode::DeepSleep => REG_SLEEP_MODE.to_be_bytes(),
            RunMode::Wakeup => REG_NORMAL_MODE.to_be_bytes(),
            RunMode::DebugDiff => REG_DIFF_MODE.to_be_bytes(),
            RunMode::DebugRawData => REG_RAW_MODE.to_be_bytes(),
            RunMode::Factory => self.prepare_factory_mode(&mut read_buffer)?,
            RunMode::DebugInfo => REG_DEBUG_MODE.to_be_bytes(),
            RunMode::UpdateFirmware => REG_UPDATE_FIRMWARE.to_be_bytes(),
            RunMode::FactoryHighDrv => REG_FACTORY_HIGH_DRV.to_be_bytes(),
            RunMode::FactoryLowDrv => REG_FACTORY_LOW_DRV.to_be_bytes(),
            RunMode::FactoryShort => REG_FACTORY_SHORT.to_be_bytes(),
            RunMode::LpScan => REG_BASE_LINE_MODE.to_be_bytes(),
        };

        let mode_cmd = mode_bytes[1];
        self.write(&mode_bytes)?;
        let mut status = [0u8; 2];
        self.write_read(&status_reg, &mut status)?;
        if status[1] != mode_cmd {
            #[cfg(feature = "defmt")]
            defmt::error!(
                "set_mode: read 0x0002 responded with 0x{:02X}, expected 0x{:02X}",
                status[1],
                mode_cmd
            );
            return Err(Error::NotReady);
        }
        self.delay.delay_ms(10_u32);

        Ok(())
    }

    /// Poll the factory register until the controller reports readiness.
    ///
    /// Executes the same write/read loop SensorLib uses before entering `Factory` mode.
    fn prepare_factory_mode(&mut self, read_buffer: &mut [u8; 4]) -> Result<[u8; 2], Error<E>> {
        for _ in 0..10 {
            if self.write(&REG_FACTORY_MODE.to_be_bytes()).is_err() {
                self.delay.delay_ms(1_u32);
                #[cfg(feature = "defmt")]
                defmt::debug!("factory mode write failed");
                continue;
            }
            self.delay.delay_ms(10_u32);
            if self
                .write_read(&REG_FACTORY_STATUS.to_be_bytes(), &mut read_buffer[..1])
                .is_err()
            {
                self.delay.delay_ms(1_u32);
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
        Err(Error::NotReady)
    }

    /// Write raw bytes (register + payload) to the controller.
    fn write(&mut self, write: &[u8]) -> Result<(), Error<E>> {
        self.i2c
            .write(CST92XX_SLAVE_ADDRESS, write)
            .map_err(Error::I2C)
    }

    /// Write bytes and then read back a response without leaving command mode.
    fn write_read(&mut self, write: &[u8], read: &mut [u8]) -> Result<(), Error<E>> {
        self.i2c
            .write_read(CST92XX_SLAVE_ADDRESS, write, read)
            .map_err(Error::I2C)
    }

    /// Read the latest touch report from `REG_READ` and translate it into `Point`s.
    ///
    /// Mirrors the async `touches()` path that reads 15 bytes, acknowledges, and filters inactive slots.
    pub fn touches(&mut self) -> Result<[Option<Point>; MAX_FINGER_NUM], Error<E>> {
        let mut buffer = [0u8; MAX_FINGER_NUM * 5 + 5];
        let mut points: [Option<Point>; MAX_FINGER_NUM] = [None; MAX_FINGER_NUM];
        let reg_bytes = REG_READ.to_be_bytes();

        self.write_read(&reg_bytes, &mut buffer)?;

        if !buffer.iter().any(|&x| x != 0) {
            return Ok(points);
        }

        let mut write_buffer = [0u8; 3];
        write_buffer[0] = reg_bytes[0];
        write_buffer[1] = reg_bytes[1];
        write_buffer[2] = CST92XX_ACK;
        self.write(&write_buffer)?;

        if buffer[0] == CST92XX_ACK || buffer[0] == 0x00 {
            return Ok(points);
        }
        if buffer[6] != CST92XX_ACK {
            return Ok(points);
        }

        if (buffer[4] & 0xF0) != 0 && (buffer[4] >> 7) == 0x01 {
            return Ok(points);
        }

        let num_points = (buffer[5] & 0x7F) as usize;
        if num_points > MAX_FINGER_NUM || num_points == 0 {
            return Ok(points);
        }

        let panel_resolution = (self.chip_info.resolution_x, self.chip_info.resolution_y);

        // `i` also drives the non-uniform `start_idx` stride into `buffer`, not just the
        // write into `points`, so `enumerate()` over `points` doesn't fit cleanly here.
        #[allow(clippy::needless_range_loop)]
        for i in 0..num_points {
            let start_idx = (i * 5) + if i == 0 { 0 } else { 2 };
            let pdat = &buffer[start_idx..start_idx + 4];

            let id = pdat[0] >> 4;
            let event = pdat[0] & 0x0F;

            if event == 0x06 && (id as usize) < MAX_FINGER_NUM {
                let raw_x = ((pdat[1] as u16) << 4) | ((pdat[3] >> 4) as u16);
                let raw_y = ((pdat[2] as u16) << 4) | ((pdat[3] & 0x0F) as u16);
                let (x, y) = self.config.transform(panel_resolution, raw_x, raw_y);

                points[i] = Some(Point {
                    track_id: id,
                    x,
                    y,
                    area: 0,
                });
            }
        }

        // Mirrors SensorLib: if the first slot never got a valid event, the
        // whole report is discarded even if a later slot parsed one.
        if points[0].is_none() {
            return Ok([None; MAX_FINGER_NUM]);
        }

        Ok(points)
    }
}
