//! `input type="file"` and `showPicker` on the web: a file
//! input's picked files read from the page, and the runner's picker entries
//! (what `showPicker` names, the agent's hold, the picked names), linked
//! when the plan has a file input or runs `showPicker`.

use exact_web::Linked;

/// Link the picker into `linked`.
pub const fn link(mut linked: Linked) -> Linked {
    linked.picker = Some(exact_runner::Picked::payload);
    linked
}
