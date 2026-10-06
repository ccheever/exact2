//! The styler, the commands and the segments, over the source string.

use exact_markdown::*;

fn utf16(source: &str, range: Range) -> String {
    let units: Vec<u16> = source.encode_utf16().collect();
    String::from_utf16(&units[range.start as usize..range.end as usize]).unwrap()
}

fn spans(source: &str, reveal: Option<Range>) -> Vec<(String, u8)> {
    style(source, reveal)
        .spans
        .iter()
        .map(|s| (utf16(source, s.range), s.style))
        .collect()
}

fn hidden(source: &str, reveal: Option<Range>) -> Vec<String> {
    style(source, reveal)
        .hidden
        .iter()
        .map(|r| utf16(source, *r))
        .collect()
}

/// Runs a command where `|` marks the selection and returns the same notation.
fn run(marked: &str, command: Command) -> String {
    let first = marked.find('|').expect("a caret");
    let rest = marked[first + 1..].find('|');
    let source: String = marked.replace('|', "");
    let start = marked[..first].encode_utf16().count() as u32;
    let end = rest.map_or(start, |r| {
        start + marked[first + 1..first + 1 + r].encode_utf16().count() as u32
    });
    let edit = edit(&source, Range::new(start, end), command);
    let after = edit.apply(&source);
    let units: Vec<u16> = after.encode_utf16().collect();
    let cut = |a: u32, b: u32| String::from_utf16(&units[a as usize..b as usize]).unwrap();
    let sel = edit.selection;
    if sel.start == sel.end {
        format!(
            "{}|{}",
            cut(0, sel.start),
            cut(sel.start, units.len() as u32)
        )
    } else {
        format!(
            "{}|{}|{}",
            cut(0, sel.start),
            cut(sel.start, sel.end),
            cut(sel.end, units.len() as u32)
        )
    }
}

#[test]
fn reading_hides_every_marker() {
    let source =
        "# Title\n\nSome **bold**, *italic*, `code`, ~~gone~~ and [a link](https://example.com).";
    assert_eq!(
        hidden(source, None),
        [
            "# ",
            "**",
            "**",
            "*",
            "*",
            "`",
            "`",
            "~~",
            "~~",
            "[",
            "](https://example.com)"
        ]
    );
    assert_eq!(
        spans(source, None),
        [
            ("bold".into(), BOLD),
            ("italic".into(), ITALIC),
            ("code".into(), CODE),
            ("gone".into(), STRIKE),
            ("a link".into(), LINK)
        ]
    );
    let styled = style(source, None);
    assert_eq!(styled.paragraphs[0].kind, ParagraphKind::Heading(1));
    assert_eq!(styled.spans[4].href, "https://example.com");
    assert_eq!(
        plain(source),
        "Title\n\nSome bold, italic, code, gone and a link."
    );
}

#[test]
fn the_selection_reveals_its_paragraph_and_no_other() {
    let source = "a **bold** and *italic*\n\nother **paragraph**";
    let caret = Range::caret(5);
    assert_eq!(hidden(source, Some(caret)), ["**", "**"]);
    assert_eq!(
        spans(source, Some(caret)),
        [
            ("**".into(), MARKER),
            ("bold".into(), BOLD),
            ("**".into(), MARKER),
            ("*".into(), MARKER),
            ("italic".into(), ITALIC),
            ("*".into(), MARKER),
            ("paragraph".into(), BOLD),
        ]
    );
    assert_eq!(hidden(source, Some(Range::caret(12))), ["**", "**"]);
    // A selection ending at the next paragraph start leaves that paragraph hidden.
    assert_eq!(hidden(source, Some(Range::new(5, 24))), ["**", "**"]);
}

#[test]
fn nested_emphasis_flattens_into_spans_that_never_overlap() {
    let source = "***both*** and **bold *both* bold**";
    assert_eq!(
        spans(source, None),
        [
            ("both".into(), BOLD | ITALIC),
            ("bold ".into(), BOLD),
            ("both".into(), BOLD | ITALIC),
            (" bold".into(), BOLD)
        ]
    );
    let styled = style(source, None);
    assert!(styled
        .spans
        .windows(2)
        .all(|w| w[0].range.end <= w[1].range.start));
    assert!(styled.hidden.windows(2).all(|w| w[0].end < w[1].start));
}

#[test]
fn words_with_underscores_and_loose_stars_are_not_emphasis() {
    assert!(spans("snake_case_name and 2 * 3 * 4 and a ~single~ tilde", None).is_empty());
    assert_eq!(
        spans("an _emphatic_ word", None),
        [("emphatic".into(), ITALIC)]
    );
    assert_eq!(hidden(r"\*not\* `**code**`", None), ["\\", "\\", "`", "`"]);
}

#[test]
fn ranges_are_utf16_code_units() {
    let source = "🎉 **né** [日本](https://example.com/日本)";
    assert_eq!(
        spans(source, None),
        [("né".into(), BOLD), ("日本".into(), LINK)]
    );
    assert_eq!(style(source, None).spans[0].range, Range::new(5, 7));
    assert_eq!(run("🎉 |né|", Command::Bold), "🎉 **|né|**");
    assert_eq!(run("🎉 **n|é**", Command::Bold), "🎉 n|é");
}

#[test]
fn links_autolinks_and_bare_urls() {
    let source = "See <https://a.example/x>, https://b.example/y(z). And ![alt text](pic.png) or mail <me@example.com>";
    let styled = style(source, None);
    let targets: Vec<(&str, u8)> = styled
        .spans
        .iter()
        .map(|s| (s.href.as_str(), s.style))
        .collect();
    assert_eq!(
        targets,
        [
            ("https://a.example/x", LINK),
            ("https://b.example/y(z)", LINK),
            ("pic.png", IMAGE),
            ("mailto:me@example.com", LINK)
        ]
    );
    assert_eq!(
        spans("**[bold link](u)**", None),
        [("bold link".into(), BOLD | LINK)]
    );
    assert_eq!(
        spans("[*styled* label](u)", None),
        [("styled".into(), ITALIC | LINK), (" label".into(), LINK)]
    );
}

