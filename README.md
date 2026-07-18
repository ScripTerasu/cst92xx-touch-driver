# CST9217 Touch Controller Driver

Driver crate for the CST9217 capacitive touch controller used on the 1.75" AMOLED modules (e.g., the Waveshare TAH75A). It supplies both blocking and async I²C helpers, shared point/status types, and register/constant definitions so your no-std application can query finger coordinates safely.

## Features

- no-std friendly, re-exported `Point` type, error handling via `Error<E>`.
- Blocking (`Cst9217Blocking<I2C>`) and async (`Cst9217<I2C>`) entry points that target the same register map.
- Shared `registers.rs`, `types.rs`, and `error.rs` so you can reuse constants or integrate the decoder into another driver.
- Optional `defmt` feature for formatted logging of touch events (the `Point`/`Error` types derive `defmt::Format`).

## Usage

### Blocking

```rust
use cst9217::{Cst9217Blocking, Error, Point};
use embedded_hal::blocking::i2c::I2c;

let mut touch = Cst9217Blocking::default();
let mut i2c = /* your I2C peripheral */;
touch.init(&mut i2c)?;
if let Some(point) = touch.get_touch(&mut i2c)? {
    // handle single point
}
```

The blocking helpers expect an `embedded-hal` I²C implementation (`Error = E`). When you need all touch points, use `get_multi_touch`, which returns a `heapless::Vec<Point, MAX_NUM_TOUCHPOINTS>`.

### Async

```rust
use cst9217::{Cst9217, GET_TOUCH_BUF_SIZE};
use embedded_hal_async::i2c::I2c;

let mut driver = Cst9217::default();
let mut temp_buf = [0u8; GET_TOUCH_BUF_SIZE];
driver.init(&mut i2c, &mut temp_buf).await?;
if let Some(point) = driver.get_touch(&mut i2c, &mut temp_buf).await? {
    // handle point
}
```

Async helpers require `embedded-hal-async` and a temporary buffer with at least `GET_TOUCH_BUF_SIZE` bytes for single-point reads. For multi-touch, allocate `GET_MULTITOUCH_BUF_SIZE` bytes.

## Constants

| Name | Description |
| --- | --- |
| `CST9217_COMMAND_REG` | Register used to enter command mode. |
| `CST9217_TOUCHPOINT_STATUS_REG` | Status register that reports readiness and touch count. |
| `CST9217_TOUCHPOINT_1_REG` | Start of the first touchpoint data block. |
| `GET_TOUCH_BUF_SIZE` | Minimum buffer length for reading one point. |
| `GET_MULTITOUCH_BUF_SIZE` | Buffer length for reading up to `MAX_NUM_TOUCHPOINTS` points. |
| `MAX_NUM_TOUCHPOINTS` | Maximum simultaneous contacts (derived from datasheet). |
| `TOUCHPOINT_ENTRY_LEN` | Bytes per touch point. |

## Errors

```rust
pub enum Error<E> {
    UnexpectedProductId, // device is not a CST9217
    I2C(E),              // pass-through I²C error
    NotReady,            // queried before the device had new data
}
```

## Optional `defmt` feature

Enable the `defmt` feature if you want `Point` and `Error` to derive `defmt::Format` for logging:

```toml
[dependencies]
cst9217 = { version = "0.1", features = ["defmt"] }
```

## Development

- `cargo fmt`
- `cargo check`

You can run the standard tooling to ensure the crate compiles before running it on hardware.

## Hardware notes

CST9217 uses a 1.75" AMOLED panel and communicates over I²C. The `TOUCH_POINT` registers return coordinate data packed into 8-byte entries. After you read a touch report, the driver clears the status register to let the controller detect the next frame.

> **TODO:** Once you validate the hardware, adjust `decode_point` or constants to match the exact report format (e.g., buffer layout, number of bytes per finger) if it differs from the initial assumptions.
