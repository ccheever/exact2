//! CSS text-decoration-line grammar.
impl crate::generated::TextDecorationLine {
    /// CSS's `text-decoration-line`: one keyword, or `underline` and
    /// `line-through` in either order. The pair's name is CSS's two
    /// keywords, which the web writes as it is (LLP 1081 D2).
    pub fn from_css(text: &str) -> Option<Self> {
        let lower = text.to_ascii_lowercase();
        let mut words: Vec<&str> = lower.split_ascii_whitespace().collect();
        words.sort_unstable();
        match words[..] {
            ["line-through", "underline"] => Some(Self::UnderlineLineThrough),
            [one] => Self::from_name(one),
            _ => None,
        }
    }
}
