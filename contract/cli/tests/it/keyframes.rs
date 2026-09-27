//! LLP 1057: `keyframes` declarations and `animation`, resolved at compile
//! time into the kernel's row — proven on the rows after boot, and on what
//! the compiler refuses.

use exact_kernel::{Animations, Kernel, StyleId};
use exact_motion::{Direction, Easing, FillMode, Property, Value};
use exact_plan::{Plan, Value as PlanValue};
use exact_runner::{DataError, DataSource, Event, Runner};
use std::path::Path;

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[PlanValue]) -> Result<PlanValue, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn fixture() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../corpus/keyframes.contract");
    std::fs::read_to_string(path).unwrap()
}

fn animation(r: &Runner<NoData>, test_id: &str) -> (bool, Animations) {
    let k = r.kernel();
    let node = k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap();
    (
        node.style.mask.has(StyleId::Animation),
        node.style.animation.clone(),
    )
}

#[test]
fn a_named_animation_reaches_the_row_with_its_keyframes_and_a_condition_can_stop_it() {
    let plan = contract::compile(&fixture()).unwrap();
    let mut r = Runner::boot(
        Plan::decode(&plan.encode()).unwrap(),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let (set, row) = animation(&r, "mark");
    assert!(set);
    let [a] = row.0.as_slice() else {
        panic!("{row:?}")
    };
    assert_eq!(a.keyframes.name, "breathe");
    assert_eq!(a.easing, Easing::EaseInOut);
    assert!(a.iterations.is_infinite());
    assert!((a.duration - 1.6).abs() < 1e-6);
    let offsets: Vec<f64> = a.keyframes.blocks.iter().map(|b| b.offset).collect();
    assert_eq!(offsets, [0.0, 0.5, 1.0]);
    assert_eq!(
        a.keyframes.blocks[0].values,
        [
            (Property::Opacity, Value::scalar(0.4)),
            (Property::Scale, Value::scalar(0.9)),
        ]
    );
    // From a `style`, through `class`: the keyframe's own easing and a
    // translate read by the kernel's row parser.
    let (_, fresh) = animation(&r, "added");
    let fresh = &fresh.0[0];
    assert_eq!(fresh.fill, FillMode::Both);
    assert_eq!(fresh.direction, Direction::Normal);
    assert_eq!(fresh.keyframes.blocks[0].easing, Some(Easing::EaseOut));
    assert_eq!(
        fresh.keyframes.blocks[0].values[1],
        (Property::Translate, Value::new(0.0, 8.0))
    );
    // `none` is CSS's: no animation, the row still authored.
    let k = r.kernel();
    let toggle = k.node_by_key(k.find_by_test_id("toggle")[0]).unwrap().id;
    r.dispatch(toggle, Event::Press).unwrap();
    assert_eq!(animation(&r, "mark").1, Animations::NONE);
}

#[test]
fn what_cannot_animate_or_resolve_is_refused_at_compile_time() {
    let app = |decls: &str, attr: &str| {
        format!("{decls}component App\n  state on = true\n  view\n    text \"a\" {attr}\n")
    };
    let breathe = "keyframes breathe\n  from\n    opacity=0\n";
    for (source, id, says) in [
        (
            app(breathe, "animation=\"pulse 1s\""),
            "lower-unknown-keyframes",
            "no `keyframes pulse` is declared; declared: `breathe`",
        ),
        (
            app(breathe, "animation=`${\"breathe\"} 1s`"),
            "lower-animation-literal",
            "resolved when the app compiles",
        ),
        (
            app(breathe, "animation=\"breathe 1s spring(100, 10, 1)\""),
            "lower-attr-value",
            "`spring()` is a `transition` extension",
        ),
        (
            app("keyframes k\n  from\n    height=10\n", ""),
            "lower-keyframes",
            "`height` cannot be in a keyframe",
        ),
        (
            app("keyframes k\n  to\n    opacity=\"lots\"\n", ""),
            "lower-attr-value",
            "`opacity` in a keyframe is not a valid `opacity`",
        ),
        (
            app("keyframes k\n  120%\n    opacity=1\n", ""),
            "lower-keyframes",
            "outside 0%–100%",
        ),
        (
            app(
                "keyframes k\n  to\n    animation-timing-function=\"spring(1, 2, 3)\"\n",
                "",
            ),
            "lower-keyframes",
            "is not a CSS easing",
        ),
        (
            app(
                "keyframes k\n  to\n    opacity=1\nkeyframes k\n  to\n    opacity=0\n",
                "",
            ),
            "syntax-duplicate-declaration",
            "keyframes `k` is declared twice",
        ),
        (
            app("keyframes k\n  to\n    opacity=1 opacity=0\n", ""),
            "syntax-duplicate-attr",
            "appears twice in `keyframes k`",
        ),
    ] {
        let error = contract::compile(&source).unwrap_err();
        assert_eq!(error.id, id, "{source}\n{error}");
        assert!(error.message.contains(says), "{error}");
    }
    // A condition may choose between literals; `none` is one.
    contract::compile(&app(
        breathe,
        "animation=(on ? \"breathe 1s infinite\" : \"none\")",
    ))
    .unwrap();
}

#[test]
fn keyframes_format_and_keep_their_percentages_whole() {
    let source = "keyframes k\n  0%,   100%\n    opacity=0.4    scale=0.9\n  50%\n    opacity=1\ncomponent App\n  view\n    text \"a\" animation=\"k 1s\"\n";
    assert_eq!(
        contract_syntax::fmt::format(source).unwrap(),
        "keyframes k\n  0%, 100%\n    opacity=0.4 scale=0.9\n  50%\n    opacity=1\ncomponent App\n  view\n    text \"a\" animation=\"k 1s\"\n"
    );
}

/// LLP 1062 D8: `each item, i in list` names the row's position, which a
/// stagger multiplies; a row that moves reads its new position, keeping its
/// identity (and an animation keeps its start: only the delay changes).
#[test]
fn each_names_the_position_and_a_moved_row_reads_its_new_one() {
    struct Keys;
    impl DataSource for Keys {
        fn query(&mut self, _: &str, args: &[PlanValue]) -> Result<PlanValue, DataError> {
            let keys = if args == [PlanValue::Bool(true)] {
                ["c", "a", "b"]
            } else {
                ["a", "b", "c"]
            };
            Ok(PlanValue::list(
                keys.into_iter().map(PlanValue::str).collect(),
            ))
        }
    }
    let source = "keyframes enter\n  from\n    opacity=0\ncomponent App\n  state moved = false\n  resource keys = keys(moved) as shape list<string>\n  action move writes moved\n    moved = true\n  view\n    column\n      button \"move\" press=move testId=\"move\"\n      each k, i in keys key=k\n        text `${i}:${k}` testId=`row-${k}` animation=`enter 300ms ${i * 40}ms both`\n";
    let plan = contract::compile(source).unwrap();
    let mut r = Runner::boot(
        plan,
        Keys,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let text = |r: &Runner<Keys>, id: &str| {
        let k = r.kernel();
        let node = k.node_by_key(k.find_by_test_id(id)[0]).unwrap();
        (
            node.key,
            node.props
                .str(exact_kernel::PropId::Text)
                .unwrap()
                .to_string(),
            node.style.animation.0[0].delay,
        )
    };
    let (c_before, c_text, c_delay) = text(&r, "row-c");
    assert_eq!(c_text, "2:c");
    assert!((c_delay - 0.08).abs() < 1e-6);
    r.act("move", vec![]).unwrap();
    let (c_after, c_text, c_delay) = text(&r, "row-c");
    assert_eq!(
        (c_after, c_text.as_str()),
        (c_before, "0:c"),
        "same row, new position"
    );
    assert!(c_delay.abs() < 1e-6);
    assert_eq!(text(&r, "row-b").1, "2:b");
    // A windowed list's rows read their positions too, and a row the window
    // keeps reads its new one when the list moves under it.
    let list = "keyframes enter\n  from\n    opacity=0\ncomponent App\n  state moved = false\n  resource keys = keys(moved) as shape list<string>\n  action move writes moved\n    moved = true\n  view\n    column\n      button \"move\" press=move testId=\"move\"\n      list height=100 item-height=20 testId=\"list\"\n        each k, i in keys key=k\n          text `${i}:${k}` testId=`row-${k}` animation=`enter 300ms ${i * 40}ms both`\n";
    let virtualized = list.replace(
        "item-height=20",
        "virtualized=true estimated-item-height=20",
    );
    for source in [list, virtualized.as_str()] {
        let mut r = Runner::boot(
            contract::compile(source).unwrap(),
            Keys,
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        let k = r.kernel();
        let view = k.node_by_key(k.find_by_test_id("list")[0]).unwrap().id;
        let port = exact_runner::ListViewport {
            height: 100.0,
            width: 200.0,
            ..Default::default()
        };
        // A collection mounts its first rows without a report.
        if !source.contains("virtualized") {
            r.list_viewport(view, port).unwrap();
        }
        let (_, c_text, c_delay) = text(&r, "row-c");
        assert_eq!(c_text, "2:c", "{source}");
        assert!((c_delay - 0.08).abs() < 1e-6);
        r.act("move", vec![]).unwrap();
        assert_eq!(text(&r, "row-c").1, "0:c", "{source}");
        assert_eq!(text(&r, "row-b").1, "2:b", "{source}");
        assert!(text(&r, "row-c").2.abs() < 1e-6);
    }
    assert_eq!(
        contract_syntax::fmt::format("component App\n  resource keys = keys(false) as shape list<string>\n  view\n    column\n      each k ,  i in keys key=k\n        text k\n").unwrap(),
        "component App\n  resource keys = keys(false) as shape list<string>\n  view\n    column\n      each k, i in keys key = k\n        text k\n"
    );
}

/// LLP 1062 D7: an `animation` template interpolates only times, so a
/// stagger is computed while the keyframes stay resolved at compile time;
/// colours may be keyframed, one fixed colour each.
#[test]
fn a_computed_delay_staggers_and_colours_keyframe() {
    let source = "keyframes enter\n  from\n    opacity=0\n    background-color=\"#ff000080\"\n\ncomponent App\n  state step = 2\n  action next writes step\n    step = step + 1\n  view\n    column\n      button press=next testId=\"next\"\n        text \"Next\"\n      text \"a\" testId=\"row\" animation=`enter 320ms ease-out ${step * 70}ms both`\n";
    let plan = contract::compile(source).unwrap();
    let mut r = Runner::boot(
        Plan::decode(&plan.encode()).unwrap(),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let (_, row) = animation(&r, "row");
    let a = &row.0[0];
    assert_eq!(a.keyframes.name, "enter");
    assert!((a.delay - 0.14).abs() < 1e-6, "{}", a.delay);
    assert!((a.duration - 0.32).abs() < 1e-6);
    let (property, value) = a.keyframes.blocks[0].values[1];
    assert_eq!(property, Property::BackgroundColor);
    assert!((value.w - 128.0 / 255.0).abs() < 1e-6, "{value:?}");
    let k = r.kernel();
    let next = k.node_by_key(k.find_by_test_id("next")[0]).unwrap().id;
    r.dispatch(next, Event::Press).unwrap();
    let (_, row) = animation(&r, "row");
    assert!((row.0[0].delay - 0.21).abs() < 1e-6);

    let app = |attr: &str| {
        format!("keyframes enter\n  from\n    opacity=0\ncomponent App\n  state n = 1\n  view\n    text \"a\" {attr}\n")
    };
    for (attr, id, says) in [
        (
            "animation=`enter ${n}ms ${n}ms ${n}ms`",
            "lower-attr-value",
            "animation=",
        ),
        (
            "animation=`enter 1s ${n}`",
            "lower-animation-literal",
            "interpolates only times",
        ),
        (
            "animation=`nope ${n}ms`",
            "lower-unknown-keyframes",
            "no `keyframes nope`",
        ),
    ] {
        let error = contract::compile(&app(attr)).unwrap_err();
        assert_eq!(error.id, id, "{attr}\n{error}");
        assert!(error.message.contains(says), "{error}");
    }
}

/// LLP 1062 D9: a keyframe takes a `light-dark()` colour, written or
/// returned by a palette function, and carries both; the host's appearance
/// picks one. grnl's welcome: each word arrives lit in the accent and eases
/// to ink, one after another.
#[test]
fn a_keyframe_takes_light_dark_through_a_palette_function() {
    struct Words;
    impl DataSource for Words {
        fn query(&mut self, _: &str, _: &[PlanValue]) -> Result<PlanValue, DataError> {
            Ok(PlanValue::list(vec![
                PlanValue::str("Hello"),
                PlanValue::str("there"),
            ]))
        }
    }
    let source = "fn accent(): string = \"light-dark(#4F6657, #B7C9AC)\"\nfn ink(): string = textTitle()\nfn textTitle(): string = \"light-dark(#171B17, #F5F5EC)\"\nkeyframes lit\n  from\n    color=accent()\n  to\n    color=ink()\ncomponent App\n  resource words = words() as shape list<string>\n  view\n    row\n      each w, i in words key=w\n        text w testId=`w-${w}` animation=`lit 900ms linear ${i * 120}ms both`\n";
    let plan = contract::compile(source).unwrap();
    let r = Runner::boot(
        Plan::decode(&plan.encode()).unwrap(),
        Words,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let k = r.kernel();
    let row = k
        .node_by_key(k.find_by_test_id("w-there")[0])
        .unwrap()
        .style
        .animation
        .clone();
    let a = &row.0[0];
    assert!((a.delay - 0.12).abs() < 1e-6);
    let unit = |c: u8| c as f64 / 255.0;
    let color = |r: u8, g: u8, b: u8| Value::rgba(unit(r), unit(g), unit(b), 1.0);
    let close = |got: &[(Property, Value)], want: Value| {
        let [(Property::Color, v)] = got else {
            panic!("{got:?}")
        };
        assert!(
            v.components()
                .iter()
                .zip(want.components())
                .all(|(a, b)| (a - b).abs() < 1e-6),
            "{v:?} vs {want:?}"
        );
    };
    close(&a.keyframes.blocks[0].values, color(0x4F, 0x66, 0x57));
    close(&a.keyframes.blocks[0].dark, color(0xB7, 0xC9, 0xAC));
    close(&a.keyframes.blocks[1].dark, color(0xF5, 0xF5, 0xEC));
    // Only what the app knows when it compiles.
    let computed = "fn tone(): string = 1 > 0 ? \"#fff\" : \"#000\"\nkeyframes k\n  to\n    color=tone()\ncomponent App\n  view\n    text \"a\"\n";
    contract::compile(computed).unwrap();
    let unknown = "fn tone(x: number): string = x > 0 ? \"#fff\" : \"#000\"\nkeyframes k\n  to\n    color=tone(now())\ncomponent App\n  view\n    text \"a\"\n";
    let error = contract::compile(unknown).unwrap_err();
    assert!(
        error.message.contains("known when the app compiles"),
        "{error}"
    );
}

fn booted(source: &str) -> Runner<NoData> {
    let plan = contract::compile(source).unwrap();
    Runner::boot(
        Plan::decode(&plan.encode()).unwrap(),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

/// A keyframe's palette function may take arguments, folded when the app
/// compiles through conditions, templates and other palette functions; and
/// `box-shadow` animates in a keyframe, its colour and geometry together
/// (LLP 1062).
#[test]
fn keyframes_fold_palette_arguments_and_animate_box_shadow() {
    let source = "fn tone(level: string, alpha: number): string = level == \"strong\" ? `rgba(29, 78, 216, ${alpha})` : \"light-dark(#000000, #ffffff)\"\nfn accent(): string = tone(\"strong\", 0.5)\nfn glow(c: string): string = `0 16px 24px ${c}`\nkeyframes k\n  from\n    color=tone(\"soft\", 1)\n    box-shadow=\"none\"\n  to\n    color=accent()\n    box-shadow=glow(\"light-dark(#1d4ed8, #ffffff80)\")\ncomponent App\n  view\n    text \"a\" testId=\"a\" animation=\"k 1s\"\n";
    let r = booted(source);
    let (_, row) = animation(&r, "a");
    let blocks = &row.0[0].keyframes.blocks;
    let unit = |c: u8| c as f64 / 255.0;
    let get = |list: &[(Property, Value)], p: Property| {
        list.iter().find(|(q, _)| *q == p).map(|(_, v)| *v)
    };
    assert_eq!(
        get(&blocks[0].values, Property::Color),
        Some(Value::rgba(0.0, 0.0, 0.0, 1.0))
    );
    assert_eq!(
        get(&blocks[0].dark, Property::Color),
        Some(Value::rgba(1.0, 1.0, 1.0, 1.0))
    );
    assert_eq!(
        get(&blocks[0].values, Property::BoxShadow),
        Some(Value::ZERO)
    );
    assert_eq!(
        get(&blocks[0].values, Property::ShadowColor),
        Some(Value::ZERO)
    );
    let lit = get(&blocks[1].values, Property::Color).unwrap();
    assert!(
        (lit.w - unit(128)).abs() < 1e-6 && (lit.x - unit(29) * lit.w).abs() < 1e-6,
        "{lit:?}"
    );
    assert_eq!(
        get(&blocks[1].values, Property::BoxShadow),
        Some(Value::four(0.0, 16.0, 24.0, 0.0))
    );
    let night = get(&blocks[1].dark, Property::ShadowColor).unwrap();
    assert!((night.w - unit(0x80)).abs() < 1e-6, "{night:?}");
    // The row round-trips its text, the shadow one declaration.
    assert!(
        row.text().contains("box-shadow:0px 16px 24px light-dark("),
        "{}",
        row.text()
    );
    assert_eq!(Animations::parse(&row.text()).unwrap(), row);
}

/// Computed times are not only `animation`'s: a `transition` and an
/// `exit-animation` template compute theirs too (LLP 1062 D7).
#[test]
fn transition_and_exit_templates_compute_their_times() {
    let source = "keyframes leave\n  to\n    opacity=0\ncomponent App\n  state n = 2\n  view\n    text \"a\" testId=\"a\" transition=`opacity ${n * 100}ms ease ${n}ms, color ${n}s` exit-animation=`leave ${n * 80}ms linear ${n * 10}ms both`\n";
    let r = booted(source);
    let k = r.kernel();
    let style = k.node_by_key(k.find_by_test_id("a")[0]).unwrap().style;
    let t = &style.transition.0;
    assert!(
        (t[0].duration - 0.2).abs() < 1e-6 && (t[0].delay - 0.002).abs() < 1e-6,
        "{t:?}"
    );
    assert!((t[1].duration - 2.0).abs() < 1e-6, "{t:?}");
    let exit = &style.exit_animation.0[0];
    assert!(
        (exit.duration - 0.16).abs() < 1e-6 && (exit.delay - 0.02).abs() < 1e-6,
        "{exit:?}"
    );
    assert_eq!(exit.keyframes.name, "leave");
}
