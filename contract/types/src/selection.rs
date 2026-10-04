//! The positional event payloads shared with the runner: `select`'s
//! (LLP 1045 D6), a file input's `change` (LLP 1069.002 D3) and `key`'s
//! optional `KeyboardEvent`.
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
    // What a `key` handler's action hears after the key when it takes one
    // more parameter, in the order `Event::Key` writes it: the DOM's
    // `KeyboardEvent` fields by their names (chat F2, kanban F27).
    shapes.map.insert(
        "KeyboardEvent".into(),
        vec![
            ("key".into(), Ty::String),
            ("shiftKey".into(), Ty::Bool),
            ("ctrlKey".into(), Ty::Bool),
            ("altKey".into(), Ty::Bool),
            ("metaKey".into(), Ty::Bool),
        ],
    );
    // One picked file, in the order `exact_runner::Picked` writes it: the
    // `app:/tmp/picked/…` path, the original name, the MIME type and size
    // of what is at the path, and for media the pixel size (orientation
    // applied) and a video's duration in seconds, `none` when unread.
    let maybe = || Ty::Option(Box::new(Ty::Number));
    shapes.map.insert(
        "Picked".into(),
        vec![
            ("path".into(), Ty::String),
            ("name".into(), Ty::String),
            ("type".into(), Ty::String),
            ("size".into(), Ty::Number),
            ("width".into(), maybe()),
            ("height".into(), maybe()),
            ("duration".into(), maybe()),
        ],
    );
}
