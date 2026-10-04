//! The positional event payloads shared with the runner: `select`'s
//! (LLP 1045 D6), a file input's `change` (LLP 1069.002 D3), the
//! pointer's (LLP 1056 §3 stage 3), `key`'s optional `KeyboardEvent` and
//! `scroll`'s optional `ScrollEvent` (chat F4).
use super::{Shapes, Ty};

/// The DOM event record a handler's event offers its action as an optional
/// last parameter, after whatever the event always carries (`key`'s name):
/// the action takes it by declaring one more parameter, or leaves it. One
/// rule for every such event (LLP 1056 §3 stage 3; chat F2, kanban F27):
/// analysis counts it (`contract_analyze::handler_arity`), the view's
/// handlers type it here, the runner appends it (`Event::record`), and the
/// JS target passes it as a trailing argument a shorter action ignores.
pub fn event_record(attr: &str) -> Option<&'static str> {
    match attr {
        "pointerdown" | "pointerup" | "pointermove" => Some("PointerEvent"),
        "key" => Some("KeyboardEvent"),
        "scroll" => Some("ScrollEvent"),
        _ => None,
    }
}

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
    // DOM's `PointerEvent`, the subset every host measures, in the order
    // `exact_runner::PointerEvent` writes it: the point from the node's
    // content box, the buttons' bits, the pressure, the device, its id.
    shapes.map.insert(
        "PointerEvent".into(),
        vec![
            ("offsetX".into(), Ty::Number),
            ("offsetY".into(), Ty::Number),
            ("buttons".into(), Ty::Number),
            ("pressure".into(), Ty::Number),
            ("pointerType".into(), Ty::String),
            ("pointerId".into(), Ty::Number),
        ],
    );
    // What a `scroll` handler's action hears after the offsets when it takes
    // one more parameter, in the order `exact_runner::ScrollEvent` writes it:
    // the scroller's own `Element` fields as the event fires, so "at the
    // end" is the web's `scrollHeight - scrollTop - clientHeight` (chat F4).
    shapes.map.insert(
        "ScrollEvent".into(),
        [
            "scrollLeft",
            "scrollTop",
            "scrollWidth",
            "scrollHeight",
            "clientWidth",
            "clientHeight",
        ]
        .map(|f| (f.into(), Ty::Number))
        .to_vec(),
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
