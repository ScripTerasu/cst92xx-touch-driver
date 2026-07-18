# CST92xx Touch Controller Driver

`cst92xx` is a portable driver for the CST92xx controllers (CST9217/CST9220) used on small AMOLED touch panels. It exposes blocking and async entry points, shared register/constants, and no-std-friendly helper types so you can plug it into any embedded project with `embedded-hal`.

## Features

- `#![no_std]` friendly, re-exported `Point` type, and rich error handling via `Error<E>`.
- Blocking (`Cst9217Blocking<I2C>`) and async (`Cst9217<I2C>`) entry points that target the same register map.
- Shared `registers.rs`, `types.rs`, and `error.rs` so you can reuse constants or integrate the decoder into another driver.
- Optional `defmt` feature for formatted logging of touch events (the `Point`/`Error` types derive `defmt::Format`).

## Usage

### Blocking

```rust
use cst92xx::{Cst9217Blocking, Error, Point};
use embedded_hal::blocking::i2c::I2c;

let mut touch = Cst9217Blocking::default();
let mut i2c = /* your I2C peripheral */;
touch.init(&mut i2c)?;
if let Some(point) = touch.get_touch(&mut i2c)? {
    // handle single point
}
```

The blocking helpers expect an `embedded-hal` I²C implementation (`Error = E`). When you need all touch points, use `get_multi_touch`, which returns a `heapless::Vec<Point, { MAX_FINGER_NUM as usize }>`.

### Async

```rust
use cst92xx::{Cst9217, TOUCHPOINT_ENTRY_LEN};
use embedded_hal_async::i2c::I2c;

let mut driver = Cst9217::default();
let mut temp_buf = [0u8; TOUCHPOINT_ENTRY_LEN];
driver.init(&mut i2c, &mut temp_buf).await?;
if let Some(point) = driver.get_touch(&mut i2c, &mut temp_buf).await? {
    // handle point
}
```

Async helpers require `embedded-hal-async` and a temporary buffer with at least `TOUCHPOINT_ENTRY_LEN` bytes for single-point reads. For multi-touch you'll need `TOUCHPOINT_ENTRY_LEN * (MAX_FINGER_NUM as usize)` bytes so the buffer can hold every contact.

Call `driver.get_model_name(&mut i2c)` (and the async variant that takes a temporary buffer) to read `REG_CHIP_INFO` and get a friendly name for the detected controller. If you already have the raw chip ID, `model_name_from_chip_id(chip_id)` maps it to "CST9217", "CST9220", or "UNKNOWN". When the optional `defmt` feature is enabled, the driver also logs the four bytes returned by `REG_CHIP_INFO` (alongside the decoded chip ID) so you can inspect the bootloader response for diagnostics.

## Constants

| Name | Description |
| --- | --- |
| `CST9220_CHIP_ID` | Chip ID reported by CST9220 parts. |
| `CST9217_CHIP_ID` | Chip ID reported by the CST9217 controller. |
| `CST92XX_SLAVE_ADDRESS` | Default I²C address (0x5A). |
| `CST92XX_BOOT_ADDRESS` | Alias used when the controller is in bootloader mode. |
| `CST92XX_ACK` | Value returned by `REG_READ` when the controller is awake. |
| `CST92XX_MEM_SIZE` | Size of the controller's flash (≈31 KB). |
| `REG_READ` | Diagnostic register used to verify that the controller is responding. |
| `REG_CHIP_INFO` | Returns the project and chip identifiers painted during the bootloader handshake. |
| `MAX_FINGER_NUM` | Maximum simultaneous contacts supported by the controller. |
| `PROGRAM_PAGE_SIZE` | Bootloader/program page size (128 bytes). |
| `TOUCHPOINT_ENTRY_LEN` | Bytes per touch point report. |

## Errors

```rust
pub enum Error<E> {
    UnexpectedChipId, // device did not present a supported identifier
    I2C(E),           // pass-through I²C error
    NotReady,         // queried before the device had new data
}
```

## Optional `defmt` feature

Enable the `defmt` feature if you want `Point` and `Error` to derive `defmt::Format` for logging:

```toml
[dependencies]
cst92xx = { version = "0.1", features = ["defmt"] }
```

## Development

- `cargo fmt`
- `cargo check`

You can run the standard tooling to ensure the crate compiles before running it on hardware.

## Hardware notes

CST9217 uses a 1.75" AMOLED panel and communicates over I²C. The `TOUCH_POINT` registers return coordinate data packed into 8-byte entries. After you read a touch report, the driver clears the status register to let the controller detect the next frame.

> **TODO:** Once you validate the hardware, adjust `decode_point` or constants to match the exact report format (e.g., buffer layout, number of bytes per finger) if it differs from the initial assumptions.

## References

- SensorLib `TouchDrvCST92xx.cpp` by Lewis He: https://github.com/lewisxhe/SensorLib/blob/baa3e0b83c256b74d9870a95d96d55595946926c/src/touch/TouchDrvCST92xx.cpp
- SensorLib `TouchDrvCST92xx.hpp` by Lewis He: https://github.com/lewisxhe/SensorLib/blob/baa3e0b83c256b74d9870a95d96d55595946926c/src/touch/TouchDrvCST92xx.hpp
