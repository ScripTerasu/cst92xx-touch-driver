#![no_std]

//! Basic driver for the CST9217 touch screen that supports both blocking and async I2C access.

pub mod error;
pub mod registers;
pub mod types;

pub use error::Error;
pub use registers::{GET_MULTITOUCH_BUF_SIZE, GET_TOUCH_BUF_SIZE};
pub use types::Point;

use core::{marker::PhantomData, str};

use crate::registers::{
    CST9217_COMMAND_REG, CST9217_I2C_ADDR_BA, CST9217_PRODUCT_ID_REG, CST9217_TOUCHPOINT_1_REG,
    CST9217_TOUCHPOINT_STATUS_REG, MAX_NUM_TOUCHPOINTS, TOUCHPOINT_ENTRY_LEN,
};

const CST9217_EXPECTED_PRODUCT_ID: &str = "CST9";

/// Blocking CST9217 driver.
pub struct Cst9217Blocking<I2C> {
    i2c_addr: u8,
    i2c: PhantomData<I2C>,
}

impl<I2C> Default for Cst9217Blocking<I2C> {
    fn default() -> Self {
        Self {
            i2c_addr: CST9217_I2C_ADDR_BA,
            i2c: PhantomData,
        }
    }
}

impl<I2C, E> Cst9217Blocking<I2C>
where
    I2C: embedded_hal::i2c::I2c<Error = E>,
{
    /// Create a driver using a custom I2C address.
    pub fn new(i2c_addr: u8) -> Self {
        Self {
            i2c_addr,
            i2c: PhantomData,
        }
    }

    /// Initialize the device by switching to command mode and verifying the product ID.
    pub fn init(&self, i2c: &mut I2C) -> Result<(), Error<E>> {
        self.write(i2c, CST9217_COMMAND_REG, 0)?;

        let mut read = [0u8; 4];
        self.read(i2c, CST9217_PRODUCT_ID_REG, &mut read)?;
        match str::from_utf8(&read) {
            Ok(product_id) => {
                if product_id != CST9217_EXPECTED_PRODUCT_ID {
                    return Err(Error::UnexpectedProductId);
                }
            }
            Err(_) => return Err(Error::UnexpectedProductId),
        }

        self.write(i2c, CST9217_TOUCHPOINT_STATUS_REG, 0)?;
        Ok(())
    }

    /// Reads a single touch point. Returns `Ok(None)` when the screen is released.
    pub fn get_touch(&self, i2c: &mut I2C) -> Result<Option<Point>, Error<E>> {
        let num_touch_points = self.get_num_touch_points(i2c)?;

        let point = if num_touch_points > 0 {
            let mut read = [0u8; TOUCHPOINT_ENTRY_LEN];
            self.read(i2c, CST9217_TOUCHPOINT_1_REG, &mut read)?;
            Some(decode_point(&read))
        } else {
            None
        };

        self.write(i2c, CST9217_TOUCHPOINT_STATUS_REG, 0)?;
        Ok(point)
    }

    /// Reads up to 5 touch points into a stack-allocated vector.
    pub fn get_multi_touch(
        &self,
        i2c: &mut I2C,
    ) -> Result<heapless::Vec<Point, MAX_NUM_TOUCHPOINTS>, Error<E>> {
        let num_touch_points = self.get_num_touch_points(i2c)?;

        let points = if num_touch_points > 0 {
            assert!(num_touch_points <= MAX_NUM_TOUCHPOINTS);
            let mut points = heapless::Vec::new();

            let mut read = [0u8; TOUCHPOINT_ENTRY_LEN * MAX_NUM_TOUCHPOINTS];
            self.read(
                i2c,
                CST9217_TOUCHPOINT_1_REG,
                &mut read[..TOUCHPOINT_ENTRY_LEN * num_touch_points],
            )?;

            for n in 0..num_touch_points {
                let start = n * TOUCHPOINT_ENTRY_LEN;
                points
                    .push(decode_point(&read[start..start + TOUCHPOINT_ENTRY_LEN]))
                    .ok();
            }

            points
        } else {
            heapless::Vec::new()
        };

        self.write(i2c, CST9217_TOUCHPOINT_STATUS_REG, 0)?;
        Ok(points)
    }

    fn get_num_touch_points(&self, i2c: &mut I2C) -> Result<usize, Error<E>> {
        let mut read = [0u8; 1];
        self.read(i2c, CST9217_TOUCHPOINT_STATUS_REG, &mut read)?;

        let status = read[0];
        let ready = (status & 0x80) > 0;
        let num_touch_points = (status & 0x0F) as usize;

        if ready {
            Ok(num_touch_points)
        } else {
            Err(Error::NotReady)
        }
    }

    fn write(&self, i2c: &mut I2C, register: u16, value: u8) -> Result<(), Error<E>> {
        let register = register.to_be_bytes();
        let cmd = [register[0], register[1], value];
        i2c.write(self.i2c_addr, &cmd).map_err(Error::I2C)
    }

    fn read(&self, i2c: &mut I2C, register: u16, buf: &mut [u8]) -> Result<(), Error<E>> {
        i2c.write_read(self.i2c_addr, &register.to_be_bytes(), buf)
            .map_err(Error::I2C)
    }
}

