#![no_std]

//! Basic driver for the CST9217 touch screen that supports both blocking and async I2C access.

pub mod error;
pub mod registers;
pub mod types;
use embassy_time::Timer;
pub use error::Error;
pub use registers::{CST92XX_SLAVE_ADDRESS, CST9217_CHIP_ID, CST9220_CHIP_ID};
pub use types::Point;

use crate::types::TouchConfig;
/// Async CST92xx driver.
pub struct CST92xx<I2C> {
    i2c: I2C,
    config: TouchConfig,
    _chip_type: u16,
}

impl<I2C, E> CST92xx<I2C>
where
    I2C: embedded_hal_async::i2c::I2c<Error = E>,
{
    /// Create a driver using a custom I2C address.
    pub fn new(i2c: I2C) -> Self {
        Self {
            i2c,
            config: TouchConfig::default(),
            _chip_type: 0,
        }
    }

    pub async fn init(&mut self) -> Result<(), Error<E>> {
        self.reset().await;
        self.get_attribute().await?;

        #[cfg(feature = "defmt")]
        defmt::debug!("Touch type:{}", self.get_model_name());
        Ok(())
    }

    pub async fn reset(&mut self) {
        Timer::after_millis(30).await;
    }

    pub async fn get_attribute(&mut self) -> Result<(), Error<E>> {
        Timer::after_millis(30).await;

        let mut buffer = [0u8; 8];
        self.write_buf(&[0xD1, 0x01]).await?;
        Timer::after_millis(10).await;

        self.write_then_read(&[0xD1, 0xFC], &mut buffer[..4])
            .await?;

        let check_code = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);

        #[cfg(feature = "defmt")]
        defmt::info!("Chip checkcode: {=u32:#010X}", check_code);

        self.write_then_read(&[0xD1, 0xF8], &mut buffer[..4])
            .await?;
        self.config.resolution_x = u16::from_le_bytes([buffer[0], buffer[1]]);
        self.config.resolution_y = u16::from_le_bytes([buffer[2], buffer[3]]);

        #[cfg(feature = "defmt")]
        defmt::info!(
            "Chip resolution X={=u16} Y={=u16}",
            self.config.resolution_x,
            self.config.resolution_y
        );

        self.write_then_read(&[0xD2, 0x04], &mut buffer[..4])
            .await?;
        self._chip_type =
            (u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) >> 16) as u16;

        let _project_id = u32::from(u16::from_le_bytes([buffer[0], buffer[1]]));

        #[cfg(feature = "defmt")]
        defmt::info!(
            "Chip type={=u16:#06X}, Project ID={=u32:#010X}",
            self._chip_type,
            _project_id
        );

        self.write_then_read(&[0xD2, 0x08], &mut buffer[..8])
            .await?;
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

        if self._chip_type != CST9217_CHIP_ID && self._chip_type != CST9220_CHIP_ID {
            #[cfg(feature = "defmt")]
            defmt::error!("Unsupported chip type: {=u16:#06X}", self._chip_type);

            return Err(Error::InvalidChipType(self._chip_type));
        }

        Ok(())
    }

    pub fn get_model_name(&self) -> &str {
        match self._chip_type {
            CST9220_CHIP_ID => "CST9220",
            CST9217_CHIP_ID => "CST9217",
            // Agrega aquí los IDs específicos que maneje la librería original
            _ => "UNKNOWN",
        }
    }

    async fn write_buf(&mut self, buf: &[u8]) -> Result<(), Error<E>> {
        self.i2c
            .write(CST92XX_SLAVE_ADDRESS, buf)
            .await
            .map_err(Error::I2C)
    }

    async fn write_then_read(
        &mut self,
        write_buf: &[u8],
        read_buf: &mut [u8],
    ) -> Result<(), Error<E>> {
        self.i2c
            .write_read(CST92XX_SLAVE_ADDRESS, write_buf, read_buf)
            .await
            .map_err(Error::I2C)
    }

    async fn write(&self, i2c: &mut I2C, register: u16, value: u8) -> Result<(), Error<E>> {
        let register = register.to_be_bytes();
        let cmd = [register[0], register[1], value];
        i2c.write(CST92XX_SLAVE_ADDRESS, &cmd)
            .await
            .map_err(Error::I2C)
    }

    async fn read(&self, i2c: &mut I2C, register: u16, buf: &mut [u8]) -> Result<(), Error<E>> {
        i2c.write_read(CST92XX_SLAVE_ADDRESS, &register.to_be_bytes(), buf)
            .await
            .map_err(Error::I2C)
    }
    // Initialize the device (requires a temporary read buffer of at least 4 bytes).
    // pub async fn init(&self, i2c: &mut I2C, buf: &mut [u8]) -> Result<(), Error<E>> {
    //     self.write(i2c, CST9217_COMMAND_REG, 0).await?;

    //     const LEN: usize = 4;
    //     assert!(buf.len() >= LEN);
    //     self.read(i2c, CST9217_PRODUCT_ID_REG, &mut buf[..LEN])
    //         .await?;
    //     match str::from_utf8(&buf[..LEN]) {
    //         Ok(product_id) => {
    //             if product_id != CST9217_EXPECTED_PRODUCT_ID {
    //                 return Err(Error::UnexpectedProductId);
    //             }
    //         }
    //         Err(_) => return Err(Error::UnexpectedProductId),
    //     }

    //     self.write(i2c, CST9217_TOUCHPOINT_STATUS_REG, 0).await?;
    //     Ok(())
    // }

    // Read a single touch point (buffer must hold at least `TOUCHPOINT_ENTRY_LEN`).
    // pub async fn get_touch(
    //     &self,
    //     i2c: &mut I2C,
    //     buf: &mut [u8],
    // ) -> Result<Option<Point>, Error<E>> {
    //     let num_touch_points = self.get_num_touch_points(i2c, buf).await?;

    //     let point = if num_touch_points > 0 {
    //         assert!(
    //             buf.len() >= TOUCHPOINT_ENTRY_LEN,
    //             "Buffer too small, use GET_TOUCH_BUF_SIZE"
    //         );
    //         self.read(
    //             i2c,
    //             CST9217_TOUCHPOINT_1_REG,
    //             &mut buf[..TOUCHPOINT_ENTRY_LEN],
    //         )
    //         .await?;
    //         Some(decode_point(buf))
    //     } else {
    //         None
    //     };

    //     self.write(i2c, CST9217_TOUCHPOINT_STATUS_REG, 0).await?;
    //     Ok(point)
    // }

    // /// Read multiple touch points (buffer must hold `num_touch_points * TOUCHPOINT_ENTRY_LEN`).
    // pub async fn get_multi_touch(
    //     &self,
    //     i2c: &mut I2C,
    //     buf: &mut [u8],
    // ) -> Result<heapless::Vec<Point, MAX_NUM_TOUCHPOINTS>, Error<E>> {
    //     let num_touch_points = self.get_num_touch_points(i2c, buf).await?;

    //     let points = if num_touch_points > 0 {
    //         assert!(num_touch_points <= MAX_NUM_TOUCHPOINTS);
    //         let mut points = heapless::Vec::new();

    //         let len: usize = num_touch_points * TOUCHPOINT_ENTRY_LEN;
    //         assert!(
    //             buf.len() >= len,
    //             "Buffer too small, use GET_MULTITOUCH_BUF_SIZE"
    //         );
    //         self.read(i2c, CST9217_TOUCHPOINT_1_REG, &mut buf[..len])
    //             .await?;

    //         for n in 0..num_touch_points {
    //             let start = n * TOUCHPOINT_ENTRY_LEN;
    //             points
    //                 .push(decode_point(&buf[start..start + TOUCHPOINT_ENTRY_LEN]))
    //                 .ok();
    //         }

    //         points
    //     } else {
    //         heapless::Vec::new()
    //     };

    //     self.write(i2c, CST9217_TOUCHPOINT_STATUS_REG, 0).await?;
    //     Ok(points)
    // }

    // async fn get_num_touch_points(&self, i2c: &mut I2C, buf: &mut [u8]) -> Result<usize, Error<E>> {
    //     assert!(!buf.is_empty());
    //     self.read(i2c, CST9217_TOUCHPOINT_STATUS_REG, &mut buf[..1])
    //         .await?;

    //     let status = buf[0];
    //     let ready = (status & 0x80) > 0;
    //     let num_touch_points = (status & 0x0F) as usize;

    //     if ready {
    //         Ok(num_touch_points)
    //     } else {
    //         Err(Error::NotReady)
    //     }
    // }

    // async fn write(&self, i2c: &mut I2C, register: u16, value: u8) -> Result<(), Error<E>> {
    //     let register = register.to_be_bytes();
    //     let cmd = [register[0], register[1], value];
    //     i2c.write(self.i2c_addr, &cmd).await.map_err(Error::I2C)
    // }

    // async fn read(&self, i2c: &mut I2C, register: u16, buf: &mut [u8]) -> Result<(), Error<E>> {
    //     i2c.write_read(self.i2c_addr, &register.to_be_bytes(), buf)
    //         .await
    //         .map_err(Error::I2C)
    // }
}

// fn decode_point(buf: &[u8]) -> Point {
//     assert!(buf.len() >= TOUCHPOINT_ENTRY_LEN);
//     Point {
//         track_id: buf[0],
//         x: u16::from_le_bytes([buf[1], buf[2]]),
//         y: u16::from_le_bytes([buf[3], buf[4]]),
//         area: u16::from_le_bytes([buf[5], buf[6]]),
//     }
// }
