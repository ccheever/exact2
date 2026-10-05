//! The positional event payloads shared with the runner: `select`'s
//! (LLP 1045 D6), a file input's `change` (LLP 1069.002 D3), the
//! pointer's (LLP 1056 §3 stage 3), `key`'s optional `KeyboardEvent` and
//! `scroll`'s optional `ScrollEvent` (chat F4), and the media session's
//! two (LLP 1098 D1, D2).
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
        // A `contextmenu` is DOM's `PointerEvent` too (UI Events): where the
        // secondary click was (studio diary R22).
        "pointerdown" | "pointerup" | "pointermove" | "contextmenu" => Some("PointerEvent"),
        "wheel" => Some("WheelEvent"),
        "drop" => Some("DragEvent"),
        "key" => Some("KeyboardEvent"),
        "scroll" => Some("ScrollEvent"),
        "press" => Some("MouseEvent"),
        "copy" | "cut" | "paste" => Some("ClipboardEvent"),
        "selectionchange" => Some("Selection"),
        // @ref LLP 1098 D2 — the Media Session's actions, as
        // `setActionHandler` hands them.
        "seekbackward" | "seekforward" | "seekto" | "previoustrack" | "nexttrack" | "stop" => {
            Some("MediaSessionActionDetails")
        }
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
    // DOM's `ClipboardEvent`, its data as plain text (`getData("text/plain")`):
    // what a paste carries; empty on copy and cut, as the DOM's is until a
    // listener sets it — the action writes the clipboard with `copyText`.
    shapes
        .map
        .insert("ClipboardEvent".into(), vec![("text".into(), Ty::String)]);
    // The part of the reader's selection inside one `text` (the reader
    // diary), what `selectionchange` hands its action, in the order
    // `exact_runner::Event::SelectionChange` writes it: the selected text
    // (DOM `Range.toString()`) and its UTF-16 start and end in the node's
    // own text, as written. Nothing selected there is "" with equal offsets.
    shapes.map.insert(
        "Selection".into(),
        vec![
            ("text".into(), Ty::String),
            ("start".into(), Ty::Number),
            ("end".into(), Ty::Number),
        ],
    );
    // DOM's `PointerEvent`, the subset every host measures, in the order
    // `exact_runner::PointerEvent` writes it: the point from the node's
    // content box, the buttons' bits, the pressure, the device, its id, and
    // the modifiers held (a `MouseEvent`'s).
    shapes.map.insert(
        "PointerEvent".into(),
        vec![
            ("offsetX".into(), Ty::Number),
            ("offsetY".into(), Ty::Number),
            ("buttons".into(), Ty::Number),
            ("pressure".into(), Ty::Number),
            ("pointerType".into(), Ty::String),
            ("pointerId".into(), Ty::Number),
            ("shiftKey".into(), Ty::Bool),
            ("ctrlKey".into(), Ty::Bool),
            ("altKey".into(), Ty::Bool),
            ("metaKey".into(), Ty::Bool),
        ],
    );
    // DOM's `MouseEvent`, the modifiers held, what a `press` action may take
    // (gallery F20: shift-click range select, ⌘-click), in the order
    // `exact_runner::KeyModifiers::mouse` writes it.
    shapes.map.insert(
        "MouseEvent".into(),
        vec![
            ("shiftKey".into(), Ty::Bool),
            ("ctrlKey".into(), Ty::Bool),
            ("altKey".into(), Ty::Bool),
            ("metaKey".into(), Ty::Bool),
        ],
    );
    // DOM's `WheelEvent` (studio diary R3), in the order
    // `exact_runner::WheelEvent` writes it: the point from the node's content
    // box, the deltas, `deltaMode` (0 pixels, 1 lines, 2 pages) and the
    // modifiers; a trackpad's pinch is a wheel with `ctrlKey`, as browsers
    // deliver one.
    let modifiers = ["shiftKey", "ctrlKey", "altKey", "metaKey"].map(|f| (f.into(), Ty::Bool));
    shapes.map.insert(
        "WheelEvent".into(),
        ["offsetX", "offsetY", "deltaX", "deltaY", "deltaMode"]
            .map(|f| (f.into(), Ty::Number))
            .into_iter()
            .chain(modifiers.clone())
            .collect(),
    );
    // DOM's `DragEvent` at a `drop` of files (studio diary R19), its
    // `dataTransfer.files` as the `doc:` handles the host minted for them
    // (LLP 1069.010 D1, as a picker's), in `exact_runner::DropEvent`'s order.
    shapes.map.insert(
        "DragEvent".into(),
        [
            ("offsetX".into(), Ty::Number),
            ("offsetY".into(), Ty::Number),
            ("files".into(), Ty::List(Box::new(Ty::String))),
        ]
        .into_iter()
        .chain(modifiers)
        .collect(),
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
    // @ref LLP 1098 D1 — the Media Session's `MediaMetadata`, by its
    // constructor's name and fields: the one compiler shape an app builds
    // (`records::COMPILER_RECORDS`), on `audio` or `video`'s `metadata=`.
    // `artwork` is one image's source, not a list of `MediaImage`s.
    shapes.map.insert(
        "MediaMetadata".into(),
        ["title", "artist", "album", "artwork"]
            .map(|f| (f.into(), Ty::String))
            .to_vec(),
    );
    // @ref LLP 1098 D2 — `MediaSessionActionDetails`, what a media session
    // action hears, in the order `exact_runner`'s `media_session` writes it:
    // the action's name, the seek's offset (the platform's, else the
    // element's own), `seekto`'s time and whether it is a fast seek.
    shapes.map.insert(
        "MediaSessionActionDetails".into(),
        vec![
            ("action".into(), Ty::String),
            ("seekOffset".into(), Ty::Number),
            ("seekTime".into(), Ty::Number),
            ("fastSeek".into(), Ty::Bool),
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
