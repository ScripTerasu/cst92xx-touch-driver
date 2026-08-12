# `esp32s3-sample`

This subproject (`examples/esp32s3-sample`) started from the `esp-generate` template for the ESP32-S3 (ESP-HAL, Embassy, defmt, and `zed`). We replaced the stock "Hello world" loop with a CST92xx touch demo that runs inside `esp-rtos` and logs touch points via `defmt`.

It is **not** a workspace member of the crate at the repository root — it has its own `Cargo.lock` and toolchain pin, and is built from within this directory.

## What's inside?

- `Cargo.toml`: defines the `esp32s3-sample` binary and pulls in `esp-hal`, `esp-rtos`, `embassy`, `defmt`, and the supporting ecosystem crates, plus `cst92xx` via a `path = "../.."` dependency.
- `src/bin/main.rs`: brings up the ESP32-S3 clocks, an async I²C bus, and the RST pin, spawns a `touch_task` that initializes the CST92xx driver and polls `touches()` in a loop, and logs chip attributes and coordinates via `defmt`.
- `.cargo/`, `.clippy.toml`, `rust-toolchain.toml`, and `build.rs`: boilerplate from `esp-generate` to pin the toolchain and lint rules.

## How to run it

1. Install the ESP toolchain (if you haven't yet):
   ```sh
   cargo install espup
   espup install
   ```
   This installs the `xtensa-esp` targets and helper tools such as `espflash` and `probe-rs`.

2. From **this directory** (`examples/esp32s3-sample`), build or run the demo:
   ```sh
   cargo build --release
   cargo run --release
   ```
   The binary targets `#![no_std]` and links against `esp-rtos`, so task setup happens automatically.

3. Flash the binary to your board (replace `/dev/ttyUSB0` with the correct port):
   ```sh
   cargo espflash --release /dev/ttyUSB0
   ```
   Use the `defmt` feature built into `espflash` or pipe the ELF through `defmt-print` to decode the logs.

4. You can also monitor the serial port directly at 115200 bps using `minicom`, `picocom`, or `screen` if you need raw output.

## Integrating the CST92xx driver

This template shows the minimal CST92xx flow:

- `main()` configures I²C (SDA on GPIO15, SCL on GPIO14, 400 kHz, async), drives RST on GPIO11 as a push-pull output idling high (the controller resets on a low pulse — see the driver [README](../../README.md#wiring-esp32-s3)), attaches it with `.with_reset(rst)`, and spawns `touch_task`.
- `touch_task` calls `touch_driver.init()` once at startup, which pulses RST and reads the chip attributes; on success it logs the model name, panel resolution, and firmware version, then loops calling `touch_driver.touches()` every 15 ms (~66 Hz) and logging each detected point's `track_id`, `x`, and `y`. A failed `init()` or a per-poll I²C error is logged with the actual `cst92xx::Error`/`esp_hal` error value, not just a generic message.
- Keep `cst92xx = { path = "../..", features = ["defmt"] }` in `Cargo.toml` to reuse the driver crate from this repository; swap the `path` dependency for a version from crates.io in your own project.

Once you confirm the wiring (I²C on GPIO15/14, RST on GPIO11), extend `touch_task` with your own gesture logic, or set an orientation/display mapping via `CST92xx::new(i2c, delay).with_reset(rst).with_config(config)`.

> Update this README whenever you change the build/flash workflow or the demo's behavior so it stays accurate for future flashes.
