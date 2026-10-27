//! The page's facts that are not media features: whether any of it can be
//! seen (the Page Visibility API), whether the device believes it is online
//! (`navigator.onLine`), whether a share sheet exists
//! (`navigator.share`), whether it takes files and an element's picture
//! (LLP 1125 D12), and whether the document pickers do
//! (`showOpenFilePicker`: studio diary R31, where a picker that closes with
//! nothing cannot say whether the person cancelled or the browser has
//! none), whether the page's window has the system's focus
//! (`document.hasFocus()`, #114), how the browser loaded the page
//! (Navigation Timing's `type`), and whether the page is put away in, and
//! how often it came back from, the back/forward cache (`pagehide` and
//! `pageshow` with `persisted`). The host observes them; the app reads them
//! by field name from one reserved source. The word also carries the
//! device's GPU tier, which an app reads as `exactViewport().gpuTier`.
//! @ref LLP 1069.000 D2; LLP 1069.003 D5; LLP 1069.010 D2, D8; LLP 1125 D12; LLP 1139.003 D2

use exact_plan::Value;

/// Reserved resource source, answered before the app data seam.
pub const SOURCE: &str = "exactPage";
/// Fields an app may declare, filled by name.
pub const FIELDS: &[&str] = &[
    "visibilityState",
    "onLine",
    "canShare",
    "canShareFiles",
    "canSharePictures",
    "canOpenFiles",
    "hasFocus",
    "navigationType",
    "persisted",
    "restores",
    "iframePartition",
];

/// `navigationType`'s words, by their wire code (bits 5–6): the web's
/// `PerformanceNavigationTiming.type`, or none where the app is no page a
/// browser loaded (a native app's launch).
pub const NAVIGATION_TYPES: [&str; 4] = ["", "navigate", "reload", "back_forward"];

/// The GPU tier's words, by their wire code (bits 11–12; 3 reads `low`).
pub const GPU_TIERS: [&str; 4] = ["high", "mid", "low", "low"];

/// What the host last said. The default is the bake's answer: a visible,
/// online page with focus and no share sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Page {
    /// `document.visibilityState == "hidden"`: nothing of the page can be
    /// seen. A window behind another is still visible.
    pub hidden: bool,
    /// Whether the app runs in the background, where the host says so
    /// apart from `hidden` (LLP 1133.025 D11 item 7): a desktop host tells
    /// `false` for a window that is not showing while the person is at the
    /// machine (a menu-bar app, a window closed to the Dock), and `true`
    /// only for a wake or a headless session. `None` where the host does
    /// not tell them apart (iOS, Android, the web), which reads `hidden`.
    pub background: Option<bool>,
    /// `navigator.onLine`: says as little as the web's; `true` promises no
    /// origin can be reached.
    pub on_line: bool,
    /// `typeof navigator.share === "function"` (LLP 1069.003 D5).
    pub can_share: bool,
    /// `share(files=[…])` takes `app:/` files: Apple's sheets, and a
    /// browser whose `navigator.canShare({files})` is true (LLP 1125 D12).
    pub can_share_files: bool,
    /// `share(files=["#id"])` takes an element's picture: iOS and macOS,
    /// never the web (LLP 1125 D12).
    pub can_share_pictures: bool,
    /// `typeof showOpenFilePicker === "function"`: `showOpenFilePicker` and
    /// `showDirectoryPicker` open a document in place, where without them
    /// they refuse with `cancel` (LLP 1069.010 D2; a Mac and an iPhone
    /// have them, Linux, Firefox and Safari do not). Save always works: a
    /// browser without its picker downloads.
    pub can_open_files: bool,
    /// An `iframe`'s `partition` is honoured: its guest is a top-level page
    /// with that partition's storage, so the app chooses its fallback before
    /// a frame mounts (Apple; never the web, LLP 1020.002 D1).
    pub iframe_partition: bool,
    /// `document.hasFocus()`: the page's window is the one the system's
    /// keyboard focus is in (window `focus` and `blur` move it). A visible
    /// window behind another's has no focus; a hidden page never has it.
    pub has_focus: bool,
    /// How the browser loaded the page, for its whole life, as an index
    /// into [`NAVIGATION_TYPES`] (0 where no browser loaded it).
    pub navigation: u8,
    /// The host is about to suspend the app (iOS and tvOS in the
    /// background, unless a `playback` session keeps it running): a
    /// sequence stops (LLP 1125 D4). No `exactPage` field reads it.
    pub suspended: bool,
    /// The page is put away, kept whole, as the back/forward cache keeps
    /// one: from its `pagehide` with `persisted` true until its `pageshow`.
    /// A Mac window hidden by its close (LLP 1069.010 D8); a minimized or
    /// covered one is only hidden.
    pub persisted: bool,
    /// How many times this launch the page came back from being put away:
    /// its `pageshow` events with `persisted` true. Counted modulo 2^16.
    pub restores: u16,
    /// The device's GPU tier as an index into [`GPU_TIERS`]: chosen once,
    /// before the GPU device is made, from the adapter's facts or the
    /// screen's class; `high` (0) under the agent until `prefer gpu-tier`
    /// (LLP 1139.003 D2). Read as `exactViewport().gpuTier`.
    pub gpu_tier: u8,
}

