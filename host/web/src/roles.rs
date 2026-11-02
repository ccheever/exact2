//! The ARIA roles the DOM's elements carry on their own.

/// The ARIA role an element has natively (HTML-AAM), which a page must not
/// restate: a button's `button`, a checkbox's `checkbox`, a link's `link`, a
/// single select's `combobox` (a restated one makes an accessibility checker
/// demand `aria-expanded`, which the native control never carries). `attr`
/// reads the element's other attributes. The JS target applies the same rule
/// to the attributes it writes ahead of time (host/web-js/src/rows.rs).
pub fn implicit_role<'a>(
    element: &str,
    attr: impl Fn(&str) -> Option<&'a str>,
) -> Option<&'static str> {
    match (element, attr("type")) {
        ("button", _) => Some("button"),
        ("input", Some("checkbox")) => Some("checkbox"),
        ("input", Some("radio")) => Some("radio"),
        ("a", _) if attr("href").is_some() => Some("link"),
        ("progress", _) => Some("progressbar"),
        ("select", _) if attr("multiple").is_none() => Some("combobox"),
        _ => None,
    }
}
