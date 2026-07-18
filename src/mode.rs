/// Supported run modes for the CST92xx controller.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RunMode {
    Normal = 0x00,
    LowPower = 0x01,
    DeepSleep = 0x02,
    Wakeup = 0x03,
    DebugDiff = 0x04,
    DebugRawData = 0x05,
    Factory = 0x06,
    DebugInfo = 0x07,
    UpdateFirmware = 0x08,
    FactoryHighDrv = 0x10,
    FactoryLowDrv = 0x11,
    FactoryShort = 0x12,
    LpScan = 0x13,
}
