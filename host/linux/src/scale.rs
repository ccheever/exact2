//! The two lengths the presenter tells the host that no row of the tree
//! says: the root font size `rem` follows (LLP 1069.000 D3), and the
//! display's scale (LLP 1115.000 D10.3), the pixels a point this host draws
//! at, which a hairline (`-exact-hairline`) is one device pixel of. The
//! presenter says the scale at boot and whenever it changes
//! (`Presenter::resize_scaled`): a desktop's `EXACT_SCALE` (1 when unset),
//! the density an Android reader was told, a Windows window's scale factor.
use super::*;

impl<D: DataSource> Host<D> {
    /// A live design batch or snapshot frame from the dev server (LLP 1121
    /// D13, stage 4), set in place in one commit: `None` when applied or
    /// already this build's design, else why not (`stale`: fetch the
    /// snapshot).
    pub fn apply_live(&mut self, frame: &str) -> Option<String> {
        let Some(edit) = exact_runner::design::Edit::parse(frame) else {
            return Some("live: not a tamber batch or snapshot".into());
        };
        match self.runner.apply_live(&edit) {
            Ok(applied) => {
                if applied.full {
                    eprintln!(
                        "exact url: live: a colour was refused (the wide colour table is full)"
                    );
                }
                self.commit(
                    &[Timed {
                        at_ms: self.now_ms,
                        receipt: applied.receipt,
                    }],
                    None,
                )
            }
            Err(exact_runner::LiveRefusal::Stale) => Some("stale".into()),
            Err(exact_runner::LiveRefusal::Structure) => Some("structure".into()),
            Err(exact_runner::LiveRefusal::Refused(why)) => Some(format!("refused: {why}")),
        }
    }

    /// The root font size `rem` follows (LLP 1069.000 D3): a relayout in one
    /// commit, no resource asked again.
    pub fn set_root_font_size(&mut self, px: f64) -> Option<String> {
        match self.runner.set_root_font_size(px) {
            Ok(Some(receipt)) => self.commit(
                &[Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ),
            Ok(None) => None,
            Err(e) => Some(format!("root font size: {e:?}")),
        }
    }

    /// Tell the runner the display's scale: a relayout in one commit where a
    /// row is a hairline, nothing where none is.
    pub fn set_display_scale(&mut self, scale: f64) -> Option<String> {
        match self.runner.set_display_scale(scale) {
            Ok(Some(receipt)) => self.commit(
                &[Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ),
            Ok(None) => None,
            Err(e) => Some(format!("display scale: {e:?}")),
        }
    }

    /// App modes (LLP 1121 D5, the agent's `prefer mode`): the window's
    /// slice set again in one commit, which paints.
    pub fn set_modes(&mut self, pins: &[(&str, &str)]) -> Option<String> {
        match self.runner.set_modes(pins) {
            Ok(Some(receipt)) => self.commit(
                &[Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ),
            Ok(None) => None,
            Err(e) => Some(format!("mode: {e:?}")),
        }
    }
}
