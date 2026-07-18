/// Represents a single touch on the screen.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Point {
    /// The touchpoint number (zero based).
    pub track_id: u8,
    /// X coordinate in screen pixels.
    pub x: u16,
    /// Y coordinate in screen pixels.
    pub y: u16,
    /// The touch area (roughly the contact size).
    pub area: u16,
}
