//! The host ABI's payload-carrying event kinds that every host decodes the
//! same way (web `exact_dispatch`, Apple `exact_dispatch`): parsed and
//! validated before any clock moves, a refusal named for the batch's `error`.

use super::Event;
use exact_plan::EventKind;

impl Event {
    /// Decode ABI kind 6 (`key`), 43 (`keyup`) or 44 (`keycapture`),
    /// 10 (contextmenu, its point when it has one), 13
    /// (scroll), 19 (media), 20 (pan), 21 (select), 28 (panrelease, LLP
    /// 1057 §10.6), 29 to 31 (the pointer's down, up and move), 32 to 34
    /// (the clipboard's copy, cut and paste), 35 (`selectionchange`), 36
    /// (beforeunload), 37 (wheel), 38 (drop), 55 and 56 (a file drag's
    /// dragenter and dragleave), 57 (a paste with files), 58 (atompress),
    /// 39 (resize, its
    /// `contentRect` as `x,y,width,height`), 40 to 42 and 48 (a field's
    /// input, change, select and replacement input), 45 (intersect,
    /// `0|1,ratio`), 46 and 47 (focusin, focusout), 50 (toggle, oldState,newState)
    /// 51 (scrollend, none), 52 (field selectionchange), or 53/54
    /// (beforeinput/input metadata) from its UTF-8 payload.
    pub fn of_host_kind(kind: u32, payload: &str) -> Result<Event, &'static str> {
        match kind {
            // A key's down (6), up (43) or capture (44, #140): the chord, then its code
            // and repeat (`KeyboardEvent::parse`).
            6 => super::KeyboardEvent::parse(payload)
                .map(Event::Key)
                .ok_or("invalid key event"),
            44 => super::KeyboardEvent::parse(payload)
                .map(Event::Keycapture)
                .ok_or("invalid key event"),
            // DOM's keyup is never a repeat.
            43 => super::KeyboardEvent::parse(payload)
                .map(|k| Event::Keyup(super::KeyboardEvent { repeat: false, ..k }))
                .ok_or("invalid key event"),
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
            // DOM's `scrollend`: no payload (LLP 1005, 2026-10-13).
            51 => Ok(Event::Scrollend),
            37 => super::WheelEvent::parse(payload)
                .map(Event::Wheel)
                .ok_or("invalid wheel event"),
            38 => super::DropEvent::parse(payload)
                .map(Event::Drop)
                .ok_or("invalid drop event"),
            // A file drag's enter (55) and leave (56): the point line alone.
            55 | 56 => super::DropEvent::parse_point(payload)
                .map(if kind == 55 {
                    Event::Dragenter
                } else {
                    Event::Dragleave
                })
                .ok_or("invalid drag event"),
            57 => Event::paste_files_payload(payload).ok_or("invalid clipboard event"),
            // A press on an inline atom (LLP 1045.000 D7).
            58 => super::AtomEvent::parse(payload)
                .map(Event::Atompress)
                .ok_or("invalid atom press"),
            45 => {
                let (hit, ratio) = payload
                    .split_once(',')
                    .ok_or("invalid intersection entry")?;
                let is_intersecting = match hit {
                    "0" => false,
                    "1" => true,
                    _ => return Err("invalid intersection entry"),
                };
                let intersection_ratio = exact_num::parse_f64(ratio)
                    .ok()
                    .filter(|r| (0.0..=1.0).contains(r))
                    .filter(|r| is_intersecting || *r == 0.0)
                    .ok_or("invalid intersection entry")?;
                Ok(Event::Intersect {
                    is_intersecting,
                    intersection_ratio,
                })
            }
            39 => super::ResizeRect::parse(payload)
                .map(Event::Resize)
                .ok_or("invalid resize rect"),
            // A text field's `input`, `change` and `select` with its
            // selection (x2apps codeedit #2).
            40..=42 | 48 | 52 => {
                Event::field_payload(kind, payload).ok_or("invalid field selection")
            }
            // DOM's bubbling focusin (46) and focusout (47, #283; 45 is `intersect`):
            // the target's id and the relatedTarget's, if any.
            46 | 47 => Event::focus_payload(kind == 46, payload).ok_or("invalid focus event"),
            53 | 54 => Event::edit_payload(kind, payload).ok_or("invalid input event"),
            50 => {
                let (old, new) = payload.split_once(',').ok_or("invalid toggle event")?;
                if ![old, new].iter().all(|s| matches!(*s, "open" | "closed")) {
                    return Err("invalid toggle event");
                }
                Ok(Event::Toggle {
                    old_open: old == "open",
                    new_open: new == "open",
                })
            }
            _ => Err("unknown event kind"),
        }
    }

    /// Decode host kind 21: formats, mixed (0/1), unavailable, the selected
    /// atom's source (LLP 1045.000 D3; never a line break), then the link
    /// remainder, separated by newlines. Token lists never contain newlines;
    /// a target may, so the final remainder is kept verbatim.
    pub fn selection_payload(payload: &str) -> Option<Self> {
        let mut parts = payload.splitn(5, '\n');
        let formats = parts.next()?;
        let mixed = match parts.next()? {
            "0" => false,
            "1" => true,
            _ => return None,
        };
        let unavailable = parts.next()?;
        let atom = parts.next()?;
        let link = parts.next()?;
        let tokens = |s: &str| s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b' ');
        if !tokens(formats) || !tokens(unavailable) {
            return None;
        }
        Some(Self::Select {
            formats: formats.into(),
            mixed,
            link: link.into(),
            unavailable: unavailable.into(),
            atom: atom.into(),
        })
    }

    /// Decode a media event carried as `name\npayload` through host kind 19.
    pub fn media_payload(payload: &str) -> Option<Self> {
        let (name, value) = payload.split_once('\n')?;
        let kind = EventKind::from_name(name)?;
        // The media session's six sit after `drop` (LLP 1098 D11): listed,
        // so the `loadedmetadata`…`canplay` run never admits them. `resize`
        // sits between `drop` and the six; it is not a media event.
        let session = super::media_session::ACTIONS.contains(&kind);
        // `fullscreenchange` (LLP 1042) is listed after them too: a boolean.
        let fullscreen = kind == EventKind::Fullscreenchange;
        if !session
            && !fullscreen
            && !(EventKind::Loadedmetadata as u8..=EventKind::Canplay as u8).contains(&(kind as u8))
            || session && super::media_session::details(value).is_none()
            || fullscreen && !matches!(value, "true" | "false")
        {
            return None;
        }
        if matches!(kind, EventKind::Timeupdate | EventKind::Durationchange)
            && !exact_num::parse_f64(value).ok()?.is_finite()
        {
            return None;
        }
        Some(Self::Media(kind, value.into()))
    }

    /// ABI kind 35: start,end,count, then count x,y,width,height rectangles,
    /// then selected text verbatim. Offsets are whole, `0 <= start <= end`.
    pub fn selection_change_payload(payload: &str) -> Option<Self> {
        let (start, rest) = payload.split_once(',')?;
        let (end, rest) = rest.split_once(',')?;
        let (count, mut text) = rest.split_once(',')?;
        let start: u32 = start.parse().ok()?;
        let end: u32 = end.parse().ok()?;
        let count: usize = count.parse().ok()?;
        if start > end || count > text.len() / 8 {
            return None;
        }
        let mut rects = Vec::new();
        for _ in 0..count {
            let mut values = [0.0_f64; 4];
            for value in &mut values {
                let (number, rest) = text.split_once(',')?;
                *value = number.parse().ok()?;
                if !value.is_finite() {
                    return None;
                }
                text = rest;
            }
            let [x, y, width, height] = values;
            if width < 0.0 || height < 0.0 {
                return None;
            }
            rects.push(crate::geometry::GeometryAnswer {
                x,
                y,
                width,
                height,
                provisional: false,
                unavailable: false,
            });
        }
        Some(Self::SelectionChange {
            text: text.into(),
            start: f64::from(start),
            end: f64::from(end),
            rects,
        })
    }

    /// Decode ABI kind 32 (`copy`), 33 (`cut`) or 34 (`paste`): the
    /// payload is the clipboard's plain text, verbatim.
    pub fn clipboard_payload(kind: u32, payload: &str) -> Option<Self> {
        let kind = match kind {
            32 => EventKind::Copy,
            33 => EventKind::Cut,
            34 => EventKind::Paste,
            _ => return None,
        };
        Some(Self::Clipboard(kind, payload.into(), Vec::new()))
    }

    /// Decode ABI kind 57, a `paste` whose clipboard holds files: the count
    /// of files, a `doc:` handle a line, then the plain text verbatim (it may
    /// hold newlines). `None` when a line is not a handle.
    pub fn paste_files_payload(payload: &str) -> Option<Self> {
        let (count, mut rest) = payload.split_once('\n')?;
        let count: usize = count.parse().ok().filter(|&n| n > 0)?;
        let mut files = Vec::with_capacity(count.min(64));
        for i in 0..count {
            let (file, after) = match rest.split_once('\n') {
                Some(split) => split,
                None if i + 1 == count => (rest, ""),
                None => return None,
            };
            if !file.starts_with("doc:/") {
                return None;
            }
            files.push(file.to_owned());
            rest = after;
        }
        Some(Self::Clipboard(EventKind::Paste, rest.into(), files))
    }
}

