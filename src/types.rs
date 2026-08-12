#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Orientation {
    pub swap_xy: bool,
    pub mirror_x: bool,
    pub mirror_y: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayMapping {
    pub target_width: u16,
    pub target_height: u16,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TouchConfig {
    pub orientation: Orientation,
    pub display_mapping: Option<DisplayMapping>,
}

impl TouchConfig {
    /// Chainable: sets the display resolution touch coordinates should be
    /// mapped to. Can be called before or after `init()` — the order never
    /// matters because nothing is cached.
    ///
    /// # Example
    /// ```
    /// # use cst92xx::TouchConfig;
    /// let config = TouchConfig::default().with_target_resolution(320, 240);
    /// ```
    pub fn with_target_resolution(mut self, width: u16, height: u16) -> Self {
        self.display_mapping = Some(DisplayMapping {
            target_width: width,
            target_height: height,
        });
        self
    }

    /// Applies swap → scale (if `display_mapping` is set and the panel's raw
    /// resolution is known) → mirror → clamp, in that order — the same order
    /// SensorLib uses in `updateXY()`.
    ///
    /// `panel_resolution` is the resolution the chip reported via
    /// `get_attribute()` (exposed as `ChipInfo`); this config neither knows
    /// nor owns it, so the driver passes it in on every call.
    ///
    /// This is a pure function: it doesn't mutate `self` or cache anything,
    /// so the order in which `orientation`/`display_mapping` are configured
    /// relative to `init()` never matters.
    pub(crate) fn transform(
        &self,
        panel_resolution: (u16, u16),
        mut x: u16,
        mut y: u16,
    ) -> (u16, u16) {
        if self.orientation.swap_xy {
            core::mem::swap(&mut x, &mut y);
        }

        let (x_max, y_max) = match self.display_mapping {
            Some(map) if panel_resolution.0 > 0 && panel_resolution.1 > 0 => {
                let scale_x = map.target_width as f32 / panel_resolution.0 as f32;
                let scale_y = map.target_height as f32 / panel_resolution.1 as f32;
                x = (x as f32 * scale_x + 0.5) as u16;
                y = (y as f32 * scale_y + 0.5) as u16;
                (map.target_width, map.target_height)
            }
            // A mapping is set but the panel resolution isn't known yet
            // (init() hasn't run): skip scaling, but still honor the
            // bounds for mirror/clamp.
            Some(map) => (map.target_width, map.target_height),
            None => (0, 0),
        };

        if self.orientation.mirror_x && x_max > 0 {
            x = x_max.saturating_sub(x);
        }
        if self.orientation.mirror_y && y_max > 0 {
            y = y_max.saturating_sub(y);
        }
        if x_max != 0 {
            x = x.min(x_max);
        }
        if y_max != 0 {
            y = y.min(y_max);
        }

        (x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_mapping_returns_raw_coordinates() {
        let cfg = TouchConfig::default();
        assert_eq!(cfg.transform((240, 320), 10, 50), (10, 50));
    }

    #[test]
    fn swap_xy_swaps_before_anything_else() {
        let cfg = TouchConfig {
            orientation: Orientation {
                swap_xy: true,
                ..Default::default()
            },
            display_mapping: None,
        };
        assert_eq!(cfg.transform((240, 320), 10, 50), (50, 10));
    }

    #[test]
    fn mirror_x_flips_within_target_bounds() {
        let cfg = TouchConfig {
            orientation: Orientation {
                mirror_x: true,
                ..Default::default()
            },
            ..TouchConfig::default().with_target_resolution(240, 320)
        };
        assert_eq!(cfg.transform((240, 320), 10, 50).0, 230);
    }

    #[test]
    fn scaling_applies_before_mirroring() {
        // 100x100 panel -> 200x200 display, with mirror_x.
        let cfg = TouchConfig {
            orientation: Orientation {
                mirror_x: true,
                ..Default::default()
            },
            ..TouchConfig::default().with_target_resolution(200, 200)
        };
        // x=10 on the panel -> scales to 20 -> mirror: 200-20=180
        assert_eq!(cfg.transform((100, 100), 10, 10).0, 180);
    }

    #[test]
    fn mapping_set_before_panel_resolution_known_skips_scaling_but_still_clamps() {
        let cfg = TouchConfig::default().with_target_resolution(240, 320);
        // panel_resolution = (0, 0): init() hasn't run yet.
        let (x, y) = cfg.transform((0, 0), 300, 400); // out of range
        assert_eq!((x, y), (240, 320)); // clamp still applies
    }

    #[test]
    fn mirror_never_panics_on_out_of_range_input() {
        // x=300 > x_max=240: saturating_sub must not panic.
        let cfg = TouchConfig {
            orientation: Orientation {
                mirror_x: true,
                ..Default::default()
            },
            ..TouchConfig::default().with_target_resolution(240, 320)
        };
        let (x, _) = cfg.transform((240, 320), 300, 0);
        assert_eq!(x, 0); // saturating_sub(240, 300) = 0
    }
}