#[test]
fn lists_tasks_quotes_and_rules() {
    let source = "- one\n  - nested\n    continued\n1. first\n- [x] done\n- [ ] todo\n> quoted **text**\n> > deeper\n\n---\n";
    let styled = style(source, None);
    let kinds: Vec<(ParagraphKind, u8, u8)> = styled
        .paragraphs
        .iter()
        .map(|p| (p.kind.clone(), p.depth, p.quote))
        .collect();
    assert_eq!(
        kinds,
        [
            (ParagraphKind::Bullet, 0, 0),
            (ParagraphKind::Bullet, 1, 0),
            (ParagraphKind::Ordered, 0, 0),
            (ParagraphKind::Task(true), 0, 0),
            (ParagraphKind::Task(false), 0, 0),
            (ParagraphKind::Body, 0, 1),
            (ParagraphKind::Body, 0, 2),
            (ParagraphKind::Rule, 0, 0),
        ]
    );
    assert_eq!(
        utf16(source, styled.paragraphs[1].range),
        "  - nested\n    continued"
    );
    let drawn: Vec<Replacement> = styled.replaced.iter().map(|r| r.with.clone()).collect();
    assert_eq!(
        drawn,
        [
            Replacement::Bullet,
            Replacement::Bullet,
            Replacement::TaskBox(true),
            Replacement::TaskBox(false),
            Replacement::Rule
        ]
    );
    assert_eq!(
        plain("- one\n- [x] done\n> quoted\n"),
        "• one\n☑ done\nquoted\n"
    );
    // An ordered item's number is text: visible, dimmed.
    assert!(spans(source, None).contains(&("1.".into(), MARKER)));
}

#[test]
fn a_fenced_block_is_a_group_revealed_together() {
    let source = "before\n\n```rust\nlet a = **not bold**;\n\n# not a heading\n```\nafter";
    let styled = style(source, None);
    let kinds: Vec<ParagraphKind> = styled.paragraphs.iter().map(|p| p.kind.clone()).collect();
    assert_eq!(
        kinds,
        [
            ParagraphKind::Body,
            ParagraphKind::Fence,
            ParagraphKind::Code("rust".into()),
            ParagraphKind::Fence,
            ParagraphKind::Body
        ]
    );
    assert_eq!(hidden(source, None), ["```rust", "```"]);
    assert!(styled.spans.is_empty());
    // A caret anywhere in the code shows both fences.
    let inside = source.find("let").unwrap() as u32;
    assert!(hidden(source, Some(Range::caret(inside))).is_empty());
    assert_eq!(
        plain(source),
        "before\n\nlet a = **not bold**;\n\n# not a heading\nafter"
    );
    // An unclosed fence runs to the end, as it does while being typed.
    assert_eq!(style("```\ncode", None).paragraphs.len(), 2);
}

#[test]
fn footnotes_number_by_first_reference_whatever_the_selection() {
    // Labels are names, not numbers: `[^7]` referenced first is 1, as GFM draws it.
    let source = "A claim[^7] and another[^note], again[^7].\n\n[^note]: With *emphasis*.\n[^7]: The source.\n[^lost]: Never referenced.";
    let styled = style(source, None);
    let marks: Vec<(String, Replacement)> = styled
        .replaced
        .iter()
        .map(|r| (utf16(source, r.range), r.with.clone()))
        .collect();
    assert_eq!(
        marks,
        [
            ("[^7]".into(), Replacement::Footnote("1".into())),
            ("[^note]".into(), Replacement::Footnote("2".into())),
            ("[^7]".into(), Replacement::Footnote("1".into())),
            ("[^note]: ".into(), Replacement::Footnote("2".into())),
            ("[^7]: ".into(), Replacement::Footnote("1".into())),
            ("[^lost]: ".into(), Replacement::Footnote("3".into())),
        ]
    );
    let index: Vec<(&str, u32, usize, bool)> = styled
        .footnotes
        .iter()
        .map(|f| {
            (
                f.label.as_str(),
                f.ordinal,
                f.references.len(),
                f.definition.is_some(),
            )
        })
        .collect();
    assert_eq!(
        index,
        [
            ("7", 1, 2, true),
            ("note", 2, 1, true),
            ("lost", 3, 0, true)
        ]
    );
    assert_eq!(styled.paragraphs[1].kind, ParagraphKind::Footnote);
    assert_eq!(spans(source, None), [("emphasis".into(), ITALIC)]);
    // Revealing the first reference changes nothing else's number.
    let caret = Range::caret(source.find("[^7]").unwrap() as u32 + 2);
    let revealed = style(source, Some(caret));
    assert_eq!(spans(source, Some(caret))[0], ("[^7]".into(), MARKER));
    assert_eq!(revealed.replaced.len(), 3);
    assert_eq!(revealed.replaced[0].with, Replacement::Footnote("2".into()));
    assert_eq!(revealed.footnotes, styled.footnotes);
    assert_eq!(plain("x[^a] y[^a]\n\n[^a]: z"), "x y\n\nz");
}

#[test]
fn the_defects_astra_traced_are_fixed() {
    // Toggling an empty fenced document made overlapping edits and panicked.
    assert_eq!(run("```\n|```", Command::CodeBlock), "|");
    assert_eq!(run("a\n```\n|```\nb", Command::CodeBlock), "a\n|b");
    // Return inside a fence never continues a list.
    assert_eq!(
        run("```\n- item|\n```", Command::Newline),
        "```\n- item\n|\n```"
    );
    assert_eq!(run("```\n> q|", Command::Newline), "```\n> q\n|");
    // The delimiter follows the authored digits.
    assert_eq!(run("01. a|\n02. b", Command::Newline), "01. a\n2. |\n3. b");
    // A code span's delimiter is longer than any run of backticks inside.
    assert_eq!(run("say |a ` b|", Command::Code), "say ``|a ` b|``");
    assert_eq!(run("say |``x``|", Command::Code), "say |x|");
    assert_eq!(
        run("say |a ``b`` c|", Command::Code),
        "say ```|a ``b`` c|```"
    );
    // An excerpt never cuts a construct open.
    let long = format!("[ok](https://e/{}) tail", "x".repeat(2000));
    assert_eq!(excerpt(&long, 4), "ok…");
    assert_eq!(excerpt(&long, 8), "ok tail");
}

