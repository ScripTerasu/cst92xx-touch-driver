/// Supported run modes for the CST92xx controller.
///
/// Values mirror `CST92_RunMode` from SensorLib's `TouchDrvCST92xx.hpp`.
/// SensorLib's `setMode()` only implements a subset of these — variants
/// documented below as "unverified" fall through to its
/// `default: return false` case in the reference implementation and are
/// mapped to a register here purely by naming convention. Treat those as
/// unproven until you've validated them against real hardware.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RunMode {
    Normal = 0x00,
    /// Unverified: not implemented by SensorLib's `setMode()`.
    LowPower = 0x01,
    /// Unverified: not implemented by SensorLib's `setMode()`.
    DeepSleep = 0x02,
    /// Unverified: not implemented by SensorLib's `setMode()`.
    Wakeup = 0x03,
    DebugDiff = 0x04,
    DebugRawData = 0x05,
    Factory = 0x06,
    DebugInfo = 0x07,
    /// Unverified: not implemented by SensorLib's `setMode()`. This driver
    /// also does not implement the bootloader protocol needed to actually
    /// push firmware once in this mode.
    UpdateFirmware = 0x08,
    FactoryHighDrv = 0x10,
    FactoryLowDrv = 0x11,
    FactoryShort = 0x12,
    /// Unverified: not implemented by SensorLib's `setMode()`.
    LpScan = 0x13,
}
