//! @ref LLP 1121 D8 (stage 8 A) — state parts: `:disabled` and `:checked`
//! fold into the rows an author writes today, per longhand, `:checked`
//! under `:disabled`; paint rows only; never on a native control or a
//! list's row; host states' rows literal.

use exact_kernel::{Color, ColorValue, Kernel, RowValue, StyleId};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Runner};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn booted(src: &str) -> Runner<NoData> {
    let plan = contract::compile(src).unwrap_or_else(|e| panic!("{e:?}"));
    let plan = contract::bake(plan, NoData).unwrap();
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn refused(src: &str) -> String {
    contract::compile(src)
        .err()
        .map(|e| e.id.to_string())
        .unwrap_or_else(|| "compiled".into())
}

fn colour(r: &Runner<NoData>, id: &str, style: StyleId) -> String {
    let key = r.kernel().find_by_test_id(id)[0];
    match r.kernel().node_by_key(key).unwrap().computed(style) {
        RowValue::ColorValue(ColorValue::Fixed(c)) | RowValue::Color(c) => hex(c),
        other => format!("{other:?}"),
    }
}

fn hex(c: Color) -> String {
    format!("#{:06x}", c.0 >> 8)
}

fn press(r: &mut Runner<NoData>, id: &str) {
    let view = {
        let k = r.kernel();
        k.node_by_key(k.find_by_test_id(id)[0]).unwrap().id
    };
    r.dispatch(view, exact_runner::Event::Press).unwrap();
}

const TOGGLE: &str = "style Toggle\n  background-color = \"#ff0000\"\n  :checked\n    background-color = \"#00ff00\"\n  :disabled\n    background-color = \"#0000ff\"\n\ncomponent App\n  state on = false\n  state busy = false\n  action flip\n    on = !on\n  action hold\n    busy = !busy\n  view\n    column\n      button press=flip testId=\"flip\"\n        text \"flip\"\n      button press=hold testId=\"hold\"\n        text \"hold\"\n      button class=Toggle press=flip disabled=busy aria-pressed=on testId=\"t\"\n        text \"t\"\n";

/// A checked, disabled toggle with both parts shows `:disabled`'s (the
/// gate's line); checked alone `:checked`'s; neither the base.
#[test]
fn checked_folds_under_disabled() {
    let mut r = booted(TOGGLE);
    assert_eq!(colour(&r, "t", StyleId::BackgroundColor), "#ff0000");
    press(&mut r, "flip");
    assert_eq!(colour(&r, "t", StyleId::BackgroundColor), "#00ff00");
    press(&mut r, "hold");
    assert_eq!(colour(&r, "t", StyleId::BackgroundColor), "#0000ff");
    press(&mut r, "flip");
    assert_eq!(colour(&r, "t", StyleId::BackgroundColor), "#0000ff");
    press(&mut r, "hold");
    assert_eq!(colour(&r, "t", StyleId::BackgroundColor), "#ff0000");
}

/// A state's `border-color` overrides each side the base wrote, through
/// `border` or a side's longhand; the node's part wins over its class's.
#[test]
fn a_state_border_colour_is_each_side_and_the_node_wins() {
    let src = "style Box\n  border = \"1px solid #ff0000\"\n  :disabled\n    border-color = \"#00ff00\"\n\ncomponent App\n  state busy = false\n  action hold\n    busy = !busy\n  view\n    column\n      button press=hold testId=\"hold\"\n        text \"hold\"\n      button class=Box disabled=busy border-left-color=\"#111111\" testId=\"b\"\n        :disabled\n          border-top-color = \"#0000ff\"\n        text \"b\"\n";
    let mut r = booted(src);
    assert_eq!(colour(&r, "b", StyleId::BorderColorTop), "#ff0000");
    assert_eq!(colour(&r, "b", StyleId::BorderColorLeft), "#111111");
    press(&mut r, "hold");
    assert_eq!(
        colour(&r, "b", StyleId::BorderColorTop),
        "#0000ff",
        "the node's own part"
    );
    assert_eq!(colour(&r, "b", StyleId::BorderColorRight), "#00ff00");
    assert_eq!(
        colour(&r, "b", StyleId::BorderColorLeft),
        "#00ff00",
        "over the base's longhand"
    );
    press(&mut r, "hold");
    assert_eq!(colour(&r, "b", StyleId::BorderColorLeft), "#111111");
}

/// `:disabled` is the author's own disabled look: no default dim.
#[test]
fn a_disabled_part_marks_the_disabled_look() {
    let plan = contract::compile(TOGGLE).unwrap();
    let marked = plan.nodes.iter().any(|n| {
        n.bindings.iter().any(|b| {
            let b = plan.binding(b);
            b.kind == exact_plan::BindingKind::Prop
                && b.id == exact_kernel::PropId::DisabledStyled as u16
        })
    });
    assert!(marked);
}

#[test]
fn a_state_part_is_refused_where_it_cannot_be() {
    let node = |part: &str| {
        format!("component App\n  state on = false\n  view\n    column\n      column press=go testId=\"b\"\n        {part}\n        text \"b\"\n  action go\n    on = !on\n")
    };
    assert_eq!(
        refused(&node(":disabled\n          width = \"10px\"")),
        "lower-state-row"
    );
    assert_eq!(
        refused(&node(":active\n          color = \"#000000\"")),
        "lower-state-row"
    );
    assert_eq!(
        refused(&node(
            ":active\n          background-color=(on ? \"#000000\" : \"#ffffff\")"
        )),
        "lower-state-literal"
    );
    assert_eq!(
        refused(&node(":pressed\n          opacity = 0.5")),
        "syntax-state"
    );
    assert_eq!(
        refused(&node(":hover:active\n          opacity = 0.5")),
        "syntax-state"
    );
    assert_eq!(
        refused(&node(
            ":hover\n          :active\n            opacity = 0.5"
        )),
        "syntax-state"
    );
    assert_eq!(refused("component App\n  view\n    column\n      text \"a\"\n      :hover\n        opacity = 0.5\n"), "syntax-state");
    assert_eq!(
        refused("component App\n  state on = false\n  view\n    input type=\"checkbox\" checked=on\n      :checked\n        background-color = \"#00ff00\"\n"),
        "lower-state-target"
    );
    assert_eq!(
        refused("component App\n  state rows = [\"a\"]\n  view\n    list flex=1\n      for row in rows key=row\n        column press=go\n          :active\n            opacity = 0.5\n          text row\n  action go\n    rows = rows\n"),
        "lower-state-target"
    );
    // A host state's literal rows compile (recorded for the hosts).
    assert_eq!(refused(&node(":active\n          background-color = \"#000000\"\n          -exact-press-scale = 0.97")), "compiled");
}

/// `contract fmt` keeps a state part's block.
#[test]
fn fmt_keeps_a_state_part() {
    let src = "style Toggle\n  background-color=\"#ff0000\"\n  :checked\n    background-color=\"#00ff00\"\n";
    assert_eq!(contract_syntax::fmt::format(src).unwrap(), src);
}

/// A host-known state's look is recorded on its node, one a state, its
/// class's and its own rows composed by longhand, each a literal row.
#[test]
fn a_host_states_look_is_recorded_one_a_state() {
    let src = "style Card\n  background-color = \"#ffffff\"\n  :active\n    background-color = \"#eeeeee\"\n    opacity = 0.9\n  :hover\n    border-color = \"#cccccc\"\n\ncomponent App\n  state on = false\n  action go\n    on = !on\n  view\n    column class=Card press=go testId=\"card\"\n      :active\n        background-color = \"#dddddd\"\n      text \"c\"\n";
    let plan = contract::compile(src).unwrap_or_else(|e| panic!("{e:?}"));
    let node = plan.looks.first().expect("a node with looks").node;
    let looks: Vec<_> = plan.looks.iter().filter(|l| l.node == node).collect();
    assert_eq!(looks.len(), plan.looks.len(), "one node's looks, by node");
    assert_eq!(
        looks.iter().map(|l| l.state).collect::<Vec<_>>(),
        [0, 1],
        "one look a state: :active, :hover"
    );
    let rows = |l: &exact_plan::LooksRow| -> Vec<(u16, String)> {
        (l.bindings.start..l.bindings.start + l.bindings.len)
            .map(|i| {
                let b = &plan.bindings[i as usize];
                let code = plan.code(b.expr);
                (b.id, format!("{:?}", code))
            })
            .collect()
    };
    let active = rows(looks[0]);
    assert_eq!(active.len(), 2, "{active:?}");
    assert!(
        active
            .iter()
            .any(|(id, _)| *id == StyleId::BackgroundColor as u16),
        "{active:?}"
    );
    let text = format!("{:?}", plan.strings);
    assert!(
        text.contains("#dddddd") && !text.contains("#eeeeee"),
        "the node's own wins"
    );
    assert!(
        active.iter().any(|(id, _)| *id == StyleId::Opacity as u16),
        "{active:?}"
    );
    assert_eq!(rows(looks[1]).len(), 4, "a border colour is its four sides");
}

/// Looks are by node, then state, whatever order the source writes the
/// states in; a plan out of that order is refused (hosts binary-search).
#[test]
fn looks_are_by_node_then_state() {
    let src = "component App\n  state on = false\n  action go\n    on = !on\n  view\n    column\n      column press=go testId=\"a\"\n        :hover\n          opacity = 0.8\n        :active\n          opacity = 0.5\n        text \"a\"\n      column press=go testId=\"b\"\n        :focus-visible\n          opacity = 0.7\n        :active\n          opacity = 0.6\n        text \"b\"\n";
    let mut plan = contract::compile(src).unwrap_or_else(|e| panic!("{e:?}"));
    let keys: Vec<_> = plan.looks.iter().map(|l| (l.node.0, l.state)).collect();
    assert_eq!(keys.len(), 4, "{keys:?}");
    assert!(
        keys[0].0 < keys[2].0 && keys[0].0 == keys[1].0 && keys[2].0 == keys[3].0,
        "{keys:?}"
    );
    assert_eq!(
        keys.iter().map(|k| k.1).collect::<Vec<_>>(),
        [0, 1, 0, 2],
        "{keys:?}"
    );
    plan.validate().unwrap();
    plan.looks.swap(0, 1);
    assert!(plan.validate().is_err(), "a state out of order is refused");
    plan.looks.swap(0, 1);
    plan.looks[1] = plan.looks[0].clone();
    assert!(plan.validate().is_err(), "a state twice is refused");
}

/// A node's own `:disabled` where it writes no `disabled` never holds: a
/// lint, never a refusal; a class's part on such a node is not judged.
#[test]
fn a_state_its_node_never_meets_is_linted() {
    let src = "style Toggle\n  :disabled\n    opacity = 0.5\n\ncomponent App\n  state on = false\n  action go\n    on = !on\n  view\n    column\n      column press=go testId=\"own\"\n        :disabled\n          opacity = 0.5\n        text \"own\"\n      column class=Toggle press=go testId=\"class\"\n        text \"class\"\n      column press=go disabled=on testId=\"met\"\n        :disabled\n          opacity = 0.5\n        text \"met\"\n";
    let (plan, warnings) = contract::compile_warned(src);
    assert!(plan.is_ok(), "{plan:?}");
    let dead: Vec<_> = warnings
        .iter()
        .filter(|w| w.id == "lower-state-dead")
        .collect();
    assert_eq!(dead.len(), 1, "{warnings:?}");
    assert_eq!(dead[0].span.line, 13, "at its row: {dead:?}");
}

/// Review round 1: a state that leaves a row unwritten in some arm keeps
/// the base's; a link keeps its colour, an `hr` its separator, unless the
/// state holds; a `:checked` row is no disabled look.
#[test]
fn a_state_keeps_the_base_it_does_not_write() {
    let src = "style A\n  background-color = \"#111111\"\n  :disabled\n    background-color = \"#0000ff\"\n\nstyle B\n  background-color = \"#222222\"\n\ncomponent App\n  state pick = true\n  state busy = false\n  action swap\n    pick = !pick\n  action hold\n    busy = !busy\n  view\n    column\n      column press=swap testId=\"swap\"\n        text \"swap\"\n      column press=hold testId=\"hold\"\n        text \"hold\"\n      column class=(pick ? A : B) disabled=busy testId=\"c\"\n        text \"c\"\n      text testId=\"p\"\n        text \"link\" href=\"https://example.com\" disabled=busy testId=\"link\"\n          :disabled\n            color = \"#808080\"\n";
    let mut r = booted(src);
    press(&mut r, "hold");
    assert_eq!(colour(&r, "c", StyleId::BackgroundColor), "#0000ff");
    press(&mut r, "swap");
    assert_eq!(
        colour(&r, "c", StyleId::BackgroundColor),
        "#222222",
        "B's base, B writing no :disabled row"
    );
    assert_eq!(colour(&r, "link", StyleId::TextColor), "#808080");
    press(&mut r, "hold");
    assert_ne!(colour(&r, "link", StyleId::TextColor), "#808080");
    let enabled = colour(&r, "link", StyleId::TextColor);
    let plain = booted("component App\n  view\n    text testId=\"p\"\n      text \"link\" href=\"https://example.com\" testId=\"link\"\n");
    assert_eq!(
        enabled,
        colour(&plain, "link", StyleId::TextColor),
        "the link's own colour while enabled"
    );
    // `:checked` opacity alone: no disabled look.
    let checked = contract::compile("component App\n  state on = false\n  action go\n    on = !on\n  view\n    column press=go disabled=on aria-pressed=on\n      :checked\n        opacity = 0.5\n      text \"c\"\n").unwrap();
    let styled = checked.nodes.iter().any(|n| {
        n.bindings.iter().any(|b| {
            let b = checked.binding(b);
            b.kind == exact_plan::BindingKind::Prop
                && b.id == exact_kernel::PropId::DisabledStyled as u16
        })
    });
    assert!(!styled);
}

#[test]
fn round_one_refusals_and_typing() {
    // A grouped list's row is a cell.
    assert_eq!(
        refused("component App\n  view\n    list appearance=\"auto\"\n      section\n        button press=go\n          :active\n            opacity = 0.5\n          text \"r\"\n  state n = 0\n  action go\n    n = n + 1\n"),
        "lower-state-target"
    );
    // A component's element as a list's row and elsewhere: refused only as the row.
    let card = "component Card\n  state n = 0\n  action go\n    n = n + 1\n  view\n    column press=go\n      :hover\n        background-color = \"#0000ff\"\n      text \"x\"\n";
    assert_eq!(
        refused(&format!(
            "component App\n  view\n    column\n      Card()\n\n{card}"
        )),
        "compiled"
    );
    // As a list's row first, then elsewhere: the row is refused, the other is not
    // judged as one (the refusal names the row, at its instance).
    assert_eq!(refused(&format!("component App\n  view\n    column\n      Card()\n      list flex=1\n        Card()\n\n{card}")), "lower-state-target");
    // A computed list in a state's border-color, as on the node.
    assert_ne!(refused("component App\n  state c = \"#ff0000\"\n  state busy = false\n  view\n    column disabled=busy\n      :disabled\n        border-color=`${c} ${c}`\n      text \"b\"\n"), "compiled");
    // A row typed as the node's own rows are: `none` an arm.
    assert_eq!(refused("component App\n  state on = false\n  state busy = false\n  view\n    column disabled=busy\n      :disabled\n        opacity=(on ? 0.5 : none)\n      text \"b\"\n"), "compiled");
    // A child in a state.
    assert_eq!(
        refused("component App\n  view\n    column\n      :hover\n        text \"x\"\n"),
        "syntax-state"
    );
    // fmt keeps a continuation after a state block on its own line.
    let src = "component App\n  state busy = false\n  view\n    column press=go disabled=busy background-color=\"#ff0000\" padding-top=10 padding-bottom=10 padding-left=12 padding-right=12\n      :disabled\n        opacity=0.5\n      testId=\"b\"\n      text \"b\"\n  action go\n    busy = !busy\n";
    let once = contract_syntax::fmt::format(src).unwrap();
    assert_eq!(contract_syntax::fmt::format(&once).unwrap(), once);
}

/// Review round 1 (Astra): a state row that computes a token's name keeps
/// every colour token, as the node's own computed row does (D7).
#[test]
fn a_computed_state_row_keeps_the_tokens_it_may_name() {
    let dir = std::env::temp_dir().join(format!("exact2-state-retained-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("app.tamber"),
        "tamber t\n\ntheme\n  brand-on = #0a7aff\n  brand-off = #888888\n",
    )
    .unwrap();
    std::fs::write(dir.join("app.contract"), "component App\n  state suffix = \"on\"\n  state busy = false\n  view\n    column disabled=busy\n      :disabled\n        background-color=`brand-${suffix}`\n      text \"b\"\n").unwrap();
    let plan =
        contract::compile_path(&dir.join("app.contract")).unwrap_or_else(|e| panic!("{e:?}"));
    let names: Vec<&str> = plan.theme.iter().map(|t| plan.str(t.name)).collect();
    assert!(
        names.contains(&"brand-on") && names.contains(&"brand-off"),
        "{names:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Review round 2: a grouped list's implied ground stays implied under a
/// state; a `match` with no value in an arm and no base is said; a `match`
/// arm with no value over a base keeps it; a refused look leaves a source
/// map's sites in step.
#[test]
fn round_two_edges() {
    // A `:disabled` background on a grouped list: its ground is a metric
    // the compiler writes, kept implied in the fold.
    assert_eq!(
        refused("component App\n  state busy = false\n  view\n    list appearance=\"auto\" disabled=busy\n      :disabled\n        background-color = \"#eeeeee\"\n      section\n        text \"r\"\n"),
        "compiled"
    );
    // No base and a match arm with no value: said, not a type error.
    assert_eq!(
        refused("component App\n  state busy = false\n  state opt: option<number> = none\n  view\n    column disabled=busy\n      :disabled\n        opacity=match opt { case some(x) => x, case none => none }\n      text \"b\"\n"),
        "lower-state-row"
    );
    // A ternary with none in its first arm and no base: a chain.
    assert_eq!(
        refused("component App\n  state busy = false\n  state on = false\n  view\n    column disabled=busy\n      :disabled\n        opacity=(on ? none : 0.5)\n      text \"b\"\n"),
        "compiled"
    );
    // A refused look in a mapped build: the compile says why, and panics not.
    let src = "component App\n  view\n    column\n      column press=go\n        :hover\n          opacity=true\n        text \"a\"\n      column\n        text \"b\"\n  state n = 0\n  action go\n    n = n + 1\n";
    let dir = std::env::temp_dir().join(format!("exact2-state-mapped-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("app.contract"), src).unwrap();
    assert!(contract::compile_path_mapped(&dir.join("app.contract")).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Review round 3: a swipe row is still a row; a grouped section's header
/// text is none; a `match` binding the name its base reads keeps the base;
/// an author's state colour over an implied one is written.
#[test]
fn round_three_edges() {
    assert_eq!(
        refused("component App\n  state rows = [\"a\"]\n  view\n    list flex=1\n      for row in rows key=row\n        column press=go delete=go aria-pressed=true\n          :checked\n            background-color = \"#00ff00\"\n          text row\n  action go\n    rows = rows\n"),
        "lower-state-target"
    );
    assert_eq!(
        refused("component App\n  state busy = false\n  view\n    list appearance=\"auto\"\n      section\n        header\n          text \"H\" disabled=busy\n            :disabled\n              opacity = 0.5\n        text \"r\"\n"),
        "compiled"
    );
    let src = "component App\n  state x = 0.7\n  state busy = false\n  state opt: option<number> = some(0.2)\n  action hold\n    busy = !busy\n  view\n    column\n      column press=hold testId=\"hold\"\n        text \"hold\"\n      column opacity=x disabled=busy testId=\"m\"\n        :disabled\n          opacity=match opt { case some(x) => none, case none => 0.5 }\n        text \"m\"\n";
    let mut r = booted(src);
    press(&mut r, "hold");
    let key = r.kernel().find_by_test_id("m")[0];
    let o = format!(
        "{:?}",
        r.kernel()
            .node_by_key(key)
            .unwrap()
            .computed(StyleId::Opacity)
    );
    assert!(
        o.contains("0.69999"),
        "none in the some arm keeps the base x, though the arm binds x: {o}"
    );
}

/// Stage 8 F: a param read only by a state part is reached (D15's
/// `PostButton.pressed`, read by `:active` alone), kept by the plan, and
/// named by the look's row as a static row names a token; one read by
/// `:disabled` folds into the node's row.
#[test]
fn a_param_read_only_by_a_state_part_is_reached() {
    let dir = std::env::temp_dir().join(format!("exact2-state-params-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("app.tamber"),
        "tamber t\n\ntheme\n  accent.fill = #0a7aff\n  ink = #111111\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "component App\n  view\n    PostButton()\n\ncomponent PostButton\n  state busy = false\n  action post\n    busy = !busy\n  params\n    fill: color = \"accent.fill\"  control=\"color\"\n    pressed: color = \"color-mix(in oklab, param.fill 88%, ink)\"  control=\"none\"\n    faded: number = \"0.4\"\n  view\n    column press=post disabled=busy background-color=\"param.fill\"\n      :active\n        background-color = \"param.pressed\"\n      :disabled\n        opacity = \"param.faded\"\n      text \"Post\"\n",
    )
    .unwrap();
    let plan =
        contract::compile_path(&dir.join("app.contract")).unwrap_or_else(|e| panic!("{e:?}"));
    let names: Vec<&str> = plan.theme.iter().map(|t| plan.str(t.name)).collect();
    let pressed = contract_lower::params::token_name("app.contract#PostButton", "pressed");
    let faded = contract_lower::params::token_name("app.contract#PostButton", "faded");
    assert!(
        names.contains(&pressed.as_str()) && names.contains(&faded.as_str()),
        "{names:?}"
    );
    assert!(plan.looks.iter().any(|l| l.state == 0), "the :active look");
    let _ = std::fs::remove_dir_all(&dir);
}
