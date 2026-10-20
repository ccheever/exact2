//! The CSS importers and the dial marks on large inputs (rt1's QUEUE entry,
//! from E23 round 5): each path linear, a large valid input in time, and a
//! shape that multiplies cut off rather than run.

use crate::common::vocab;
use exact_tamber::import::{css_blocks, css_read, marks_of, shadcn, tailwind};
use exact_tamber::{parse, Op};
use std::time::{Duration, Instant};

/// `f`'s result and how long it took.
fn timed<T>(f: impl FnOnce() -> T) -> (T, Duration) {
    let t = Instant::now();
    let out = f();
    (out, t.elapsed())
}

#[test]
fn nesting_reads_each_level_and_stops_at_its_cap() {
    // As deep as the cap: its declaration is read.
    let at = |levels: usize| format!("{}--x: 1;{}", ".a{".repeat(levels), "}".repeat(levels));
    let blocks = css_blocks(&at(64));
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].context.len(), 64);
    // One deeper: not read, and reported with its line (an importer counts
    // it as one value, reported).
    let (blocks, deep) = css_read(&format!("\n\n/* a\ncomment */{}", at(65)));
    assert!(blocks.is_empty());
    assert_eq!(
        (
            deep.len(),
            deep[0].prelude.as_str(),
            deep[0].line,
            deep[0].why
        ),
        (1, ".a", 4, "nested past 64 rules")
    );
    let import =
        shadcn::import(&at(65), "shadcn", &parse("tamber app\n").unwrap(), &vocab()).unwrap();
    assert_eq!((import.counted.source, import.counted.reported), (1, 1));
    assert_eq!(
        import.unmapped[0].why,
        "not read: nested past 64 rules (line 1)"
    );
    // Its line is its prelude's, where that starts.
    let (_, deep) = css_read(&format!(
        "{}\n.deep\n{{--x: 1;}}{}",
        ".a{".repeat(64),
        "}".repeat(64)
    ));
    assert_eq!((deep[0].prelude.as_str(), deep[0].line), (".deep", 2));
    // Far deeper, closed or not: no stack spent per level past the cap.
    let (blocks, took) = timed(|| css_blocks(&at(200_000)));
    assert!(blocks.is_empty());
    let (blocks, took_open) = timed(|| css_blocks(&".a{".repeat(200_000)));
    assert!(blocks.is_empty());
    // And wide: many sibling blocks, each read once.
    let wide: String = (0..20_000)
        .map(|i| format!(".t{i}{{--x{i}: #{i:06x};}}"))
        .collect();
    let (blocks, took_wide) = timed(|| css_blocks(&wide));
    assert_eq!(blocks.len(), 20_000);
    // 64 long preludes around many small rules: each block would copy them
    // all; past 8 MiB of copies, the rest are reported instead.
    let long = format!(".{}{{", "x".repeat(4_000));
    let many: String = (0..20_000)
        .map(|i| format!(".r{{--v{i}: #000;}}"))
        .collect();
    let css = format!("{}{many}{}", long.repeat(63), "}".repeat(63));
    let ((blocks, deep), took_copied) = timed(|| css_read(&css));
    assert!(!blocks.is_empty() && !deep.is_empty());
    assert!(deep
        .iter()
        .all(|d| d.why == "its enclosing rules copy past 8 MiB"));
    assert_eq!(blocks.len() + deep.len(), 20_000);
    // A custom property full of braces, deep in rules: the marks read once,
    // not again at each level; and braces it leaves open, each matched once.
    let braces = format!(
        "{}--x: {{{}}};{}",
        ".a{".repeat(64),
        "{}".repeat(250_000),
        "}".repeat(64)
    );
    let (_, took_braces) = timed(|| css_read(&braces));
    let open = format!(":root{{--x: {};}}", "{".repeat(100_000));
    let (_, took_unmatched) = timed(|| css_read(&open));
    // A long prelude around many declarations: each declaration's label
    // copies it, which the cap counts.
    let label = format!(
        "@layer {}{{:root{{{}}}}}",
        "x".repeat(4_000),
        (0..6_000)
            .map(|i| format!("--c{i}: #000;"))
            .collect::<String>()
    );
    let (import, took_labels) =
        timed(|| shadcn::import(&label, "shadcn", &parse("tamber app\n").unwrap(), &vocab()));
    let import = import.unwrap();
    assert!(import.unmapped.iter().any(|u| u
        .why
        .starts_with("not read: its enclosing rules copy past 8 MiB")));
    for t in [
        took,
        took_open,
        took_wide,
        took_copied,
        took_braces,
        took_unmatched,
        took_labels,
    ] {
        assert!(t < Duration::from_secs(BOUND), "{t:?}");
    }
}