#[cfg(test)]
mod tests {
    use super::Event;

    #[test]
    fn selection_rectangles_preserve_geometry_and_verbatim_text() {
        let event = Event::of_host_kind(35, "3,9,2,-4,12,30,18,0,30,8,18,a, b\nc").unwrap();
        let Event::SelectionChange {
            text,
            start,
            end,
            rects,
        } = event
        else {
            panic!("selection event")
        };
        assert_eq!((text.as_str(), start, end), ("a, b\nc", 3.0, 9.0));
        assert_eq!(rects.len(), 2);
        assert_eq!(
            rects[0].value(),
            exact_plan::Value::record(vec![
                exact_plan::Value::Number(-4.0),
                exact_plan::Value::Number(12.0),
                exact_plan::Value::Number(30.0),
                exact_plan::Value::Number(18.0),
                exact_plan::Value::Bool(false),
                exact_plan::Value::Bool(false),
            ])
        );
        assert_eq!(rects[1].y + rects[1].height, 48.0);
    }

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
            Ok(Event::Clipboard(exact_plan::EventKind::Paste, text, files)) if text == "a\tb\n1\t2" && files.is_empty()
        ));
        // A paste with files: their count, a handle a line, then the text.
        assert!(matches!(
            Event::of_host_kind(57, "2\ndoc:/1/a.png\ndoc:/2/b c.txt\nline\nnext"),
            Ok(Event::Clipboard(exact_plan::EventKind::Paste, text, files))
                if text == "line\nnext" && files == ["doc:/1/a.png", "doc:/2/b c.txt"]
        ));
        assert!(matches!(
            Event::of_host_kind(57, "1\ndoc:/3/image.png"),
            Ok(Event::Clipboard(_, text, files)) if text.is_empty() && files == ["doc:/3/image.png"]
        ));
        for bad in ["0\n", "1\n/tmp/a.png\n", "2\ndoc:/1/a", "x\ndoc:/1/a\n", ""] {
            assert_eq!(
                Event::of_host_kind(57, bad).err(),
                Some("invalid clipboard event"),
                "{bad:?}"
            );
        }
        assert!(matches!(
            Event::of_host_kind(55, "3,4,Shift"),
            Ok(Event::Dragenter(d)) if d.offset_y == 4.0 && d.held.shift && d.files.is_empty()
        ));
        assert!(matches!(
            Event::of_host_kind(56, "0,0"),
            Ok(Event::Dragleave(_))
        ));
        assert!(matches!(
            Event::of_host_kind(56, "0,0\ndoc:/1/a"),
            Err("invalid drag event")
        ));
        assert!(matches!(
            Event::of_host_kind(35, "3,9,0,a, b\nc"),
            Ok(Event::SelectionChange { text, start: 3.0, end: 9.0, rects }) if text == "a, b\nc" && rects.is_empty()
        ));
        for refused in [
            "9,3,0,x",
            "1,2",
            "-1,2,0,x",
            "a,2,0,x",
            "0,1,1,0,0,-1,2,x",
            "0,1,1,NaN,0,1,2,x",
            "0,1,2,0,0,1,2,x",
        ] {
            assert_eq!(
                Event::of_host_kind(35, refused).err(),
                Some("invalid text selection"),
                "{refused}"
            );
        }
        assert_eq!(
            Event::of_host_kind(47, "1:a1b"),
            Ok(Event::Focusout {
                target: "a".into(),
                related_target: Some("b".into())
            })
        );
        assert_eq!(
            Event::of_host_kind(46, "typed").err(),
            Some("invalid focus event")
        );
        assert_eq!(
            Event::of_host_kind(99, "").err(),
            Some("unknown event kind")
        );
    }
}
