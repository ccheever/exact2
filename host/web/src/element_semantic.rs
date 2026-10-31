//! The HTML element a node's `semanticTag` names, where it names one: the
//! landmarks, `time`, `dialog`, `hr`, and HTML's table and description list.

/// The element for `tag`, a `semanticTag`; `None` where the node's type
/// decides (a `textarea`, an `option`, an `audio`).
pub(super) fn element(tag: &str) -> Option<&'static str> {
    const ELEMENTS: &[&str] = &[
        "time", "main", "header", "nav", "section", "footer", "article", "aside", "dialog",
        // @ref LLP 1021 D1 — HTML's separator, void: the kernel gives it no
        // children, and its UA rows are the node's own.
        "hr",
        // @ref LLP 1153 D7 — HTML's own elements, laid out by the browser's
        // table algorithm and its UA sheet (`display: table` and the rest);
        // the plan carries no grid rows for them.
        "table", "caption", "thead", "tbody", "tfoot", "tr", "th", "td", "dl", "dt", "dd",
    ];
    ELEMENTS.iter().find(|e| **e == tag).copied()
}

/// Whether HTML's UA sheet makes the element `tag` names something other
/// than a block (`table-cell`, `table-caption`): a box that holds a folded
/// text keeps that display, where a `div` is made a block (`blocks`).
pub(super) fn not_block(tag: &str) -> bool {
    matches!(
        tag,
        "table" | "caption" | "thead" | "tbody" | "tfoot" | "tr" | "th" | "td"
    )
}