#[test]
#[ignore = "async lane: a 2 s wall-clock bound; 2.3–4.4 s at load 140–180 (2026-09-23)"]
fn adversarial_input_stays_linear() {
    let start = std::time::Instant::now();
    let brackets = "[".repeat(20_000) + "]";
    let closers = "a* ".repeat(20_000);
    let marks = "**a** ".repeat(20_000);
    let url = format!("https://e/{}", ")".repeat(20_000));
    for source in [&brackets, &closers, &marks, &url] {
        style(source, None);
        segments(source, 0);
    }
    assert!(
        start.elapsed() < std::time::Duration::from_secs(2),
        "{:?}",
        start.elapsed()
    );
    assert_eq!(style(&marks, None).spans.len(), 20_000);
}

#[test]
fn inline_commands_toggle() {
    assert_eq!(
        run("make |this| bold", Command::Bold),
        "make **|this|** bold"
    );
    assert_eq!(run("make **th|is** bold", Command::Bold), "make th|is bold");
    assert_eq!(
        run("make **|this|** bold", Command::Bold),
        "make |this| bold"
    );
    assert_eq!(run("caret |here", Command::Italic), "caret *|*here");
    assert_eq!(run("caret *|*here", Command::Italic), "caret |here");
    assert_eq!(run("pad | this |out", Command::Code), "pad  `|this|` out");
    assert_eq!(run("a |b\nc| d", Command::Strike), "a ~~|b~~\n~~c|~~ d");
    assert_eq!(run("***b|oth***", Command::Italic), "**b|oth**");
    assert_eq!(
        run("a |word| here", Command::Link(String::new())),
        "a [word](|) here"
    );
    assert_eq!(
        run("a |word| here", Command::Link("https://x.dev".into())),
        "a [|word|](https://x.dev) here"
    );
    assert_eq!(
        run(
            "a [wo|rd](https://x.dev) here",
            Command::Link(String::new())
        ),
        "a wo|rd here"
    );
}

#[test]
fn block_commands_toggle_lines() {
    assert_eq!(run("a tit|le", Command::Heading(2)), "## a tit|le");
    assert_eq!(run("## a tit|le", Command::Heading(2)), "a tit|le");
    assert_eq!(run("## a tit|le", Command::Heading(1)), "# a tit|le");
    assert_eq!(run("|one\n\ntwo|", Command::Bullet), "- |one\n\n- two|");
    assert_eq!(run("- |one\n- two|", Command::Ordered), "1. |one\n2. two|");
    assert_eq!(run("1. one|", Command::Task), "- [ ] one|");
    assert_eq!(run("- [ ] one|", Command::ToggleTask), "- [x] one|");
    assert_eq!(run("- [x] one|", Command::ToggleTask), "- [ ] one|");
    assert_eq!(run("|", Command::Bullet), "- |");
    assert_eq!(run("|a\nb|", Command::Quote), "> |a\n> b|");
    assert_eq!(run("> a|\n> b", Command::Quote), "a|\n> b");
    assert_eq!(run("- a|", Command::Indent), "  - a|");
    assert_eq!(run("  - a|\n\t- b", Command::Outdent), "- a|\n\t- b");
    assert_eq!(
        run("x\n|let a;\nlet b;|\ny", Command::CodeBlock),
        "x\n```\n|let a;\nlet b;|\n```\ny"
    );
    assert_eq!(
        run("x\n```\nlet |a;\n```\ny", Command::CodeBlock),
        "x\nlet |a;\ny"
    );
}

#[test]
fn return_continues_a_list_and_ends_it_on_an_empty_item() {
    assert_eq!(run("- one|", Command::Newline), "- one\n- |");
    assert_eq!(run("- one\n- |", Command::Newline), "- one\n|");
    assert_eq!(run("  * a|b", Command::Newline), "  * a\n  * |b");
    assert_eq!(run("- [x] done|", Command::Newline), "- [x] done\n- [ ] |");
    assert_eq!(run("> said|", Command::Newline), "> said\n> |");
    assert_eq!(run("> - item|", Command::Newline), "> - item\n> - |");
    assert_eq!(
        run("1. a|\n2. b\n   - x\n3. c\n\n1. other", Command::Newline),
        "1. a\n2. |\n3. b\n   - x\n4. c\n\n1. other"
    );
    assert_eq!(run("9) nine|", Command::Newline), "9) nine\n10) |");
    assert_eq!(run("plain|", Command::Newline), "plain\n|");
    assert_eq!(
        run("```\n    deep|\n```", Command::Newline),
        "```\n    deep\n    |\n```"
    );
    assert_eq!(run("-| one", Command::Newline), "-\n| one");
}

#[test]
fn a_footnote_command_numbers_itself_and_opens_its_definition() {
    assert_eq!(
        run("A claim|.", Command::Footnote),
        "A claim[^1].\n\n[^1]: |"
    );
    assert_eq!(
        run("A[^1] claim|.\n\n[^1]: one\n", Command::Footnote),
        "A[^1] claim[^2].\n\n[^1]: one\n\n[^2]: |"
    );
}