#[test]
fn a_selector_that_doubles_is_none_before_it_is_built() {
    // `& &` doubles the selector at each level: 2^40 of `.dark` by the end.
    let css = format!(
        ":root{{--primary: #ffffff;}}.dark{{{}--primary: #000000;{}}}",
        "& &{".repeat(40),
        "}".repeat(40)
    );
    let (import, took) =
        timed(|| shadcn::import(&css, "shadcn", &parse("tamber app\n").unwrap(), &vocab()));
    let import = import.unwrap();
    assert_eq!(import.counted.source, 2);
    assert!(took < Duration::from_secs(BOUND), "{took:?}");
}

#[test]
fn css_alias_chains_resolve_each_link_once() {
    // 80 chains of 250 links: each colour at a chain's foot, each next
    // variable naming the one before; every token resolves, each link
    // followed once (each token walking its chain was 20,000 × 250).
    const CHAINS: usize = 80;
    const LINKS: usize = 250;
    let mut css = String::from(":root{");
    for c in 0..CHAINS {
        css += &format!("--k{c}-0: #{c:06x};");
        for i in 1..LINKS {
            css += &format!("--k{c}-{i}: var(--k{c}-{});", i - 1);
        }
    }
    css += "}";
    let (import, took) =
        timed(|| shadcn::import(&css, "shadcn", &parse("tamber app\n").unwrap(), &vocab()));
    let import = import.unwrap();
    let n = CHAINS * LINKS;
    assert_eq!(
        (import.counted.source, import.counted.mapped),
        (n, n),
        "{:?}",
        import.unmapped.first()
    );
    assert!(
        took < Duration::from_secs(BOUND),
        "{n} aliases took {took:?}"
    );
    // A chain past 256 links, as the DTCG importer has it: each variable
    // further down is reported, and the rest map.
    let mut css = String::from(":root{--c0: #102030;");
    for i in 1..300 {
        css += &format!("--c{i}: var(--c{});", i - 1);
    }
    css += "}";
    let import = shadcn::import(&css, "shadcn", &parse("tamber app\n").unwrap(), &vocab()).unwrap();
    assert_eq!(import.counted.mapped, 257);
    assert_eq!(import.unmapped.len(), 300 - 257);
    assert!(import
        .unmapped
        .iter()
        .all(|u| u.why == "an alias chain longer than 256"));
}

