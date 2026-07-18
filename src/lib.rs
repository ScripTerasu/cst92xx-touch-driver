#![no_std]

//! Basic driver for the CST92xx touch screen that supports both blocking and async I2C access.

pub mod error;
pub mod mode;
pub mod registers;
pub mod types;
use embassy_time::Timer;
pub use error::Error;
pub use mode::RunMode;
pub use types::Point;

use crate::mode::RunMode;
use crate::registers::{
    CST92XX_ACK, CST92XX_SLAVE_ADDRESS, CST9217_CHIP_ID, CST9220_CHIP_ID, MAX_FINGER_NUM,
    REG_BASE_LINE_MODE, REG_DEBUG_MODE, REG_DIFF_MODE, REG_FACTORY_MODE, REG_LOW_POWER_MODE,
    REG_NORMAL_MODE, REG_RAW_MODE, REG_READ, REG_SLEEP_MODE,
};
use crate::types::TouchConfig;
/// Async CST92xx driver.
pub struct CST92xx<I2C> {
    i2c: I2C,
    config: TouchConfig,
    chip_type: u16,
}

impl<I2C, E> CST92xx<I2C>
where
    I2C: embedded_hal_async::i2c::I2c<Error = E>,
{
    pub fn new(i2c: I2C) -> Self {
        Self {
            i2c,
            config: TouchConfig::default(),
            chip_type: 0,
        }
    }

    pub async fn init(&mut self) -> Result<(), Error<E>> {
        self.reset().await;
        self.get_attribute().await?;

        #[cfg(feature = "defmt")]
        defmt::debug!("Touch type:{}", self.model_name());
        Ok(())
    }

    pub async fn reset(&mut self) {
        Timer::after_millis(30).await;
    }

    pub async fn get_attribute(&mut self) -> Result<(), Error<E>> {
        Timer::after_millis(30).await;

        let mut buffer = [0u8; 8];
        self.write(&[0xD1, 0x01]).await?;
        Timer::after_millis(10).await;

        self.write_read(&[0xD1, 0xFC], &mut buffer[..4]).await?;

        let check_code = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);

        #[cfg(feature = "defmt")]
        defmt::info!("Chip checkcode: {=u32:#010X}", check_code);

        self.write_read(&[0xD1, 0xF8], &mut buffer[..4]).await?;
        self.config.resolution_x = u16::from_le_bytes([buffer[0], buffer[1]]);
        self.config.resolution_y = u16::from_le_bytes([buffer[2], buffer[3]]);

        #[cfg(feature = "defmt")]
        defmt::info!(
            "Chip resolution X={=u16} Y={=u16}",
            self.config.resolution_x,
            self.config.resolution_y
        );

        self.write_read(&[0xD2, 0x04], &mut buffer[..4]).await?;
        self.chip_type =
            (u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) >> 16) as u16;

        let _project_id = u32::from(u16::from_le_bytes([buffer[0], buffer[1]]));

        #[cfg(feature = "defmt")]
        defmt::info!(
            "Chip type={=u16:#06X}, Project ID={=u32:#010X}",
            self.chip_type,
            _project_id
        );

        self.write_read(&[0xD2, 0x08], &mut buffer[..8]).await?;
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

    pub async fn sleep(&mut self) -> Result<(), Error<E>> {
        // Enter debug/command mode.
        self.set_mode(RunMode::DebugInfo).await?;

        // Send sleep command.
        let buffer = REG_SLEEP_MODE.to_be_bytes();
        self.write(&buffer).await?;

        Ok(())
    }

    pub fn model_name(&self) -> &str {
        match self.chip_type {
            CST9220_CHIP_ID => "CST9220",
            CST9217_CHIP_ID => "CST9217",
            // Agrega aquí los IDs específicos que maneje la librería original
            _ => "UNKNOWN",
        }
    }

    pub async fn set_mode(&mut self, mode: RunMode) -> Result<(), Error<E>> {
        let mut ready = false;
        let mut read_buffer = [0u8; 4];

        for _ in 0..3 {
            if self.write(&[0xD1, 0x1E]).await.is_err() {
                Timer::after_millis(200).await;
                continue;
            }
            if self.write(&[0xD1, 0x1E]).await.is_err() {
                Timer::after_millis(200).await;
                continue;
            }
            if self
                .write_read(&[0x00, 0x02], &mut read_buffer)
                .await
                .is_err()
            {
                Timer::after_millis(200).await;
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
            RunMode::Factory => self.prepare_factory_mode(&mut read_buffer).await?,
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
        self.write(&mode_bytes).await?;
        let mut status = [0u8; 2];
        self.write_read(&[0x00, 0x02], &mut status).await?;
        if status[1] != mode_cmd {
            #[cfg(feature = "defmt")]
            defmt::error!(
                "set_mode: read 0x0002 responded with 0x{:02X}, expected 0x{:02X}",
                status[1],
                mode_cmd
            );
            return Err(Error::NotReady);
        }
        Timer::after_millis(10).await;
        Ok(())
    }

    async fn prepare_factory_mode(
        &mut self,
        read_buffer: &mut [u8; 4],
    ) -> Result<[u8; 2], Error<E>> {
        for _ in 0..10 {
            let reg_bytes = REG_FACTORY_MODE.to_be_bytes();
            if self.write(&reg_bytes).await.is_err() {
                Timer::after_millis(1).await;
                #[cfg(feature = "defmt")]
                defmt::debug!("factory mode write failed");
                continue;
            }
            Timer::after_millis(10).await;
            if self
                .write_read(&[0x00, 0x09], &mut read_buffer[..1])
                .await
                .is_err()
            {
                Timer::after_millis(1).await;
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

    async fn write(&mut self, write: &[u8]) -> Result<(), Error<E>> {
        self.i2c
            .write(CST92XX_SLAVE_ADDRESS, write)
            .await
            .map_err(Error::I2C)
    }

    async fn write_read(&mut self, write: &[u8], read: &mut [u8]) -> Result<(), Error<E>> {
        self.i2c
            .write_read(CST92XX_SLAVE_ADDRESS, write, read)
            .await
            .map_err(Error::I2C)
    }

    pub async fn touches(&mut self) -> Result<[Option<Point>; MAX_FINGER_NUM], Error<E>> {
        // El buffer pasa a ser de 15 bytes automáticamente (2 * 5 + 5)
        let mut buffer = [0u8; MAX_FINGER_NUM * 5 + 5];
        let mut points: [Option<Point>; MAX_FINGER_NUM] = [None; MAX_FINGER_NUM];
        let reg_bytes = REG_READ.to_be_bytes();

        // El bus I2C ahora solo transfiere 15 bytes en lugar de 30. ¡Mucho más rápido!
        self.write_read(&reg_bytes, &mut buffer).await?;

        if !buffer.iter().any(|&x| x != 0) {
            return Ok(points);
        }

        let mut write_buffer = [0u8; 3];
        write_buffer[0] = reg_bytes[0];
        write_buffer[1] = reg_bytes[1];
        write_buffer[2] = CST92XX_ACK;
        self.write(&write_buffer).await?;

        if buffer[0] == CST92XX_ACK || buffer[0] == 0x00 {
            return Ok(points);
        }
        if buffer[6] != CST92XX_ACK {
            return Ok(points);
        }

        if (buffer[4] & 0xF0) != 0 && (buffer[4] >> 7) == 0x01 {
            return Ok(points);
        }

        // Limitamos el parseo al nuevo máximo configurado
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