/// Async CST9217 driver.
pub struct Cst9217<I2C> {
    i2c_addr: u8,
    i2c: PhantomData<I2C>,
}

impl<I2C> Default for Cst9217<I2C> {
    fn default() -> Self {
        Self {
            i2c_addr: CST9217_I2C_ADDR_BA,
            i2c: PhantomData,
        }
    }
}

impl<I2C, E> Cst9217<I2C>
where
    I2C: embedded_hal_async::i2c::I2c<Error = E>,
{
    /// Create a driver using a custom I2C address.
    pub fn new(i2c_addr: u8) -> Self {
        Self {
            i2c_addr,
            i2c: PhantomData,
        }
    }

    /// Initialize the device (requires a temporary read buffer of at least 4 bytes).
    pub async fn init(&self, i2c: &mut I2C, buf: &mut [u8]) -> Result<(), Error<E>> {
        self.write(i2c, CST9217_COMMAND_REG, 0).await?;

        const LEN: usize = 4;
        assert!(buf.len() >= LEN);
        self.read(i2c, CST9217_PRODUCT_ID_REG, &mut buf[..LEN])
            .await?;
        match str::from_utf8(&buf[..LEN]) {
            Ok(product_id) => {
                if product_id != CST9217_EXPECTED_PRODUCT_ID {
                    return Err(Error::UnexpectedProductId);
                }
            }
            Err(_) => return Err(Error::UnexpectedProductId),
        }

        self.write(i2c, CST9217_TOUCHPOINT_STATUS_REG, 0).await?;
        Ok(())
    }

    /// Read a single touch point (buffer must hold at least `TOUCHPOINT_ENTRY_LEN`).
    pub async fn get_touch(
        &self,
        i2c: &mut I2C,
        buf: &mut [u8],
    ) -> Result<Option<Point>, Error<E>> {
        let num_touch_points = self.get_num_touch_points(i2c, buf).await?;

        let point = if num_touch_points > 0 {
            assert!(
                buf.len() >= TOUCHPOINT_ENTRY_LEN,
                "Buffer too small, use GET_TOUCH_BUF_SIZE"
            );
            self.read(
                i2c,
                CST9217_TOUCHPOINT_1_REG,
                &mut buf[..TOUCHPOINT_ENTRY_LEN],
            )
            .await?;
            Some(decode_point(buf))
        } else {
            None
        };

        self.write(i2c, CST9217_TOUCHPOINT_STATUS_REG, 0).await?;
        Ok(point)
    }

    /// Read multiple touch points (buffer must hold `num_touch_points * TOUCHPOINT_ENTRY_LEN`).
    pub async fn get_multi_touch(
        &self,
        i2c: &mut I2C,
        buf: &mut [u8],
    ) -> Result<heapless::Vec<Point, MAX_NUM_TOUCHPOINTS>, Error<E>> {
        let num_touch_points = self.get_num_touch_points(i2c, buf).await?;

        let points = if num_touch_points > 0 {
            assert!(num_touch_points <= MAX_NUM_TOUCHPOINTS);
            let mut points = heapless::Vec::new();

            let len: usize = num_touch_points * TOUCHPOINT_ENTRY_LEN;
            assert!(
                buf.len() >= len,
                "Buffer too small, use GET_MULTITOUCH_BUF_SIZE"
            );
            self.read(i2c, CST9217_TOUCHPOINT_1_REG, &mut buf[..len])
                .await?;

            for n in 0..num_touch_points {
                let start = n * TOUCHPOINT_ENTRY_LEN;
                points
                    .push(decode_point(&buf[start..start + TOUCHPOINT_ENTRY_LEN]))
                    .ok();
            }

            points
        } else {
            heapless::Vec::new()
        };

        self.write(i2c, CST9217_TOUCHPOINT_STATUS_REG, 0).await?;
        Ok(points)
    }

    async fn get_num_touch_points(&self, i2c: &mut I2C, buf: &mut [u8]) -> Result<usize, Error<E>> {
        assert!(!buf.is_empty());
        self.read(i2c, CST9217_TOUCHPOINT_STATUS_REG, &mut buf[..1])
            .await?;

        let status = buf[0];
        let ready = (status & 0x80) > 0;
        let num_touch_points = (status & 0x0F) as usize;

        if ready {
            Ok(num_touch_points)
        } else {
            Err(Error::NotReady)
        }
    }

    async fn write(&self, i2c: &mut I2C, register: u16, value: u8) -> Result<(), Error<E>> {
        let register = register.to_be_bytes();
        let cmd = [register[0], register[1], value];
        i2c.write(self.i2c_addr, &cmd).await.map_err(Error::I2C)
    }

    async fn read(&self, i2c: &mut I2C, register: u16, buf: &mut [u8]) -> Result<(), Error<E>> {
        i2c.write_read(self.i2c_addr, &register.to_be_bytes(), buf)
            .await
            .map_err(Error::I2C)
    }
}

fn decode_point(buf: &[u8]) -> Point {
    assert!(buf.len() >= TOUCHPOINT_ENTRY_LEN);
    Point {
        track_id: buf[0],
        x: u16::from_le_bytes([buf[1], buf[2]]),
        y: u16::from_le_bytes([buf[3], buf[4]]),
        area: u16::from_le_bytes([buf[5], buf[6]]),
    }
}
