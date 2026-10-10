//! Grouped row decoration: one background layer in the row's existing box.
use exact_kernel::{NodeFacts, NodeType, PropId};

pub(crate) fn row(node: &NodeFacts<'_>, mut css: String) -> String {
    if node.props.bool(PropId::GroupedRowSeparator).is_none() {
        return css;
    }
    let value = |name: &str, fallback: &str| {
        css.split(';')
            .filter_map(|d| d.strip_prefix(name))
            .next_back()
            .unwrap_or(fallback)
            .to_string()
    };
    let native =
        node.node_type == NodeType::Control && node.props.str(PropId::Type) == Some("button");
    let inset = value("padding-left:", if native { "16px" } else { "0px" });
    let color = value("border-bottom-color:", "light-dark(#3c3c431f, #54545880)");
    let transition = value("transition:", "");
    // Background colour uses the last layer's clip; authored image and
    // attachment keep their own layer, including text clipping and fixed.
    for (name, alias, fallback) in [
        ("background-image:", "--exact-grouped-background:", "none"),
        (
            "background-clip:",
            "--exact-grouped-background-clip:",
            "border-box",
        ),
        (
            "background-attachment:",
            "--exact-grouped-background-attachment:",
            "scroll",
        ),
    ] {
        let v = css
            .split(';')
            .filter_map(|d| d.strip_prefix(name))
            .next_back()
            .unwrap_or(fallback)
            .to_string();
        css.push_str(alias);
        css.push_str(&v);
        css.push(';');
    }
    css.push_str(&format!(
        "--exact-grouped-inset:{inset};--exact-grouped-separator:{color};"
    ));
    if !transition.is_empty() {
        css.push_str("transition:");
        css.push_str(&transition_css(&transition));
        css.push(';');
    }
    css
}

/// Mirror a canonical border-colour transition onto the row's typed ink.
/// `all` already includes the registered custom property; explicit entries
/// keep their order so the browser applies the same last matching entry.
pub fn transition_css(text: &str) -> String {
    let mut out = Vec::new();
    let mut start = 0;
    let mut depth = 0;
    for (i, c) in text
        .char_indices()
        .chain(std::iter::once((text.len(), ',')))
    {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                let entry = text[start..i].trim();
                out.push(entry.to_string());
                if let Some(rest) = entry
                    .strip_prefix("border-bottom-color ")
                    .or_else(|| entry.strip_prefix("border-color "))
                {
                    out.push(format!("--exact-grouped-separator {rest}"));
                }
                start = i + 1;
            }
            _ => {}
        }
    }
    out.join(",")
}

/// Keep separator ink in the same keyframes as its authored border colour.
/// This also serves server documents and animations adopted by the JS target.
pub fn keyframes_css(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find("border-bottom-color:") {
        let Some(end) = rest[at..].find(';').map(|i| at + i) else {
            break;
        };
        out.push_str(&rest[..=end]);
        out.push_str("--exact-grouped-separator:");
        out.push_str(&rest[at + "border-bottom-color:".len()..=end]);
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn colour_motion_keeps_its_curve_delay_and_last_matching_entry() {
        assert_eq!(super::transition_css("border-bottom-color 1s cubic-bezier(0,0,1,1) 2s,opacity 3s,border-color 4s linear"),
            "border-bottom-color 1s cubic-bezier(0,0,1,1) 2s,--exact-grouped-separator 1s cubic-bezier(0,0,1,1) 2s,opacity 3s,border-color 4s linear,--exact-grouped-separator 4s linear");
        assert_eq!(super::keyframes_css("@keyframes ink{0%{border-bottom-color:#0000ff;}100%{border-bottom-color:#ff0000;opacity:.5;}}"),
            "@keyframes ink{0%{border-bottom-color:#0000ff;--exact-grouped-separator:#0000ff;}100%{border-bottom-color:#ff0000;--exact-grouped-separator:#ff0000;opacity:.5;}}");
    }
}
