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
    /// Encadenable: setea hacia qué resolución de pantalla mapear las
    /// coordenadas táctiles. Puede llamarse antes o después de `init()` —
    /// el orden no importa porque nada se cachea.
    ///
    /// # Ejemplo
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

    /// Aplica swap → escala (si hay `display_mapping` y se conoce la
    /// resolución cruda del panel) → mirror → clamp, en ese orden — el mismo
    /// orden que SensorLib usa en `updateXY()`.
    ///
    /// `panel_resolution` es la resolución que el chip reportó en
    /// `get_attribute()` (vía `ChipInfo`); esta config no la conoce ni la
    /// posee, así que se la pasa el driver en cada llamada.
    ///
    /// Es una función pura: no muta `self` ni cachea nada, por lo que el
    /// orden en que se configuran `orientation`/`display_mapping` respecto
    /// a `init()` nunca importa.
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
            // Hay mapping pero aún no se conoce la resolución del panel
            // (init() todavía no corrió): no se escala, pero sí se
            // respetan los bounds para mirror/clamp.
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
        // Panel 100x100 -> pantalla 200x200, con mirror_x.
        let cfg = TouchConfig {
            orientation: Orientation {
                mirror_x: true,
                ..Default::default()
            },
            ..TouchConfig::default().with_target_resolution(200, 200)
        };
        // x=10 en panel -> escala a 20 -> mirror: 200-20=180
        assert_eq!(cfg.transform((100, 100), 10, 10).0, 180);
    }

    #[test]
    fn mapping_set_before_panel_resolution_known_skips_scaling_but_still_clamps() {
        let cfg = TouchConfig::default().with_target_resolution(240, 320);
        // panel_resolution = (0, 0): init() aún no corrió.
        let (x, y) = cfg.transform((0, 0), 300, 400); // fuera de rango
        assert_eq!((x, y), (240, 320)); // clamp sigue aplicando
    }

    #[test]
    fn mirror_never_panics_on_out_of_range_input() {
        // x=300 > x_max=240: saturating_sub no debe panickear.
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
