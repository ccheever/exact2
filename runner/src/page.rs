//! The page's facts that are not media features: whether any of it can be
//! seen (the Page Visibility API), whether the device believes it is online
//! (`navigator.onLine`), whether a share sheet exists
//! (`navigator.share`), and whether the document pickers do
//! (`showOpenFilePicker`: studio diary R31, where a picker that closes with
//! nothing cannot say whether the person cancelled or the browser has
//! none), and whether the page's window has the system's focus
//! (`document.hasFocus()`, #114). The host observes them; the app reads them
//! by field name from one reserved source.
//! @ref LLP 1069.000 D2; LLP 1069.003 D5; LLP 1069.010 D2

use exact_plan::Value;

/// Reserved resource source, answered before the app data seam.
pub const SOURCE: &str = "exactPage";
/// Fields an app may declare, filled by name.
pub const FIELDS: &[&str] = &[
    "visibilityState",
    "onLine",
    "canShare",
    "canOpenFiles",
    "hasFocus",
];

/// What the host last said. The default is the bake's answer: a visible,
/// online page with focus and no share sheet.
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
    /// `document.hasFocus()`: the page's window is the one the system's
    /// keyboard focus is in (window `focus` and `blur` move it). A visible
    /// window behind another's has no focus; a hidden page never has it.
    pub has_focus: bool,
}

impl Default for Page {
    fn default() -> Self {
        Self {
            hidden: false,
            on_line: true,
            can_share: false,
            can_open_files: false,
            has_focus: true,
        }
    }
}

impl Page {
    /// The hosts' wire form: bit 0 hidden, bit 1 offline, bit 2 can share,
    /// bit 3 can open files, bit 4 without focus; other bits are ignored.
    /// Zero is visible, online, focused, no share sheet, no pickers.
    pub fn from_bits(bits: u32) -> Self {
        Self {
            hidden: bits & 1 != 0,
            on_line: bits & 2 == 0,
            can_share: bits & 4 != 0,
            can_open_files: bits & 8 != 0,
            has_focus: bits & 16 == 0,
        }
    }

    /// The inverse of [`Page::from_bits`].
    pub fn bits(self) -> u32 {
        u32::from(self.hidden)
            | (u32::from(!self.on_line) << 1)
            | (u32::from(self.can_share) << 2)
            | (u32::from(self.can_open_files) << 3)
            | (u32::from(!self.has_focus) << 4)
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
            "hasFocus" => Some(Value::Bool(self.has_focus)),
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
        for bits in 0..32 {
            assert_eq!(Page::from_bits(bits).bits(), bits);
        }
        let page = Page::from_bits(0b1111);
        assert!(page.hidden && !page.on_line && page.can_share && page.can_open_files);
        assert!(!Page::from_bits(0b111).can_open_files);
        assert_eq!(page.visibility_state(), "hidden");
        // Bit 4 says the page has no focus, so zero (the bake's) has it.
        assert!(page.has_focus && Page::default().has_focus);
        assert!(!Page::from_bits(16).has_focus);
        assert_eq!(
            Page::from_bits(16).field("hasFocus"),
            Some(exact_plan::Value::Bool(false))
        );
    }
}
