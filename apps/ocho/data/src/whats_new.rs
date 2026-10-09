//! What's New (whats_new.rs, ui/whats_new_ui.rs; #259): the recent changes
//! bundled with this build, never the latest remote build. Upstream's
//! build script bundles `git log --first-parent -60
//! --format=%H%x09%cs%x09%s HEAD` into the binary; here the app's module
//! hands the same text in (`WhatsNew::new`), so no `fleet` command is asked.
//! Opened from the View menu ("What’s New…"), the palette ("What’s New —
//! recent changes", command `WhatsNew`) and the Settings page's "What’s
//! New" button; opening it closes Settings first.
//!
//! `view` is the `WhatsNew` shape:
//!
//! ```text
//! shape WhatsNewRow
//!   kind: string        // date | change
//!   id: string          // "whats-new-change-N" (a change), "" (a date)
//!   date: string        // a date row's "2026-09-30"
//!   title: string       // a change's subject
//!   commit: string      // a change's short hash (7)
//!   url: string         // "https://github.com/eiiot/fleet/commit/{hash}"
//! shape WhatsNew
//!   visible: bool
//!   title: string       // "What’s New"
//!   build: string       // "Running build abc1234 · 2026-09-30", "" without history
//!   subtitle: string    // "Recent changes included in this version of Ocho and Fleet."
//!   rows: list<WhatsNewRow>
//!   empty: string       // the no-history text, "" with history
//!   footer: string      // "{n} recent changes · Click a change to view on GitHub · ↑ ↓ scroll"
//!   closeLabel: string  // "Close", hint "Esc" (id "whats-new-close")
//!   scrollSeq: number   // moves when a key scrolls; apply once per value
//!   scrollBy: number    // px: ±40 per ↑/↓, ±page (0.6 × window height) per PageUp/PageDown/Space
//!   scrollTo: string    // "top" | "bottom" | ""
//! ```
//!
//! Drawing (whats_new_ui.rs, 1:1; TEXT_SM 12 px, TEXT_XS 11 px): a dialog
//! card (ui.rs `card`: 680 px wide, `max_w_full`, `max_h_full`, column, bg
//! `elevated`, `rounded_lg` (8), `border_1` `border`, `shadow_lg`, clipped),
//! centred on the 45 % black backdrop (`py_6`). Header row `px_4 pt_3 pb_2`
//! (16 / 12 / 8): "What’s New" semibold `flex_1`, the Close button (ui.rs
//! `action_button`, hint "Esc" 11 px `muted`). Then `px_4 pb_2`, 12 px
//! `muted`: the build line and the subtitle, one per line. The rows (id
//! "whats-new-rows") scroll, `px_4 pb_3`: a date row `pt_4 pb_2` (16 / 8),
//! 11 px semibold `muted`; a change row `px_2 py_2` `rounded_md` (6), hover
//! bg `hover`, a row `gap_3` (12) of the title (12 px, `flex_1`, wrapping)
//! and the short hash (11 px Menlo `muted`); a click opens `url`. The empty
//! text `py_4` `muted`. Footer `px_4 py_3`, `border_t_1` `border`, 11 px
//! `muted`.

use crate::picker::canon;
use serde_json::{json, Value as Json};

/// The commit page a change links to.
pub const COMMIT_URL: &str = "https://github.com/eiiot/fleet/commit/";
/// The dialog's width, px.
pub const WIDTH: f64 = 680.0;
/// ↑ / ↓ scroll, px.
pub const LINE_SCROLL: f64 = 40.0;
/// PageUp / PageDown / Space scroll, as a share of the window's height.
pub const PAGE_SCROLL: f64 = 0.6;
/// The dialog without bundled history.
pub const NO_HISTORY: &str = "Change history isn’t available for this build. Builds made from a Git checkout include recent changes here.";

/// One change: a first-parent commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    /// The full hash (40 hex digits).
    pub commit: String,
    /// The commit date, `YYYY-MM-DD`.
    pub date: String,
    /// The subject line.
    pub title: String,
}

impl Change {
    /// The first seven hex digits.
    pub fn short_commit(&self) -> &str {
        &self.commit[..7]
    }

    /// The change on GitHub.
    pub fn url(&self) -> String {
        format!("{COMMIT_URL}{}", self.commit)
    }
}

/// `hash\tdate\tsubject` lines, malformed ones skipped, git's order kept.
pub fn parse(history: &str) -> Vec<Change> {
    history
        .lines()
        .filter_map(|line| {
            let mut fields = line.splitn(3, '\t');
            let commit = fields.next()?;
            let date = fields.next()?;
            let title = fields.next()?.trim();
            if commit.len() != 40
                || !commit.bytes().all(|b| b.is_ascii_hexdigit())
                || date.is_empty()
                || title.is_empty()
            {
                return None;
            }
            Some(Change {
                commit: commit.into(),
                date: date.into(),
                title: title.into(),
            })
        })
        .collect()
}

/// What a key did in the dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WhatsNewKey {
    /// Esc: the dialog closed.
    Close,
    /// A scroll was asked (`scrollSeq` moved).
    Scrolled,
    /// Any other key: swallowed while the dialog is up.
    Swallowed,
}

/// The dialog.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WhatsNew {
    /// The bundled changes, newest first.
    pub changes: Vec<Change>,
    /// The dialog is up.
    pub open: bool,
    scroll_seq: u64,
    scroll_by: f64,
    scroll_to: &'static str,
}

