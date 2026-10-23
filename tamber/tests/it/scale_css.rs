//! The CSS importers and the dial marks on large inputs (rt1's QUEUE entry,
//! from E23 round 5): each path linear, a large valid input read as it should
//! be, and a shape that multiplies cut off rather than run. Linear is held as
//! a ratio, not a wall clock: an input four times the size takes under eight
//! times as long (four for a linear path, sixteen for a quadratic one), each
//! the best of three in the thread's own CPU time, which a loaded machine
//! leaves be.

use crate::common::vocab;
use exact_tamber::import::{css_blocks, css_read, marks_of, shadcn, tailwind};
use exact_tamber::{parse, Op};
use std::time::Duration;

/// That `run` grows linearly on `make`'s inputs: its CPU time at `4n`
/// within eight times its time at `n`. Each is timed as a batch of as many
/// runs as take the input at `n` past 50 ms (one clock reading too coarse
/// for a short run), the batches at `n` and `4n` the same count and taken in
/// turn, the best of three each (built before the clock starts).
fn linear<I>(what: &str, n: usize, make: impl Fn(usize) -> I, run: impl Fn(&I)) {
    let batch = |input: &I, runs: u32| {
        let t = cpu();
        for _ in 0..runs {
            run(input);
        }
        cpu() - t
    };
    let (small, large) = (make(n), make(4 * n));
    let mut runs = 1;
    while runs < 1 << 16 && batch(&small, runs) < Duration::from_millis(50) {
        runs *= 2;
    }
    let (mut a, mut b) = (Duration::MAX, Duration::MAX);
    for _ in 0..3 {
        a = a.min(batch(&small, runs));
        b = b.min(batch(&large, runs));
    }
    assert!(
        b <= a * 8,
        "{what}: {a:?} at {n}, {b:?} at {} ({runs} runs each): not linear",
        4 * n
    );
}

/// The CPU time this thread has run: what it did, not what it waited for.
#[cfg(unix)]
fn cpu() -> Duration {
    let mut t = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `t` is a valid timespec for the call to fill.
    let ok = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut t) };
    assert_eq!(ok, 0, "the thread's CPU clock");
    Duration::new(t.tv_sec as u64, t.tv_nsec as u32)
}

/// Elsewhere, the wall clock.
#[cfg(not(unix))]
fn cpu() -> Duration {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed()
}

fn design() -> exact_tamber::Design {
    parse("tamber app\n").unwrap()
}

fn tailwind_of(css: &str) -> exact_tamber::import::Import {
    // (Built once, outside the timed runs.)
    thread_local! {
        static DESIGN: exact_tamber::Design = design();
    }
    DESIGN.with(|d| tailwind::import(css, "", d).unwrap())
}

