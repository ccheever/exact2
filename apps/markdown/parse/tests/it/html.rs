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

/// An image block of a line at the top level, which no paragraph holds.
fn image(href: &str, alt: &str) -> Block {
    Block {
        kind: Kind::Image,
        href: href.into(),
        text: alt.into(),
        bare: true,
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
    // Self-closing, unquoted, upper-case and over several lines, with a reference in the URL:
    // a tag its first line does not end is inline HTML, a paragraph.
    let blocks = here("<IMG\n  SRC=https://example.com/a?x=1&amp;y=2\n  width=50% />");
    assert_eq!(
        blocks,
        [Block {
            bare: false,
            ..image("https://example.com/a?x=1&y=2", "")
        }]
    );
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
            // The refused light source is gone, and nothing takes its place. Its
            // markup is not one tag alone on its line: inline HTML, a paragraph.
            Block {
                bare: false,
                ..image("docs/dark.webp", "")
            },
        ]
    );
    // A drive path in a source is a `file:` URL to the sanitizer. (Each picture
    // below is inline HTML, a paragraph.)
    let paragraph = |block: Block| Block {
        bare: false,
        ..block
    };
    let drive = r#"<picture><source media="(prefers-color-scheme: dark)" srcset="C:\shots\dark.png"><img src="C:\shots\light.png"></picture>"#;
    assert_eq!(here(drive), [paragraph(image(r"C:\shots\light.png", ""))]);
    assert_eq!(
        with_schemes(drive, &["http", "https", "file"]),
        [paragraph(Block {
            dark: r"C:\shots\dark.png".into(),
            ..image(r"C:\shots\light.png", "")
        })]
    );
    // A light source alone: the image's own source is the dark appearance's.
    let blocks = here(
        r#"<picture><source media="( PREFERS-COLOR-SCHEME : Light )" srcset="l.png"><img src="d.png"></picture>"#,
    );
    assert_eq!(
        blocks,
        [paragraph(Block {
            dark: "d.png".into(),
            ..image("l.png", "")
        })]
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
    assert_eq!(fields[12].as_str(), Some(""));
    let doc = parse(
        "<p align=\"center\"><img src=\"a.png\"></p>",
        &str::to_string,
    );
    let Value::Record(fields) = markdown_parse::value::block(0, &doc.blocks[0]) else {
        panic!("a block is a record");
    };
    assert_eq!(fields[12].as_str(), Some("center"));
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
    // nothing, and what lies between is the document's, in its `align`.
    let blocks = here("<div align=\"center\">\n\n<img src=\"a.png\">\n\nText\n\n</div>\n\n</div>");
    assert_eq!(blocks.len(), 3);
    assert_eq!(
        blocks[0],
        Block {
            align: "center".into(),
            ..image("a.png", "")
        }
    );
    assert_eq!(markdown_parse::plain(&blocks[1].runs), "Text");
    assert_eq!(blocks[1].align, "center");
    // An end tag that closes nothing the markup left open stays as written.
    assert_eq!(blocks[2].kind, Kind::Html);
    assert_eq!(blocks[2].text, "</div>");
    assert_eq!(blocks[2].align, "");
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

/// A reader that draws images inside a line: each image's run names its
/// source, its link and its alt text.
fn inline(source: &str) -> Vec<Block> {
    let draw = |image: markdown_parse::html::Image| markdown_parse::Run {
        text: image.alt.clone(),
        href: format!("{}|{}", image.src, image.link),
        ..markdown_parse::Run::default()
    };
    let html = Html {
        inline: Some(&draw),
        ..Html::GITHUB
    };
    parse_with(source, &str::to_string, &|| false, html)
        .unwrap()
        .blocks
}

/// A paragraph's runs as `href` for each image and `_` for a space.
fn line_of(block: &Block) -> Vec<String> {
    assert_eq!(block.kind, Kind::Paragraph, "{block:?}");
    block
        .runs
        .iter()
        .map(|r| {
            if r.href.is_empty() {
                r.text.replace(' ', "_")
            } else {
                r.href.clone()
            }
        })
        .collect()
}

#[test]
fn images_html_lays_out_side_by_side_are_one_line() {
    // At the top level, an image goes on with the one before it, a blank
    // line or not, until a block comes between, as in HTML's tree.
    let blocks = inline("<img src=\"a.png\">\n<img src=\"b.png\">\n\n<img src=\"c.png\">");
    assert_eq!(blocks.len(), 1);
    assert_eq!(
        line_of(&blocks[0]),
        ["a.png|", "_", "b.png|", "_", "c.png|"]
    );
    // No white space between two images: no space between them.
    assert_eq!(
        line_of(&inline("<img src=\"a.png\"><img src=\"b.png\">")[0]),
        ["a.png|", "b.png|"]
    );
    // A paragraph, a `<p>`, a `<div>` or a `<br>` ends the line; an image
    // alone on its line is an image block, as before. Markup that starts no
    // HTML block (a tag with more after it on its line) is CommonMark's
    // inline HTML: a paragraph of its own.
    for (source, count) in [
        (
            "<img src=\"a.png\"> <img src=\"b.png\">\n\n<img src=\"c.png\">",
            2,
        ),
        (
            "<a href=\"x\"><img src=\"a.png\"></a>\n\n<img src=\"c.png\">",
            2,
        ),
        ("<img src=\"a.png\">\n\nText\n\n<img src=\"b.png\">", 3),
        ("<p><img src=\"a.png\"></p>\n<img src=\"b.png\">", 2),
        ("<img src=\"a.png\">\n<div><img src=\"b.png\"></div>", 2),
        ("<div><img src=\"a.png\"><br><img src=\"b.png\"></div>", 2),
    ] {
        let blocks = inline(source);
        assert_eq!(blocks.len(), count, "{source}");
        // The last image is on a line of its own.
        assert_eq!(blocks[count - 1].kind, Kind::Image, "{source}");
    }
    // The images inside one `<p>` or `<div>` share its line; the sanitizer
    // drops a `<center>` and keeps what is in it, which then goes on with
    // the images around it.
    let blocks = inline("<p>\n<img src=\"a.png\">\n<img src=\"b.png\">\n</p>");
    assert_eq!(line_of(&blocks[0]), ["a.png|", "_", "b.png|"]);
    let blocks = inline("<center><img src=\"a.png\"></center>\n<img src=\"b.png\">");
    assert_eq!(line_of(&blocks[0]), ["a.png|", "_", "b.png|"]);
    // An element left open at a blank line goes on with the images after it.
    let blocks =
        inline("<div>\n<img src=\"a.png\">\n\n<img src=\"b.png\">\n</div>\n\n<img src=\"c.png\">");
    assert_eq!(blocks.len(), 2);
    assert_eq!(line_of(&blocks[0]), ["a.png|", "_", "b.png|"]);
    assert_eq!(blocks[1], image("c.png", ""));
    // A reader that draws images only as blocks draws them one under the other.
    assert_eq!(
        here("<img src=\"a.png\">\n<img src=\"b.png\">"),
        [image("a.png", ""), image("b.png", "")]
    );
}

#[test]
fn a_p_or_div_align_is_the_text_align_html_gives_it() {
    // A `<p>` holds the line, so it is no bare line.
    let center = |block: Block| Block {
        align: "center".into(),
        bare: false,
        ..block
    };
    assert_eq!(
        here("<p align=\"center\">\n  <img src=\"a.png\">\n</p>"),
        [center(image("a.png", ""))]
    );
    let blocks = inline("<p align=\"center\"><img src=\"a.png\"> <img src=\"b.png\"></p>");
    assert_eq!(blocks[0].align, "center");
    assert_eq!(line_of(&blocks[0]), ["a.png|", "_", "b.png|"]);
    // A `<div>`'s `middle` is `center` too, and the innermost `align` wins.
    for (source, aligns) in [
        ("<div align=\"MIDDLE\"><img src=\"a.png\"></div>", &["center"][..]),
        ("<p align=\"middle\"><img src=\"a.png\"></p>", &[""]),
        ("<p align=\"top\"><img src=\"a.png\"></p>", &[""]),
        ("<div align=\"right\"><p align=\"justify\"><img src=\"a.png\"></p><img src=\"b.png\"></div>", &["justify", "right"]),
        ("<center><img src=\"a.png\"></center>", &[""]),
    ] {
        let got: Vec<String> = here(source).into_iter().map(|b| b.align).collect();
        assert_eq!(got, aligns, "{source}");
    }
    // A `<div align>` left open at a blank line holds every block up to its
    // end tag, as GitHub READMEs write it.
    let blocks = here(
        "<div align=\"center\">\n\n# Title\n\n<img src=\"a.png\">\n\n- an item\n\n</div>\n\nAfter",
    );
    let got: Vec<(Kind, &str)> = blocks.iter().map(|b| (b.kind, b.align.as_str())).collect();
    assert_eq!(
        got,
        [
            (Kind::Heading, "center"),
            (Kind::Image, "center"),
            (Kind::Item, "center"),
            (Kind::Paragraph, ""),
        ]
    );
}

#[test]
fn an_image_inside_a_link_carries_the_link() {
    let blocks =
        inline("<a href=\"https://example.com/docs\"><img src=\"c.png\" alt=\"Docs\"></a>");
    assert_eq!(blocks.len(), 1);
    assert_eq!(line_of(&blocks[0]), ["c.png|https://example.com/docs"]);
    assert_eq!(blocks[0].runs[0].text, "Docs");
    // A relative link is kept; a scheme the sanitizer does not allow is
    // dropped, and its image is drawn with no link.
    let blocks = inline("<p align=\"center\"><a href=\"/docs\"><img src=\"a.png\"></a> <a href=\"javascript:alert(1)\"><img src=\"b.png\"></a></p>");
    assert_eq!(line_of(&blocks[0]), ["a.png|/docs", "_", "b.png|"]);
    // Inside a line of text.
    let blocks = inline("See <a href=\"mailto:a@b.c\"><img src=\"a.png\" alt=\"mail\"></a> here.");
    let drawn = blocks[0].runs.iter().find(|r| r.text == "mail").unwrap();
    assert_eq!(drawn.href, "a.png|mailto:a@b.c");
    // A link holding anything else, or one inside a link, stays as written;
    // so does any link for a reader that draws images only as blocks.
    for source in [
        "<a href=\"https://example.com\">text</a>",
        "<a href=\"https://example.com\"><img src=\"a.png\">text</a>",
        "<a href=\"x\"><a href=\"y\"><img src=\"a.png\"></a></a>",
    ] {
        let blocks = inline(source);
        assert!(
            matches!(blocks[0].kind, Kind::Html | Kind::Paragraph),
            "{source}"
        );
        assert!(blocks[0].runs.iter().all(|r| r.href.is_empty()), "{source}");
    }
    assert_eq!(
        here("<a href=\"https://example.com\"><img src=\"a.png\"></a>")[0].kind,
        Kind::Html
    );
}

#[test]
fn a_line_no_paragraph_holds_is_bare() {
    // A `<div>`'s line and a line at the top level have no margins; a `<p>`'s and inline HTML's
    // (a paragraph of its own) have a paragraph's, as a Markdown image's has.
    let bare = |source: &str| here(source).iter().map(|b| b.bare).collect::<Vec<_>>();
    assert_eq!(bare(r#"<div><img src="a.png"></div>"#), [true]);
    assert_eq!(bare(r#"<img src="b.png">"#), [true]);
    assert_eq!(
        bare("<img src=\"b.png\">\n\n<img src=\"c.png\">"),
        [true, true]
    );
    assert_eq!(bare(r#"<p align="center"><img src="c.png"></p>"#), [false]);
    assert_eq!(
        bare(r#"<img src="d.png"> <img src="e.png"> after"#),
        [false]
    );
    assert_eq!(bare("![f](f.png)"), [false]);
}
