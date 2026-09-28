//! `openAuthSession` on the web (LLP 1069.006): the page's word on a
//! session and the runner's checks of one, linked when the app grants
//! `auth.session` (the generated entry reads the bake's grant ceiling), so
//! an app that signs nobody in carries none of it.

use exact_web::Linked;

/// Link auth into `linked`.
pub const fn link(mut linked: Linked) -> Linked {
    linked.auth = true;
    linked
}
