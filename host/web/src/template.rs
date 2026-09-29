//! One element's projection, for an ahead-of-time target: the tag, the DOM
//! props and the CSS this host computes for a kernel node, by the same rules
//! the live host and the document writer use ([`tag_for`], [`props_for`],
//! [`crate::css::css_text`], [`host_css`]). The web build's JS target compiles a
//! plan's static rows through this at build, so its stylesheet is this
//! host's CSS, never a second mapping.

use super::{
    browser_kernel, css_style, font_names, host_css, in_button, props_for, svg_props, tag_for,
};
use exact_kernel::{Kernel, SortedMap, ViewId};
use exact_plan::Plan;

/// An element as the live host would create it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parts {
    /// The element's tag (a canvas is still `canvas`; the host wraps it).
    pub tag: String,
    /// DOM props by the DOM's names, before the document's attribute rules.
    pub props: SortedMap<String, String>,
    /// The inline CSS the live host would write, host additions included.
    pub css: String,
    /// Rows this host does not lower, with the reason.
    pub skipped: Vec<(String, &'static str)>,
}

/// The kernel the web host keeps: no layout engine.
pub fn kernel() -> Kernel {
    browser_kernel()
}

/// [`Parts`] for `id` in `kernel`, or `None` for a view it does not hold.
pub fn parts(kernel: &Kernel, plan: &Plan, id: ViewId) -> Option<Parts> {
    let node = kernel.node(id)?;
    let tag = tag_for(&node, in_button(kernel, &node));
    let mut props = props_for(&node);
    svg_props(kernel, &node, &mut props);
    let (text, skipped) = crate::css::css_text(&css_style(kernel, &node), &font_names(plan));
    let css = host_css(&node, text, tag);
    Some(Parts {
        tag: tag.to_string(),
        props,
        css,
        skipped: skipped
            .into_iter()
            .map(|s| (s.row.name().to_string(), s.reason))
            .collect(),
    })
}
