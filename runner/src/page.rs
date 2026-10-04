//! The page's facts that are not media features: whether any of it can be
//! seen (the Page Visibility API), whether the device believes it is online
//! (`navigator.onLine`), whether a share sheet exists
//! (`navigator.share`), and whether the document pickers do
//! (`showOpenFilePicker`: studio diary R31, where a picker that closes with
//! nothing cannot say whether the person cancelled or the browser has
//! none). The host observes them; the app reads them by field name from one
//! reserved source.
//! @ref LLP 1069.000 D2; LLP 1069.003 D5; LLP 1069.010 D2

use exact_plan::Value;

/// Reserved resource source, answered before the app data seam.
pub const SOURCE: &str = "exactPage";
/// Fields an app may declare, filled by name.
pub const FIELDS: &[&str] = &["visibilityState", "onLine", "canShare", "canOpenFiles"];

/// What the host last said. The default is the bake's answer: a visible,
/// online page with no share sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Page {
    /// `document.visibilityState == "hidden"`: nothing of the page can be
    /// seen. A window behind another is still visible.
    pub hidden: bool,
    /// `navigator.onLine`: says as little as the web's; `true` promises no
    /// origin can be reached.
    pub on_line: bool,
    /// `typeof navigator.share === "function"` (LLP 1069.003 D5).
    pub can_share: bool,
    /// `typeof showOpenFilePicker === "function"`: `showOpenFilePicker` and
    /// `showDirectoryPicker` open a document in place, where without them
    /// they refuse with `cancel` (LLP 1069.010 D2; a Mac and an iPhone
    /// have them, Linux, Firefox and Safari do not). Save always works: a
    /// browser without its picker downloads.
    pub can_open_files: bool,
}

impl Default for Page {
    fn default() -> Self {
        Self {
            hidden: false,
            on_line: true,
            can_share: false,
            can_open_files: false,
        }
    }
}

impl Page {
    /// The hosts' wire form: bit 0 hidden, bit 1 offline, bit 2 can share,
    /// bit 3 can open files; other bits are ignored. Zero is visible,
    /// online, no share sheet, no pickers.
    pub fn from_bits(bits: u32) -> Self {
        Self {
            hidden: bits & 1 != 0,
            on_line: bits & 2 == 0,
            can_share: bits & 4 != 0,
            can_open_files: bits & 8 != 0,
        }
    }

    /// The inverse of [`Page::from_bits`].
    pub fn bits(self) -> u32 {
        u32::from(self.hidden)
            | (u32::from(!self.on_line) << 1)
            | (u32::from(self.can_share) << 2)
            | (u32::from(self.can_open_files) << 3)
    }

    /// `"visible"` or `"hidden"`, the web's words.
    pub fn visibility_state(self) -> &'static str {
        if self.hidden {
            "hidden"
        } else {
            "visible"
        }
    }

    /// Fill a declared field; unknown names are refused.
    pub fn field(self, name: &str) -> Option<Value> {
        match name {
            "visibilityState" => Some(Value::str(self.visibility_state())),
            "onLine" => Some(Value::Bool(self.on_line)),
            "canShare" => Some(Value::Bool(self.can_share)),
            "canOpenFiles" => Some(Value::Bool(self.can_open_files)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Page;

    #[test]
    fn bits_round_trip_and_zero_is_the_default() {
        assert_eq!(Page::from_bits(0), Page::default());
        for bits in 0..16 {
            assert_eq!(Page::from_bits(bits).bits(), bits);
        }
        let page = Page::from_bits(0b1111);
        assert!(page.hidden && !page.on_line && page.can_share && page.can_open_files);
        assert!(!Page::from_bits(0b111).can_open_files);
        assert_eq!(page.visibility_state(), "hidden");
    }
}
