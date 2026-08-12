pub const CST9220_CHIP_ID: u16 = 0x9220;
pub const CST9217_CHIP_ID: u16 = 0x9217;

pub const REG_READ: u16 = 0xD000;
pub const REG_DEBUG_MODE: u16 = 0xD101;
pub const REG_SLEEP_MODE: u16 = 0xD105;
pub const REG_DIS_LOW_POWER_SCAN_MODE: u16 = 0xD106;
pub const REG_NORMAL_MODE: u16 = 0xD109;
pub const REG_RAW_MODE: u16 = 0xD10A;
pub const REG_DIFF_MODE: u16 = 0xD10D;
pub const REG_BASE_LINE_MODE: u16 = 0xD10E;
pub const REG_LOW_POWER_MODE: u16 = 0xD10F;
pub const REG_FACTORY_MODE: u16 = 0xD114;
pub const REG_FACTORY_HIGH_DRV: u16 = 0xD110;
pub const REG_FACTORY_LOW_DRV: u16 = 0xD111;
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

pub const CST92XX_SLAVE_ADDRESS: u8 = 0x5A;
pub const CST92XX_BOOT_ADDRESS: u8 = CST92XX_SLAVE_ADDRESS;
pub const CST92XX_ACK: u8 = 0xAB;
pub const CST92XX_MEM_SIZE: u32 = 0x007F80;

pub const MAX_FINGER_NUM: usize = 2;
pub const PROGRAM_PAGE_SIZE: usize = 128;
pub const TOUCHPOINT_ENTRY_LEN: usize = 8;
