//! Exact run output around UTF-8 cursor boundaries and bare-URL recognition.
use markdown_parse::{parse, Kind, Run};

fn linked(text: &str, target: &str) -> Run {
    Run {
        text: text.into(),
        href: format!("resolved:{target}"),
        ..Run::default()
    }
}

fn runs(source: &str) -> Vec<Run> {
    let doc = parse(source, &|target| format!("resolved:{target}"));
    assert_eq!(doc.blocks.len(), 1);
    assert_eq!(doc.blocks[0].kind, Kind::Paragraph);
    doc.blocks.into_iter().next().unwrap().runs
}

#[test]
fn bare_urls_preserve_unicode_and_leave_punctuation_and_unicode_spaces_outside() {
    let first = "https://example.com/é/東京?q=🦀";
    let second = "http://example.net/路径";
    assert_eq!(
        runs(&format!(
            "café 🦀 東京 {first}, then {second}!\u{2003}here e\u{301}"
        )),
        vec![
            Run::text("café 🦀 東京 "),
            linked(first, first),
            Run::text(", then "),
            linked(second, second),
            Run::text("!\u{2003}here e\u{301}")
        ],
    );
}

#[test]
fn every_inline_cursor_path_can_be_followed_by_a_unicode_bare_url() {
    let url = "https://example.com/尾🦀";
    let cases = [
        ("é🦀h ", vec![Run::text("é🦀h ")]),
        ("\\*é h ", vec![Run::text("*é h ")]),
        ("\\éh ", vec![Run::text("\\éh ")]),
        (
            "`é🦀`h ",
            vec![
                Run {
                    text: "é🦀".into(),
                    code: true,
                    ..Run::default()
                },
                Run::text("h "),
            ],
        ),
        (
            "**é🦀**h ",
            vec![
                Run {
                    text: "é🦀".into(),
                    bold: true,
                    ..Run::default()
                },
                Run::text("h "),
            ],
        ),
        (
            "*é🦀*h ",
            vec![
                Run {
                    text: "é🦀".into(),
                    italic: true,
                    ..Run::default()
                },
                Run::text("h "),
            ],
        ),
        (
            "[é🦀](./路径)h ",
            vec![linked("é🦀", "./路径"), Run::text("h ")],
        ),
        ("![é🦀](image.png)h ", vec![Run::text("é🦀h ")]),
        (
            "see <https://example.org/é>h ",
            vec![
                Run::text("see "),
                linked("https://example.org/é", "https://example.org/é"),
                Run::text("h "),
            ],
        ),
        ("`é h ", vec![Run::text("`é h ")]),
        ("[é h ", vec![Run::text("[é h ")]),
    ];
    for (prefix, mut expected) in cases {
        expected.push(linked(url, url));
        assert_eq!(
            runs(&format!("{prefix}{url}")),
            expected,
            "prefix: {prefix}"
        );
    }
}

#[test]
fn false_starts_and_short_urls_remain_plain_without_losing_unicode() {
    let text = "h hé ħ http https https:/bad http://a h🦀h 東京 h\u{301}";
    assert_eq!(runs(text), vec![Run::text(text)]);
}

#[test]
fn a_long_unicode_paragraph_keeps_all_text_and_its_final_bare_link() {
    // Repeated ordinary 'h' characters trigger the same path as the 4 MiB
    // stress input. No timing assertion: elapsed-time comparisons are opt-in.
    let body = "h café 🦀 東京 e\u{301} h👩‍👩‍👧‍👦 hello ".repeat(2048);
    let url = "https://example.com/终点";
    let result = runs(&format!("{body}{url}!"));
    assert_eq!(
        result,
        vec![Run::text(&body), linked(url, url), Run::text("!")]
    );
    assert_eq!(markdown_parse::plain(&result), format!("{body}{url}!"));
}
