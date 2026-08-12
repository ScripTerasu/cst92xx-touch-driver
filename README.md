# CST92xx Touch Controller Driver

[![CI](https://github.com/ScripTerasu/cst92xx-touch-driver/actions/workflows/ci.yml/badge.svg)](https://github.com/ScripTerasu/cst92xx-touch-driver/actions/workflows/ci.yml)

`cst92xx` is a `no_std` driver for the CST92xx family of capacitive touch controllers (CST9217, CST9220) used on small AMOLED touch panels, ported from [SensorLib's `TouchDrvCST92xx`][sensorlib-cpp] C++ driver to idiomatic `embedded-hal`. It exposes one `CST92xx` type backed by either async or blocking I²C, shared register/type modules, and the `RunMode` enum so you can plug it into any embedded project.

See [CHANGELOG.md](CHANGELOG.md) for release history.

## Feature flags

| Feature | Default | Effect |
| --- | --- | --- |
| `async` | yes | `CST92xx::new(i2c, delay)` over `embedded_hal_async::i2c::I2c` + `embedded_hal_async::delay::DelayNs`. |
| `blocking` | no | `CST92xx::new(i2c, delay)` over `embedded_hal::i2c::I2c` + `embedded_hal::delay::DelayNs`. |
| `defmt` | no | Derives `defmt::Format` on `Point`, `ChipInfo`, `TouchConfig`, `RunMode`, and `Error` for logging. |

`async` and `blocking` both export a `CST92xx` type at the crate root and are **mutually exclusive** — enabling both (e.g. `cargo build --all-features`) fails to compile with a clear error. To use the blocking driver:

```toml
[dependencies]
cst92xx = { version = "0.1", default-features = false, features = ["blocking"] }
```

## Usage example (async)

```rust
use cst92xx::{CST92xx, RunMode};
use embedded_hal_async::delay::DelayNs;
use embedded_hal_async::i2c::I2c;

let mut driver = CST92xx::new(i2c, delay); // any embedded_hal_async::delay::DelayNs works,
                                            // e.g. embassy_time::Delay if you already use embassy
// Optional: attach a real RST line and/or an orientation/display mapping.
// let mut driver = CST92xx::new(i2c, delay).with_reset(rst_pin).with_config(config);

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

## Usage example (blocking)

```rust
use cst92xx::{CST92xx, RunMode};
use embedded_hal::delay::DelayNs;
use embedded_hal::i2c::I2c;

fn scan<I2C, D, E>(i2c: I2C, delay: D) -> Result<(), cst92xx::Error<E>>
where
    I2C: I2c<Error = E>,
    D: DelayNs,
{
    let mut driver = CST92xx::new(i2c, delay);

    driver.init()?;

    let touches = driver.touches()?;
    for point in touches.iter().flatten() {
        // handle point
    }

    driver.set_mode(RunMode::LowPower)?;
    Ok(())
}
```

## API overview

| Item | Description |
| --- | --- |
| `CST92xx<I2C, Delay>` (feature `async`) | Async driver over `embedded-hal-async::i2c::I2c` + `embedded-hal-async::delay::DelayNs`. Provides `init`, `touches`, `sleep`, `set_mode`, `chip_info`, `model_name`, `with_reset`, and `with_config`. |
| `CST92xx<I2C, Delay>` (feature `blocking`) | Sync counterpart over `embedded_hal::i2c::I2c` + `embedded_hal::delay::DelayNs`. Mirrors the async API so business logic reads the same between runtimes. |
| `ChipInfo` | Chip metadata discovered by `init()`/`get_attribute()` — chip type, panel resolution, project ID, firmware version, checksum. Read-only; fetch it with `driver.chip_info()`. |
| `Point` | Touch descriptor returned by `touches()`, already passed through `TouchConfig::transform()`. Includes `track_id`, `(x, y)`, and `area` (currently always `0` — the controller report this driver decodes doesn't carry contact area). |
| `TouchConfig` / `Orientation` / `DisplayMapping` | Optional coordinate transform (axis swap, mirroring, scaling to a target display resolution) applied to every `Point` from `touches()`. Set it via `driver.with_config(...)`; see `TouchConfig::with_target_resolution`. |
| `RunMode` | Enum describing every controller mode (normal, debug, factory, etc.) for `set_mode`. Several variants aren't implemented by SensorLib's reference `setMode()` and are mapped here by register-naming convention only — see the per-variant docs before relying on them. |
| `NoResetPin` | Default `RST` type when no hardware reset pin is attached (via `.with_reset(pin)`). `reset()` still runs its settle delay, just without toggling anything. |
| `Error<E>` | Driver error type, generic over the I²C error type `E`. |

- `touches()` reads the `REG_READ` report, filters inactive slots, and applies `TouchConfig::transform()` before returning.
- `sleep()` and `set_mode()` mirror SensorLib's command sequence for switching run modes.
- Use `driver.model_name()` for a human-readable chip name, or `driver.chip_info()` for the full `ChipInfo` (resolution, firmware version, checksum, etc.); enabling `defmt` also logs these during `get_attribute()`.

## Reset pin

Both drivers default to `NoResetPin`, a no-op `OutputPin` — `reset()` still waits out the settle delay but never drives a real pin, which is fine if you only rely on power-on reset. To drive a real `RST` line:

```rust
let mut driver = CST92xx::new(i2c, delay).with_reset(rst_pin);
```

The low-pulse width used before releasing reset is a conservative default, not sourced from the datasheet — tune it if your hardware needs a different value.

## Constants

The full register map and protocol constants live in [`registers.rs`](src/registers.rs) (see docs.rs for the complete, documented list). The most commonly useful ones:

| Name | Description |
| --- | --- |
| `CST9220_CHIP_ID` / `CST9217_CHIP_ID` | Chip IDs reported by each part, matched against `ChipInfo::chip_type`. |
| `CST92XX_SLAVE_ADDRESS` | Default I²C address (0x5A). |
| `MAX_FINGER_NUM` | Maximum simultaneous contacts this driver decodes. |

## Errors

```rust
pub enum Error<E> {
    InvalidFirmware,   // get_attribute() saw an unflashed chip
    InvalidCheckCode,  // get_attribute() saw a garbled attribute read
    InvalidChipType(u16), // get_attribute() saw an unsupported chip type
    I2C(E),            // pass-through I²C error
    NotReady,          // set_mode()/touches() queried before the device responded as expected
}
```

## Optional `defmt` feature

Enable the `defmt` feature if you want the helper types and log statements to derive `defmt::Format`:

```toml
[dependencies]
cst92xx = { version = "0.1", features = ["defmt"] }
```

## Development

```sh
cargo fmt
cargo test                                           # default (async) feature
cargo test --no-default-features --features blocking
cargo clippy --all-targets -- -D warnings
cargo clippy --no-default-features --features blocking --all-targets -- -D warnings
```

Run the usual tooling before deploying to hardware. `cargo build --all-features` is expected to fail — `async` and `blocking` are mutually exclusive (see [Feature flags](#feature-flags)).

## Hardware notes

This driver targets the CST9217 controller on the [Waveshare ESP32-S3 Touch AMOLED 1.75C module](https://docs.waveshare.com/ESP32-S3-Touch-AMOLED-1.75C), communicating over I²C. The touch report registers return coordinate data packed into 8-byte entries, and after reading a report the driver acknowledges it so the controller can detect the next frame.

A few behaviors are ported from SensorLib but not yet re-validated against physical hardware after the most recent refactor, and are worth re-checking if you hit issues:

- The reset pulse timing (see [Reset pin](#reset-pin)).
- The `RunMode` variants documented as unverified (not implemented by SensorLib's reference `setMode()`).
- `touches()` skips the acknowledgment write entirely when the raw report buffer reads back all-zero, which SensorLib's reference implementation doesn't do — SensorLib always acknowledges after a successful read, regardless of content.

### Wiring (ESP32-S3)

- I²C SDA → GPIO15
- I²C SCL → GPIO14
- IRQ (touch interrupt) → GPIO40
- RESET / RST pin → GPIO11 (assert low to reset)
- Power the module with 3.3 V and keep the touch controller powered before releasing reset.

## References

- [SensorLib `TouchDrvCST92xx.cpp`][sensorlib-cpp] by Lewis He
- [SensorLib `TouchDrvCST92xx.hpp`](https://github.com/lewisxhe/SensorLib/blob/baa3e0b83c256b74d9870a95d96d55595946926c/src/touch/TouchDrvCST92xx.hpp) by Lewis He

[sensorlib-cpp]: https://github.com/lewisxhe/SensorLib/blob/baa3e0b83c256b74d9870a95d96d55595946926c/src/touch/TouchDrvCST92xx.cpp