#[test]
fn tailwind_writes_each_key_out_once() {
    // A wide theme: 5,000 type keys, each a size and a leading that names
    // one shared key.
    const N: usize = 5_000;
    let mut css = String::from("@theme {\n  --leading-snug: 1.375;\n");
    for i in 0..N {
        css += &format!(
            "  --text-k{i}: {}rem;\n  --text-k{i}--line-height: var(--leading-snug);\n",
            1 + i % 4
        );
    }
    css += "}\n";
    let (import, took) = timed(|| tailwind::import(&css, "", &parse("tamber app\n").unwrap()));
    let import = import.unwrap();
    assert_eq!(
        import.counted.mapped,
        2 * N + 1,
        "{:?}",
        import.unmapped.first()
    );
    assert!(took < Duration::from_secs(BOUND), "{took:?}");
    // A chain of 16 that doubles at each key, read by 2,000 keys: each key
    // written out once, and the whole cut off past what one import writes.
    let mut css = String::from("@theme {\n  --leading-l0: 1;\n");
    for l in 1..=15 {
        css += &format!(
            "  --leading-l{l}: calc(var(--leading-l{p}) + var(--leading-l{p}));\n",
            p = l - 1
        );
    }
    for i in 0..2_000 {
        css += &format!("  --text-w{i}: 1rem;\n  --text-w{i}--line-height: var(--leading-l15);\n");
    }
    css += "}\n";
    let (import, took) = timed(|| tailwind::import(&css, "", &parse("tamber app\n").unwrap()));
    let import = import.unwrap();
    assert!(
        import
            .unmapped
            .iter()
            .any(|u| u.why == "its `var()`s write out past 64 KiB"),
        "{:?}",
        import.unmapped.iter().map(|u| &u.why).collect::<Vec<_>>()
    );
    assert!(took < Duration::from_secs(BOUND), "{took:?}");
}

#[test]
fn wide_themes_read_each_key_once() {
    // Tailwind: 20,000 keys, and an `inline` key the stylesheet redefines
    // that 2,000 keys read down a chain.
    let mut css = String::from("@theme inline {\n  --color-k0: #000000;\n");
    for i in 1..2_000 {
        css += &format!("  --color-k{i}: var(--color-k{});\n", i - 1);
    }
    css += "}\n@theme {\n";
    for i in 0..20_000 {
        css += &format!("  --spacing-s{i}: {}px;\n", i % 64);
    }
    css += "}\n:root { --color-k0: #ffffff; }\n";
    let (import, took) = timed(|| tailwind::import(&css, "", &parse("tamber app\n").unwrap()));
    let import = import.unwrap();
    assert_eq!(import.counted.source, 22_001);
    assert!(took < Duration::from_secs(BOUND), "{took:?}");
    // shadcn: 20,000 lengths of the other kinds.
    let css = format!(
        ":root{{{}}}",
        (0..20_000)
            .map(|i| format!("--radius-r{i}: {}px;", i % 64))
            .collect::<String>()
    );
    let (import, took) =
        timed(|| shadcn::import(&css, "shadcn", &parse("tamber app\n").unwrap(), &vocab()));
    assert_eq!(import.unwrap().counted.source, 20_000);
    assert!(took < Duration::from_secs(BOUND), "{took:?}");
}

#[test]
fn marks_look_each_token_up_once() {
    // 3,000 tokens, each set, and a dial every one of them reads.
    const N: usize = 3_000;
    let mut design = String::from("tamber t\n\ndials\n  accent: color = #1083fe\n\ntheme\n");
    for i in 0..N {
        design += &format!(
            "  c{i} = color-mix(in oklch, accent {}%, #000000)\n",
            i % 100
        );
    }
    let mut ops: Vec<Op> = (0..N)
        .map(|i| Op::SetToken {
            name: format!("c{i}"),
            when: Vec::new(),
            value: format!("#{i:06x}"),
            at: None,
            comment: None,
        })
        .collect();
    ops.push(Op::SetDial {
        name: "accent".into(),
        value: "#ff0000".into(),
    });
    let (marks, took) = timed(|| marks_of(&design, &ops, &vocab()));
    let (changes, pins) = marks.unwrap();
    assert_eq!(pins.len(), N);
    assert_eq!(changes[0].impact.len(), N);
    assert!(took < Duration::from_secs(BOUND), "{took:?}");
}

// A loaded machine's debug build, with room: each path is linear now
// (measured in §10).
const BOUND: u64 = 5;

