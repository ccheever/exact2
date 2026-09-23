//! The generated inputs exercise the shipped Markdown parser, without a renderer mock.
use markdown_parse::{parse, Kind};
use markdown_stress_data::fixture::{generate, Profile, MAX_BYTES, SIZES};

#[test]
fn deterministic_inputs_stay_inside_the_reader_limit() {
    for profile in Profile::ALL {
        for size in SIZES {
            let text = generate(profile, size).unwrap();
            assert!(
                text.len() <= size,
                "{}: {} > {size}",
                profile.name(),
                text.len()
            );
            assert!(
                text.len() > size * 9 / 10,
                "fixture must actually fill its budget"
            );
            assert_eq!(text, generate(profile, size).unwrap());
            assert!(text.ends_with('\n'));
        }
    }
    for size in [0, 1, 17_000, MAX_BYTES + 1, usize::MAX] {
        assert!(generate(Profile::Mixed, size).is_err());
    }
    assert!(Profile::parse("unknown").is_err());
}

#[test]
fn profiles_exercise_distinct_real_parser_shapes() {
    let read = |profile| parse(&generate(profile, SIZES[0]).unwrap(), &str::to_owned);
    let long = read(Profile::Paragraph);
    assert_eq!(
        long.blocks.len(),
        2,
        "one heading and one unsplit paragraph"
    );
    assert_eq!(long.blocks[1].kind, Kind::Paragraph);
    assert!(
        long.blocks[1]
            .runs
            .iter()
            .map(|r| r.text.len())
            .sum::<usize>()
            > 15_000
    );
    let code = read(Profile::Code);
    assert_eq!(
        code.blocks.len(),
        2,
        "one heading and one unsplit fenced block"
    );
    assert_eq!(code.blocks[1].kind, Kind::Code);
    assert!(code.blocks[1].text.contains("let sample_000001"));
    let table = read(Profile::Table);
    assert!(
        table
            .blocks
            .iter()
            .filter(|b| b.kind == Kind::TableRow)
            .count()
            > 100
    );
    assert_eq!(table.blocks.iter().filter(|b| b.header).count(), 1);
    assert!(table
        .blocks
        .iter()
        .filter(|b| b.kind == Kind::TableRow)
        .all(|b| b.cells.len() == 4));
    let many = read(Profile::Blocks);
    assert!(many.blocks.len() > 150);
    let mixed = read(Profile::Mixed);
    for kind in [
        Kind::Heading,
        Kind::Paragraph,
        Kind::Code,
        Kind::TableRow,
        Kind::Quote,
        Kind::Item,
    ] {
        assert!(
            mixed.blocks.iter().any(|b| b.kind == kind),
            "missing {kind:?}"
        );
    }
    assert!(mixed.blocks.iter().flat_map(|b| &b.runs).any(|r| r.bold));
    assert!(mixed.blocks.iter().flat_map(|b| &b.runs).any(|r| r.code));
}

#[test]
#[ignore = "opt-in 4 MiB single-block parse; run with --ignored, not the short correctness loop"]
fn giant_single_blocks_remain_giant_after_parsing() {
    for profile in [Profile::Paragraph, Profile::Code] {
        let doc = parse(&generate(profile, MAX_BYTES).unwrap(), &str::to_owned);
        assert_eq!(doc.blocks.len(), 2);
        let b = &doc.blocks[1];
        let bytes = b.text.len() + b.runs.iter().map(|r| r.text.len()).sum::<usize>();
        assert!(
            bytes > 4_000_000,
            "a page must not silently split the pathological block"
        );
    }
}