#[test]
fn embeds_are_lone_urls_of_known_providers() {
    let video = embed("https://www.youtube.com/watch?t=9&v=dQw4w9WgXcQ#x").unwrap();
    assert_eq!(
        (video.provider, video.id.as_str()),
        (Provider::YouTube, "dQw4w9WgXcQ")
    );
    assert_eq!(
        video.frame,
        "https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ?start=9"
    );
    assert_eq!(
        video.poster,
        "https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg"
    );
    assert_eq!(
        embed("https://youtu.be/dQw4w9WgXcQ?si=abc").unwrap().id,
        "dQw4w9WgXcQ"
    );
    assert_eq!(
        embed("https://youtube.com/shorts/abc_-123").unwrap().id,
        "abc_-123"
    );
    assert_eq!(
        embed("https://x.com/someone/status/1234567890?s=20")
            .unwrap()
            .provider,
        Provider::X
    );
    assert_eq!(
        embed("https://twitter.com/someone/status/1234567890")
            .unwrap()
            .id,
        "1234567890"
    );
    assert_eq!(
        embed("https://www.instagram.com/reel/Cabc123/")
            .unwrap()
            .frame,
        "https://www.instagram.com/reel/Cabc123/embed"
    );
    assert_eq!(
        embed("https://www.tiktok.com/@someone/video/7300000000000000000")
            .unwrap()
            .provider
            .name(),
        "tiktok"
    );
    assert_eq!(embed("https://vm.tiktok.com/ZMabc123/").unwrap().frame, "");
    for other in [
        "https://example.com/watch?v=abc",
        "https://x.com/someone",
        "https://youtube.com/",
        "ftp://youtu.be/abc",
    ] {
        assert_eq!(embed(other), None, "{other}");
    }
}

#[test]
fn segments_cut_out_what_text_cannot_be() {
    let source = "# Post\n\nIntro with https://youtu.be/inline12345 inline.\n\nhttps://youtu.be/dQw4w9WgXcQ\n\n![A chart](chart.png \"title\")\n\n| a | b \\| c |\n|---|---|\n| *1* | 2 |\n\nOutro.";
    let parts = segments(source, 0);
    assert_eq!(parts.len(), 5);
    assert_eq!(
        parts[0],
        Segment::Text("# Post\n\nIntro with https://youtu.be/inline12345 inline.")
    );
    assert!(
        matches!(&parts[1], Segment::Embed { caption: "", embed } if embed.id == "dQw4w9WgXcQ")
    );
    assert_eq!(
        parts[2],
        Segment::Image {
            caption: "A chart",
            src: "chart.png"
        }
    );
    assert_eq!(
        parts[3],
        Segment::Table {
            rows: vec![vec!["a", "b \\| c"], vec!["*1*", "2"]]
        }
    );
    assert_eq!(parts[4], Segment::Text("Outro."));
    assert_eq!(style(source, None).paragraphs[2].kind, ParagraphKind::Embed);
}

#[test]
fn figures_carry_captions_and_videos_are_images_by_extension() {
    let source = "[The talk](https://youtu.be/dQw4w9WgXcQ)\n\n![Our launch](launch.MP4?v=2)\n\n![](photo.jpg)\n\n[plain link](https://example.com)\n\nA [link](https://youtu.be/dQw4w9WgXcQ) in prose.";
    let parts = segments(source, 0);
    assert!(
        matches!(&parts[0], Segment::Embed { caption: "The talk", embed } if embed.provider == Provider::YouTube)
    );
    assert_eq!(
        parts[1],
        Segment::Video {
            caption: "Our launch",
            src: "launch.MP4?v=2"
        }
    );
    assert_eq!(
        parts[2],
        Segment::Image {
            caption: "",
            src: "photo.jpg"
        }
    );
    assert_eq!(
        parts[3],
        Segment::Text(
            "[plain link](https://example.com)\n\nA [link](https://youtu.be/dQw4w9WgXcQ) in prose."
        )
    );
    let styled = style(source, None);
    let kinds: Vec<ParagraphKind> = styled.paragraphs.iter().map(|p| p.kind.clone()).collect();
    assert_eq!(
        kinds,
        [
            ParagraphKind::Embed,
            ParagraphKind::Video,
            ParagraphKind::Image,
            ParagraphKind::Body,
            ParagraphKind::Body
        ]
    );
    // The caption is the styled text; its syntax hides like any link's.
    assert_eq!(
        spans(source, None)[..2],
        [("The talk".into(), LINK), ("Our launch".into(), IMAGE)]
    );
    assert!(hidden(source, None).contains(&"](https://youtu.be/dQw4w9WgXcQ)".into()));
    assert_eq!(plain("![Our launch](launch.mp4)"), "Our launch");
    assert!(video("a/b.webm#t=3") && !video("a.png") && !video("mp4"));
}

#[test]
fn a_figure_command_makes_its_own_paragraph_with_the_caret_in_the_caption() {
    assert_eq!(
        run("text|", Command::Figure("p.jpg".into())),
        "text\n\n![|](p.jpg)"
    );
    assert_eq!(
        run("a\n\n|\n\nb", Command::Figure("p.jpg".into())),
        "a\n\n![|](p.jpg)\n\nb"
    );
    assert_eq!(
        run("a|b", Command::Figure("v.mp4".into())),
        "a\n\n![|](v.mp4)\n\nb"
    );
    assert_eq!(run("|", Command::Figure("p.jpg".into())), "![|](p.jpg)");
}

#[test]
fn a_limit_cuts_text_between_blocks_and_never_inside_a_fence() {
    let source = "one\n\ntwo\n\n```\na\n\nb\n```\n\nthree";
    let parts = segments(source, 8);
    assert_eq!(
        parts,
        [
            Segment::Text("one\n\ntwo"),
            Segment::Text("```\na\n\nb\n```"),
            Segment::Text("three")
        ]
    );
    let joined: usize = parts
        .iter()
        .map(|p| if let Segment::Text(t) = p { t.len() } else { 0 })
        .sum();
    assert_eq!(joined + 4, source.len());
}