#[test]
fn chains_past_256_links_stop_there() {
    // A root chain of 5,000 colour links read on both sides: each block's
    // walk stops at 256 links, and what is further down is reported.
    let mut css = String::from(":root{--c0: #102030;");
    for i in 1..5_000 {
        css += &format!("--c{i}: var(--c{});", i - 1);
    }
    css += "}.dark{--c4999: var(--c4998);}";
    let (import, took) =
        timed(|| shadcn::import(&css, "shadcn", &parse("tamber app\n").unwrap(), &vocab()));
    let import = import.unwrap();
    assert!(import.counted.mapped >= 257, "{:?}", import.counted);
    assert!(took < Duration::from_secs(BOUND), "{took:?}");
    // shadcn's other kinds: `--shadow-sm` through 256 references to a length
    // is a length; through 257, no kind at all (not its name's, a shadow).
    let kind = |links: usize| {
        let mut css = String::from(":root{--shadow-sm: var(--a1);");
        for i in 1..links {
            css += &format!("--a{i}: var(--a{});", i + 1);
        }
        css += &format!("--a{links}: 4px;}}");
        let import =
            shadcn::import(&css, "shadcn", &parse("tamber app\n").unwrap(), &vocab()).unwrap();
        import
            .unmapped
            .iter()
            .find(|u| u.name == "--shadow-sm")
            .map(|u| u.why.clone())
    };
    let at = kind(256);
    let past = kind(257);
    // (256: mapped as a length, so not reported at all.)
    assert_eq!(at, None);
    assert_ne!(at, past, "{at:?}");
    assert!(
        past.as_deref().is_some_and(|w| !w.contains("box-shadow")),
        "{past:?}"
    );
}

#[test]
fn long_values_are_reported_and_wide_sheets_read_once() {
    // A value past 4 KiB, a name past 256 bytes and a prelude past 4 KiB are
    // reported, each one value, not copied about.
    let css = format!(
        ":root{{--big: rgb({}0 0 0);--{}: #000;--ok: #fff;}}.{}{{--x: #000;}}",
        "0".repeat(5_000),
        "n".repeat(300),
        "p".repeat(5_000)
    );
    let import = shadcn::import(&css, "shadcn", &parse("tamber app\n").unwrap(), &vocab()).unwrap();
    let whys: Vec<&str> = import.unmapped.iter().map(|u| u.why.as_str()).collect();
    assert_eq!(import.counted.source, 4);
    assert!(
        whys.contains(&"not read: a custom property's value past 4 KiB (line 1)"),
        "{whys:?}"
    );
    assert!(
        whys.contains(&"not read: a custom property's name past 256 bytes (line 1)"),
        "{whys:?}"
    );
    assert!(
        whys.contains(&"not read: its prelude past 4 KiB (line 1)"),
        "{whys:?}"
    );
    // 20,000 empty rules inside a long prelude: the context is not copied
    // for each.
    let css = format!(".{}{{{}}}", "q".repeat(4_000), ".x{}".repeat(20_000));
    let (_, took_empty) = timed(|| css_read(&css));
    // Tailwind: 18,000 keys and 12,000 resets of one name, each found by name.
    let mut css = String::from("@theme {\n");
    for i in 0..18_000 {
        css += &format!("  --text-x{i}: 1rem;\n");
    }
    for _ in 0..12_000 {
        css += "  --text-x: initial;\n";
    }
    css += "}\n";
    let (_, took_resets) = timed(|| tailwind::import(&css, "", &parse("tamber app\n").unwrap()));
    // 30,000 colour declarations, each token's sources grouped by name.
    let css = format!(
        ":root{{{}}}",
        (0..30_000)
            .map(|i| format!("--c{i}: #{:06x};", i))
            .collect::<String>()
    );
    let (import, took_sources) =
        timed(|| shadcn::import(&css, "shadcn", &parse("tamber app\n").unwrap(), &vocab()));
    assert_eq!(import.unwrap().counted.source, 30_000);
    for t in [took_empty, took_resets, took_sources] {
        assert!(t < Duration::from_secs(BOUND), "{t:?}");
    }
}
