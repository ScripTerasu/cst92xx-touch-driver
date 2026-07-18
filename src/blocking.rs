use embedded_hal::delay::DelayNs;
use embedded_hal::i2c::I2c;

use crate::error::Error;
use crate::mode::RunMode;
use crate::registers::{
    CST92XX_ACK, CST92XX_SLAVE_ADDRESS, CST9217_CHIP_ID, CST9220_CHIP_ID, MAX_FINGER_NUM,
    REG_BASE_LINE_MODE, REG_DEBUG_MODE, REG_DIFF_MODE, REG_FACTORY_MODE, REG_LOW_POWER_MODE,
    REG_NORMAL_MODE, REG_RAW_MODE, REG_READ, REG_SLEEP_MODE,
};
use crate::types::{Point, TouchConfig};

/// Blocking CST92xx driver.
pub struct BlockingCST92xx<I2C, DELAY> {
    i2c: I2C,
    delay: DELAY,
    config: TouchConfig,
    chip_type: u16,
}

impl<I2C, E, DELAY> BlockingCST92xx<I2C, DELAY>
where
    I2C: I2c<Error = E>,
    DELAY: DelayNs,
{
    /// Create a new blocking CST92xx driver.
    pub fn new(i2c: I2C, delay: DELAY) -> Self {
        Self {
            i2c,
            delay,
            config: TouchConfig::default(),
            chip_type: 0,
        }
    }

    /// Take ownership of the I2C bus and delay provider.
    pub fn into_inner(self) -> (I2C, DELAY) {
        (self.i2c, self.delay)
    }

    /// Initialize the controller (reset + attribute read) and ensure the chip is supported.
    pub fn init(&mut self) -> Result<(), Error<E>> {
        self.reset();
        self.get_attribute()?;

        #[cfg(feature = "defmt")]
        defmt::debug!("Touch type:{}", self.model_name());
        Ok(())
    }

    /// Simple delay helper that mimics the hardware reset timing.
    pub fn reset(&mut self) {
        self.delay.delay_ms(30_u32);
    }

    /// Read the controller metadata (checkcode, resolution, chip/version) and validate the chip.
    pub fn get_attribute(&mut self) -> Result<(), Error<E>> {
        self.delay.delay_ms(30_u32);

        let mut buffer = [0u8; 8];
        self.write(&[0xD1, 0x01])?;
        self.delay.delay_ms(10_u32);

        self.write_read(&[0xD1, 0xFC], &mut buffer[..4])?;

        let check_code = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);

        #[cfg(feature = "defmt")]
        defmt::info!("Chip checkcode: {=u32:#010X}", check_code);

        self.write_read(&[0xD1, 0xF8], &mut buffer[..4])?;
        self.config.resolution_x = u16::from_le_bytes([buffer[0], buffer[1]]);
        self.config.resolution_y = u16::from_le_bytes([buffer[2], buffer[3]]);

        #[cfg(feature = "defmt")]
        defmt::info!(
            "Chip resolution X={=u16} Y={=u16}",
            self.config.resolution_x,
            self.config.resolution_y
        );

        self.write_read(&[0xD2, 0x04], &mut buffer[..4])?;
        self.chip_type =
            (u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) >> 16) as u16;

        let _project_id = u32::from(u16::from_le_bytes([buffer[0], buffer[1]]));

        #[cfg(feature = "defmt")]
        defmt::info!(
            "Chip type={=u16:#06X}, Project ID={=u32:#010X}",
            self.chip_type,
            _project_id
        );

        self.write_read(&[0xD2, 0x08], &mut buffer[..8])?;
        let fw_version = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);

        let _checksum = u32::from_le_bytes([buffer[4], buffer[5], buffer[6], buffer[7]]);

        #[cfg(feature = "defmt")]
        defmt::info!(
            "Chip IC version={=u32:#010X}, checksum={=u32:#010X}",
            fw_version,
            _checksum
        );

        if fw_version == 0xA5A5A5A5 {
            #[cfg(feature = "defmt")]
            defmt::error!("Chip doesn't have firmware.");

            return Err(Error::InvalidFirmware);
        }

        if (check_code & 0xFFFF_0000) != 0xCACA_0000 {
            #[cfg(feature = "defmt")]
            defmt::error!("Firmware info read error.");

            return Err(Error::InvalidCheckCode);
        }

        if self.chip_type != CST9217_CHIP_ID && self.chip_type != CST9220_CHIP_ID {
            #[cfg(feature = "defmt")]
            defmt::error!("Unsupported chip type: {=u16:#06X}", self.chip_type);

            return Err(Error::InvalidChipType(self.chip_type));
        }

        Ok(())
    }

    /// Request the controller to enter sleep via the ESP32-defined register sequence.
    pub fn sleep(&mut self) -> Result<(), Error<E>> {
        self.set_mode(RunMode::DebugInfo)?;

        let buffer = REG_SLEEP_MODE.to_be_bytes();
        self.write(&buffer)?;

        Ok(())
    }

    /// Return a human-friendly model string derived from the cached chip ID.
    pub fn model_name(&self) -> &str {
        match self.chip_type {
            CST9220_CHIP_ID => "CST9220",
            CST9217_CHIP_ID => "CST9217",
            _ => "UNKNOWN",
        }
    }

    /// Switch to a controller run mode (normal, debug, factory, etc.)
    pub fn set_mode(&mut self, mode: RunMode) -> Result<(), Error<E>> {
        let mut ready = false;
        let mut read_buffer = [0u8; 4];

        for _ in 0..3 {
            if self.write(&[0xD1, 0x1E]).is_err() {
                self.delay.delay_ms(200_u32);
                continue;
            }
            if self.write(&[0xD1, 0x1E]).is_err() {
                self.delay.delay_ms(200_u32);
                continue;
            }
            if self.write_read(&[0x00, 0x02], &mut read_buffer).is_err() {
                self.delay.delay_ms(200_u32);
                continue;
            }
            if read_buffer[1] == 0x1E {
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
        defmt::debug!("set_work_mode: {:?}", mode);

        let mode_bytes = match mode {
            RunMode::Normal => {
                #[cfg(feature = "defmt")]
                defmt::debug!("mode -> NORMAL");
                REG_NORMAL_MODE.to_be_bytes()
            }
            RunMode::LowPower => {
                #[cfg(feature = "defmt")]
                defmt::debug!("mode -> LOW_POWER");
                REG_LOW_POWER_MODE.to_be_bytes()
            }
            RunMode::DeepSleep => {
                #[cfg(feature = "defmt")]
                defmt::debug!("mode -> DEEP_SLEEP");
                REG_SLEEP_MODE.to_be_bytes()
            }
            RunMode::Wakeup => {
                #[cfg(feature = "defmt")]
                defmt::debug!("mode -> WAKEUP");
                REG_NORMAL_MODE.to_be_bytes()
            }
            RunMode::DebugDiff => {
                #[cfg(feature = "defmt")]
                defmt::debug!("mode -> DEBUG_DIFF");
                REG_DIFF_MODE.to_be_bytes()
            }
            RunMode::DebugRawData => {
                #[cfg(feature = "defmt")]
                defmt::debug!("mode -> DEBUG_RAWDATA");
                REG_RAW_MODE.to_be_bytes()
            }
            RunMode::Factory => self.prepare_factory_mode(&mut read_buffer)?,
            RunMode::DebugInfo => {
                #[cfg(feature = "defmt")]
                defmt::debug!("mode -> DEBUG_INFO");
                REG_DEBUG_MODE.to_be_bytes()
            }
            RunMode::UpdateFirmware => {
                #[cfg(feature = "defmt")]
                defmt::debug!("mode -> UPDATE_FIRMWARE");
                [0xD1, 0x08]
            }
            RunMode::FactoryHighDrv => {
                #[cfg(feature = "defmt")]
                defmt::debug!("mode -> FACTORY_HIGH_DRV");
                [0xD1, 0x10]
            }
            RunMode::FactoryLowDrv => {
                #[cfg(feature = "defmt")]
                defmt::debug!("mode -> FACTORY_LOW_DRV");
                [0xD1, 0x11]
            }
            RunMode::FactoryShort => {
                #[cfg(feature = "defmt")]
                defmt::debug!("mode -> FACTORY_SHORT");
                [0xD1, 0x12]
            }
            RunMode::LpScan => {
                #[cfg(feature = "defmt")]
                defmt::debug!("mode -> LP_SCAN");
                REG_BASE_LINE_MODE.to_be_bytes()
            }
        };

        let mode_cmd = mode_bytes[1];
        self.write(&mode_bytes)?;
        let mut status = [0u8; 2];
        self.write_read(&[0x00, 0x02], &mut status)?;
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

    fn prepare_factory_mode(&mut self, read_buffer: &mut [u8; 4]) -> Result<[u8; 2], Error<E>> {
        for _ in 0..10 {
            let reg_bytes = REG_FACTORY_MODE.to_be_bytes();
            if self.write(&reg_bytes).is_err() {
                self.delay.delay_ms(1_u32);
                #[cfg(feature = "defmt")]
                defmt::debug!("factory mode write failed");
                continue;
            }
            self.delay.delay_ms(10_u32);
            if self
                .write_read(&[0x00, 0x09], &mut read_buffer[..1])
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
                return Ok([0xD1, 0x19]);
            }
        }
        Err(Error::NotReady)
    }

    fn write(&mut self, write: &[u8]) -> Result<(), Error<E>> {
        self.i2c
            .write(CST92XX_SLAVE_ADDRESS, write)
            .map_err(Error::I2C)
    }

    fn write_read(&mut self, write: &[u8], read: &mut [u8]) -> Result<(), Error<E>> {
        self.i2c
            .write_read(CST92XX_SLAVE_ADDRESS, write, read)
            .map_err(Error::I2C)
    }

    /// Read the latest touch report from `REG_READ` and translate it into `Point`s.
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

        for i in 0..num_points {
            let start_idx = (i * 5) + if i == 0 { 0 } else { 2 };
            let pdat = &buffer[start_idx..start_idx + 4];

            let id = pdat[0] >> 4;
            let event = pdat[0] & 0x0F;

            if event == 0x06 && (id as usize) < MAX_FINGER_NUM {
                let x = ((pdat[1] as u16) << 4) | ((pdat[3] >> 4) as u16);
                let y = ((pdat[2] as u16) << 4) | ((pdat[3] & 0x0F) as u16);

                points[i] = Some(Point {
                    track_id: id,
                    x,
                    y,
                    area: 0,
                });
            }
        }

        Ok(points)
    }
}
