//! Raw HTML images: what `<img>` and `<picture>` draw, read as GitHub's
//! sanitizer reads them, and the markup that stays literal. @ref LLP 1033 D9

use markdown_parse::{parse, parse_with, Block, Html, Kind};

fn here(source: &str) -> Vec<Block> {
    parse(source, &|target: &str| target.to_string()).blocks
}

fn with_schemes(source: &str, schemes: &[&str]) -> Vec<Block> {
    let html = Html {
        schemes,
        ..Html::GITHUB
    };
    parse_with(source, &str::to_string, &|| false, html)
        .unwrap()
        .blocks
}

fn image(href: &str, alt: &str) -> Block {
    Block {
        kind: Kind::Image,
        href: href.into(),
        text: alt.into(),
        ..Block::default()
    }
}

#[test]
fn an_img_line_is_an_image_with_its_alt_and_authored_size() {
    let blocks = here(r#"<img src="docs/shot.png" alt="The panel" width="400" height=" 300.4 ">"#);
    assert_eq!(
        blocks,
        [Block {
            width: 400,
            height: 300,
            ..image("docs/shot.png", "The panel")
        }]
    );
    // Self-closing, unquoted, upper-case and over several lines, with a reference in the URL.
    let blocks = here("<IMG\n  SRC=https://example.com/a?x=1&amp;y=2\n  width=50% />");
    assert_eq!(blocks, [image("https://example.com/a?x=1&y=2", "")]);
}

#[test]
fn a_picture_moves_its_colour_scheme_sources_onto_its_image() {
    // Upstream's own case (T3 Code, packages/shared/src/markdownPipeline.test.ts).
    let blocks = here(
        &[
            "<picture>",
            r#"  <source media="(prefers-color-scheme: dark)" srcset="docs/dark.webp 1x, docs/dark@2x.webp 2x">"#,
            r#"  <source media="(prefers-color-scheme: dark)" srcset="docs/dark-unused.webp">"#,
            r#"  <source media="(min-width: 600px)" srcset="docs/wide.webp">"#,
            r#"  <img src="docs/light.webp" alt="Studio">"#,
            "</picture>",
            "",
            r#"<picture><source media="(prefers-color-scheme: light)" srcset="javascript:alert(1)"><img src="docs/dark.webp"></picture>"#,
        ]
        .join("\n"),
    );
    assert_eq!(
        blocks,
        [
            Block {
                dark: "docs/dark.webp".into(),
                ..image("docs/light.webp", "Studio")
            },
            // The refused light source is gone, and nothing takes its place.
            image("docs/dark.webp", ""),
        ]
    );
    // A drive path in a source is a `file:` URL to the sanitizer.
    let drive = r#"<picture><source media="(prefers-color-scheme: dark)" srcset="C:\shots\dark.png"><img src="C:\shots\light.png"></picture>"#;
    assert_eq!(here(drive), [image(r"C:\shots\light.png", "")]);
    assert_eq!(
        with_schemes(drive, &["http", "https", "file"]),
        [Block {
            dark: r"C:\shots\dark.png".into(),
            ..image(r"C:\shots\light.png", "")
        }]
    );
    // A light source alone: the image's own source is the dark appearance's.
    let blocks = here(
        r#"<picture><source media="( PREFERS-COLOR-SCHEME : Light )" srcset="l.png"><img src="d.png"></picture>"#,
    );
    assert_eq!(
        blocks,
        [Block {
            dark: "d.png".into(),
            ..image("l.png", "")
        }]
    );
}

#[test]
fn a_source_naming_another_scheme_loads_nothing() {
    let blocks = here(
        "<img src=\"javascript:alert(1)\" alt=\"x\">\n<img src=\"HTTPS://example.com/a.png\">\n<img src=\"data:image/png;base64,AA\">\n<img src=\"a/b:c.png\">",
    );
    assert_eq!(
        blocks,
        [
            image("", "x"),
            image("", ""),
            image("", ""),
            image("a/b:c.png", "")
        ]
    );
    let blocks = with_schemes(
        "<img src=\"file:///tmp/a.png\">",
        &["http", "https", "file"],
    );
    assert_eq!(blocks, [image("file:///tmp/a.png", "")]);
}

#[test]
fn the_link_resolver_sees_each_source() {
    let blocks = parse(
        "<picture><source media=\"(prefers-color-scheme: dark)\" srcset=\"d.png\"><img src=\"l.png\"></picture>",
        &|target| format!("/docs/{target}"),
    )
    .blocks;
    assert_eq!(blocks[0].href, "/docs/l.png");
    assert_eq!(blocks[0].dark, "/docs/d.png");
}

#[test]
fn an_image_line_ends_the_paragraph_above_it() {
    let blocks = here("Here is the panel:\n<img src=\"shot.png\">\nand after it.");
    assert_eq!(blocks.len(), 3);
    assert_eq!(markdown_parse::plain(&blocks[0].runs), "Here is the panel:");
    assert_eq!(blocks[1], image("shot.png", ""));
    assert_eq!(markdown_parse::plain(&blocks[2].runs), "and after it.");
}

#[test]
fn markup_holding_anything_else_stays_literal() {
    for source in [
        "<picture><img src=\"a.png\"><span>x</span></picture>",
        "<img src=\"a.png",
        "<image src=\"a.png\">",
        "<div>x</div>",
        "<div><img src=\"a.png\">x</div>",
        "<a href=\"https://example.com\"><img src=\"a.png\"></a>",
        "<span><img src=\"a.png\"></span>",
        "<div><source srcset=\"a.png\"><img src=\"a.png\"></div>",
        "<div><img src=\"a.png\"></p></div>",
        "<strong></strong>",
    ] {
        let blocks = here(source);
        assert_eq!(blocks[0].kind, Kind::Html, "{source}");
        assert_eq!(blocks[0].text, source, "{source}");
    }
    // The images that end their lines are drawn; the line after them is what it is.
    let blocks = here("<img src=\"a.png\">\n<div>x</div>");
    assert_eq!(blocks[0], image("a.png", ""));
    assert_eq!(blocks[1].kind, Kind::Html);
    // A picture with no image draws nothing.
    assert!(here("<picture><source srcset=\"a.png\"></picture>").is_empty());
}

#[test]
fn literal_html_keeps_every_line_as_written() {
    let doc = parse_with(
        "<img src=\"a.png\">",
        &str::to_string,
        &|| false,
        Html::LITERAL,
    )
    .unwrap();
    assert_eq!(doc.blocks[0].kind, Kind::Html);
    assert_eq!(doc.blocks[0].text, "<img src=\"a.png\">");
}

#[test]
fn an_image_value_carries_its_dark_source_and_size() {
    use exact_plan::Value;
    let doc = parse(
        "<picture><source media=\"(prefers-color-scheme: dark)\" srcset=\"d.png\"><img src=\"l.png\" width=\"320\"></picture>",
        &str::to_string,
    );
    let Value::Record(fields) = markdown_parse::value::block(0, &doc.blocks[0]) else {
        panic!("a block is a record");
    };
    assert_eq!(fields[1].as_str(), Some("image"));
    assert_eq!(fields[5].as_str(), Some("l.png"));
    assert_eq!(fields[9].as_str(), Some("d.png"));
    assert_eq!(fields[10].as_number(), Some(320.0));
    assert_eq!(fields[11].as_number(), Some(0.0));
}

#[test]
fn image_markup_reads_ahead_a_bounded_distance() {
    // So a document of lines that open an image and never close it parses in
    // linear time: each of these reads ahead at most the bound.
    let blocks = here(&"<img src=\"x\n".repeat(2_000));
    assert_eq!(blocks.len(), 2_000);
    assert!(blocks.iter().all(|b| b.kind == Kind::Html));
    // A picture longer than the bound is not read as one.
    let sources = "<source srcset=\"x.png\">\n".repeat(1_000);
    let blocks = here(&format!(
        "<picture>\n{sources}<img src=\"a.png\"></picture>"
    ));
    assert_eq!(blocks[0].kind, Kind::Html);
    let sources = "<source srcset=\"x.png\">\n".repeat(100);
    let blocks = here(&format!(
        "<picture>\n{sources}<img src=\"a.png\"></picture>"
    ));
    assert_eq!(blocks, [image("a.png", "")]);
}

#[test]
fn an_img_inside_a_line_of_text_is_drawn_where_it_stands() {
    let doc = parse(
        "See <img src=\"a.png\" alt=\"the panel\"> here, not `<img src=\"b.png\">`.",
        &str::to_string,
    );
    assert_eq!(doc.blocks.len(), 1);
    assert_eq!(doc.blocks[0].kind, Kind::Paragraph);
    // A reader that draws images only as blocks shows its alt text, as it
    // shows a Markdown image inside a line.
    assert_eq!(
        markdown_parse::plain(&doc.blocks[0].runs),
        "See the panel here, not <img src=\"b.png\">."
    );
    // Text after an image on its line makes the line a paragraph, as
    // CommonMark reads it, and so does a picture.
    for source in [
        "<img src=\"a.png\" alt=\"x\"> and a caption",
        "<picture><img src=\"a.png\" alt=\"x\"></picture> and a caption",
    ] {
        let blocks = here(source);
        assert_eq!(blocks[0].kind, Kind::Paragraph, "{source}");
        assert_eq!(markdown_parse::plain(&blocks[0].runs), "x and a caption");
    }
    // The caller may draw it: its run is given the image's sources as written.
    let draw = |image: markdown_parse::html::Image| markdown_parse::Run {
        text: format!("[{}]", image.alt),
        href: format!("{}|{}|{}", image.src, image.dark, image.width),
        ..markdown_parse::Run::default()
    };
    let html = Html {
        inline: Some(&draw),
        ..Html::GITHUB
    };
    let source =
        "**Before <img src=\"a.png\" alt=\"A\" width=\"20\" data-dark-src=\"d.png\"> after**";
    let doc = parse_with(source, &|t| format!("/{t}"), &|| false, html).unwrap();
    let runs = &doc.blocks[0].runs;
    assert_eq!(markdown_parse::plain(runs), "Before [A] after");
    let drawn = runs.iter().find(|r| r.text == "[A]").unwrap();
    assert_eq!(drawn.href, "a.png|d.png|20");
    assert!(drawn.bold);
    // Literal HTML keeps it as written.
    let doc = parse_with(
        "See <img src=\"a.png\" alt=\"x\">",
        &str::to_string,
        &|| false,
        Html::LITERAL,
    )
    .unwrap();
    assert_eq!(
        markdown_parse::plain(&doc.blocks[0].runs),
        "See <img src=\"a.png\" alt=\"x\">"
    );
}

#[test]
fn an_img_inside_elements_that_draw_nothing_is_an_image() {
    for source in [
        "<div><img src=\"a.png\"></div>",
        "<p align=\"center\">\n  <img src=\"a.png\">\n</p>",
        "<center><strong><img src=\"a.png\"></strong><br></center>",
        "<figure><!-- shot --><em><img src=\"a.png\"></em></figure>",
        // A `<div>` start closes an open `<p>`; an end tag that closes
        // nothing is ignored.
        "<p><div><img src=\"a.png\"></span></div>",
    ] {
        assert_eq!(here(source), [image("a.png", "")], "{source}");
    }
    let blocks = here(
        "<div>\n<picture><source media=\"(prefers-color-scheme: dark)\" srcset=\"d.png\"><img src=\"l.png\"></picture>\n<img src=\"b.png\" width=\"20\">\n</div>",
    );
    assert_eq!(
        blocks,
        [
            Block {
                dark: "d.png".into(),
                ..image("l.png", "")
            },
            Block {
                width: 20,
                ..image("b.png", "")
            },
        ]
    );
    // Blank lines inside the element: its start tag and its end tag draw
    // nothing, and what lies between is the document's.
    let blocks = here("<div align=\"center\">\n\n<img src=\"a.png\">\n\nText\n\n</div>\n\n</div>");
    assert_eq!(blocks.len(), 3);
    assert_eq!(blocks[0], image("a.png", ""));
    assert_eq!(markdown_parse::plain(&blocks[1].runs), "Text");
    // An end tag that closes nothing the markup left open stays as written.
    assert_eq!(blocks[2].kind, Kind::Html);
    assert_eq!(blocks[2].text, "</div>");
}

#[test]
fn an_unclosed_picture_is_closed_by_the_blank_line_or_the_end() {
    let picture = "<picture>\n<source media=\"(prefers-color-scheme: dark)\" srcset=\"d.png\">\n<img src=\"l.png\" alt=\"Shot\">";
    let drawn = Block {
        dark: "d.png".into(),
        ..image("l.png", "Shot")
    };
    assert_eq!(here(picture), std::slice::from_ref(&drawn));
    let blocks = here(&format!("{picture}\n\nAfter."));
    assert_eq!(blocks[0], drawn);
    assert_eq!(markdown_parse::plain(&blocks[1].runs), "After.");
    // A picture's end tag after the blank line closes it and draws nothing.
    assert_eq!(here(&format!("{picture}\n\n</picture>")), [drawn]);
    // So does the end of a paragraph, for a picture inside a line.
    let doc = parse(
        "Shot: <picture><img src=\"l.png\" alt=\"Shot\">",
        &str::to_string,
    );
    assert_eq!(markdown_parse::plain(&doc.blocks[0].runs), "Shot: Shot");
    // An unfinished tag is not repaired.
    assert_eq!(here("<picture><img src=\"l.png\"")[0].kind, Kind::Html);
}

#[test]
fn character_references_decode_before_a_source_is_checked() {
    let blocks = here(
        &[
            r#"<img src="javascript&colon;alert(1)" alt="&copy; 2026 &amp &notit; &not;x">"#,
            r#"<img src="java&Tab;script:alert(1)">"#,
            r#"<img src="&#x6A;avascript:alert(1)">"#,
            r#"<img src="a.png?x=1&ampy=2&amp=3&lt;&#0;&#x80;&#X1F600">"#,
        ]
        .join("\n"),
    );
    assert_eq!(
        blocks,
        [
            image("", "\u{a9} 2026 & &notit; \u{ac}x"),
            image("", ""),
            image("", ""),
            image("a.png?x=1&ampy=2&amp=3<\u{fffd}\u{20ac}\u{1f600}", ""),
        ]
    );
}

#[test]
fn an_imgs_own_dark_light_and_local_sources_are_read() {
    let blocks = here(
        &[
            r#"<img src="l.png" data-dark-src="d.png">"#,
            r#"<img src="x.png" data-light-src="l.png">"#,
            r#"<img src="l.png" data-dark-src="javascript:x" data-light-src="C:\l.png">"#,
            r#"<img src="a.png" data-local-src="b.png">"#,
            r#"<img src="C:\a.png" data-local-src="b.png">"#,
            // The image's own source wins over a `<source>`, even one the
            // sanitizer refuses.
            r#"<picture><source media="(prefers-color-scheme: dark)" srcset="s.png"><img src="l.png" data-dark-src="javascript:x"></picture>"#,
            r#"<picture><source media="(prefers-color-scheme: dark)" srcset="s.png"><img src="l.png" data-dark-src="o.png"></picture>"#,
        ]
        .join("\n"),
    );
    assert_eq!(
        blocks,
        [
            Block {
                dark: "d.png".into(),
                ..image("l.png", "")
            },
            // A light source alone: the image's own source is the dark appearance's.
            Block {
                dark: "x.png".into(),
                ..image("l.png", "")
            },
            image("l.png", ""),
            image("b.png", ""),
            image(r"C:\a.png", ""),
            image("l.png", ""),
            Block {
                dark: "o.png".into(),
                ..image("l.png", "")
            },
        ]
    );
}
