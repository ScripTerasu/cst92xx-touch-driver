use crate::registers::{CST9217_CHIP_ID, CST9220_CHIP_ID};

/// Un punto de toque detectado, ya transformado según `TouchConfig`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Point {
    pub track_id: u8,
    pub x: u16,
    pub y: u16,
    pub area: u16,
}

/// Metadata del chip, descubierta y validada por `get_attribute()`.
/// Solo lectura para el usuario — es estado de hardware, no config.
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
    /// Nombre de modelo derivado de `chip_type`. Calculado en el momento,
    /// no cacheado.
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
