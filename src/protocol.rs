//! Pure, I/O-free pieces of the CST92xx protocol shared between the blocking
//! and async drivers.
//!
//! Everything here takes already-read bytes and returns a decoded/validated
//! result — no I2C, no delays. Keeping these in one place means a protocol
//! fix (or bug) only has to be made once instead of drifting between the two
//! driver implementations, which is exactly what happened with the reset
//! pin before this module existed.

use crate::info::{ChipInfo, Point};
use crate::mode::RunMode;
use crate::registers::{
    CST92XX_ACK, CST9217_CHIP_ID, CST9220_CHIP_ID, MAX_FINGER_NUM, REG_BASE_LINE_MODE,
    REG_DEBUG_MODE, REG_DIFF_MODE, REG_FACTORY_HIGH_DRV, REG_FACTORY_LOW_DRV, REG_FACTORY_SHORT,
    REG_LOW_POWER_MODE, REG_NORMAL_MODE, REG_RAW_MODE, REG_SLEEP_MODE, REG_UPDATE_FIRMWARE,
};
use crate::types::TouchConfig;

/// Attribute-read validation failures, before the caller maps them into
/// `Error<E>` (this type never needs to know about the I2C error type).
pub(crate) enum AttributeError {
    Firmware,
    CheckCode,
    ChipType(u16),
}

/// Validate an already-decoded `ChipInfo` against SensorLib's `getAttribute()`
/// checks (unflashed firmware, garbled checkcode, unsupported chip type).
pub(crate) fn validate_chip_info(check_code: u32, info: &ChipInfo) -> Result<(), AttributeError> {
    if info.fw_version == 0xA5A5_A5A5 {
        return Err(AttributeError::Firmware);
    }
    if (check_code & 0xFFFF_0000) != 0xCACA_0000 {
        return Err(AttributeError::CheckCode);
    }
    if info.chip_type != CST9217_CHIP_ID && info.chip_type != CST9220_CHIP_ID {
        return Err(AttributeError::ChipType(info.chip_type));
    }
    Ok(())
}

/// The register bytes `set_mode()` should write for a given [`RunMode`].
///
/// `RunMode::Factory` has no fixed bytes — it needs the factory-readiness
/// handshake (`prepare_factory_mode()` in each driver), which is I/O-bound
/// and therefore not something this pure module can do.
pub(crate) enum ModeBytes {
    Fixed([u8; 2]),
    NeedsFactoryHandshake,
}

pub(crate) fn mode_bytes(mode: RunMode) -> ModeBytes {
    match mode {
        RunMode::Normal => ModeBytes::Fixed(REG_NORMAL_MODE.to_be_bytes()),
        RunMode::LowPower => ModeBytes::Fixed(REG_LOW_POWER_MODE.to_be_bytes()),
        RunMode::DeepSleep => ModeBytes::Fixed(REG_SLEEP_MODE.to_be_bytes()),
        RunMode::Wakeup => ModeBytes::Fixed(REG_NORMAL_MODE.to_be_bytes()),
        RunMode::DebugDiff => ModeBytes::Fixed(REG_DIFF_MODE.to_be_bytes()),
        RunMode::DebugRawData => ModeBytes::Fixed(REG_RAW_MODE.to_be_bytes()),
        RunMode::Factory => ModeBytes::NeedsFactoryHandshake,
        RunMode::DebugInfo => ModeBytes::Fixed(REG_DEBUG_MODE.to_be_bytes()),
        RunMode::UpdateFirmware => ModeBytes::Fixed(REG_UPDATE_FIRMWARE.to_be_bytes()),
        RunMode::FactoryHighDrv => ModeBytes::Fixed(REG_FACTORY_HIGH_DRV.to_be_bytes()),
        RunMode::FactoryLowDrv => ModeBytes::Fixed(REG_FACTORY_LOW_DRV.to_be_bytes()),
        RunMode::FactoryShort => ModeBytes::Fixed(REG_FACTORY_SHORT.to_be_bytes()),
        RunMode::LpScan => ModeBytes::Fixed(REG_BASE_LINE_MODE.to_be_bytes()),
    }
}

/// Decode a `REG_READ` report into touch points, applying `config`'s
/// coordinate transform.
///
/// Callers must have already checked whether `buffer` is all-zero (in which
/// case no acknowledgment should be sent and there's nothing to decode) and
/// sent the acknowledgment write themselves — both are I/O concerns this
/// pure function doesn't touch. Everything from there on (validity bytes,
/// cover-screen gesture, per-point unpacking, and SensorLib's "clear
/// everything if the first slot is invalid" rule) lives here.
pub(crate) fn decode_touch_report(
    buffer: &[u8],
    config: &TouchConfig,
    panel_resolution: (u16, u16),
) -> [Option<Point>; MAX_FINGER_NUM] {
    let mut points: [Option<Point>; MAX_FINGER_NUM] = [None; MAX_FINGER_NUM];

    if buffer[0] == CST92XX_ACK || buffer[0] == 0x00 {
        return points;
    }
    if buffer[6] != CST92XX_ACK {
        return points;
    }
    if (buffer[4] & 0xF0) != 0 && (buffer[4] >> 7) == 0x01 {
        return points;
    }

    let num_points = (buffer[5] & 0x7F) as usize;
    if num_points > MAX_FINGER_NUM || num_points == 0 {
        return points;
    }

    // `i` also drives the non-uniform `start_idx` stride into `buffer`, not just the
    // write into `points`, so `enumerate()` over `points` doesn't fit cleanly here.
    #[allow(clippy::needless_range_loop)]
    for i in 0..num_points {
        let start_idx = (i * 5) + if i == 0 { 0 } else { 2 };
        let pdat = &buffer[start_idx..start_idx + 4];

        let id = pdat[0] >> 4;
        let event = pdat[0] & 0x0F;

        if event == 0x06 && (id as usize) < MAX_FINGER_NUM {
            let raw_x = ((pdat[1] as u16) << 4) | ((pdat[3] >> 4) as u16);
            let raw_y = ((pdat[2] as u16) << 4) | ((pdat[3] & 0x0F) as u16);
            let (x, y) = config.transform(panel_resolution, raw_x, raw_y);

            points[i] = Some(Point {
                track_id: id,
                x,
                y,
                area: 0,
            });
        }
    }

    // Mirrors SensorLib: if the first slot never got a valid event, the
    // whole report is discarded even if a later slot parsed one.
    if points[0].is_none() {
        return [None; MAX_FINGER_NUM];
    }

    points
}
