//! The positional event payloads shared with the runner: `select`'s, a
//! file input's `change`, the
//! pointer's, `key`'s and `keyup`'s optional `KeyboardEvent`,
//! `scroll`'s optional `ScrollEvent`, `reorderdrop`'s optional
//! `ReorderEvent`, and the `InputEvent` of `input`, `change`
//! and a text field's `select`, with the
//! `setSelectionRange` command's arguments, and the media session's
//! two.
use super::{err, infer, Scope, Shapes, Ty, TypeError};
use contract_syntax::{Expr, Span};

/// HTML's range replacement, with the field's id first.
pub(super) fn range_text_args(
    args: &[Expr],
    scope: &Scope,
    shapes: &Shapes,
    span: Span,
) -> Result<(), TypeError> {
    let types: Vec<Ty> = args
        .iter()
        .map(|a| infer(a, scope, shapes))
        .collect::<Result<_, _>>()?;
    if matches!(
        types.as_slice(),
        [Ty::String, Ty::String]
            | [Ty::String, Ty::String, Ty::Number, Ty::Number]
            | [Ty::String, Ty::String, Ty::Number, Ty::Number, Ty::String]
    ) {
        return Ok(());
    }
    err(
        "type-set-range-text",
        "`setRangeText(id, text[, start, end[, selectionMode]])`: the field's id and replacement strings, optional UTF-16 offsets, and optionally `select`, `start`, `end` or `preserve` (default)",
        span,
    )
}

/// `setSelectionRange("id", start, end[, direction])`: HTML's method, its
/// field named first by `id`.
pub(super) fn selection_range_args(
    args: &[Expr],
    scope: &Scope,
    shapes: &Shapes,
    span: Span,
) -> Result<(), TypeError> {
    let types: Vec<Ty> = args
        .iter()
        .map(|a| infer(a, scope, shapes))
        .collect::<Result<_, _>>()?;
    if matches!(types.as_slice(), [id, Ty::Number, Ty::Number] if id.is_string())
        || matches!(types.as_slice(), [id, Ty::Number, Ty::Number, direction] if id.is_string() && direction.is_string())
    {
        return Ok(());
    }
    err(
        "type-set-selection-range",
        "`setSelectionRange(\"id\", start, end)` or `setSelectionRange(\"id\", start, end, \"backward\")`: the node's `id`, the UTF-16 offsets, and optionally `forward`, `backward` or `none`",
        span,
    )
}

/// The DOM event record a handler's event offers its action as an optional
/// last parameter, after whatever the event always carries (`key`'s name):
/// the action takes it by declaring one more parameter, or leaves it. One
/// rule for every such event:
/// analysis counts it (`contract_analyze::handler_arity`), the view's
/// handlers type it here, the runner appends it (`Event::record`), and the
/// JS target passes it as a trailing argument a shorter action ignores.
pub fn event_record(attr: &str) -> Option<&'static str> {
    match attr {
        // A `contextmenu` is DOM's `PointerEvent` too (UI Events): where the
        // secondary click was.
        "pointerdown" | "pointerup" | "pointermove" | "contextmenu" => Some("PointerEvent"),
        "wheel" => Some("WheelEvent"),
        "drop" | "dragenter" | "dragleave" => Some("DragEvent"),
        "atompress" => Some("AtomEvent"),
        "key" | "keycapture" | "keyup" => Some("KeyboardEvent"),
        "scroll" => Some("ScrollEvent"),
        "toggle" => Some("ToggleEvent"),
        "press" => Some("MouseEvent"),
        "copy" | "cut" | "paste" => Some("ClipboardEvent"),
        "selectionchange" => Some("Selection"),
        // The target as HTML's `input` and `change` leave it: its value,
        // checked state and selection.
        "input" | "change" | "beforeinput" => Some("InputEvent"),
        "focusin" | "focusout" => Some("FocusEvent"),
        "resize" => Some("DOMRectReadOnly"),
        "intersect" => Some("IntersectionObserverEntry"),
        "reorderdrop" => Some("ReorderEvent"),
        // Web MIDI's `MIDIMessageEvent`, with the input,
        // runner time and the transport's beat.
        "midimessage" => Some("MIDIMessageEvent"),
        // The Media Session's actions, as
        // `setActionHandler` hands them.
        "seekbackward" | "seekforward" | "seekto" | "previoustrack" | "nexttrack" | "stop" => {
            Some("MediaSessionActionDetails")
        }
        // What an iframe's guest shows, seen from
        // outside; `framenavigate` is an iframe's `navigate`
        // (`contract_syntax::typed_event`).
        "framenavigate" | "currententrychange" | "navigatesuccess" | "navigateerror" => {
            Some("FrameNavigation")
        }
        // A guest's request for a window, by Chrome's
        // `<webview>` event and Electron's disposition names; and a download
        // it started, by Electron's `DownloadItem` states.
        "newwindow" => Some("FrameNewWindow"),
        "download" => Some("FrameDownload"),
        // The answer to `captureScreenshot`: the file
        // and its size, or why there is none.
        "capture" => Some("FrameCapture"),
        // An agent-controlled frame's dialog, by WebDriver
        // BiDi's `browsingContext.userPromptOpened`.
        "userpromptopened" => Some("FrameUserPrompt"),
        _ => None,
    }
}