impl Default for Page {
    fn default() -> Self {
        Self {
            hidden: false,
            background: None,
            on_line: true,
            can_share: false,
            can_share_files: false,
            can_share_pictures: false,
            can_open_files: false,
            iframe_partition: false,
            has_focus: true,
            navigation: 0,
            suspended: false,
            persisted: false,
            restores: 0,
            gpu_tier: 0,
        }
    }
}

impl Page {
    /// The hosts' wire form: bit 0 hidden, bit 1 offline, bit 2 can share,
    /// bit 3 can open files, bit 4 without focus, bits 5–6 the navigation
    /// type, bit 7 about to be suspended, bit 8 shares files, bit 9 shares
    /// an element's picture, bit 10 put away (`persisted`), bits 11–12 the
    /// GPU tier (0 high, 1 mid, 2 low), bit 13 an `iframe`'s `partition`
    /// honoured, bit 14 the host tells `background` and bit 15 its value,
    /// bits 16–31 the restores; other bits
    /// are ignored. Zero is visible, online, focused, no share sheet, no
    /// pickers, no navigation, never put away, a `high` GPU tier.
    pub fn from_bits(bits: u32) -> Self {
        Self {
            hidden: bits & 1 != 0,
            background: (bits & (1 << 14) != 0).then_some(bits & (1 << 15) != 0),
            on_line: bits & 2 == 0,
            can_share: bits & 4 != 0,
            can_open_files: bits & 8 != 0,
            has_focus: bits & 16 == 0,
            navigation: ((bits >> 5) & 3) as u8,
            suspended: bits & 128 != 0,
            can_share_files: bits & 256 != 0,
            can_share_pictures: bits & 512 != 0,
            persisted: bits & 1024 != 0,
            gpu_tier: ((bits >> 11) & 3) as u8,
            iframe_partition: bits & (1 << 13) != 0,
            restores: (bits >> 16) as u16,
        }
    }

    /// The inverse of [`Page::from_bits`].
    pub fn bits(self) -> u32 {
        u32::from(self.hidden)
            | (u32::from(!self.on_line) << 1)
            | (u32::from(self.can_share) << 2)
            | (u32::from(self.can_open_files) << 3)
            | (u32::from(!self.has_focus) << 4)
            | (u32::from(self.navigation & 3) << 5)
            | (u32::from(self.suspended) << 7)
            | (u32::from(self.can_share_files) << 8)
            | (u32::from(self.can_share_pictures) << 9)
            | (u32::from(self.persisted) << 10)
            | (u32::from(self.gpu_tier & 3) << 11)
            | (u32::from(self.iframe_partition) << 13)
            | self.background.map_or(0, |b| 1 << 14 | u32::from(b) << 15)
            | (u32::from(self.restores) << 16)
    }

