//! The fixed record order of `textarea decorations=` (LLP 1114.007).
//! The producer bridge serializes these records as named JSON fields.

use super::{Shapes, Ty};

pub(crate) fn declare(shapes: &mut Shapes) {
    shapes.map.insert(
        "TextDecorationSpan".into(),
        vec![
            ("name".into(), Ty::String),
            ("start".into(), Ty::Number),
            ("end".into(), Ty::Number),
        ],
    );
    shapes.map.insert(
        "TextDecorations".into(),
        vec![
            ("value".into(), Ty::String),
            (
                "spans".into(),
                Ty::List(Box::new(Ty::Record("TextDecorationSpan".into()))),
            ),
        ],
    );
}
