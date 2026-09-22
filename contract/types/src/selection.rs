//! The positional `select` payload shared with the runner (LLP 1045 D6).
use super::{Shapes, Ty};

pub(super) fn declare(shapes: &mut Shapes) {
    shapes.map.insert(
        "MarkdownSelection".into(),
        vec![
            ("formats".into(), Ty::String),
            ("mixed".into(), Ty::Bool),
            ("link".into(), Ty::String),
            ("unavailable".into(), Ty::String),
        ],
    );
}
