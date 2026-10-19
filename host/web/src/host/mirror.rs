//! What the page has of each view (`Host::mirror`), so a batch sends only
//! what moved.

use exact_kernel::SortedMap;

/// What the page has of a view, to send only what moved.
#[derive(Debug, Clone, Default, PartialEq)]
pub(in crate::host) struct Mirror {
    pub(in crate::host) props: SortedMap<String, String>,
    pub(in crate::host) css: String,
    pub(in crate::host) children: Vec<exact_kernel::ViewId>,
    /// Created inside a `<button>`, where a container is a `<span>`.
    pub(in crate::host) in_button: bool,
    /// Created with handlers (a text that has some is never folded).
    pub(in crate::host) handled: bool,
    /// Its state looks' page rules (LLP 1121 D8, `looks::rules`).
    pub(in crate::host) looks: String,
}
