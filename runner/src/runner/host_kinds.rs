//! The host ABI's payload-carrying event kinds that every host decodes the
//! same way (web `exact_dispatch`, Apple `exact_dispatch`): parsed and
//! validated before any clock moves, a refusal named for the batch's `error`.

use super::Event;

impl Event {
    /// Decode ABI kind 13 (scroll), 19 (media), 20 (pan), 21 (select) or
    /// 28 (panrelease, LLP 1057 §10.6) from its UTF-8 payload.
    pub fn of_host_kind(kind: u32, payload: &str) -> Result<Event, &'static str> {
        match kind {
            13 => Event::scroll_payload(payload).ok_or("invalid scroll coordinates"),
            19 => Event::media_payload(payload).ok_or("invalid media event"),
            // @ref LLP 1043.000 §3 D8 — 18 is reorder, 19 is media.
            20 => Event::pan_payload(payload).ok_or("invalid pan deltas"),
            21 => Event::selection_payload(payload).ok_or("invalid Markdown selection"),
            28 => Event::pan_release_payload(payload).ok_or("invalid pan release velocity"),
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
        assert_eq!(
            Event::of_host_kind(99, "").err(),
            Some("unknown event kind")
        );
    }
}