#[test]
fn excerpts_are_one_plain_line() {
    let source = "# A **Title**\n\nThe first paragraph has [a link](https://example.com) and `code`.\n\n- a list\n";
    assert_eq!(
        excerpt(source, 200),
        "A Title The first paragraph has a link and code. • a list"
    );
    assert_eq!(excerpt(source, 24), "A Title The first…");
    assert_eq!(excerpt("", 10), "");
    assert_eq!(excerpt("Supercalifragilistic", 5), "Super…");
    let long = "word ".repeat(10_000);
    assert_eq!(excerpt(&long, 9), "word word…");
}

/// Every document in this repository styles, edits and segments without a
/// panic, with sorted ranges inside the source, and with hidden markers never
/// swallowing a newline outside a fence.
#[test]
fn this_repositorys_own_documents_hold_the_invariants() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let mut files = vec![root.join("README.md"), root.join("apps/markdown/README.md")];
    for dir in ["queue", "llp"] {
        files.extend(
            std::fs::read_dir(root.join(dir))
                .unwrap()
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "md")),
        );
    }
    assert!(files.len() > 50);
    for file in files {
        let source = std::fs::read_to_string(&file).unwrap();
        let units = source.encode_utf16().count() as u32;
        let styled = style(&source, None);
        let name = file.display();
        assert!(
            styled
                .spans
                .windows(2)
                .all(|w| w[0].range.end <= w[1].range.start),
            "{name}: spans overlap"
        );
        assert!(
            styled.hidden.windows(2).all(|w| w[0].end < w[1].start),
            "{name}: hidden ranges overlap"
        );
        assert!(
            styled
                .paragraphs
                .windows(2)
                .all(|w| w[0].range.end <= w[1].range.start),
            "{name}: paragraphs overlap"
        );
        for range in styled
            .spans
            .iter()
            .map(|s| s.range)
            .chain(styled.hidden.iter().copied())
            .chain(styled.replaced.iter().map(|r| r.range))
        {
            assert!(
                range.start < range.end && range.end <= units,
                "{name}: {range:?}"
            );
        }
        let text: usize = segments(&source, 4096)
            .iter()
            .map(|s| if let Segment::Text(t) = s { t.len() } else { 0 })
            .sum();
        assert!(text <= source.len(), "{name}");
        assert!(
            plain(&source).len() <= source.len() + styled.replaced.len() * 3,
            "{name}"
        );
        // Commands reanalyze the whole source; the short documents are enough.
        for at in [units / 3, units].into_iter().filter(|_| units < 12_000) {
            for command in [
                Command::Bold,
                Command::Newline,
                Command::Bullet,
                Command::CodeBlock,
                Command::Quote,
                Command::Footnote,
            ] {
                let done = edit(&source, Range::caret(at), command);
                let after = done.apply(&source);
                assert!(
                    done.selection.end <= after.encode_utf16().count() as u32,
                    "{name}"
                );
                style(&after, Some(done.selection));
            }
        }
    }
}

#[test]
fn pieces_flatten_a_document_into_runs_one_engine_paints() {
    let source = "# Title\n\nA **bold** [link](https://e.dev) and `code`.\n\n- one\n- [x] two\n  wrapped\n\n> quoted\n\n```\nlet a;\n```\n\n---\n\nEnd[^1].\n\n[^1]: Note.";
    let out = pieces(source);
    let text: String = out.iter().map(|p| p.text.as_str()).collect();
    assert_eq!(
        text,
        "Title\n\nA bold link and code.\n\n• one\n☑ two\nwrapped\n\n▎ quoted\n\nlet a;\n\n──────────\n\nEnd[1].\n\n[1] Note."
    );
    let title = &out[0];
    assert!(
        (title.scale - 1.6).abs() < 1e-6 && title.weight == 700 && title.text == "Title",
        "{title:?}"
    );
    assert!(out.iter().any(|p| p.text == "bold" && p.weight == 700));
    assert!(out
        .iter()
        .any(|p| p.text == "link" && p.href == "https://e.dev" && p.role == Role::Link));
    assert!(out
        .iter()
        .any(|p| p.text == "code" && p.mono && p.role == Role::Code));
    assert!(out.iter().any(|p| p.text.starts_with("let a;") && p.mono));
    assert!(out
        .iter()
        .any(|p| p.text == "quoted" && p.role == Role::Quote));
    assert!(out
        .iter()
        .any(|p| p.text == "[1]" && p.scale < 1.0 && p.role == Role::Marker));
    // A gap between blocks is a short line; every source piece maps back to the source.
    assert_eq!(
        out.iter()
            .filter(|p| p.text == "\n" && p.scale < 1.0)
            .count(),
        7
    );
    for p in out.iter().filter(|p| p.source.is_some()) {
        let r = p.source.unwrap();
        let back = utf16(source, r);
        assert!(
            p.text
                .trim_end_matches('\n')
                .starts_with(back.trim_end_matches('\n').split('\n').next().unwrap()),
            "{:?} vs {:?}",
            p.text,
            back
        );
    }
    assert_eq!(pieces(""), Vec::<Piece>::new());
    assert_eq!(pieces("plain")[0].text, "plain");
}

