//! A bound `appearance` says `none` as element.rs marks a written one
//! (`data-exact-said~="appearance"`, LLP 1075.003 §9.10): the bar's rules
//! (neutral.css, index.html) and navigation.js, which projects after every
//! commit, read it, so a header bound to `none` is no bar while it is.

use super::Em;
use std::fmt::Write as _;

impl Em<'_> {
    /// The node's said words (`said`, the element's static ones) with
    /// `appearance` while binding `f` reads `none`.
    pub(super) fn said_appearance(&mut self, e: &str, f: &str, said: Option<&String>) {
        let words: Vec<&str> = said
            .map(|s| {
                s.split(' ')
                    .filter(|w| !w.is_empty() && *w != "appearance")
                    .collect()
            })
            .unwrap_or_default();
        let words = serde_json::to_string(&words).unwrap();
        let p = self.uses.rt("P");
        let _ = write!(
            self.out,
            "{p}({e},\"data-exact-said\",()=>{{const v=({f})(),w={words};if(/^\\s*none\\s*$/i.test(v??\"\"))w.push(\"appearance\");return w.length?w.join(\" \"):null}});"
        );
    }
}