fn shadcn_of(css: &str) -> exact_tamber::import::Import {
    // (Built once, outside the timed runs.)
    thread_local! {
        static BUILT: (exact_tamber::Design, exact_tamber::Vocab) = (design(), vocab());
    }
    BUILT.with(|(d, v)| shadcn::import(css, "shadcn", d, v).unwrap())
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
    let import = shadcn_of(&at(65));
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
    assert!(css_blocks(&at(200_000)).is_empty());
    assert!(css_blocks(&".a{".repeat(200_000)).is_empty());
    linear("deep, closed", 50_000, at, |css| {
        css_blocks(css);
    });
    linear(
        "deep, open",
        50_000,
        |n| ".a{".repeat(n),
        |css| {
            css_blocks(css);
        },
    );
    // And wide: many sibling blocks, each read once.
    let wide = |n: usize| -> String {
        (0..n)
            .map(|i| format!(".t{i}{{--x{i}: #{i:06x};}}"))
            .collect()
    };
    assert_eq!(css_blocks(&wide(20_000)).len(), 20_000);
    linear("wide", 5_000, wide, |css| {
        css_blocks(css);
    });
    // 64 long preludes around many small rules: each block would copy them
    // all; past 8 MiB of copies, the rest are reported instead.
    // (The preludes grow with the rules: the sheet four times the size.)
    let copied = |n: usize| {
        let long = format!(".{}{{", "x".repeat(n / 5));
        let many: String = (0..n).map(|i| format!(".r{{--v{i}: #000;}}")).collect();
        format!("{}{many}{}", long.repeat(63), "}".repeat(63))
    };
    let (blocks, deep) = css_read(&copied(20_000));
    assert!(!blocks.is_empty() && !deep.is_empty());
    assert!(deep
        .iter()
        .all(|d| d.why == "its enclosing rules copy past 8 MiB"));
    assert_eq!(blocks.len() + deep.len(), 20_000);
    linear("copied contexts", 5_000, copied, |css| {
        css_read(css);
    });
    // A custom property full of braces, deep in rules: the marks read once,
    // not again at each level; and braces it leaves open, each matched once.
    let braces = |n: usize| {
        format!(
            "{}--x: {{{}}};{}",
            ".a{".repeat(64),
            "{}".repeat(n),
            "}".repeat(64)
        )
    };
    linear("a property's braces", 62_500, braces, |css| {
        css_read(css);
    });
    let open = |n: usize| format!(":root{{--x: {};}}", "{".repeat(n));
    linear("unmatched braces", 25_000, open, |css| {
        css_read(css);
    });
    // A long prelude around many declarations: each declaration's label
    // copies it, which the cap counts.
    let label = |n: usize| {
        format!(
            "@layer {}{{:root{{{}}}}}",
            "x".repeat(4_000),
            (0..n).map(|i| format!("--c{i}: #000;")).collect::<String>()
        )
    };
    let import = shadcn_of(&label(6_000));
    assert!(import.unmapped.iter().any(|u| u
        .why
        .starts_with("not read: its enclosing rules copy past 8 MiB")));
    // (Each label copies the prelude, so it stays one size, both sizes under
    // the cap: 1,000 labels, each copied twice, which the full size is past.)
    linear("labels", 250, label, |css| {
        shadcn_of(css);
    });
}

#[test]
fn a_selector_that_doubles_is_none_before_it_is_built() {
    // `& &` doubles the selector at each level: 2^40 of `.dark` by the end.
    let doubling = |levels: usize| {
        format!(
            ":root{{--primary: #ffffff;}}.dark{{{}--primary: #000000;{}}}",
            "& &{".repeat(levels),
            "}".repeat(levels)
        )
    };
    assert_eq!(shadcn_of(&doubling(40)).counted.source, 2);
    linear("doubling selectors", 10, doubling, |css| {
        shadcn_of(css);
    });
}

#[test]
fn css_alias_chains_resolve_each_link_once() {
    // Chains of 250 links: each colour at a chain's foot, each next variable
    // naming the one before; every token resolves, each link followed once
    // (each token walking its chain was 20,000 × 250).
    const LINKS: usize = 250;
    let chains = |n: usize| {
        let mut css = String::from(":root{");
        for c in 0..n {
            css += &format!("--k{c}-0: #{c:06x};");
            for i in 1..LINKS {
                css += &format!("--k{c}-{i}: var(--k{c}-{});", i - 1);
            }
        }
        css + "}"
    };
    let import = shadcn_of(&chains(80));
    let n = 80 * LINKS;
    assert_eq!(
        (import.counted.source, import.counted.mapped),
        (n, n),
        "{:?}",
        import.unmapped.first()
    );
    linear("alias chains", 20, chains, |css| {
        shadcn_of(css);
    });
    // And by depth, under the cut: one chain of 40 links and of 160, each
    // link followed once (each token rewalking its chain is the square).
    let deep = |links: usize| {
        let mut css = String::from(":root{--d0: #102030;");
        for i in 1..links {
            css += &format!("--d{i}: var(--d{});", i - 1);
        }
        css + "}"
    };
    linear("one chain's depth", 40, deep, |css| {
        shadcn_of(css);
    });
    // A chain past 256 links, as the DTCG importer has it: each variable
    // further down is reported, and the rest map.
    let mut css = String::from(":root{--c0: #102030;");
    for i in 1..300 {
        css += &format!("--c{i}: var(--c{});", i - 1);
    }
    css += "}";
    let import = shadcn_of(&css);
    assert_eq!(import.counted.mapped, 257);
    assert_eq!(import.unmapped.len(), 300 - 257);
    assert!(import
        .unmapped
        .iter()
        .all(|u| u.why == "an alias chain longer than 256"));
}