#[test]
fn a_list_item_hangs_its_marker_in_its_indent_as_the_browser_does() {
    // LLP 1045 D4: a list item's paragraph starts 40 px in per level (the
    // UA sheet's `padding-inline-start`), its marker outside, its end at the
    // indent; a continuation line starts at the indent with no marker.
    let out =
        pieces("Body\n\n- one\n  more\n  - two\n    - three\n\n9. nine\n10) ten\n\n> - [ ] quoted");
    let hung: Vec<(&str, f32)> = out
        .iter()
        .filter(|p| p.hang)
        .map(|p| (p.text.as_str(), p.indent))
        .collect();
    assert_eq!(
        hung,
        [
            ("• ", 40.0),
            ("◦ ", 80.0),
            ("▪ ", 120.0),
            ("9. ", 40.0),
            ("10) ", 40.0),
            ("▎ ☐ ", 40.0)
        ]
    );
    // Every piece of an item carries its indent, its last newline too, and
    // nothing outside a list has one.
    let one = out.iter().position(|p| p.text == "• ").unwrap();
    assert_eq!(out[one + 1].text, "one\nmore");
    assert_eq!(out[one + 1].indent, 40.0);
    assert_eq!(
        (out[one + 2].text.as_str(), out[one + 2].indent),
        ("\n", 40.0)
    );
    assert!(!out[one + 1].hang);
    assert!(out[..one].iter().all(|p| p.indent == 0.0 && !p.hang));
    assert!(out
        .iter()
        .any(|p| p.text.starts_with("nine") && p.indent == 40.0 && p.source.is_some()));
    let text: String = out.iter().map(|p| p.text.as_str()).collect();
    assert!(text.ends_with("9. nine\n10) ten\n\n▎ ☐ quoted"), "{text:?}");
}

#[test]
fn toolbar_facts_use_utf16_and_distinguish_mixed_content() {
    let source = "## 😀 **bold** and plain [link](https://e.dev)\n\n- [ ] task";
    let selected = selection(source, Range::caret(9));
    assert_eq!(selected.formats, "bold heading2");
    assert!(!selected.mixed);
    let selected = selection(source, Range::new(8, 22));
    assert_eq!(selected.formats, "heading2");
    assert!(selected.mixed);
    let pos = source[..source.find("link]").unwrap()]
        .encode_utf16()
        .count() as u32;
    assert_eq!(
        selection(source, Range::caret(pos + 1)).link,
        "https://e.dev"
    );
    assert_eq!(
        selection(source, Range::caret(source.encode_utf16().count() as u32)).formats,
        "task"
    );
    let code = selection("```rust\nlet x = 1;\n```", Range::caret(10));
    assert_eq!(code.formats, "codeblock");
    assert!(code.unavailable.split_whitespace().any(|s| s == "bold"));
    assert!(wire::edit("```\nx\n```", Range::caret(5), "bold", "").contains("unavailable"));
}

