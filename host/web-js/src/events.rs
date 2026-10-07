//! Which rt.js export hears each event kind: the motion and input pieces'
//! (`piece`, called with the element and the handler) and the event
//! families `on` takes (`binder`), each imported only where a plan binds it.

use exact_plan::EventKind;

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
        EventKind::Key | EventKind::Keyup => Some("onKey"),
        EventKind::Beforeunload => Some("onUnload"),
        EventKind::Submit => Some("onSubmit"),
        EventKind::Message => Some("onMessage"),
        EventKind::Scroll => Some("onScroll"),
        EventKind::Refresh => Some("onRefresh"),
        EventKind::Dblclick => Some("onDblclick"),
        EventKind::Pointerdown | EventKind::Pointerup | EventKind::Pointermove => Some("onPointer"),
        EventKind::Contextmenu => Some("onContextmenu"),
        EventKind::Wheel => Some("onWheel"),
        EventKind::Drop => Some("onFileDrop"),
        EventKind::Blur | EventKind::Focus => Some("onFocus"),
        EventKind::Copy | EventKind::Cut | EventKind::Paste => Some("onClipboard"),
        EventKind::Selectionchange => Some("onSelectionChange"),
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
            Keyup,
            Beforeunload,
            Submit,
            Message,
            Scroll,
            Refresh,
            Dblclick,
            Pointerdown,
            Pointerup,
            Pointermove,
            Contextmenu,
            Wheel,
            Drop,
            Blur,
            Focus,
            Copy,
            Cut,
            Paste,
            Selectionchange,
        ] {
            assert!(binder(k).is_some(), "{k:?}");
        }
        for k in [Load, Cancel, Play, Pause, Timeupdate] {
            assert!(binder(k).is_none(), "{k:?}");
        }
    }
}