#[test]
fn tailwind_writes_each_key_out_once() {
    // A wide theme: type keys, each a size and a leading that names one
    // shared key.
    let theme = |n: usize| {
        let mut css = String::from("@theme {\n  --leading-snug: 1.375;\n");
        for i in 0..n {
            css += &format!(
                "  --text-k{i}: {}rem;\n  --text-k{i}--line-height: var(--leading-snug);\n",
                1 + i % 4
            );
        }
        css + "}\n"
    };
    let import = tailwind_of(&theme(5_000));
    assert_eq!(
        import.counted.mapped,
        2 * 5_000 + 1,
        "{:?}",
        import.unmapped.first()
    );
    linear("type keys", 1_250, theme, |css| {
        tailwind_of(css);
    });
    // A chain of 16 that doubles at each key, read by many keys: each key
    // written out once, and the whole cut off past what one import writes.
    let doubling = |n: usize| {
        let mut css = String::from("@theme {\n  --leading-l0: 1;\n");
        for l in 1..=15 {
            css += &format!(
                "  --leading-l{l}: calc(var(--leading-l{p}) + var(--leading-l{p}));\n",
                p = l - 1
            );
        }
        for i in 0..n {
            css +=
                &format!("  --text-w{i}: 1rem;\n  --text-w{i}--line-height: var(--leading-l15);\n");
        }
        css + "}\n"
    };
    let import = tailwind_of(&doubling(2_000));
    assert!(
        import
            .unmapped
            .iter()
            .any(|u| u.why == "its `var()`s write out past 64 KiB"),
        "{:?}",
        import.unmapped.iter().map(|u| &u.why).collect::<Vec<_>>()
    );
    linear("doubling var()s", 500, doubling, |css| {
        tailwind_of(css);
    });
}

#[test]
fn wide_themes_read_each_key_once() {
    // Tailwind: many keys, and an `inline` key the stylesheet redefines that
    // a tenth of them read down a chain.
    let theme = |n: usize| {
        let mut css = String::from("@theme inline {\n  --color-k0: #000000;\n");
        for i in 1..n / 10 {
            css += &format!("  --color-k{i}: var(--color-k{});\n", i - 1);
        }
        css += "}\n@theme {\n";
        for i in 0..n {
            css += &format!("  --spacing-s{i}: {}px;\n", i % 64);
        }
        css + "}\n:root { --color-k0: #ffffff; }\n"
    };
    let import = tailwind_of(&theme(20_000));
    assert_eq!(import.counted.source, 22_001);
    linear("tailwind keys", 5_000, theme, |css| {
        tailwind_of(css);
    });
    // shadcn: lengths of the other kinds.
    let lengths = |n: usize| {
        format!(
            ":root{{{}}}",
            (0..n)
                .map(|i| format!("--radius-r{i}: {}px;", i % 64))
                .collect::<String>()
        )
    };
    assert_eq!(shadcn_of(&lengths(20_000)).counted.source, 20_000);
    linear("shadcn lengths", 5_000, lengths, |css| {
        shadcn_of(css);
    });
}

#[test]
fn marks_look_each_token_up_once() {
    // Tokens, each set, and a dial every one of them reads.
    let marked = |n: usize| {
        let mut design = String::from("tamber t\n\ndials\n  accent: color = #1083fe\n\ntheme\n");
        for i in 0..n {
            design += &format!(
                "  c{i} = color-mix(in oklch, accent {}%, #000000)\n",
                i % 100
            );
        }
        let mut ops: Vec<Op> = (0..n)
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
        (design, ops)
    };
    let (design, ops) = marked(3_000);
    let vocab = vocab();
    let (changes, pins) = marks_of(&design, &ops, &vocab).unwrap();
    assert_eq!(pins.len(), 3_000);
    assert_eq!(changes[0].impact.len(), 3_000);
    linear("marks", 750, marked, |(design, ops)| {
        marks_of(design, ops, &vocab).unwrap();
    });
}

