//! Bad data is not a reason to restart.
use super::*;

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