    /// Whether the app runs in the background: what the host told, else
    /// whether the page is hidden. Admission's `"foreground"`, the package
    /// context's `foreground` and the sampled sources' pause read it; Page
    /// Visibility reads `hidden` (LLP 1133.025 D11 item 7).
    pub fn in_background(self) -> bool {
        self.background.unwrap_or(self.hidden)
    }

    /// The GPU tier's word: `low`, `mid` or `high`.
    pub fn gpu_tier(&self) -> &'static str {
        GPU_TIERS[usize::from(self.gpu_tier & 3)]
    }

    /// How the browser loaded the page, the web's word, or `""` in a
    /// native app.
    pub fn navigation_type(&self) -> &'static str {
        NAVIGATION_TYPES[usize::from(self.navigation & 3)]
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
            "canShareFiles" => Some(Value::Bool(self.can_share_files)),
            "canSharePictures" => Some(Value::Bool(self.can_share_pictures)),
            "canOpenFiles" => Some(Value::Bool(self.can_open_files)),
            "hasFocus" => Some(Value::Bool(self.has_focus)),
            "navigationType" => Some(Value::str(self.navigation_type())),
            "persisted" => Some(Value::Bool(self.persisted)),
            "restores" => Some(Value::Number(f64::from(self.restores))),
            "iframePartition" => Some(Value::Bool(self.iframe_partition)),
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
        for bits in (0..2048).chain([1 << 16, 3 << 16 | 1024, 1 << 13, !0xC000, !0x8000, !0]) {
            assert_eq!(Page::from_bits(bits).bits(), bits);
        }
        // LLP 1133.025 D11 item 7: background follows hidden unless told.
        assert!(Page::from_bits(1).in_background());
        assert!(!Page::from_bits(1 | 1 << 14).in_background());
        assert!(Page::from_bits(1 << 14 | 1 << 15).in_background());
        assert!(!Page::from_bits(1 << 14 | 1 << 15).hidden);
        // Put away, then back once: bit 10 and the count above bit 16.
        let away = Page::from_bits(1024);
        assert!(away.persisted && away.restores == 0);
        assert_eq!(
            Page::from_bits(2 << 16).field("restores"),
            Some(exact_plan::Value::Number(2.0))
        );
        assert_eq!(away.field("persisted"), Some(exact_plan::Value::Bool(true)));
        assert_eq!(
            Page::default().field("restores"),
            Some(exact_plan::Value::Number(0.0))
        );
        // LLP 1139.003 D2: bits 11–12, zero `high`, beside `persisted` (bit 10).
        assert_eq!(Page::default().gpu_tier(), "high");
        assert_eq!(Page::from_bits(1 << 11).gpu_tier(), "mid");
        assert_eq!(Page::from_bits(2 << 11 | 1024).gpu_tier(), "low");
        assert!(Page::from_bits(2 << 11 | 1024).persisted);
        for bits in [1 << 11, 2 << 11 | 1024 | 5 << 16] {
            assert_eq!(Page::from_bits(bits).bits(), bits);
        }
        let page = Page::from_bits(0b1111);
        assert!(page.hidden && !page.on_line && page.can_share && page.can_open_files);
        assert!(!Page::from_bits(0b111).can_open_files);
        let files = Page::from_bits(256);
        assert!(files.can_share_files && !files.can_share_pictures && !files.can_share);
        assert!(Page::from_bits(512).can_share_pictures);
        // LLP 1020.002 D1: bit 13, the web's zero `false`.
        assert!(Page::from_bits(1 << 13).iframe_partition && !Page::default().iframe_partition);
        assert_eq!(
            Page::from_bits(1 << 13).field("iframePartition"),
            Some(exact_plan::Value::Bool(true))
        );
        assert_eq!(
            Page::from_bits(2 << 5).field("navigationType"),
            Some(exact_plan::Value::str("reload"))
        );
        assert_eq!(
            Page::default().field("navigationType"),
            Some(exact_plan::Value::str(""))
        );
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
