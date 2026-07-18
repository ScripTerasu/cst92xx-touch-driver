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
pub const REG_CHIP_INFO: u16 = 0xD101;

pub const CST92XX_SLAVE_ADDRESS: u8 = 0x5A;
pub const CST92XX_BOOT_ADDRESS: u8 = CST92XX_SLAVE_ADDRESS;
pub const CST92XX_ACK: u8 = 0xAB;
pub const CST92XX_MEM_SIZE: u32 = 0x007F80;

pub const MAX_FINGER_NUM: u8 = 2;
pub const PROGRAM_PAGE_SIZE: usize = 128;
pub const TOUCHPOINT_ENTRY_LEN: usize = 8;