#[test]
fn chains_past_256_links_stop_there() {
    // A root chain of colour links read on both sides: each block's walk
    // stops at 256 links, and what is further down is reported.
    let chain = |n: usize| {
        let mut css = String::from(":root{--c0: #102030;");
        for i in 1..n {
            css += &format!("--c{i}: var(--c{});", i - 1);
        }
        css + &format!("}}.dark{{--c{}: var(--c{});}}", n - 1, n - 2)
    };
    let import = shadcn_of(&chain(5_000));
    assert!(import.counted.mapped >= 257, "{:?}", import.counted);
    linear("a long chain", 1_250, chain, |css| {
        shadcn_of(css);
    });
    // shadcn's other kinds: `--shadow-sm` through 256 references to a length
    // is a length; through 257, no kind at all (not its name's, a shadow).
    let kind = |links: usize| {
        let mut css = String::from(":root{--shadow-sm: var(--a1);");
        for i in 1..links {
            css += &format!("--a{i}: var(--a{});", i + 1);
        }
        css += &format!("--a{links}: 4px;}}");
        shadcn_of(&css)
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
    // A value past 4 KiB, the design checker's bound, refuses the import with
    // that bound's id, naming the property and not echoing the value.
    let big = format!(
        ":root{{--big: rgb({}0 0 0);--ok: #fff;}}",
        "0".repeat(5_000)
    );
    let refused =
        shadcn::import(&big, "shadcn", &parse("tamber app\n").unwrap(), &vocab()).unwrap_err();
    assert_eq!(refused[0].id, "tamber-value-size", "{refused:?}");
    assert!(
        refused[0].message.starts_with("`--big`") && refused[0].message.len() < 100,
        "{refused:?}"
    );
    // A name past 256 bytes and a prelude past 4 KiB are reported, each one
    // value, not copied about.
    let css = format!(
        ":root{{--{}: #000;--ok: #fff;}}.{}{{--x: #000;}}",
        "n".repeat(300),
        "p".repeat(5_000)
    );
    let import = shadcn_of(&css);
    let whys: Vec<&str> = import.unmapped.iter().map(|u| u.why.as_str()).collect();
    assert_eq!(import.counted.source, 3);
    assert!(
        whys.contains(&"not read: a custom property's name past 256 bytes (line 1)"),
        "{whys:?}"
    );
    assert!(
        whys.contains(&"not read: its prelude past 4 KiB (line 1)"),
        "{whys:?}"
    );
    // Empty rules inside a long prelude: the context is not copied for each.
    // (The prelude grows with the rules, as the label's above.)
    let empty = |n: usize| format!(".{}{{{}}}", "q".repeat(n / 5), ".x{}".repeat(n));
    linear("empty rules", 5_000, empty, |css| {
        css_read(css);
    });
    // Tailwind: keys, and two thirds as many resets of one name, each found
    // by name.
    let resets = |n: usize| {
        let mut css = String::from("@theme {\n");
        for i in 0..n {
            css += &format!("  --text-x{i}: 1rem;\n");
        }
        for _ in 0..n * 2 / 3 {
            css += "  --text-x: initial;\n";
        }
        css + "}\n"
    };
    linear("resets", 4_500, resets, |css| {
        tailwind_of(css);
    });
    // Colour declarations, each token's sources grouped by name.
    let colours = |n: usize| {
        format!(
            ":root{{{}}}",
            (0..n)
                .map(|i| format!("--c{i}: #{:06x};", i))
                .collect::<String>()
        )
    };
    assert_eq!(shadcn_of(&colours(30_000)).counted.source, 30_000);
    linear("colours", 7_500, colours, |css| {
        shadcn_of(css);
    });
}
