//! CSS `text-box-edge` as the web writes it (Inline Layout 3 §4.2).

/// The `text-box-edge` declaration: one row holds both edges.
pub fn push_edge(edge: &str, out: &mut String) {
    out.push_str("text-box-edge:");
    out.push_str(text_box_edge(edge));
    out.push(';');
}

/// `text-box-edge`'s row as CSS writes it: a lone `cap` or `ex` is no edge
/// in CSS, so its `text` under edge is spelled out.
pub fn text_box_edge(edge: &str) -> &'static str {
    match edge {
        "text" => "text",
        "cap" => "cap text",
        "ex" => "ex text",
        "text-alphabetic" => "text alphabetic",
        "cap-alphabetic" => "cap alphabetic",
        "ex-alphabetic" => "ex alphabetic",
        _ => "auto",
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_edge_is_css_grammar() {
        let cases = [
            ("auto", "auto"),
            ("text", "text"),
            ("cap", "cap text"),
            ("ex", "ex text"),
            ("text-alphabetic", "text alphabetic"),
            ("cap-alphabetic", "cap alphabetic"),
            ("ex-alphabetic", "ex alphabetic"),
        ];
        for (row, css) in cases {
            assert_eq!(super::text_box_edge(row), css);
        }
    }
}
