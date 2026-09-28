//! `frame` and `measure` on the web (LLP 1051.000 D4): the page answers
//! through one import, reached only through what this registers, so an app
//! whose actions read no geometry carries neither the import nor its glue.

use exact_web::Linked;

/// Link the `geometry` capability into `linked`.
pub const fn link(mut linked: Linked) -> Linked {
    linked.geometry = Some(&exact_web_geometry::PAGE);
    linked
}
