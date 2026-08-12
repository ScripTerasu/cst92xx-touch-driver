/// Chip ID reported by CST9220 parts (see `ChipInfo::chip_type`).
pub const CST9220_CHIP_ID: u16 = 0x9220;
/// Chip ID reported by CST9217 parts (see `ChipInfo::chip_type`).
pub const CST9217_CHIP_ID: u16 = 0x9217;

/// Touch report register read by `touches()`; also used to acknowledge a
/// report once it's been consumed.
pub const REG_READ: u16 = 0xD000;
/// Switches into debug-info mode. Also doubles as the "enter command mode"
/// write `get_attribute()` issues before reading chip attributes — SensorLib
/// uses the same register for both.
pub const REG_DEBUG_MODE: u16 = 0xD101;
/// Puts the controller into deep sleep. Written by both `sleep()` and
/// `set_mode(RunMode::DeepSleep)`.
pub const REG_SLEEP_MODE: u16 = 0xD105;
/// Disables the low-power scan mode. Defined by SensorLib but not currently
/// written by this driver — no `RunMode` variant maps to it yet.
pub const REG_DIS_LOW_POWER_SCAN_MODE: u16 = 0xD106;
/// Switches to normal scanning mode. Written by `set_mode` for both
/// `RunMode::Normal` and `RunMode::Wakeup`.
pub const REG_NORMAL_MODE: u16 = 0xD109;
/// Switches to raw-data debug mode (`RunMode::DebugRawData`).
pub const REG_RAW_MODE: u16 = 0xD10A;
/// Switches to diff debug mode (`RunMode::DebugDiff`).
pub const REG_DIFF_MODE: u16 = 0xD10D;
/// Switches to baseline/low-power scan mode (`RunMode::LpScan`, unverified —
/// see the note on that variant).
pub const REG_BASE_LINE_MODE: u16 = 0xD10E;
/// Switches to low-power mode (`RunMode::LowPower`, unverified — see the
/// note on that variant).
pub const REG_LOW_POWER_MODE: u16 = 0xD10F;
/// Requests factory mode; `set_mode(RunMode::Factory)` polls
/// [`REG_FACTORY_STATUS`] after writing this until the controller reports
/// readiness.
pub const REG_FACTORY_MODE: u16 = 0xD114;
/// Factory high-drive test mode (`RunMode::FactoryHighDrv`).
pub const REG_FACTORY_HIGH_DRV: u16 = 0xD110;
/// Factory low-drive test mode (`RunMode::FactoryLowDrv`).
pub const REG_FACTORY_LOW_DRV: u16 = 0xD111;
/// Factory short-circuit test mode (`RunMode::FactoryShort`).
pub const REG_FACTORY_SHORT: u16 = 0xD112;

/// Reads the firmware checkcode. SensorLib `getAttribute()`, `0xD1/0xFC`.
pub const REG_CHECK_CODE: u16 = 0xD1FC;
/// Reads the panel resolution. SensorLib `getAttribute()`, `0xD1/0xF8`.
pub const REG_RESOLUTION: u16 = 0xD1F8;
/// Reads chip type + project ID. SensorLib `getAttribute()`, `0xD2/0x04`.
pub const REG_CHIP_TYPE: u16 = 0xD204;
/// Reads firmware version + checksum. SensorLib `getAttribute()`, `0xD2/0x08`.
pub const REG_FW_VERSION: u16 = 0xD208;
/// Handshake register polled before writing a new run mode.
pub const REG_MODE_HANDSHAKE: u16 = 0xD11E;
/// Echoes back the last mode command written; used to confirm `set_mode`.
pub const REG_MODE_STATUS: u16 = 0x0002;
/// Factory-mode readiness status, polled by `prepare_factory_mode`.
pub const REG_FACTORY_STATUS: u16 = 0x0009;
/// Command written once factory mode reports ready.
pub const REG_FACTORY_READY: u16 = 0xD119;
/// Enters the bootloader-driven firmware update mode.
///
/// Not present in SensorLib's `setMode()` switch (falls through to its
/// `default: return false`); mapped here by register-naming convention and
/// unverified against real hardware. This driver does not implement the
/// bootloader protocol needed to actually push firmware once in this mode.
pub const REG_UPDATE_FIRMWARE: u16 = 0xD108;

/// The controller's fixed 7-bit I²C address.
pub const CST92XX_SLAVE_ADDRESS: u8 = 0x5A;
/// Address the controller answers to while in bootloader mode. Only
/// meaningful for firmware-update flows, which this driver doesn't
/// implement — kept for parity with SensorLib.
pub const CST92XX_BOOT_ADDRESS: u8 = CST92XX_SLAVE_ADDRESS;
/// Acknowledgment byte written back to [`REG_READ`] after consuming a touch
/// report, and expected in specific bytes of that report.
pub const CST92XX_ACK: u8 = 0xAB;
/// Size of the controller's flash (~31 KB). Only relevant to firmware-update
/// flows, which this driver doesn't implement — kept for parity with
/// SensorLib.
pub const CST92XX_MEM_SIZE: u32 = 0x007F80;

/// Maximum simultaneous touch contacts this driver decodes.
pub const MAX_FINGER_NUM: usize = 2;
/// Bootloader program page size in bytes. Only relevant to firmware-update
/// flows, which this driver doesn't implement — kept for parity with
/// SensorLib.
pub const PROGRAM_PAGE_SIZE: usize = 128;
/// Bytes per touch point entry in a `REG_READ` report.
pub const TOUCHPOINT_ENTRY_LEN: usize = 8;
