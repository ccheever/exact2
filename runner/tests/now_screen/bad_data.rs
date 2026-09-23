//! Bad data is not a reason to restart; what still poisons, and what a
//! poisoned runner leaks (nothing).
use super::*;

/// A repeated key is the data's error, not a reason to restart.
struct Duplicates;

impl DataSource for Duplicates {
    fn query(&mut self, source: &str, _args: &[Value]) -> Result<Value, DataError> {
        match source {
            "stations" => Ok(Value::list(vec![])),
            "departures" => Ok(Value::list(vec![
                departure("same", 1.0, 1.0),
                departure("same", 2.0, 2.0),
            ])),
            other => Err(DataError::UnknownSource(other.into())),
        }
    }
}

/// `label` answers `some` until `poke` has run, then `none`, which the
/// view unwraps: a trap while the tree changes, the one way left to poison.
pub(super) struct Fragile;

impl DataSource for Fragile {
    fn query(&mut self, _: &str, args: &[Value]) -> Result<Value, DataError> {
        Ok(match args[0].as_number() {
            Some(0.0) => Value::some(Value::str("ok")),
            _ => Value::NONE,
        })
    }
}

pub(super) fn fragile() -> Runner<Fragile> {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let number = b.primitive(TypeKind::Number);
    let string = b.primitive(TypeKind::String);
    let label = b.option(string);
    let zero = b.constant(&Value::Number(0.));
    let pokes = b.slot("pokes", number, zero);
    let ticks = b.slot("ticks", number, zero);
    let mut arg = Asm::new();
    arg.load_slot(pokes);
    let arg = b.code(arg);
    let resource = b.resource("label", "label", &[arg], label, None);
    let mut text = Asm::new();
    text.load_resource(resource).op(Opcode::Unwrap, &[]);
    let text = b.code(text);
    b.node(
        NodeType::Text as u8,
        None,
        None,
        0,
        &[BindingsRow {
            kind: BindingKind::Prop,
            id: PropId::Text as u16,
            expr: text,
        }],
        &[],
        None,
    );
    let scheme = b.str("setScheme");
    let dark = b.str("dark");
    for (name, slot) in [("poke", pokes), ("tick", ticks)] {
        let mut body = Asm::new();
        body.str(dark)
            .command(scheme, 1)
            .load_slot(slot)
            .number(1.)
            .op(Opcode::Add, &[])
            .store_slot(slot)
            .op(Opcode::Unit, &[]);
        let body = b.code(body);
        let action = b.action(name, &[], &[slot], body);
        if name == "tick" {
            b.timer(1000, action);
        }
    }
    Runner::boot(
        b.finish().unwrap(),
        Fragile,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

#[test]
fn repeated_keys_render_in_order_and_a_poisoned_runner_leaks_no_commands() {
    let (plan, _) = now_screen();
    let r = Runner::boot(
        plan,
        Duplicates,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(!r.is_poisoned());
    assert!(
        r.journal()
            .any(|l| l.contains("repeats; this row is d1:s:same")),
        "{:?}",
        r.journal().collect::<Vec<_>>()
    );
    // A runner poisoned after commit leaks no commands, even ones queued earlier.
    let mut r = fragile();
    r.act("tick", vec![]).unwrap();
    assert!(matches!(
        r.act("poke", vec![]),
        Err(RunnerError::Instance(
            exact_runner::instance::InstanceError::Trap(Trap::UnwrapNone { .. })
        ))
    ));
    assert!(r.is_poisoned());
    assert!(r.take_commands().is_empty(), "no command survives a poison");
    assert!(matches!(r.act("tick", vec![]), Err(RunnerError::Poisoned)));
}

/// Data a row's grammar refuses is CSS's invalid value at computed-value
/// time: the row is unset, a journal line says so, and the runner goes on.
#[test]
fn an_invalid_value_from_data_unsets_its_row_and_is_journaled() {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let string = b.primitive(TypeKind::String);
    let number = b.primitive(TypeKind::Number);
    let color = b.constant(&Value::str("#112233"));
    let color = b.slot("color", string, color);
    let one = b.constant(&Value::Number(1.));
    let position = b.slot("position", number, one);
    let read = |b: &mut PlanBuilder, slot| {
        let mut a = Asm::new();
        a.load_slot(slot);
        b.code(a)
    };
    let (background, set_in) = (read(&mut b, color), read(&mut b, position));
    b.node(
        NodeType::View as u8,
        None,
        None,
        0,
        &[
            BindingsRow {
                kind: BindingKind::Style,
                id: StyleId::BackgroundColor as u16,
                expr: background,
            },
            BindingsRow {
                kind: BindingKind::Prop,
                id: PropId::AccessibilityPosInSet as u16,
                expr: set_in,
            },
        ],
        &[],
        None,
    );
    for (name, slot, ty) in [("paint", color, string), ("place", position, number)] {
        let mut body = Asm::new();
        body.load_param(0).store_slot(slot).op(Opcode::Unit, &[]);
        let body = b.code(body);
        b.action(name, &[("value", ty)], &[slot], body);
    }
    let mut r = Runner::boot(
        b.finish().unwrap(),
        Schedule::default(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let view = r.roots()[0];
    let has_background = |r: &Runner<Schedule>| {
        let mut row = exact_kernel::StyleMask::default();
        row.set(StyleId::BackgroundColor);
        r.kernel().node(view).unwrap().style.mask.intersects(row)
    };
    let position = |r: &Runner<Schedule>| {
        r.kernel()
            .node(view)
            .unwrap()
            .props
            .get(PropId::AccessibilityPosInSet)
            .cloned()
    };
    assert!(has_background(&r));
    r.act("paint", vec![Value::str("not a colour")]).unwrap();
    assert!(!r.is_poisoned());
    assert!(!has_background(&r), "unset, as CSS does an invalid value");
    assert!(r
        .journal()
        .any(|l| l.contains("invalid background-color value \"not a colour\"; unset")));
    r.act("paint", vec![Value::str("#445566")]).unwrap();
    assert!(has_background(&r));
    assert_eq!(position(&r), Some(exact_kernel::PropValue::Int(1)));
    r.act("place", vec![Value::Number(1.5)]).unwrap();
    assert_eq!(position(&r), None);
    r.act("place", vec![Value::Number(3.)]).unwrap();
    assert_eq!(position(&r), Some(exact_kernel::PropValue::Int(3)));
}
