//! The host ABI's payload-carrying event kinds that every host decodes the
//! same way (web `exact_dispatch`, Apple `exact_dispatch`): parsed and
//! validated before any clock moves, a refusal named for the batch's `error`.

use super::Event;

impl Event {
    /// Decode ABI kind 10 (contextmenu, its point when it has one), 13
    /// (scroll), 19 (media), 20 (pan), 21 (select), 28 (panrelease, LLP
    /// 1057 §10.6), 29 to 31 (the pointer's down, up and move), 32 to 34
    /// (the clipboard's copy, cut and paste), 35 (`selectionchange`), 36
    /// (beforeunload), 37 (wheel), 38 (drop) or 39 (resize, its
    /// `contentRect` as `x,y,width,height`) from its UTF-8 payload.
    pub fn of_host_kind(kind: u32, payload: &str) -> Result<Event, &'static str> {
        match kind {
            13 => Event::scroll_payload(payload).ok_or("invalid scroll coordinates"),
            19 => Event::media_payload(payload).ok_or("invalid media event"),
            // @ref LLP 1043.000 §3 D8 — 18 is reorder, 19 is media.
            20 => Event::pan_payload(payload).ok_or("invalid pan deltas"),
            21 => Event::selection_payload(payload).ok_or("invalid Markdown selection"),
            28 => Event::pan_release_payload(payload).ok_or("invalid pan release velocity"),
            // LLP 1056 §3 stage 3: the pointer's record.
            29..=31 => Event::pointer_payload(kind, payload).ok_or("invalid pointer event"),
            // The clipboard's copy, cut and paste, with its plain text.
            32..=34 => Event::clipboard_payload(kind, payload).ok_or("invalid clipboard event"),
            // The reader's selection inside a `text`: its offsets, then its text.
            35 => Event::selection_change_payload(payload).ok_or("invalid text selection"),
            10 => Event::contextmenu_payload(payload).ok_or("invalid contextmenu event"),
            // Studio diary R17, R3, R19: DOM's beforeunload, wheel and drop.
            36 => Ok(Event::Beforeunload),
            37 => super::WheelEvent::parse(payload)
                .map(Event::Wheel)
                .ok_or("invalid wheel event"),
            38 => super::DropEvent::parse(payload)
                .map(Event::Drop)
                .ok_or("invalid drop event"),
            39 => super::ResizeRect::parse(payload)
                .map(Event::Resize)
                .ok_or("invalid resize rect"),
            _ => Err("unknown event kind"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Event;

    #[test]
    fn kinds_decode_and_refuse_by_name() {
        assert!(matches!(
            Event::of_host_kind(20, "3,-4"),
            Ok(Event::Pan(3.0, -4.0))
        ));
        assert!(matches!(
            Event::of_host_kind(28, "900,-12.5"),
            Ok(Event::PanRelease(900.0, -12.5))
        ));
        assert_eq!(
            Event::of_host_kind(28, "NaN,0").err(),
            Some("invalid pan release velocity")
        );
        assert_eq!(
            Event::of_host_kind(13, "x").err(),
            Some("invalid scroll coordinates")
        );
        assert!(matches!(
            Event::of_host_kind(34, "a\tb\n1\t2"),
            Ok(Event::Clipboard(exact_plan::EventKind::Paste, text)) if text == "a\tb\n1\t2"
        ));
        assert!(matches!(
            Event::of_host_kind(35, "3,9,a, b\nc"),
            Ok(Event::SelectionChange { text, start: 3.0, end: 9.0 }) if text == "a, b\nc"
        ));
        for refused in ["9,3,x", "1,2", "-1,2,x", "a,2,x"] {
            assert_eq!(
                Event::of_host_kind(35, refused).err(),
                Some("invalid text selection"),
                "{refused}"
            );
        }
        assert_eq!(
            Event::of_host_kind(99, "").err(),
            Some("unknown event kind")
        );
    }
}
