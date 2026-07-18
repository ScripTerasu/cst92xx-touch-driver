# `esp32s3-sample`

This subproject (`examples/esp32s3-sample`) started from the `esp-generate` template for the ESP32-S3 (ESP-HAL, Embassy, defmt, and `zed`). We replaced the stock “Hello world” loop with a CST92xx touch demo that runs inside `esp-rtos`, logs touch points via `defmt`, and keeps a maintenance task that periodically resets the controller or switches mode.

## What’s inside?

- `Cargo.toml`: defines the `esp32s3-sample` binary and pulls in `esp-hal`, `esp-rtos`, `embassy`, `defmt`, and the supporting ecosystem crates.
- `src/bin/main.rs`: starts the RTOS, spins up the CST92xx driver task, and launches a maintenance task that issues timed commands (reset/mode changes) while the touch task logs coordinates.
- `.cargo/`, `.clippy.toml`, `rust-toolchain.toml`, and `build.rs`: boilerplate from `esp-generate` to pin the toolchain and lint rules.

## How to run it

1. Install the ESP toolchain (if you haven’t yet):
   ```sh
   cargo install espup
   espup install
   ```
   This installs the `xtensa-esp` targets and helper tools such as `espflash` and `probe-rs`.

2. Build or run the demo directly from the workspace root:
   ```sh
   cargo build -p esp32s3-sample --release
   cargo run -p esp32s3-sample --release
   ```
   The binary targets `#![no_std]` and links against `esp-rtos`, so the emulator task setup happens automatically.

3. Flash the binary to your board (replace `/dev/ttyUSB0` with the correct port):
   ```sh
   cargo espflash -p esp32s3-sample --release /dev/ttyUSB0
   ```
   Use the `defmt` feature built into `espflash` or pipe the ELF through `defmt-print` to decode the logs.

4. You can also monitor the serial port directly at 115200 bps using `minicom`, `picocom`, or `screen` if you need raw output.

## Integrating the CST92xx driver

This template already shows the CST92xx flow:

- `touch_task` initializes the driver and continuously logs the touch points returned by `touch_driver.touches()`.
- `maintenance_task` talks to the same driver via an `embassy_sync::channel`, sending resets (and future `RunMode` commands) once per minute.
- Keep `cst92xx = { path = "../..", features = ["defmt"] }` in `Cargo.toml` to reuse the driver crate from the workspace.

Once you confirm the wiring (I²C on GPIO15/14, IRQ on GPIO40), you can copy/paste `touch_task` into `examples/esp32s3-touch` or extend it with your gesture logic.

## Need a different demo?

If you prefer a more specialized touch example, try `examples/esp32s3-touch` in the repository root: it already resets the CST92xx/CST9217 controller, reads touches, and logs coordinates through `defmt`.

> Update this README whenever you change the build/flash workflow so it stays accurate for future flashes.
