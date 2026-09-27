//! The claim, held on the CPU painter: what the data crate measured is what
//! the host laid out. Card heights, column line counts, flowed fragments and
//! ASCII rows are compared against cosmic-text's own layout of the same text
//! in the same face.
use exact_kernel::{PropId, ViewId};
use exact_linux::{presenter::PainterChoice, Presenter};
use exact_logic::exact_plan::Value;
use exact_logic::exact_runner::DataSource;
use reflow_data::Reflow;

const VIEWPORT: (f32, f32) = (1000., 900.);
/// `contentWidth` at that viewport (`min(1080, 1000 - 80)`) and `pageInner`.
const CONTENT: f64 = 920.;
const INNER: f64 = 846.;
/// The magazine page's inner width at its default (`contentWidth - 56 - 2`).
const MAGAZINE: f64 = 862.;

fn boot(scene: &str) -> Presenter<Reflow> {
    let plan = contract::compile_path(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../app.contract"
    )))
    .unwrap();
    let (mut p, error) = Presenter::boot_with(
        &plan.encode(),
        Reflow::default(),
        VIEWPORT,
        1.,
        std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/..")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.frame();
    p.tap(id(&p, &format!("scene-{scene}"))).unwrap();
    p.frame();
    p
}

fn ids(p: &Presenter<Reflow>, prefix: &str) -> Vec<(String, ViewId)> {
    let k = p.host().kernel();
    let mut out: Vec<(String, ViewId)> = k
        .arena()
        .iter_live()
        .filter_map(|slot| {
            let n = k.node_by_key(k.arena().key(slot))?;
            let name = n.props.str(PropId::TestId)?;
            name.starts_with(prefix).then(|| (name.to_owned(), n.id))
        })
        .collect();
    out.sort();
    out
}

fn id(p: &Presenter<Reflow>, name: &str) -> ViewId {
    ids(p, name)
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap_or_else(|| panic!("no node {name}"))
        .1
}

fn record(v: &Value) -> &[Value] {
    match v {
        Value::Record(f) => f,
        _ => panic!("record"),
    }
}

fn list(v: &Value) -> &[Value] {
    match v {
        Value::List(f) => f,
        _ => panic!("list"),
    }
}

fn query(source: &str, args: &[f64]) -> Value {
    let args: Vec<Value> = args.iter().map(|&n| Value::Number(n)).collect();
    Reflow::default().query(source, &args).unwrap()
}

fn painted_lines(p: &Presenter<Reflow>, view: ViewId) -> usize {
    p.paragraph(view).unwrap().layout_runs().count()
}

#[test]
fn masonry_cards_are_exactly_as_tall_as_the_arithmetic_said() {
    let p = boot("masonry");
    let masonry = query("masonry", &[CONTENT, 1.]);
    let mut predicted = std::collections::HashMap::new();
    for column in list(&record(&masonry)[3]) {
        for card in list(&record(column)[1]) {
            let f = record(card);
            predicted.insert(
                f[0].as_str().unwrap().to_owned(),
                f[6].as_number().unwrap() as f32,
            );
        }
    }
    let cards = ids(&p, "card-");
    assert_eq!(cards.len(), predicted.len());
    let mut mismatched = Vec::new();
    for (name, view) in &cards {
        let frame = p.host().kernel().node(*view).unwrap().frame;
        let want = predicted[name];
        if (frame.height - want).abs() > 0.5 {
            mismatched.push(format!(
                "{name}: laid out {} for {want} predicted",
                frame.height
            ));
        }
    }
    assert!(mismatched.is_empty(), "{}", mismatched.join("\n"));
    // The columns balance: the tallest and shortest stacks differ by less than
    // one tall card.
    let heights: Vec<f32> = ids(&p, "col-")
        .iter()
        .map(|(_, v)| p.host().kernel().node(*v).unwrap().frame.height)
        .collect();
    let (min, max) = heights
        .iter()
        .fold((f32::MAX, 0f32), |(a, b), &h| (a.min(h), b.max(h)));
    assert!(heights.len() == 3 && max - min < 320., "{heights:?}");
}

