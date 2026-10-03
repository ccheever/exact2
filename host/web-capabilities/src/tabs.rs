//! A navigation root's tabs in the web's document (LLP 1075.003 §3.7): each
//! tab's stack, as `navigation.js` shows it, linked when a plan binds
//! `aria-controls`, so an app without tabs carries none of the walk.

use exact_web::Linked;

/// Link tabs into `linked`.
pub const fn link(mut linked: Linked) -> Linked {
    linked.tabs = Some(exact_web::document::tab_routes);
    linked
}
