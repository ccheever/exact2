//! @ref LLP 1098 D2, D11 — the media session's six actions, `setActionHandler`'s
//! names, as a host sends them through host kind 19 (`name\npayload`), and
//! the `MediaSessionActionDetails` an action may take as its last parameter
//! (`contract_types::event_record`). The payload is `seekOffset seekTime
//! fastSeek`: two finite numbers of 0 or more, then `0` or `1`. The record's
//! `action` is the event's own name, never carried. No runner state:
//! ownership, publication and the readback are the hosts'.

use exact_plan::{EventKind, Value};

/// The six, after `drop` in `EventKind` (`plan/tables/format.json`): not in
/// the `loadedmetadata`…`canplay` run, so `media_payload` lists them.
pub(super) const ACTIONS: [EventKind; 6] = [
    EventKind::Seekbackward,
    EventKind::Seekforward,
    EventKind::Seekto,
    EventKind::Previoustrack,
    EventKind::Nexttrack,
    EventKind::Stop,
];

/// The payload's three fields, or `None` for anything else.
pub(super) fn details(payload: &str) -> Option<(f64, f64, bool)> {
    let mut tokens = payload.split(' ');
    let mut time = || {
        exact_num::parse_f64(tokens.next()?)
            .ok()
            .filter(|n| n.is_finite() && *n >= 0.0)
    };
    let (offset, time) = (time()?, time()?);
    let fast = match tokens.next()? {
        "0" => false,
        "1" => true,
        _ => return None,
    };
    tokens.next().is_none().then_some((offset, time, fast))
}

/// The `MediaSessionActionDetails` for one of the six, in the compiler's
/// field order: `action`, `seekOffset`, `seekTime`, `fastSeek`.
pub(super) fn record(kind: EventKind, payload: &str) -> Option<Value> {
    let (offset, time, fast) = details(payload).filter(|_| ACTIONS.contains(&kind))?;
    Some(Value::record(vec![
        Value::str(kind.name()),
        Value::Number(offset),
        Value::Number(time),
        Value::Bool(fast),
    ]))
}

#[cfg(test)]
mod tests {
    use crate::{DataError, DataSource, Event, Runner};
    use exact_kernel::Kernel;
    use exact_plan::Value;

    struct NoData;
    impl DataSource for NoData {
        fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
            panic!("unexpected query {source}")
        }
    }

    const SOURCE: &str = r#"component Player
  state seek = 0
  state heard = ""
  state fast = false
  state skips = 0
  action skipBy(sign: number, d: MediaSessionActionDetails)
    seek = seek + sign * d.seekOffset
    heard = d.action
  action seekAt(d: MediaSessionActionDetails)
    seek = d.seekTime
    fast = d.fastSeek
    heard = d.action
  action next
    skips = skips + 1
  action stopped(d: MediaSessionActionDetails)
    heard = d.action
  view
    audio "assets/a.mp3" testId="audio" metadata=MediaMetadata(title="T", artist="A", album="", artwork="") seekbackwardOffset=15 seekforwardOffset=30 seekbackward=skipBy(-1) seekforward=skipBy(1) seekto=seekAt nexttrack=next stop=stopped
"#;

    fn boot() -> (Runner<NoData>, exact_kernel::ViewId) {
        let plan = contract::compile(SOURCE).unwrap();
        let r = Runner::boot(
            plan,
            NoData,
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        let key = r.kernel().find_by_test_id("audio")[0];
        let id = r.kernel().node_by_key(key).unwrap().id;
        (r, id)
    }

    fn send(r: &mut Runner<NoData>, id: exact_kernel::ViewId, wire: &str) {
        let event = Event::media_payload(wire).unwrap_or_else(|| panic!("{wire:?} refused"));
        r.dispatch(id, event).unwrap();
    }

    #[test]
    fn each_payload_decodes_to_its_record() {
        for (wire, record) in [
            ("seekforward\n15 0 0", ("seekforward", 15.0, 0.0, false)),
            ("seekbackward\n10 0 0", ("seekbackward", 10.0, 0.0, false)),
            ("seekto\n0 600.5 1", ("seekto", 0.0, 600.5, true)),
            ("previoustrack\n0 0 0", ("previoustrack", 0.0, 0.0, false)),
            ("nexttrack\n0 0 0", ("nexttrack", 0.0, 0.0, false)),
            ("stop\n0 0 0", ("stop", 0.0, 0.0, false)),
        ] {
            let event = Event::media_payload(wire).unwrap();
            let (action, offset, time, fast) = record;
            assert_eq!(
                event.record(),
                Some(Value::record(vec![
                    Value::str(action),
                    Value::Number(offset),
                    Value::Number(time),
                    Value::Bool(fast),
                ])),
                "{wire:?}"
            );
        }
        // The other media events offer no record.
        assert_eq!(Event::media_payload("play\n").unwrap().record(), None);
    }

    #[test]
    fn a_malformed_payload_or_another_kind_is_refused() {
        for wire in [
            "seekforward\n",
            "seekforward\n15",
            "seekforward\n15 0",
            "seekforward\n15 0 2",
            "seekforward\n15 0 true",
            "seekforward\n15 0 0 0",
            "seekforward\n-15 0 0",
            "seekto\n0 NaN 0",
            "seekto\n0 inf 0",
            "seekto\n0  1 0",
            // Kinds after `canplay` that are not the session's stay refused.
            "drop\n0 0 0",
            "input\n0 0 0",
            "pan\n0 0 0",
            "press\n",
        ] {
            assert!(Event::media_payload(wire).is_none(), "{wire:?}");
        }
    }

    #[test]
    fn the_actions_reach_the_app_with_or_without_the_record() {
        let (mut r, id) = boot();
        send(&mut r, id, "seekforward\n30 0 0");
        send(&mut r, id, "seekforward\n30 0 0");
        send(&mut r, id, "seekbackward\n15 0 0");
        assert_eq!(r.slot("seek"), Some(&Value::Number(45.0)));
        assert_eq!(r.slot("heard"), Some(&Value::str("seekbackward")));
        send(&mut r, id, "seekto\n0 600 1");
        assert_eq!(r.slot("seek"), Some(&Value::Number(600.0)));
        assert_eq!(r.slot("fast"), Some(&Value::Bool(true)));
        assert_eq!(r.slot("heard"), Some(&Value::str("seekto")));
        // An action that leaves the record runs.
        send(&mut r, id, "nexttrack\n0 0 0");
        assert_eq!(r.slot("skips"), Some(&Value::Number(1.0)));
        send(&mut r, id, "stop\n0 0 0");
        assert_eq!(r.slot("heard"), Some(&Value::str("stop")));
        // One the element does not handle is no handler's.
        let refused = r.dispatch(id, Event::media_payload("previoustrack\n0 0 0").unwrap());
        assert!(refused.is_err(), "{refused:?}");
    }
}