#[test]
fn magazine_columns_hold_exactly_the_lines_they_were_cut_to() {
    let p = boot("magazine");
    let magazine = query("magazine", &[MAGAZINE]);
    let columns = list(&record(&magazine)[3]);
    assert_eq!(record(&magazine)[0].as_number(), Some(3.));
    for (i, column) in columns.iter().enumerate() {
        let f = record(column);
        let view = id(&p, f[0].as_str().unwrap());
        let painted = painted_lines(&p, view);
        let cut = f[2].as_number().unwrap() as usize;
        assert_eq!(
            painted, cut,
            "mag-{i}: cosmic-text painted {painted} lines, cut at {cut}"
        );
    }
}

#[test]
fn the_spread_flows_every_column_around_the_obstacles_without_clipping() {
    let p = boot("spread");
    let spread = query("spread", &[INNER, 21.]);
    assert_eq!(record(&spread)[0].as_number(), Some(2.));
    let mut shaped = 0;
    for (name, view) in ids(&p, "spread-") {
        if !name.starts_with("spread-") || !name[7..].chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let node = p.host().kernel().node(view).unwrap();
        assert!(!node.flow_refusal().is_some(), "{name}: flow skipped");
        shaped += usize::from(!node.flow_shapes().is_empty());
        let para = p.paragraph(view).unwrap();
        let source: String = node.text_runs().iter().map(|r| &*r.text).collect();
        assert!(!para.fragments().is_empty(), "{name}: no fragments");
        let mut end = 0;
        let mut slots = Vec::new();
        for f in para.fragments() {
            assert_eq!(f.start, end, "{name}: source reordered or lost");
            end = f.end;
            exact_textflow::intervals(
                node.flow_shapes(),
                f.y,
                f.y + para.flow_line_height(),
                node.frame.width,
                0.,
                &mut slots,
            );
            if f.width > 0. {
                assert!(
                    slots
                        .iter()
                        .any(|&(a, b)| f.x >= a - 0.01 && f.x + f.width <= b + 0.01),
                    "{name}: fragment {f:?} intersects an obstacle: {slots:?}"
                );
            }
            assert!(
                f.y + para.flow_line_height() <= node.frame.height + 0.1,
                "{name}: the host needed more room than the data crate cut: {} of {}",
                f.y + para.flow_line_height(),
                node.frame.height
            );
        }
        assert_eq!(end, source.len(), "{name}: trailing bytes lost");
    }
    assert!(shaped >= 2, "both columns meet an obstacle");
}

