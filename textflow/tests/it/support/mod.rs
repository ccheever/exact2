use exact_textflow::{Cursor, LineRange, Options, Prepared};

pub fn advance(text: &str) -> f32 {
    text.chars()
        .map(|ch| match ch {
            '\u{ad}' | '\u{200b}' | '\u{200d}' | '\u{fe0f}' | '\u{fe0e}' | '\n' | '\r'
            | '\u{2028}' | '\u{2029}' | '\u{2060}' => 0.0,
            '\u{300}'..='\u{36f}' | '\u{650}' | '\u{94d}' => 0.0,
            '\u{3000}'..='\u{9fff}' => 16.0,
            _ => 8.0,
        })
        .sum()
}
pub fn prepare(text: &str, options: Options) -> Prepared {
    Prepared::new(text, options, &mut |r: std::ops::Range<usize>| {
        if options.white_space == exact_textflow::WhiteSpace::Normal
            && text[r.clone()].chars().all(char::is_whitespace)
        {
            8.0
        } else {
            advance(&text[r])
        }
    })
}
pub fn lines(p: &Prepared, width: f32) -> Vec<LineRange> {
    let mut result = Vec::new();
    let mut cursor = Cursor::default();
    while let Some(line) = p.next_line(cursor, width) {
        assert!(line.end.byte > cursor.byte);
        cursor = line.end;
        result.push(line);
    }
    result
}
pub const CORPUS: &[&str] = &[
    "",
    "   ",
    "one two three four five",
    "  one   two   ",
    "one\ntwo\n",
    "a\r\nb\u{2028}c",
    "\n\n",
    "ab\u{ad}cdef",
    "中文日本語",
    "「「字」中文",
    "https://example.com/a/b?x=1&y=2",
    "👩‍💻👍🏽❤️ a",
    "e\u{301}e\u{301}",
    "مرحبا بالعالم שלום עולם",
    "foo\u{a0}bar",
    "foo\u{200b}bar",
    "\u{200b}\u{200b}",
];