#[test]
fn editing_wire_keeps_source_offsets_and_escapes_data() {
    assert_eq!(
        wire::edit("😀 word", Range::new(3, 7), "bold", ""),
        r#"{"replacements":[[3,3,"**"],[7,7,"**"]],"selection":[5,9]}"#
    );
    assert_eq!(
        run("a [wo|rd](old) here", Command::Link("https://new".into())),
        "a [|word|](https://new) here"
    );
    assert!(wire::edit("", Range::caret(0), "heading", "7").contains("error"));
    assert!(wire::selection("[x](u)", Range::caret(2)).contains(r#""link":"u""#));
    let s = style("- [ ] task\n\n**other**", Some(Range::caret(8)));
    assert!(s.replaced.is_empty());
    assert!(s.spans.iter().any(|s| s.style & MARKER != 0));
    assert_eq!(s.hidden.len(), 2);
}

#[test]
fn reading_many_blocks_matches_their_individual_styles() {
    let block = "## A **title**\n\nSome *text* and [a link](https://example.com).\n\n- item\n- [x] done\n\n";
    for n in [1, 32, 512] {
        let source = block.repeat(n);
        let pieces = pieces(&source);
        let text: String = pieces.iter().map(|p| p.text.as_str()).collect();
        assert_eq!(text.matches("A title").count(), n);
        assert_eq!(
            pieces
                .iter()
                .filter(|p| p.href == "https://example.com")
                .count(),
            n
        );
        assert!(pieces
            .iter()
            .filter_map(|p| p.source)
            .all(|r| r.end <= source.len() as u32));
    }
}

#[test]
fn link_commands_escape_destination_syntax_on_insert_and_update() {
    for target in [
        "https://example.com/a)b",
        "https://example.com/a(b)c",
        "https://example.com/a\\b",
    ] {
        let first = edit("label", Range::new(0, 5), Command::Link(target.into()));
        let source = first.apply("label");
        assert_eq!(plain(&source), "label", "{source}");
        assert_eq!(selection(&source, first.selection).link, target, "{source}");
        let changed = edit(
            "[label](old)",
            Range::caret(3),
            Command::Link(target.into()),
        );
        let source = changed.apply("[label](old)");
        assert_eq!(plain(&source), "label", "{source}");
        assert_eq!(
            selection(&source, changed.selection).link,
            target,
            "{source}"
        );
    }
    let changed = edit(
        "<https://old>",
        Range::caret(5),
        Command::Link("https://new".into()),
    );
    assert_eq!(changed.apply("<https://old>"), "[https://old](https://new)");
}

#[test]
fn an_empty_link_caption_keeps_its_toolbar_target() {
    let change = edit(
        "",
        Range::caret(0),
        Command::Link("https://example.com/a)b".into()),
    );
    let source = change.apply("");
    let state = selection(&source, change.selection);
    assert_eq!(state.formats, "link");
    assert_eq!(state.link, "https://example.com/a)b");
}

#[test]
fn pieces_keep_unicode_sources_and_replacements_across_many_blocks() {
    let unit = "**é** [🦀](https://e.dev)\n\n> *שלום*\n\n[^a]: note\n\nend[^a].\n\n";
    let count = 128;
    let source = unit.repeat(count);
    let out = pieces(&source);
    let text: String = out.iter().map(|p| p.text.as_str()).collect();
    assert_eq!(
        text,
        "é 🦀\n\n▎ שלום\n\n[1] note\n\nend[1].\n\n"
            .repeat(count)
            .trim_end_matches('\n')
    );
    let expected = [
        ("é", 2, 3),
        (" ", 5, 6),
        ("🦀", 7, 9),
        ("שלום", 30, 34),
        ("note", 43, 47),
        ("end", 49, 52),
        (".", 56, 57),
    ];
    let stride = unit.encode_utf16().count() as u32;
    let actual: Vec<_> = out
        .iter()
        .filter_map(|p| p.source.map(|r| (p.text.as_str(), r.start, r.end)))
        .collect();
    let expected: Vec<_> = (0..count as u32)
        .flat_map(|n| {
            expected.map(|(text, start, end)| (text, n * stride + start, n * stride + end))
        })
        .collect();
    assert_eq!(actual, expected);
    assert!(out
        .iter()
        .filter(|p| p.text == "é")
        .all(|p| p.weight == 700));
    assert!(out
        .iter()
        .filter(|p| p.text == "🦀")
        .all(|p| p.role == Role::Link && p.href == "https://e.dev"));
    assert!(out
        .iter()
        .filter(|p| p.text == "שלום")
        .all(|p| p.role == Role::Quote && p.italic));
    assert!(out
        .iter()
        .filter(|p| p.text == "[1]" || p.text == "[1] ")
        .all(|p| p.role == Role::Marker && p.scale == 0.75 && p.source.is_none()));
}

#[test]
fn repeated_mixed_emphasis_preserves_partial_runs_and_nested_styles() {
    let unit = "***é** tail* **a *b* c** ~~x **y** z~~ _end_ ";
    let source = unit.repeat(256);
    let units: Vec<_> = source.encode_utf16().collect();
    let styled = style(&source, None);
    let expected = [
        ("é", BOLD | ITALIC),
        (" tail", ITALIC),
        ("a ", BOLD),
        ("b", BOLD | ITALIC),
        (" c", BOLD),
        ("x ", STRIKE),
        ("y", STRIKE | BOLD),
        (" z", STRIKE),
        ("end", ITALIC),
    ];
    assert_eq!(styled.spans.len(), expected.len() * 256);
    for (span, (text, flags)) in styled.spans.iter().zip(expected.iter().cycle()) {
        assert_eq!(span.style, *flags);
        assert_eq!(
            String::from_utf16(&units[span.range.start as usize..span.range.end as usize]).unwrap(),
            *text
        );
    }
    assert_eq!(plain(&source), "é tail a b c x y z end ".repeat(256));
}

#[test]
fn autolink_search_boundaries_preserve_nested_starts_and_labels() {
    let cases: &[(&str, &[&str])] = &[
        ("<<ok:tail>", &["ok:tail"]),
        ("<<me@example.com>", &["mailto:<me@example.com"]),
        ("<@<no>> <@@> <@a@b> <a@b@> <a@b>", &["mailto:a@b"]),
        ("<bad <ok:tail>", &["ok:tail"]),
        ("[<bad](outer) <ok:tail>", &["outer", "ok:tail"]),
        (
            "<bad [<me@example.com>](outer) <ok:tail>",
            &["mailto:me@example.com", "ok:tail"],
        ),
        ("<bad [<@<no>>](outer) <ok:tail>", &["outer", "ok:tail"]),
        ("> <bad\n> <me@example.com>", &["mailto:me@example.com"]),
        (
            "<é:foo> <a:foo> <aa:> <a.:foo> <é@x>",
            &["a.:foo", "mailto:é@x"],
        ),
        ("<bad`<inner`<ok:tail>", &["ok:tail"]),
    ];
    for (source, expected) in cases {
        let links: Vec<_> = style(source, None)
            .spans
            .into_iter()
            .filter(|span| span.style & LINK != 0)
            .map(|span| span.href)
            .collect();
        assert_eq!(links, *expected, "{source}");
    }
    let prefix = "😀".to_owned() + &"<".repeat(2048);
    let source = format!("{prefix}<aa:tail>");
    let styled = style(&source, None);
    assert_eq!(styled.spans.len(), 1);
    assert_eq!(styled.spans[0].href, "aa:tail");
    assert_eq!(styled.spans[0].range, Range::new(2051, 2058));
    assert_eq!(plain(&source), format!("{prefix}aa:tail"));
}

#[test]
fn code_search_preserves_escapes_skipped_openers_and_label_boundaries() {
    type ExpectedSpan<'a> = (&'a str, u8, &'a str);
    let cases: &[(&str, &[ExpectedSpan<'_>])] = &[
        ("\\``x`", &[("x", CODE, "")]),
        ("``x\\``", &[("x\\", CODE, "")]),
        (
            "a `x` \\``b` ``c`d``",
            &[("x", CODE, ""), ("b", CODE, ""), ("c`d", CODE, "")],
        ),
        (
            "> \\``é`\n> <aa:`x> rest `😀`",
            &[("é", CODE, ""), ("aa:`x", LINK, "aa:`x"), ("😀", CODE, "")],
        ),
        (
            "<aa:`x> rest `y`",
            &[("aa:`x", LINK, "aa:`x"), ("y", CODE, "")],
        ),
        (
            "[x](target`x) rest `y`",
            &[("x", LINK, "target`x"), ("y", CODE, "")],
        ),
        ("<aa:`x> rest `", &[("aa:`x", LINK, "aa:`x")]),
        (
            "[`a`](/`target) `b`",
            &[("a", CODE | LINK, "/`target"), ("b", CODE, "")],
        ),
        (
            "<aa:`a> ``b` c``",
            &[("aa:`a", LINK, "aa:`a"), ("b` c", CODE, "")],
        ),
        ("\\\\``x`", &[]),
        ("[`a](u) b`", &[("a](u) b", CODE, "")]),
        ("[`a`](u) `b`", &[("a", CODE | LINK, "u"), ("b", CODE, "")]),
    ];
    for (source, expected) in cases {
        let actual: Vec<_> = style(source, None)
            .spans
            .into_iter()
            .map(|span| (utf16(source, span.range), span.style, span.href))
            .collect();
        let expected: Vec<_> = expected
            .iter()
            .map(|(text, flags, href)| ((*text).to_owned(), *flags, (*href).to_owned()))
            .collect();
        assert_eq!(actual, expected, "{source}");
    }
    let bounded = "[<aa:`x> rest `y](u) z`";
    assert!(style(bounded, None)
        .spans
        .iter()
        .all(|span| span.style & CODE == 0));
}

#[test]
fn indexed_unmatched_code_runs_preserve_later_partial_runs_and_links() {
    let mut source = String::from("start ");
    for len in 4..68 {
        source.push_str(&"`".repeat(len));
        source.push_str("x ");
    }
    source.push_str("\\``é` [label](u) <aa:`x> ``b`c``");
    assert_eq!(
        spans(&source, None),
        [
            ("é".into(), CODE),
            ("label".into(), LINK),
            ("aa:`x".into(), LINK),
            ("b`c".into(), CODE),
        ]
    );
}

#[test]
fn footnotes_ignore_literal_and_split_openers_before_numbering_definitions() {
    let source = "[^orphan]: No references.\n\n`[^code]` and \\[^escaped] and ![^image](u).\n\n> [\n> ^split]\n\n- First[^b]\n\n[^b]: Definition refers[^a].\n\nLater [link[^a]](u) and[^b].\n\n[^a]: A.";
    let styled = style(source, None);
    let notes: Vec<_> = styled
        .footnotes
        .iter()
        .map(|f| {
            (
                f.label.as_str(),
                f.ordinal,
                f.references.len(),
                f.definition.is_some(),
            )
        })
        .collect();
    assert_eq!(
        notes,
        [("b", 1, 2, true), ("a", 2, 2, true), ("orphan", 3, 0, true)]
    );
    for note in &styled.footnotes {
        for &reference in &note.references {
            assert_eq!(utf16(source, reference), format!("[^{}]", note.label));
        }
    }
    assert_eq!(
        style(source, Some(Range::new(0, source.len() as u32))).footnotes,
        styled.footnotes
    );
}

#[test]
fn footnotes_in_later_content_ranges_keep_utf16_positions_and_reference_order() {
    let source =
        "> [\n> ^split]\n> 😀 later[^é]\n\n[^unused]: plain\n[^é]: cites[^tail]\n[^tail]: end";
    let styled = style(source, None);
    let notes: Vec<_> = styled
        .footnotes
        .iter()
        .map(|f| {
            (
                f.label.as_str(),
                f.ordinal,
                f.references.len(),
                f.definition.is_some(),
            )
        })
        .collect();
    assert_eq!(
        notes,
        [
            ("é", 1, 1, true),
            ("tail", 2, 1, true),
            ("unused", 3, 0, true)
        ]
    );
    let reference = styled.footnotes[0].references[0];
    assert_eq!(utf16(source, reference), "[^é]");
    assert_eq!(style(source, Some(reference)).footnotes, styled.footnotes);
}

#[test]
fn figure_commands_encode_url_syntax_for_inline_and_block_readers() {
    for (target, encoded) in [
        ("https://example.com/a)b", "https://example.com/a%29b"),
        ("https://example.com/a(b)", "https://example.com/a%28b%29"),
        ("https://example.com/a\\b", "https://example.com/a%5Cb"),
        ("https://example.com/a b", "https://example.com/a%20b"),
        ("https://example.com/a\nb", "https://example.com/a%0Ab"),
    ] {
        let change = edit("", Range::caret(0), Command::Figure(target.into()));
        let source = change.apply("");
        assert_eq!(source, format!("![]({encoded})"));
        assert_eq!(
            segments(&source, 0),
            [Segment::Image {
                caption: "",
                src: encoded
            }]
        );
        assert_eq!(
            style(&source, None).paragraphs[0].kind,
            ParagraphKind::Image
        );
        assert_eq!(plain(&source), "");
    }
}

#[test]
fn deeply_nested_link_labels_stay_literal_after_the_bound() {
    const CHILD: &str = "EXACT_MARKDOWN_NESTING_CHILD";
    if std::env::var_os(CHILD).is_some() {
        let count = 20_000;
        let source = format!("{}x{}", "[".repeat(count), "](u)".repeat(count));
        let styled = style(&source, None);
        assert!(styled.hidden.len() <= 64);
        assert!(plain(&source).contains("[x](u)"));
        return;
    }
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "deeply_nested_link_labels_stay_literal_after_the_bound",
        ])
        .env(CHILD, "1")
        .status()
        .unwrap();
    assert!(status.success(), "nested link parser subprocess: {status}");
    assert_eq!(plain("[**bold** and [inner](v)](u)"), "bold and inner");
}

#[test]
fn unmatched_destinations_keep_text_and_balanced_escaped_links_work() {
    for count in [4_000, 16_000] {
        let source = "[x](".repeat(count);
        let styled = style(&source, None);
        assert!(styled.hidden.is_empty());
        assert!(styled.spans.is_empty());
    }
    for (source, href) in [
        ("[label](a(b)c)", "a(b)c"),
        (r"[label](a\)b)", "a)b"),
        (r"[label](a\(b)", "a(b"),
        ("[outer [inner](v)](u)", "u"),
    ] {
        assert!(
            style(source, None)
                .spans
                .iter()
                .any(|span| span.href == href),
            "{source}"
        );
    }
}

#[test]
fn many_distinct_footnotes_keep_first_reference_order() {
    for count in [4_000, 16_000] {
        let mut source = String::from("[^unused]: unused\n\n[^f0]: first\n\n");
        for i in (0..count).rev() {
            source.push_str(&format!("[^f{i}] "));
        }
        source.push_str("[^f0]\n\n[^last]: unused last");
        let styled = style(&source, None);
        assert_eq!(styled.footnotes.len(), count + 2);
        assert_eq!(styled.footnotes[0].label, format!("f{}", count - 1));
        assert_eq!(styled.footnotes[count - 1].label, "f0");
        assert_eq!(styled.footnotes[count - 1].references.len(), 2);
        assert_eq!(styled.footnotes[count].label, "unused");
    }
}
