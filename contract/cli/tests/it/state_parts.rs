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
    let node = plan
        .nodes
        .iter()
        .find(|n| n.looks.len > 0)
        .expect("a node with looks");
    let looks: Vec<_> = (node.looks.start..node.looks.start + node.looks.len)
        .map(|i| &plan.looks[i as usize])
        .collect();
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
