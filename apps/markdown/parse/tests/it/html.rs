//! Raw HTML images: what `<img>` and `<picture>` draw, read as GitHub's
//! sanitizer reads them, and the markup that stays literal. @ref LLP 1033 D9

use markdown_parse::{parse, parse_with, Block, Html, Kind};

fn here(source: &str) -> Vec<Block> {
    parse(source, &|target: &str| target.to_string()).blocks
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
            "",
            r#"<picture><source media="(prefers-color-scheme: dark)" srcset="C:\shots\dark.png"><img src="C:\shots\light.png"></picture>"#,
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
            Block {
                dark: r"C:\shots\dark.png".into(),
                ..image(r"C:\shots\light.png", "")
            },
        ]
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
    let file = Html {
        images: true,
        schemes: &["http", "https", "file"],
    };
    let doc = parse_with(
        "<img src=\"file:///tmp/a.png\">",
        &str::to_string,
        &|| false,
        file,
    )
    .unwrap();
    assert_eq!(doc.blocks, [image("file:///tmp/a.png", "")]);
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
        "<img src=\"a.png\"> and a caption",
        "<picture><img src=\"a.png\"><span>x</span></picture>",
        "<picture><img src=\"a.png\">",
        "<img src=\"a.png",
        "<image src=\"a.png\">",
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
