//! The fixed record order of `textarea decorations=` (LLP 1114.007) and of
//! a Markdown field's `atomFaces` and `atompress` (LLP 1045.000 D2, D7).
//! The producer bridge serializes `TextDecorations` and `AtomFace` as named
//! JSON fields.

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
    // A field is never defaulted (LLP 1035.005.000 D3): an empty `color` or
    // `backgroundColor` is the kind's paint, as an empty label is the default.
    shapes.map.insert(
        "AtomFace".into(),
        vec![
            ("source".into(), Ty::String),
            ("label".into(), Ty::String),
            ("icon".into(), Ty::String),
            ("title".into(), Ty::String),
            ("color".into(), Ty::String),
            ("backgroundColor".into(), Ty::String),
        ],
    );
    // In the order `exact_runner::AtomEvent` writes it.
    shapes.map.insert(
        "AtomEvent".into(),
        vec![
            ("kind".into(), Ty::String),
            ("source".into(), Ty::String),
            ("href".into(), Ty::String),
            ("start".into(), Ty::Number),
            ("end".into(), Ty::Number),
            ("shiftKey".into(), Ty::Bool),
            ("ctrlKey".into(), Ty::Bool),
            ("altKey".into(), Ty::Bool),
            ("metaKey".into(), Ty::Bool),
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
