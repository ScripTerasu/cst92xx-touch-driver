use crate::registers::{CST9217_CHIP_ID, CST9220_CHIP_ID, MAX_FINGER_NUM};

/// A detected touch point, already transformed according to `TouchConfig`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Point {
    pub track_id: u8,
    pub x: u16,
    pub y: u16,
    pub area: u16,
}

/// What one `read_event()` call found in the controller's touch report.
///
/// `touches()` returns the same empty array for "the finger was lifted",
/// "nothing new since the last read" and "invalid frame". That is fine for
/// drawing, but not for press/release logic: on a Waveshare 1.75" board a
/// finger resting on the panel keeps returning the pressed report, lifting
/// it produces one release report (first byte `0x00`), and after that every
/// read returns the acknowledged marker (`0xAB`) until the next touch. This
/// enum keeps those cases apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum TouchEvent {
    /// At least one finger is on the panel. Same array `touches()` returns.
    Pressed([Option<Point>; MAX_FINGER_NUM]),
    /// The controller reported that the finger(s) left the panel (or a
    /// valid report with no active point, e.g. the cover-screen gesture).
    Released,
    /// No new report: the last one was already acknowledged, the frame was
    /// not valid, or the buffer was all zeros.
    NoNewData,
}

/// Chip metadata, discovered and validated by `get_attribute()`.
/// Read-only for callers — this is hardware state, not configuration.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ChipInfo {
    pub chip_type: u16,
    pub resolution_x: u16,
    pub resolution_y: u16,
    pub project_id: u32,
    pub fw_version: u32,
    pub checksum: u32,
}

impl ChipInfo {
    /// Model name derived from `chip_type`. Computed on the fly, not cached.
    pub fn model_name(&self) -> &'static str {
        match self.chip_type {
            CST9220_CHIP_ID => "CST9220",
            CST9217_CHIP_ID => "CST9217",
            _ => "UNKNOWN",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_name_maps_known_chip_ids() {
        let info = ChipInfo {
            chip_type: CST9220_CHIP_ID,
            ..Default::default()
        };
        assert_eq!(info.model_name(), "CST9220");
    }

    #[test]
    fn model_name_unknown_for_unrecognized_id() {
        let info = ChipInfo {
            chip_type: 0xFFFF,
            ..Default::default()
        };
        assert_eq!(info.model_name(), "UNKNOWN");
    }
}
