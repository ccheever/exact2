//! Which rt.js export hears each event kind: the motion and input pieces'
//! (`piece`, called with the element and the handler) and the event
//! families `on` takes (`binder`), each imported only where a plan binds it.

use exact_plan::{EventKind, HandlersRow, Plan};

/// A package view's declared event (LLP 1133.021 §9 item 2): a `message`
/// handler whose one bound argument is the literal `exact:event:<name>`
/// (contract-lower's `native_typed.rs`): its name and its action's
/// parameter types, as `rt.js` `viewEvent` decodes the arguments.
pub(crate) fn declared(plan: &Plan, h: &HandlersRow) -> Option<(String, Vec<String>)> {
    if h.event != EventKind::Message || h.args.len != 1 {
        return None;
    }
    let first = h.args.iter().next()?;
    let literal = crate::style::literal(plan, plan.code(plan.arg(first).expr))?;
    let name = literal.as_str()?.strip_prefix("exact:event:")?.to_owned();
    let params = (plan.action(h.action).params.iter())
        .map(|p| crate::emit::surface_type_json(plan, plan.param(p).ty))
        .collect();
    Some((name, params))
}

/// The motion or input piece's export for an event, if it is one.
pub(crate) fn piece(event: EventKind) -> Option<&'static str> {
    match event {
        EventKind::Swiperight => Some("onSwipe"),
        EventKind::Pan => Some("onPan"),
        EventKind::Panrelease => Some("onPanRelease"),
        EventKind::Select => Some("onSelect"),
        EventKind::Heightrelease => Some("onHeight"),
        EventKind::Transformgeometry => Some("onTGeom"),
        EventKind::Transformrelease => Some("onTRelease"),
        EventKind::Reorderdrop => Some("onDrop"),
        EventKind::Resize => Some("onResize"),
        EventKind::Intersect => Some("onIntersect"),
        _ => None,
    }
}

/// The binder `on` takes for an event's family; any other event is a plain
/// listener.
pub(crate) fn binder(event: EventKind) -> Option<&'static str> {
    match event {
        EventKind::Press => Some("onPress"),
        EventKind::Change | EventKind::Input => Some("onValue"),
        EventKind::Hover => Some("onHover"),
        EventKind::Key | EventKind::Keycapture | EventKind::Keyup => Some("onKey"),
        EventKind::Beforeinput => Some("onBeforeInput"),
        EventKind::Beforeunload => Some("onUnload"),
        EventKind::Midimessage => Some("onMIDI"),
        EventKind::Submit => Some("onSubmit"),
        EventKind::Message => Some("onMessage"),
        EventKind::Scroll => Some("onScroll"),
        EventKind::Scrollend => Some("onScrollend"),
        EventKind::Refresh => Some("onRefresh"),
        EventKind::Dblclick => Some("onDblclick"),
        EventKind::Pointerdown | EventKind::Pointerup | EventKind::Pointermove => Some("onPointer"),
        EventKind::Contextmenu => Some("onContextmenu"),
        EventKind::Wheel => Some("onWheel"),
        EventKind::Drop => Some("onFileDrop"),
        EventKind::Dragenter | EventKind::Dragleave => Some("onFileDrag"),
        EventKind::Atompress => Some("onAtompress"),
        EventKind::Blur | EventKind::Focus => Some("onFocus"),
        EventKind::Focusin | EventKind::Focusout => Some("onFocusWithin"),
        EventKind::Copy | EventKind::Cut | EventKind::Paste => Some("onClipboard"),
        EventKind::Selectionchange => Some("onSelectionChange"),
        EventKind::Toggle => Some("onToggle"),
        // @ref LLP 1020.002 D2 — an iframe's guest (the root's `navigate`
        // is `navigateTo`, emit.rs), with its `FrameNavigation`.
        EventKind::Navigate
        | EventKind::Currententrychange
        | EventKind::Navigatesuccess
        | EventKind::Navigateerror => Some("onFrameNavigation"),
        // @ref LLP 1020.002 D7 — a capture's `FrameCapture`, always refused on the web.
        EventKind::Capture => Some("onFrameCapture"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every event family `on`'s old switch handled has its binder; the
    /// kinds that took its plain default still have none.
    #[test]
    fn every_special_event_has_its_binder() {
        use EventKind::*;
        for k in [
            Press,
            Change,
            Input,
            Hover,
            Key,
            Keycapture,
            Keyup,
            Beforeunload,
            Submit,
            Message,
            Scroll,
            Scrollend,
            Refresh,
            Dblclick,
            Pointerdown,
            Pointerup,
            Pointermove,
            Contextmenu,
            Wheel,
            Drop,
            Dragenter,
            Dragleave,
            Atompress,
            Blur,
            Focus,
            Focusin,
            Focusout,
            Copy,
            Cut,
            Paste,
            Selectionchange,
            Currententrychange,
            Navigatesuccess,
            Navigateerror,
            Capture,
        ] {
            assert!(binder(k).is_some(), "{k:?}");
        }
        for k in [Load, Cancel, Play, Pause, Timeupdate] {
            assert!(binder(k).is_none(), "{k:?}");
        }
    }
}
