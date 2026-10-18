//! The fixed record order of `textarea decorations=` (LLP 1114.007).
//! The producer bridge serializes these records as named JSON fields.

use super::{Shapes, Ty};

pub(crate) fn declare(shapes: &mut Shapes) {
    let list = |name: &str| Ty::List(Box::new(Ty::Record(name.into())));
    let number = |name: &str| (name.into(), Ty::Number);
    let string = |name: &str| (name.into(), Ty::String);
    shapes.map.insert(
        "TextDecorationRange".into(),
        vec![number("start"), number("end")],
    );
    shapes.map.insert(
        "TextDecorationSpan".into(),
        vec![string("name"), number("start"), number("end")],
    );
    shapes.map.insert(
        "TextDecorationBlock".into(),
        vec![
            string("name"),
            number("start"),
            number("end"),
            string("marker"),
        ],
    );
    shapes.map.insert(
        "TextDecorationHidden".into(),
        vec![
            number("start"),
            number("end"),
            (
                "reveal".into(),
                Ty::Option(Box::new(Ty::Record("TextDecorationRange".into()))),
            ),
        ],
    );
    shapes.map.insert(
        "TextDecorationAtom".into(),
        vec![
            string("name"),
            number("start"),
            number("end"),
            string("label"),
        ],
    );
    shapes.map.insert(
        "TextDecorations".into(),
        vec![
            string("value"),
            ("spans".into(), list("TextDecorationSpan")),
            ("blocks".into(), list("TextDecorationBlock")),
            ("hidden".into(), list("TextDecorationHidden")),
            ("atoms".into(), list("TextDecorationAtom")),
        ],
    );
}
