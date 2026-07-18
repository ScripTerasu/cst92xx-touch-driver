# CST92xx Touch Controller Driver

`cst92xx` is a portable async driver for the CST92xx controllers (CST9217/CST9220) used on small AMOLED touch panels. It exposes no-std-friendly helpers, shared register/constants, and the `RunMode` enum so you can plug it into any embedded project built on `embedded-hal_async`.

## Features

- `#![no_std]` friendly with optional `defmt` logging for instrumentation.
- `CST92xx<I2C>` exposes `init`, `touches`, `sleep`, and `set_mode` so you can reproduce the SensorLib flow while staying async.
- The crate re-exports `RunMode` at the root, so you can import it alongside `CST92xx` without reaching into submodules.
- Shared `registers.rs`, `types.rs`, and `error.rs` let you reuse constants or integrate the decoder directly into another driver.
- Optional `defmt` feature makes `Point`, `TouchConfig`, `RunMode`, and `Error` printable for debugging.

## Usage example (async)

```rust
use cst92xx::{CST92xx, RunMode};
use embedded_hal_async::i2c::I2c;

let mut driver = CST92xx::new(i2c);

// 1. Initialize the controller (reset + attribute read)
driver.init().await?;

// 2. Fetch all touch points (up to `MAX_FINGER_NUM` entries)
let touches = driver.touches().await?;
for point in touches.iter().flatten() {
    // handle point
}

// 3. Enter a low-power or debug mode when needed
driver.set_mode(RunMode::LowPower).await?;
```

- `touches()` reads the `REG_READ` report and returns an array, filtering inactive slots automatically.
- `sleep()` and `set_mode()` mirror SensorLib’s command sequence for switching run modes.
- Use `driver.get_model_name()` (or `model_name_from_chip_id`) to log the controller identity; enabling `defmt` also reports the raw `REG_CHIP_INFO` bytes for diagnostics.

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
    InvalidFirmware,
    InvalidCheckCode,
    InvalidChipType(u16),
    I2C(E),           // pass-through I²C error
    NotReady,         // queried before the device had new data
}
```

## Optional `defmt` feature

Enable the `defmt` feature if you want the helper types and log statements to derive `defmt::Format`:

```toml
[dependencies]
cst92xx = { version = "0.1", features = ["defmt"] }
```

## Development

- `cargo fmt`
- `cargo check`

Run the usual tooling before deploying to hardware.

## Hardware notes

This driver has been tested with the Waveshare ESP32-S3 Touch AMOLED 1.75C module: https://docs.waveshare.com/ESP32-S3-Touch-AMOLED-1.75C. It exposes a CST9217 controller and communicates over I²C. The `TOUCH_POINT` registers return coordinate data packed into 8-byte entries, and after you read a touch report the driver clears the status register so the controller can detect the next frame.

### Wiring (ESP32-S3)

- I²C SDA → GPIO15
- I²C SCL → GPIO14
- IRQ (touch interrupt) → GPIO40
- RESET / RST pin → GPIO11 (assert low to reset)
- Power the module with 3.3 V and keep the touch controller powered before releasing reset.

> **TODO:** Once you validate the hardware, adjust `decode_point` or constants to match the exact report format (e.g., buffer layout, number of bytes per finger).

## References

- SensorLib `TouchDrvCST92xx.cpp` by Lewis He: https://github.com/lewisxhe/SensorLib/blob/baa3e0b83c256b74d9870a95d96d55595946926c/src/touch/TouchDrvCST92xx.cpp
- SensorLib `TouchDrvCST92xx.hpp` by Lewis He: https://github.com/lewisxhe/SensorLib/blob/baa3e0b83c256b74d9870a95d96d55595946926c/src/touch/TouchDrvCST92xx.hpp
