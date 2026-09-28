//! `filter` and `clip-path` (LLP 1055.000 D10, D14; CSS Filter Effects and
//! CSS Masking): their grammars, linked by use on the web (LLP 1047 D2).
//!
//! The two parsers, and the path grammar `clip-path: path()` reads, are
//! ~4 KiB of a web core that none of Caltrain, RealWorld or the video player
//! uses. A web artifact links them when its plan binds either row, and a
//! plan that binds one unlinked is refused at boot (D6), so an unlinked
//! artifact never parses one. Native artifacts and the compiler parse
//! without it.

use crate::clip::ClipPath;
use crate::svg::filter::FilterList;

/// A row's grammar: its value, or `None` when CSS refuses the text.
type Grammar<T> = fn(&str) -> Option<T>;

/// The grammars a link registers.
pub(crate) struct Effects {
    pub(crate) filter: Grammar<FilterList>,
    pub(crate) clip_path: Grammar<ClipPath>,
}

const GRAMMARS: Effects = Effects {
    filter: FilterList::grammar,
    clip_path: ClipPath::grammar,
};

/// The grammars, once linked ([`link`]). Only a wasm artifact reads it.
#[cfg(target_arch = "wasm32")]
static LINKED: std::sync::OnceLock<&'static Effects> = std::sync::OnceLock::new();

/// Link `filter`'s and `clip-path`'s grammars: until a web artifact calls
/// this, a value for either row does not parse.
pub fn link() {
    #[cfg(target_arch = "wasm32")]
    let _ = LINKED.set(&GRAMMARS);
}

/// `css` by the grammar `pick` names, on the web once linked.
pub(crate) fn parse<T>(css: &str, pick: fn(&Effects) -> Grammar<T>) -> Option<T> {
    #[cfg(target_arch = "wasm32")]
    return LINKED.get().and_then(|effects| pick(effects)(css));
    #[cfg(not(target_arch = "wasm32"))]
    pick(&GRAMMARS)(css)
}
