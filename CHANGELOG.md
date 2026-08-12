# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `ChipInfo`, holding the chip metadata `get_attribute()` discovers (chip type, panel resolution, project ID, firmware version, checksum), retrievable via `driver.chip_info()`.
- `TouchConfig`, `Orientation`, and `DisplayMapping` for an optional coordinate transform (axis swap, mirroring, scaling to a target display resolution) applied to every `Point` returned by `touches()`. Set it with `driver.with_config(...)`.
- Optional hardware reset pin support via `.with_reset(pin)` on both drivers, defaulting to a no-op `NoResetPin`.
- `CHANGELOG.md` (this file).
- `docs/CST9217.pdf`, the reference datasheet.

### Fixed

- The crate failed to compile: `src/lib.rs` was missing `mod info;`/`mod reset_pin;` declarations, and both drivers referenced `TouchConfig` fields that had moved to `ChipInfo`.
- `reset()` never actually toggled the reset pin on either driver — it only ran the settle delay. It now drives the pin low, waits, then releases it, matching SensorLib's `getAttribute()`, which resets the chip before every attribute read.
- `touches()` never applied `TouchConfig::transform()` to the coordinates it returned, even though the transform was implemented and unit-tested.
- `touches()` could return a second touch slot even when the first slot's event was invalid; SensorLib discards the whole report in that case, and this driver now matches that.
- `cargo build --all-features` (and docs.rs's default build) failed: the `async` and `blocking` features both export a `CST92xx` type at the crate root, colliding when both are enabled. This now fails fast with a clear `compile_error!` instead of a confusing re-export error, and docs.rs is pinned to the default feature set.
- `cargo test` with the default (`async`) feature also tried to compile `tests/blocking.rs`, and vice versa. Both are now declared as `[[test]]` targets gated by `required-features`.
- Two test files (`tests/info.rs`, `tests/types.rs`) were integration tests written as if they were inline unit tests (`use super::*`, which doesn't resolve there); moved inline into `src/info.rs` and `src/types.rs`.
- `REG_CHIP_INFO` duplicated the value of `REG_DEBUG_MODE` and was unused; removed in favor of named constants (`REG_CHIP_TYPE`, `REG_FW_VERSION`, and others) for the registers `get_attribute()`/`set_mode()` actually read and write.
- A false-positive `clippy::large_stack_frames` on the ESP32-S3 example's `touch_task`, caused by the lint counting the whole async state machine (stored in `embassy_executor`'s static task pool) as call-stack usage. Confirmed via `objdump` on the built firmware that the real frame is 192 bytes, and scoped an `#[allow]` to that function instead of raising the crate-wide threshold.
- Assorted stray Spanish-language comments and docstrings translated to English for consistency with the rest of the crate.

### Changed

- `CST92xx`'s async and blocking backends are now gated behind `async` (default) and `blocking` Cargo features, replacing the previous split of always compiling both under different type names (`CST92xx` vs. `BlockingCST92xx`).
- Magic register bytes (e.g. `[0xD1, 0xFC]`) in `get_attribute()`/`set_mode()` replaced with named constants in `registers.rs`.
- `RunMode` variants not implemented by SensorLib's reference `setMode()` (`LowPower`, `DeepSleep`, `Wakeup`, `UpdateFirmware`, `LpScan`) are now documented as unverified — they're mapped to a register by naming convention only, with no reference implementation to validate against.
- README rewritten to match the current API (feature flags, `ChipInfo`, `TouchConfig`, reset pin), replacing stale references to methods and registers that no longer exist.

## [0.1.0] - 2026-07-18

Initial release: async CST92xx driver (`embedded-hal-async`), followed by a blocking counterpart (`embedded-hal` + `DelayNs`), shared register/error/mode types, `defmt` support, and an ESP32-S3 Waveshare AMOLED sample project.

[Unreleased]: https://github.com/ScripTerasu/cst92xx-touch-driver/compare/0.1.0...HEAD
[0.1.0]: https://github.com/ScripTerasu/cst92xx-touch-driver/releases/tag/0.1.0