#[test]
fn spread_quote_contains_its_painted_lines_across_resizes() {
    let mut p = boot("spread");
    let mut failures = Vec::new();
    // The reported two-column failure, phone widths, both sides of the
    // typography/column breakpoints, and a return to the original width.
    for width in [
        760., 360., 390., 699., 700., 733., 734., 1033., 1034., 1200., 760.,
    ] {
        assert!(p.resize(width, 900.).is_none());
        p.frame();
        let kernel = p.host().kernel();
        let quote = kernel.node(id(&p, "spread-quote")).unwrap();
        let text = kernel.node(quote.children()[0]).unwrap();
        let painted_bottom = p
            .paragraph(text.id)
            .unwrap()
            .layout_runs()
            .map(|line| line.line_top + line.line_height)
            .fold(0f32, f32::max)
            + text.frame.y;
        if painted_bottom > quote.frame.y + quote.frame.height - 20. + 0.1 {
            failures.push(format!(
                "width {width}: quote paint ends at {painted_bottom}, padded box ends at {}",
                quote.frame.y + quote.frame.height - 20.
            ));
        }
        let disc = kernel.node(id(&p, "spread-disc")).unwrap();
        if disc.frame.y < quote.frame.y + quote.frame.height {
            failures.push(format!("width {width}: illustration overlaps quote"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_balls_part_the_paragraph_on_the_clock() {
    let mut p = boot("balls");
    for time in [16., 1200., 4000.] {
        assert!(p.clock(time).1.is_none());
        p.frame();
        let view = id(&p, "balls-prose");
        let node = p.host().kernel().node(view).unwrap();
        assert_eq!(node.flow_shapes().len(), 3, "clock {time}");
        let para = p.paragraph(view).unwrap();
        assert!(!para.fragments().is_empty());
        let mut slots = Vec::new();
        for f in para.fragments() {
            exact_textflow::intervals(
                node.flow_shapes(),
                f.y,
                f.y + para.flow_line_height(),
                node.frame.width,
                0.,
                &mut slots,
            );
            if f.width > 0. {
                assert!(
                    slots
                        .iter()
                        .any(|&(a, b)| f.x >= a - 0.01 && f.x + f.width <= b + 0.01),
                    "clock {time}: fragment {f:?} meets a ball"
                );
            }
        }
    }
}

#[test]
fn ascii_rows_measure_to_the_frame_and_naive_rows_drift() {
    let mut p = boot("ascii");
    let width = 440.;
    let widths = |p: &Presenter<Reflow>| -> Vec<(String, f32)> {
        ids(p, "row-")
            .iter()
            .map(|(name, v)| {
                // An all-space row trimmed to nothing has no paragraph.
                let w = p.paragraph(*v).map_or(0., |para| {
                    para.layout_runs().map(|r| r.line_w).fold(0f32, f32::max)
                });
                (name.clone(), w)
            })
            .collect()
    };
    let measured = widths(&p);
    let widest = measured.iter().map(|(_, w)| *w).fold(0f32, f32::max);
    assert!(widest > width * 0.9 && widest <= width + 12., "{widest}");
    p.tap(id(&p, "ascii-naive")).unwrap();
    p.frame();
    let naive = widths(&p);
    let drift: f32 = measured
        .iter()
        .zip(&naive)
        .map(|((a, m), (b, n))| {
            assert_eq!(a, b);
            (m - n).abs()
        })
        .sum();
    assert!(
        drift > 300.,
        "the naive rows drift only {drift} px in total"
    );
}

#[test]
fn the_wall_mounts_a_window_and_scrolls_it() {
    let mut p = boot("wall");
    let before = ids(&p, "wall-");
    let mounted = before
        .iter()
        .filter(|(n, _)| n != "wall-scroll" && n != "wall")
        .count();
    assert!(mounted > 6 && mounted < 200, "{mounted}");
    let scroll = id(&p, "wall-scroll");
    let frame = p.host().kernel().node(scroll).unwrap().frame;
    p.wheel_at(
        frame.x + frame.width / 2.,
        frame.y + frame.height / 2.,
        0.,
        6000.,
    );
    p.frame();
    let (_, top) = p.scroll_of(scroll);
    assert!(top > 1000., "{top}");
    let after = ids(&p, "wall-");
    assert_ne!(before, after, "the window did not move with the scroll");
    let highest = after
        .iter()
        .filter_map(|(n, _)| n.strip_prefix("wall-")?.parse::<usize>().ok())
        .max()
        .unwrap();
    assert!(highest > 30, "{highest}");

    for width in [360.0, 600.0, 1000.0, 1001.0] {
        assert!(p.resize(width, 900.0).is_none());
        p.frame();
        let scroll = id(&p, "wall-scroll");
        let frame = p.host().kernel().node(scroll).unwrap().frame;
        let (_, top) = p.scroll_of(scroll);
        let expected = query(
            "wall",
            &[frame.width as f64, top as f64, frame.height as f64, 1.0],
        );
        let fields = record(&expected);
        let origin = p.host().kernel().node(id(&p, "wall")).unwrap().frame;
        let cards = list(&fields[5]);
        let mounted: Vec<_> = ids(&p, "wall-")
            .into_iter()
            .filter(|(name, _)| {
                name.strip_prefix("wall-")
                    .is_some_and(|index| index.parse::<usize>().is_ok())
            })
            .collect();
        assert_eq!(mounted.len(), cards.len(), "width {width}");
        for card in cards {
            let card = record(card);
            let name = card[0].as_str().unwrap();
            let view = id(&p, name);
            let actual = p.host().kernel().node(view).unwrap().frame;
            assert_eq!(
                (actual.x - origin.x) as f64,
                card[2].as_number().unwrap(),
                "{name} x at {width}"
            );
            assert_eq!(
                (actual.y - origin.y) as f64,
                card[3].as_number().unwrap(),
                "{name} y at {width}"
            );
            assert_eq!(
                actual.height as f64,
                card[4].as_number().unwrap(),
                "{name} h at {width}"
            );
            assert_eq!(
                actual.width as f64,
                fields[1].as_number().unwrap(),
                "{name} w at {width}"
            );
        }
    }
}