pub(super) fn declare(shapes: &mut Shapes) {
    // The Navigation API's names for what an iframe's
    // guest shows; the runner's `FrameNavigation::value` is in this order.
    shapes.map.insert(
        "FrameNavigation".into(),
        vec![
            ("url".into(), Ty::String),
            ("destination".into(), Ty::String),
            ("navigationId".into(), Ty::Number),
            ("navigationType".into(), Ty::String),
            ("title".into(), Ty::String),
            ("icons".into(), Ty::List(Box::new(Ty::String))),
            ("canGoBack".into(), Ty::Bool),
            ("canGoForward".into(), Ty::Bool),
            ("loading".into(), Ty::Bool),
            ("partitioned".into(), Ty::Bool),
            ("controller".into(), Ty::String),
            ("error".into(), Ty::String),
            ("message".into(), Ty::String),
            ("recovering".into(), Ty::Bool),
        ],
    );
    // The runner's `frame_navigation::other` builds it
    // in this order: the `app:/` path the command named, the PNG's size in
    // pixels, and on failure a DOMException name and the platform's words.
    shapes.map.insert(
        "FrameCapture".into(),
        vec![
            ("path".into(), Ty::String),
            ("width".into(), Ty::Number),
            ("height".into(), Ty::Number),
            ("error".into(), Ty::String),
            ("message".into(), Ty::String),
        ],
    );
    // The runner's `frame_navigation::other` builds it
    // in this order: BiDi's `userPromptOpened` fields.
    shapes.map.insert(
        "FrameUserPrompt".into(),
        vec![
            ("type".into(), Ty::String),
            ("message".into(), Ty::String),
            ("defaultValue".into(), Ty::String),
        ],
    );
    // Geometry's `DOMRect`, by its constructor's fields:
    // what `getBoundingClientRect()` gives, and a capture's `clip=`.
    shapes.map.insert(
        "DOMRect".into(),
        ["x", "y", "width", "height"]
            .map(|f| (f.into(), Ty::Number))
            .to_vec(),
    );
    // The runner's `frame_navigation::new_window` and
    // `download` build these in this order.
    shapes.map.insert(
        "FrameNewWindow".into(),
        vec![
            ("url".into(), Ty::String),
            ("disposition".into(), Ty::String),
        ],
    );
    shapes.map.insert(
        "FrameDownload".into(),
        vec![
            ("url".into(), Ty::String),
            ("suggestedFilename".into(), Ty::String),
            ("state".into(), Ty::String),
            ("path".into(), Ty::String),
        ],
    );
    // `exact_runner::midi`'s order: the input's `id`, the
    // message's bytes, when it arrived in runner time, and the transport's
    // beat then (−1 while stopped).
    shapes.map.insert(
        "MIDIMessageEvent".into(),
        vec![
            ("input".into(), Ty::String),
            ("data".into(), Ty::List(Box::new(Ty::Number))),
            ("timeStamp".into(), Ty::Number),
            ("beat".into(), Ty::Number),
        ],
    );
    shapes.map.insert(
        "IntersectionObserverEntry".into(),
        vec![
            ("isIntersecting".into(), Ty::Bool),
            ("intersectionRatio".into(), Ty::Number),
        ],
    );
    shapes.map.insert(
        "FocusEvent".into(),
        vec![
            ("target".into(), Ty::String),
            ("relatedTarget".into(), Ty::Option(Box::new(Ty::String))),
        ],
    );
    shapes.map.insert(
        "ToggleEvent".into(),
        vec![
            ("oldState".into(), Ty::String),
            ("newState".into(), Ty::String),
        ],
    );
    shapes.map.insert(
        "MarkdownSelection".into(),
        vec![
            ("formats".into(), Ty::String),
            ("mixed".into(), Ty::Bool),
            ("link".into(), Ty::String),
            ("unavailable".into(), Ty::String),
            // The selected atom's source when the selection is exactly one
            // atom, else "".
            ("atom".into(), Ty::String),
        ],
    );
    // What a `key` or `keyup` handler's action hears after the key when it
    // takes one more parameter, in the order `exact_runner::KeyboardEvent`
    // writes it: the DOM's `KeyboardEvent` fields by their names, then the
    // physical key (`code`: `KeyB`, `Digit1`,
    // `MetaLeft`, whatever the layout types there; "" where the host cannot
    // tell) and whether a keydown is the platform's auto-repeat (`repeat`),
    // then whether it came while an input method composed text
    // (`isComposing`, false where the host hands handlers no composing key).
    shapes.map.insert(
        "KeyboardEvent".into(),
        vec![
            ("key".into(), Ty::String),
            ("shiftKey".into(), Ty::Bool),
            ("ctrlKey".into(), Ty::Bool),
            ("altKey".into(), Ty::Bool),
            ("metaKey".into(), Ty::Bool),
            ("code".into(), Ty::String),
            ("repeat".into(), Ty::Bool),
            ("isComposing".into(), Ty::Bool),
        ],
    );
    // What `input` and `change` hand an action that takes one more
    // parameter, and a text field's `select` its payload: the target's own
    // fields as the event fires, by the
    // DOM's names, as `ScrollEvent` carries the scroller's — its `value`
    // (a checkbox's or a radio's `value`, `on` when it has none), whether it
    // is `checked`, and a text field's selection in UTF-16 units with its
    // direction (`forward`, `backward` or `none`); a control that has no
    // text selection reports 0, 0 and `none`. In the order
    // `exact_runner::Event::record` writes it.
    shapes.map.insert(
        "InputEvent".into(),
        vec![
            ("value".into(), Ty::String),
            ("checked".into(), Ty::Bool),
            ("selectionStart".into(), Ty::Number),
            ("selectionEnd".into(), Ty::Number),
            ("selectionDirection".into(), Ty::String),
            ("inputType".into(), Ty::String),
            ("data".into(), Ty::Option(Box::new(Ty::String))),
            ("cancelable".into(), Ty::Bool),
            ("isComposing".into(), Ty::Bool),
        ],
    );
    // DOM's `ClipboardEvent`, its data as plain text (`getData("text/plain")`)
    // and its `clipboardData.files` as the `doc:` handles the host minted for
    // them, image data with no file among them as a PNG:
    // what a paste carries; empty on copy and cut, as the DOM's is until a
    // listener sets it — the action writes the clipboard with `copyText`.
    shapes.map.insert(
        "ClipboardEvent".into(),
        vec![
            ("text".into(), Ty::String),
            ("files".into(), Ty::List(Box::new(Ty::String))),
        ],
    );
    // The part of the reader's selection inside one `text`, what
    // `selectionchange` hands its action, in the order
    // `exact_runner::Event::SelectionChange` writes it: the selected text
    // (DOM `Range.toString()`) and its UTF-16 start and end in the node's
    // own text, as written, followed by viewport rectangle snapshots.
    // Nothing selected there is "" with equal offsets and no rectangles.
    shapes.map.insert(
        "Selection".into(),
        vec![
            ("text".into(), Ty::String),
            ("start".into(), Ty::Number),
            ("end".into(), Ty::Number),
            (
                "rects".into(),
                Ty::List(Box::new(Ty::Record("Geometry".into()))),
            ),
        ],
    );
    // DOM's `PointerEvent`, the subset every host measures, in the order
    // `exact_runner::PointerEvent` writes it: the point from the node's
    // content box, the button that changed and the buttons' bits (3 and 8
    // back, 4 and 16 forward), the pressure, the device, its id, the point
    // from the viewport (`frame()`'s space), and the modifiers
    // held (a `MouseEvent`'s).
    shapes.map.insert(
        "PointerEvent".into(),
        vec![
            ("offsetX".into(), Ty::Number),
            ("offsetY".into(), Ty::Number),
            ("button".into(), Ty::Number),
            ("buttons".into(), Ty::Number),
            ("pressure".into(), Ty::Number),
            ("pointerType".into(), Ty::String),
            ("pointerId".into(), Ty::Number),
            ("clientX".into(), Ty::Number),
            ("clientY".into(), Ty::Number),
            ("shiftKey".into(), Ty::Bool),
            ("ctrlKey".into(), Ty::Bool),
            ("altKey".into(), Ty::Bool),
            ("metaKey".into(), Ty::Bool),
        ],
    );
    // What a `reorderdrop` action hears after the row's key and the key it
    // lands before, when it takes one more parameter: the
    // source list's `id` and the target's, SortableJS's `from` and `to`.
    // Within one list they are equal.
    shapes.map.insert(
        "ReorderEvent".into(),
        vec![("from".into(), Ty::String), ("to".into(), Ty::String)],
    );
    // DOM's `MouseEvent`, what a `press` action may take: the modifiers held
    // (shift-click range select, ⌘-click) and the point from the viewport,
    // 0, 0 from a keyboard or assistive press (a menu opens at the press), in
    // the order `exact_runner::MouseEvent` writes it.
    shapes.map.insert(
        "MouseEvent".into(),
        vec![
            ("shiftKey".into(), Ty::Bool),
            ("ctrlKey".into(), Ty::Bool),
            ("altKey".into(), Ty::Bool),
            ("metaKey".into(), Ty::Bool),
            ("clientX".into(), Ty::Number),
            ("clientY".into(), Ty::Number),
        ],
    );
    // DOM's `WheelEvent`, in the order
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
    // DOM's `DragEvent` at a `drop` of files, its
    // `dataTransfer.files` as the `doc:` handles the host minted for them
    // (as a picker's), in `exact_runner::DropEvent`'s order.
    // A `dragenter` or `dragleave` has none: the DOM protects a drag's files
    // until it drops.
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
    // end" is the web's `scrollHeight - scrollTop - clientHeight`.
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
        .into_iter()
        .chain([
            ("firstVisibleKey".into(), Ty::Option(Box::new(Ty::String))),
            ("firstVisibleOffset".into(), Ty::Number),
        ])
        .collect(),
    );
    // What a `resize` handler's action hears after the content box's width
    // and height when it takes one more parameter, in the order
    // `exact_runner::ResizeRect` writes it: ResizeObserverEntry's
    // `contentRect`, DOM's `DOMRectReadOnly` (x and y are the padding's left
    // and top, the content box's place in the padding box).
    shapes.map.insert(
        "DOMRectReadOnly".into(),
        [
            "x", "y", "width", "height", "top", "right", "bottom", "left",
        ]
        .map(|f| (f.into(), Ty::Number))
        .to_vec(),
    );
    // The Media Session's `MediaMetadata`, by its
    // constructor's name and fields: the one compiler shape an app builds
    // (`records::COMPILER_RECORDS`), on `audio` or `video`'s `metadata=`.
    // `artwork` is one image's source, not a list of `MediaImage`s.
    shapes.map.insert(
        "MediaMetadata".into(),
        ["title", "artist", "album", "artwork"]
            .map(|f| (f.into(), Ty::String))
            .to_vec(),
    );
    // `MediaSessionActionDetails`, what a media session
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

/// HTML's popover methods, with an explicit source element for opening.
pub(super) fn popover_args(
    name: &str,
    args: &[Expr],
    scope: &Scope,
    shapes: &Shapes,
    span: Span,
) -> Result<(), TypeError> {
    let valid = match args {
        [id] => infer(id, scope, shapes)? == Ty::String,
        [id, Expr::NamedArg(option, source, _)] if name != "hidePopover" && option == "source" => {
            infer(id, scope, shapes)? == Ty::String && infer(source, scope, shapes)? == Ty::String
        }
        _ => false,
    };
    if valid {
        return Ok(());
    }
    err("type-popover-command", "popover commands take an element id string; showPopover and togglePopover also take source= followed by an element id string", span)
}