impl WhatsNew {
    /// The dialog over `history` (the `git log` text the build bundles).
    pub fn new(history: &str) -> Self {
        Self {
            changes: parse(history),
            ..Default::default()
        }
    }

    /// Open it (Settings closes first, the model's part).
    pub fn show(&mut self) {
        self.open = true;
        self.scroll_seq += 1;
        self.scroll_by = 0.0;
        self.scroll_to = "top";
    }

    /// Close it (Esc, "Close", a click on the backdrop).
    pub fn close(&mut self) {
        self.open = false;
    }

    fn scroll(&mut self, by: f64, to: &'static str) -> WhatsNewKey {
        self.scroll_seq += 1;
        self.scroll_by = by;
        self.scroll_to = to;
        WhatsNewKey::Scrolled
    }

    /// A key while the dialog is up; `window_height` sizes a page.
    pub fn key(&mut self, name: &str, window_height: f64) -> WhatsNewKey {
        let page = window_height * PAGE_SCROLL;
        match canon(name).as_str() {
            "escape" => {
                self.close();
                WhatsNewKey::Close
            }
            "down" => self.scroll(LINE_SCROLL, ""),
            "up" => self.scroll(-LINE_SCROLL, ""),
            "pagedown" | "space" => self.scroll(page, ""),
            "pageup" => self.scroll(-page, ""),
            "home" => self.scroll(0.0, "top"),
            "end" => self.scroll(0.0, "bottom"),
            _ => WhatsNewKey::Swallowed,
        }
    }

    /// The `WhatsNew` shape.
    pub fn view(&self) -> Json {
        let mut rows = Vec::new();
        let mut previous = "";
        for (index, change) in self.changes.iter().enumerate() {
            if change.date != previous {
                rows.push(json!({
                    "kind": "date", "id": "", "date": change.date, "title": "", "commit": "", "url": "",
                }));
                previous = &change.date;
            }
            rows.push(json!({
                "kind": "change",
                "id": format!("whats-new-change-{index}"),
                "date": "",
                "title": change.title,
                "commit": change.short_commit(),
                "url": change.url(),
            }));
        }
        let build = self
            .changes
            .first()
            .map(|c| format!("Running build {} · {}", c.short_commit(), c.date))
            .unwrap_or_default();
        json!({
            "visible": self.open,
            "title": "What’s New",
            "build": build,
            "subtitle": "Recent changes included in this version of Ocho and Fleet.",
            "rows": rows,
            "empty": if self.changes.is_empty() { NO_HISTORY } else { "" },
            "footer": format!(
                "{} recent changes · Click a change to view on GitHub · ↑ ↓ scroll",
                self.changes.len()
            ),
            "closeLabel": "Close",
            "scrollSeq": self.scroll_seq,
            "scrollBy": self.scroll_by,
            "scrollTo": self.scroll_to,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_git_order_and_unicode_subjects() {
        let history = concat!(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\t2026-09-30\tAdd What's New… (#258)\n",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\t2026-09-29\tFix tabs\tand spaces\n",
        );
        let changes = parse(history);
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].date, "2026-09-30");
        assert_eq!(changes[0].title, "Add What's New… (#258)");
        assert_eq!(changes[0].short_commit(), "aaaaaaa");
        assert_eq!(changes[1].title, "Fix tabs\tand spaces");
        assert_eq!(
            changes[0].url(),
            "https://github.com/eiiot/fleet/commit/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
    }

    #[test]
    fn missing_or_malformed_history_is_safe() {
        assert!(parse("").is_empty());
        assert!(parse("invalid\nshort\t2026-09-30\ttitle\n").is_empty());
        assert!(parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\t2026-09-30\t  ").is_empty());
        assert!(parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/\t2026-09-30\ttitle").is_empty());
        let v = WhatsNew::new("").view();
        assert_eq!(v["empty"], NO_HISTORY);
        assert_eq!(v["build"], "");
        assert_eq!(
            v["footer"],
            "0 recent changes · Click a change to view on GitHub · ↑ ↓ scroll"
        );
    }

    #[test]
    fn rows_group_by_date_and_keys_scroll() {
        let history = concat!(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\t2026-09-30\tOne\n",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\t2026-09-30\tTwo\n",
            "cccccccccccccccccccccccccccccccccccccccc\t2026-09-29\tThree\n",
        );
        let mut w = WhatsNew::new(history);
        w.show();
        let v = w.view();
        assert_eq!(v["visible"], true);
        assert_eq!(v["build"], "Running build aaaaaaa · 2026-09-30");
        let kinds: Vec<&str> = v["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["kind"].as_str().unwrap())
            .collect();
        assert_eq!(kinds, ["date", "change", "change", "date", "change"]);
        assert_eq!(v["rows"][4]["id"], "whats-new-change-2");
        assert_eq!(w.key("ArrowDown", 1000.0), WhatsNewKey::Scrolled);
        assert_eq!(w.view()["scrollBy"], 40.0);
        w.key("PageUp", 1000.0);
        assert_eq!(w.view()["scrollBy"], -600.0);
        w.key("End", 1000.0);
        assert_eq!(w.view()["scrollTo"], "bottom");
        assert_eq!(w.key("q", 1000.0), WhatsNewKey::Swallowed);
        assert_eq!(w.key("Escape", 1000.0), WhatsNewKey::Close);
        assert!(!w.open);
    }
}
