//! `touch-action`: what a value leaves to the platform (LLP 1057.001 §2).

impl crate::generated::TouchAction {
    /// Whether the value leaves pinch zoom to the platform: `auto`,
    /// `manipulation` or any value naming `pinch-zoom` (LLP 1057.001 §2).
    pub fn pinch_zoom(self) -> bool {
        matches!(self, Self::Auto | Self::Manipulation) || self.name().ends_with("pinch-zoom")
    }
    /// The same value's pan axes alone: `pinch-zoom` dropped, which leaves
    /// `none` when it named nothing else. What a pan decides by.
    pub fn pans(self) -> Self {
        match self.name().strip_suffix("pinch-zoom") {
            Some("") => Self::None,
            Some(rest) => Self::from_name(rest.trim_end()).unwrap_or(Self::None),
            None => self,
        }
    }
}
