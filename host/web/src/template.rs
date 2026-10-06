//! One element's projection, for an ahead-of-time target: the tag, the DOM
//! props and the CSS this host computes for a kernel node, by the same rules
//! the live host and the document writer use ([`tag_for`], [`props_for`],
//! [`crate::css::css_text`], [`host_css`]). The web build's JS target compiles a
//! plan's static rows through this at build, so its stylesheet is this
//! host's CSS, never a second mapping — but for `rem` and `em`, which it
//! writes as authored ([`crate::css::css_text_relative`]): the page keeps no
//! kernel to re-resolve them when a font size changes, and the browser does.

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
    parts_with(kernel, plan, id, false, false)
}

/// [`parts`], for a node a build knows has nothing the template leaves out
/// that would keep its text its own box (no bound row or prop but its
/// text, no handler, not a repeated row): such a text may fold into its
/// box's content as the live host folds it (LLP 1007.001); `child_may_fold`
/// says the same of the node's only child.
pub fn parts_with(
    kernel: &Kernel,
    plan: &Plan,
    id: ViewId,
    may_fold: bool,
    child_may_fold: bool,
) -> Option<Parts> {
    let node = kernel.node(id)?;
    let tag = tag_for(&node, in_button(kernel, &node));
    let mut props = props_for(&node);
    svg_props(kernel, &node, &mut props);
    let (text, skipped) =
        crate::css::css_text_relative(&css_style(kernel, &node), &font_names(plan));
    let css = super::element::contents(
        host_css(&node, text, tag),
        may_fold && super::element::folded(kernel, &node, false),
        super::element::touch_scoped(kernel, &node),
    );
    let holds = child_may_fold && super::element::holds_folded(kernel, &node, &|_| false);
    let css = super::element::blocks(css, holds);
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
