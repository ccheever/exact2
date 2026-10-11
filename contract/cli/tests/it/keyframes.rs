//! `keyframes` declarations and `animation` (LLP 1055 D5), with what the
//! grnl port added on them (LLP 1062): palette functions of literal
//! arguments in a keyframe, `light-dark()`, `box-shadow` and `border-color`
//! keyframes, computed times and `each` positions — proven on the rows the
//! runner resolves after boot, and on what the compiler refuses.

use exact_kernel::{Kernel, StyleId};
use exact_motion::{Animations, Direction, Easing, FillMode, Property, Value};
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
    assert_eq!(a.name, "breathe");
    assert_eq!(a.easing, Easing::EaseInOut);
    assert!(a.iterations.is_infinite());
    assert!((a.duration - 1.6).abs() < 1e-6);
    let offsets: Vec<f64> = a.keyframes.0.iter().map(|b| b.offset).collect();
    assert_eq!(offsets, [0.0, 0.5, 1.0]);
    assert_eq!(
        a.keyframes.0[0].values,
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
    assert_eq!(fresh.keyframes.0[0].easing, Some(Easing::EaseOut));
    assert_eq!(
        fresh.keyframes.0[0].values[1],
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
    let breathe = "keyframes breathe\n  from opacity=0\n";
    for (source, id, says) in [
        (
            app(breathe, "animation=\"pulse 1s\""),
            "lower-animation-name",
            "no `keyframes pulse` is declared",
        ),
        (
            app("keyframes k\n  from height=10\n", ""),
            "lower-keyframe-property",
            "`height` cannot animate",
        ),
        (
            app("keyframes k\n  to opacity=\"lots\"\n", ""),
            "lower-keyframes",
            "`keyframes k`",
        ),
        (
            app("keyframes k\n  120% opacity=1\n", ""),
            "syntax-keyframe-selector",
            "between 0% and 100%",
        ),
        (
            app(
                "keyframes k\n  to animation-timing-function=\"-exact-spring(1, 2, 3)\"\n",
                "",
            ),
            "lower-keyframes",
            "`keyframes k`",
        ),
        (
            app("keyframes k\n  to opacity=1 opacity=0\n", ""),
            "syntax-duplicate-attr",
            "appears twice in one keyframe",
        ),
        (
            app("keyframes k\n  to color=performanceNow()\n", ""),
            "lower-keyframes",
            "known when the app compiles",
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
    let source = "keyframes k\n  0%,   100% opacity=0.4    scale=0.9\n  50% opacity=1\ncomponent App\n  view\n    text \"a\" animation=\"k 1s\"\n";
    assert_eq!(
        contract_syntax::fmt::format(source).unwrap(),
        "keyframes k\n  0%, 100% opacity=0.4 scale=0.9\n  50% opacity=1\ncomponent App\n  view\n    text \"a\" animation=\"k 1s\"\n"
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
    let source = "keyframes enter\n  from opacity=0\ncomponent App\n  state moved = false\n  resource keys = keys(moved) as shape list<string>\n  action move\n    moved = true\n  view\n    column\n      button \"move\" press=move testId=\"move\"\n      each k, i in keys key=k\n        text `${i}:${k}` testId=`row-${k}` animation=`enter 300ms ${i * 40}ms both`\n";
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
    // A virtualized list's rows read their positions too, and a row the
    // window keeps reads its new one when the list moves under it.
    let list = "keyframes enter\n  from opacity=0\ncomponent App\n  state moved = false\n  resource keys = keys(moved) as shape list<string>\n  action move\n    moved = true\n  view\n    column\n      button \"move\" press=move testId=\"move\"\n      list height=100 virtualized=true estimated-item-height=20 overflow-x=\"hidden\" testId=\"list\"\n        each k, i in keys key=k\n          text `${i}:${k}` testId=`row-${k}` animation=`enter 300ms ${i * 40}ms both`\n";
    {
        let source = list;
        let mut r = Runner::boot(
            contract::compile(source).unwrap(),
            Keys,
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        // A collection mounts its first rows without a report.
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
    let source = "keyframes enter\n  from opacity=0 background-color=\"#ff000080\"\n\ncomponent App\n  state step = 2\n  action next\n    step = step + 1\n  view\n    column\n      button press=next testId=\"next\"\n        text \"Next\"\n      text \"a\" testId=\"row\" animation=`enter 320ms ease-out ${step * 70}ms both`\n";
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
    assert_eq!(a.name, "enter");
    assert!((a.delay - 0.14).abs() < 1e-6, "{}", a.delay);
    assert!((a.duration - 0.32).abs() < 1e-6);
    let (property, value) = a.keyframes.0[0].values[1];
    assert_eq!(property, Property::BackgroundColor);
    assert!((value.w - 128.0 / 255.0).abs() < 1e-6, "{value:?}");
    let k = r.kernel();
    let next = k.node_by_key(k.find_by_test_id("next")[0]).unwrap().id;
    r.dispatch(next, Event::Press).unwrap();
    let (_, row) = animation(&r, "row");
    assert!((row.0[0].delay - 0.21).abs() < 1e-6);

    // Which keyframes play may be computed too: the runner resolves the
    // name, and a name no rule has starts nothing (LLP 1055 D5).
    contract::compile("keyframes enter\n  from opacity=0\ncomponent App\n  state n = 1\n  view\n    text \"a\" animation=`nope ${n}ms`\n").unwrap();
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
    let source = "fn accent(): string = \"light-dark(#4F6657, #B7C9AC)\"\nfn ink(): string = textTitle()\nfn textTitle(): string = \"light-dark(#171B17, #F5F5EC)\"\nkeyframes lit\n  from color=accent()\n  to color=ink()\ncomponent App\n  resource words = words() as shape list<string>\n  view\n    row\n      each w, i in words key=w\n        text w testId=`w-${w}` animation=`lit 900ms linear ${i * 120}ms both`\n";
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
    close(&a.keyframes.0[0].values, color(0x4F, 0x66, 0x57));
    close(&a.keyframes.0[0].dark, color(0xB7, 0xC9, 0xAC));
    close(&a.keyframes.0[1].dark, color(0xF5, 0xF5, 0xEC));
    // Only what the app knows when it compiles.
    let computed = "fn tone(): string = 1 > 0 ? \"#fff\" : \"#000\"\nkeyframes k\n  to color=tone()\ncomponent App\n  view\n    text \"a\"\n";
    contract::compile(computed).unwrap();
    let unknown = "fn tone(x: number): string = x > 0 ? \"#fff\" : \"#000\"\nkeyframes k\n  to color=tone(performanceNow())\ncomponent App\n  view\n    text \"a\"\n";
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
    let source = "fn tone(level: string, alpha: number): string = level == \"strong\" ? `rgba(29, 78, 216, ${alpha})` : \"light-dark(#000000, #ffffff)\"\nfn accent(): string = tone(\"strong\", 0.5)\nfn glow(c: string): string = `0 16px 24px ${c}`\nkeyframes k\n  from color=tone(\"soft\", 1) box-shadow=\"none\"\n  to color=accent() box-shadow=glow(\"light-dark(#1d4ed8, #ffffff80)\")\ncomponent App\n  view\n    text \"a\" testId=\"a\" animation=\"k 1s\"\n";
    let r = booted(source);
    let (_, row) = animation(&r, "a");
    let blocks = &row.0[0].keyframes.0;
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
        (lit.w - 0.5).abs() < 1e-6 && (lit.x - unit(29) * lit.w).abs() < 1e-6,
        "{lit:?}"
    );
    assert_eq!(
        get(&blocks[1].values, Property::BoxShadow),
        Some(Value::four(0.0, 16.0, 24.0, 0.0))
    );
    let night = get(&blocks[1].dark, Property::ShadowColor).unwrap();
    assert!((night.w - unit(0x80)).abs() < 1e-6, "{night:?}");
    // The rule round-trips its CSS, the shadow one declaration.
    let css = row.0[0].keyframes.css();
    assert!(
        css.contains("box-shadow:0px 16px 24px light-dark("),
        "{css}"
    );
    assert_eq!(
        exact_motion::Keyframes::parse(&css).unwrap(),
        row.0[0].keyframes
    );
}

#[test]
fn a_branching_palette_stops_with_a_compile_diagnostic() {
    let mut source = String::from("fn f0(): number = 0.1\n");
    for n in 1..=12 {
        let p = n - 1;
        source.push_str(&format!("fn f{n}(): number = f{p}() + f{p}() + f{p}()\n"));
    }
    source.push_str("keyframes k\n  from opacity=f12()\ncomponent App\n  view\n    text \"a\"\n");
    let error = contract::compile(&source).unwrap_err();
    assert_eq!(error.id, "lower-keyframes");
    assert_eq!(error.pass, "lower");
}

#[test]
fn keyframe_folding_matches_the_vms_operators_and_number_text() {
    for condition in [
        "-0 == 0",
        "-0 != 0",
        "(0 / 0) == (0 / 0)",
        "(0 / 0) != (0 / 0)",
        "-5 % 3 == -2",
        "5 % -3 == 2",
        "5.5 % 2 == 1.5",
        "1 + 2 * 3 - 4 / 2 == 5",
        "1 < 2 && 2 <= 2 && 3 > 2 && 3 >= 3",
        "!false || false",
        "\"ab\" + \"cd\" == \"abcd\"",
        "`${-0}` == \"0\"",
        "`${0.000001}` == \"0.000001\"",
        "`${0.0000001}` == \"1e-7\"",
        "`${1000000000000000000000}` == \"1e+21\"",
        "`${1 / 0}` == \"Infinity\"",
        "`${0 / 0}` == \"NaN\"",
        "`${true}:${false}:${1.25}` == \"true:false:1.25\"",
    ] {
        let source = format!(
            "fn value(): number = ({condition}) ? 0.25 : 0.75\nkeyframes k\n  from opacity=value()\ncomponent App\n  view\n    text `${{value()}}` testId=\"a\" animation=\"k 1s\"\n"
        );
        let r = booted(&source);
        let (_, row) = animation(&r, "a");
        let k = r.kernel();
        let node = k.node_by_key(k.find_by_test_id("a")[0]).unwrap();
        let runtime: f64 = node.text_runs()[0].text.parse().unwrap();
        assert_eq!(
            row.0[0].keyframes.0[0].values,
            [(Property::Opacity, Value::scalar(runtime))],
            "{condition}"
        );
    }
}

/// Computed times are not only `animation`'s: a `transition` and an
/// `-exact-exit-animation` template compute theirs too (LLP 1062 D7).
#[test]
fn transition_and_exit_templates_compute_their_times() {
    let source = "keyframes leave\n  to opacity=0\ncomponent App\n  state n = 2\n  view\n    text \"a\" testId=\"a\" transition=`opacity ${n * 100}ms ease ${n}ms, color ${n}s` -exact-exit-animation=`leave ${n * 80}ms linear ${n * 10}ms both`\n";
    let r = booted(source);
    let k = r.kernel();
    let style = k.node_by_key(k.find_by_test_id("a")[0]).unwrap().style;
    let t = &style.transition.0;
    assert!(
        (t[0].duration - 0.2).abs() < 1e-6 && (t[0].delay - 0.002).abs() < 1e-6,
        "{t:?}"
    );
    assert!((t[1].duration - 2.0).abs() < 1e-6, "{t:?}");
    let exit = &style.rare.exit_animation.0[0];
    assert!(
        (exit.duration - 0.16).abs() < 1e-6 && (exit.delay - 0.02).abs() < 1e-6,
        "{exit:?}"
    );
    assert_eq!(exit.name, "leave");
}

/// A named colour in a keyframe, a keyframed `light-dark()` and a shadow
/// compile as they do on a node: motion and the kernel share one table.
#[test]
fn a_keyframe_takes_the_colour_names_a_node_takes() {
    let source = "keyframes glow\n  from color=\"gray\" background-color=\"light-dark(white, black)\" box-shadow=\"0 0 4px rebeccapurple\"\n  to color=\"#000\"\ncomponent App\n  view\n    text \"a\" testId=\"a\" animation=\"glow 1s\" color=\"Gray\"\n";
    contract::compile(source).unwrap();
}

/// A hex color is written bare as CSS writes it, in a keyframe as on a node
/// (x2apps dash: every keyframe color failed as `unexpected '#'`): `#` and
/// 3, 4, 6 or 8 hex digits is the same string as its quoted spelling. A `#`
/// that is no color says what `#` is and is not.
#[test]
fn a_bare_hex_color_is_its_quoted_string_and_another_hash_says_why() {
    let source = |c: &str| {
        format!("keyframes flash\n  from background-color={c}\n  to background-color={c}\n\ncomponent App\n  view\n    text \"x\" color={c} animation=\"flash 1s\"\n")
    };
    for (bare, quoted) in [
        ("#1f9d6244", "\"#1f9d6244\""),
        ("#fff", "\"#fff\""),
        ("#ABCDEF", "\"#ABCDEF\""),
    ] {
        let plan = contract::compile(&source(bare)).unwrap();
        assert_eq!(
            plan.encode(),
            contract::compile(&source(quoted)).unwrap().encode(),
            "{bare}"
        );
    }
    for bad in ["#12345", "#ggg", "#fff0x", "# a comment"] {
        let e = contract::compile(&source(bad)).unwrap_err();
        let text = format!("{e:?}");
        assert!(
            text.contains("a hex color is `#` and 3, 4, 6 or 8 hex digits")
                && text.contains("a comment starts with `//`"),
            "{bad}: {text}"
        );
    }
}

/// Chess diary #4: `translate` takes percentages of the box's own border
/// box, as CSS does — in an attribute, bound to state, and in a keyframe —
/// and the engine's `translate` carries them beside the lengths (x, y, then
/// percentages), interpolated componentwise as CSS interpolates `calc()`.
#[test]
fn translate_takes_percentages_of_the_box_everywhere_it_takes_lengths() {
    let source = "keyframes slide\n  from translate=\"-50% 0\"\n  to translate=\"10px 25%\"\ncomponent App\n  state wide = false\n  action widen\n    wide = true\n  view\n    column\n      box testId=\"centred\" position=\"absolute\" left=\"50%\" top=\"50%\" width=120 height=40 translate=\"-50% -50%\"\n      box testId=\"bound\" width=80 height=10 translate=wide ? \"12px -100%\" : \"0\"\n      box testId=\"slides\" width=10 height=10 animation=\"slide 1s\"\n      button testId=\"widen\" press=widen\n        text \"w\"\n";
    let mut r = booted(source);
    let rows = |r: &Runner<NoData>, id: &str| {
        let k = r.kernel();
        let s = k.node_by_key(k.find_by_test_id(id)[0]).unwrap().style;
        (
            (s.translate.x, s.translate.y),
            (s.translate_percent.x, s.translate_percent.y),
            exact_kernel::motion::targets(s)[0].1,
        )
    };
    assert_eq!(
        rows(&r, "centred"),
        (
            (0.0, 0.0),
            (-50.0, -50.0),
            Value::four(0.0, 0.0, -50.0, -50.0)
        )
    );
    assert_eq!(rows(&r, "bound").1, (0.0, 0.0));
    let k = r.kernel();
    let widen = k.node_by_key(k.find_by_test_id("widen")[0]).unwrap().id;
    r.dispatch(widen, Event::Press).unwrap();
    assert_eq!(
        rows(&r, "bound"),
        (
            (12.0, 0.0),
            (0.0, -100.0),
            Value::four(12.0, 0.0, 0.0, -100.0)
        )
    );
    let (_, row) = animation(&r, "slides");
    let frames: Vec<_> = row.0[0].keyframes.0.iter().map(|f| f.values[0]).collect();
    assert_eq!(
        frames,
        [
            (Property::Translate, Value::four(0.0, 0.0, -50.0, 0.0)),
            (Property::Translate, Value::four(10.0, 0.0, 0.0, 25.0)),
        ]
    );
    // CSS text for the web's keyframe rules: the browser resolves them.
    assert_eq!(
        exact_motion::animation::value_css(Property::Translate, frames[1].1),
        "10px 25%"
    );
    assert_eq!(
        exact_motion::animation::value_css(Property::Translate, Value::four(-4.0, 0.0, -50.0, 0.0)),
        "calc(-4px + -50%) 0px"
    );
    for refused in ["-50%%", "calc(10px + 5%)", "50% 50% 10%"] {
        let error = contract::compile(&format!(
            "component App\n  view\n    box translate=\"{refused}\"\n"
        ))
        .unwrap_err();
        assert_eq!(error.id, "lower-attr-value", "{refused}: {error:?}");
    }
}
