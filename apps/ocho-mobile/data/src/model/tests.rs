use super::*;
use serde_json::json;

fn paired() -> Model {
    let mut m = Model::default();
    m.load(
        Some(r#"{"relay":"https://fleet-relay.fly.dev","machine":"mac","token":"t","name":"Mac"}"#),
        None,
        None,
    );
    m
}

fn answer(version: u64, windows: bool) -> serde_json::Value {
    json!({
        "instance": "i", "version": version, "machine": "mac",
        "fleet": {"machines": [{"id": "mac", "name": "Mac", "last": {"sessions": [
            {"id": "s1", "state": "running", "tmux_pane": "%1", "capabilities": ["attach"]}
        ]}}]},
        "desktop": {"windows": if windows { json!([{"tabs": [
            {"key": "mac:s1:false", "title": "One · Mac", "machine": "mac", "session": "s1"}
        ], "active": 1}]) } else { json!([]) }},
        "peers": [{"machine_id": "redwood", "name": "redwood", "enabled": true}]
    })
}

#[test]
fn polls_follow_the_cursor_and_keep_the_layout() {
    let mut m = paired();
    let (url, bearer) = m.poll_request().unwrap();
    assert!(url.ends_with("/m/mac/api/fleet?view=phone&wait=25s&since=0"));
    let mut tagged = answer(3, true);
    tagged["tag"] = json!("ab12");
    m.poll_done(Ok(tagged));
    m.tick(10_000.0);
    let (url, _) = m.poll_request().unwrap();
    assert!(url.ends_with("&since=3&instance=i&tag=ab12"), "{url}");
    m.poll_done(Ok(answer(4, true)));
    assert_eq!(bearer, "Bearer t");
    m.poll_done(Ok(answer(4, true)));
    assert!(m.fresh);
    assert_eq!(m.windows.len(), 1);
    assert!(m
        .writes
        .iter()
        .any(|(k, v)| *k == KEY_DESKTOP && v.is_some()));
    m.tick(10.0);
    let (url, _) = m.poll_request().unwrap();
    assert!(url.ends_with("since=4&instance=i"));
}

#[test]
fn the_relay_saying_the_mac_is_gone_switches_to_a_peer_at_once() {
    let mut m = paired();
    m.tick(1_000.0);
    m.poll_request();
    m.poll_done(Ok(answer(1, true)));
    // The lid closes: the relay answers the next poll "not connected", and
    // the quick retry half a second later says it again.
    for _ in 0..2 {
        m.tick(m.poll.next_at);
        m.poll_request();
        m.poll_done(Err((
            502,
            r#"{"error":"machine is not connected to the relay"}"#.into(),
        )));
    }
    assert!(m.now - 1_000.0 <= 3_000.0, "{}", m.now);
    let probe = m.probe_request().expect("a peer is tried at once");
    assert!(probe.ends_with("/m/redwood/api/health"));
    m.probe_done(true);
    assert_eq!(m.via, "redwood");
    // The peer runs no desktop: the Mac's layout stays.
    m.tick(m.now);
    m.poll_request();
    m.poll_done(Ok(answer(9, false)));
    assert_eq!(m.windows.len(), 1);
    // A relaunch is a few seconds off the relay: home is tried again soon,
    // and the phone goes back once it answers twice in a row.
    m.tick(m.now + FIRST_HOME_CHECK_MS + 1.0);
    assert!(m.probe_request().unwrap().ends_with("/m/mac/api/health"));
    m.probe_done(true);
    assert_eq!(m.via, "redwood", "one answer is not yet steady");
    m.tick(m.now + HOME_STEADY_MS + 1.0);
    assert!(m.probe_request().unwrap().ends_with("/m/mac/api/health"));
    m.probe_done(true);
    assert_eq!(m.via, "mac");
}

#[test]
fn a_mac_that_keeps_dropping_must_hold_longer_before_the_phone_goes_back() {
    let gone = || {
        Err((
            502,
            r#"{"error":"machine is not connected to the relay"}"#.to_string(),
        ))
    };
    let mut m = paired();
    m.tick(1_000.0);
    m.poll_request();
    m.poll_done(Ok(answer(1, true)));
    let mut looks_to_return = Vec::new();
    for _ in 0..3 {
        // Home drops: the phone moves to the peer.
        for _ in 0..2 {
            m.tick(m.poll.next_at.max(m.now));
            m.poll_request();
            m.poll_done(gone());
        }
        m.probe_request().expect("a peer is tried");
        m.probe_done(true);
        assert_eq!(m.via, "redwood");
        m.tick(m.now);
        m.poll_request();
        m.poll_done(Ok(answer(2, false)));
        // Count the looks home it takes to go back.
        let mut looks = 0;
        while m.via != "mac" {
            m.tick(m.now + HOME_CHECK_MS + 1.0);
            if m.probe_request().is_some() {
                looks += 1;
                m.probe_done(true);
            }
        }
        looks_to_return.push(looks);
        // Back home, and it drops again within a minute.
        m.tick(m.now + 1.0);
        m.poll_request();
        m.poll_done(Ok(answer(3, true)));
    }
    assert_eq!(
        looks_to_return,
        vec![2, 4, 6],
        "each flap asks home to hold longer"
    );
}

#[test]
fn a_dropped_mid_request_counts_as_gone() {
    let mut m = paired();
    m.tick(1_000.0);
    m.poll_request();
    m.poll_done(Ok(answer(1, true)));
    for _ in 0..2 {
        m.tick(m.poll.next_at);
        m.poll_request();
        m.poll_done(Err((502, r#"{"error":"machine disconnected"}"#.into())));
    }
    assert!(m.probe_request().is_some());
}

#[test]
fn timeouts_still_wait_before_looking_elsewhere() {
    let mut m = paired();
    m.tick(1_000.0);
    m.poll_request();
    m.poll_done(Ok(answer(1, true)));
    // Ambiguous: it may be this phone's network, not the Mac.
    for _ in 0..3 {
        m.tick(m.poll.next_at);
        m.poll_request();
        m.poll_done(Err((0, "The request timed out.".into())));
    }
    assert!(
        m.probe_request().is_none(),
        "three quick timeouts stay home"
    );
    while m.probe_queue.is_empty() {
        m.tick(m.poll.next_at);
        m.poll_request();
        m.poll_done(Err((0, "The request timed out.".into())));
    }
    assert!(m.now - 1_000.0 >= FAILOVER_AFTER_MS);
    assert!(m
        .probe_request()
        .unwrap()
        .ends_with("/m/redwood/api/health"));
}

fn unread_answer(version: u64, state: &str, unread_at: u64, peer: bool) -> serde_json::Value {
    json!({
        "instance": if peer { "p" } else { "i" }, "version": version, "machine": if peer { "redwood" } else { "mac" },
        "fleet": {"machines": [{"id": "mac", "name": "Mac", "last": {"sessions": [
            {"id": "s1", "state": state, "tmux_pane": "%1", "capabilities": ["attach"]}
        ]}}]},
        "desktop": {"windows": if peer { json!([]) } else { json!([{"tabs": [
            {"key": "mac:s1:false", "title": "One · Mac", "machine": "mac", "session": "s1", "unread_at": unread_at}
        ], "active": 0}]) }},
        "peers": [{"machine_id": "redwood", "name": "redwood", "enabled": true}]
    })
}

#[test]
fn opening_a_tab_the_desktop_marks_unread_reads_it_there_too() {
    let mut m = paired();
    m.poll_request();
    m.poll_done(Ok(unread_answer(1, "idle", 1_000, false)));
    assert!(m.unread("mac:s1:false"), "the desktop's dot");
    m.open("mac", "s1");
    assert!(!m.unread("mac:s1:false"), "read here at once");
    let (url, _, body) = m.reads_request().unwrap();
    assert!(url.ends_with("/m/mac/api/desktop/read"));
    assert_eq!(body, r#"{"keys":["mac:s1:false"]}"#);
    m.reads_done(Ok(()));
    // The Mac's next answer has applied the mark.
    m.tick(m.now);
    m.poll_request();
    m.poll_done(Ok(unread_answer(2, "idle", 0, false)));
    assert!(!m.unread("mac:s1:false"));
    m.close();
    // A later turn is unread again.
    m.poll_request();
    m.poll_done(Ok(unread_answer(3, "idle", 9_000, false)));
    assert!(m.unread("mac:s1:false"));
}

#[test]
fn a_failed_read_is_sent_again() {
    let mut m = paired();
    m.poll_request();
    m.poll_done(Ok(unread_answer(1, "idle", 1_000, false)));
    m.open("mac", "s1");
    m.reads_request().unwrap();
    m.reads_done(Err(502));
    assert!(!m.unread("mac:s1:false"), "still read here");
    m.tick(m.now + 10_000.0);
    assert!(m.reads_request().is_some());
}

#[test]
fn while_the_mac_is_away_this_phone_keeps_the_unread_record() {
    let mut m = paired();
    m.poll_request();
    m.poll_done(Ok(unread_answer(1, "running", 0, false)));
    m.via = "redwood".into();
    m.poll_request();
    m.poll_done(Ok(unread_answer(1, "running", 0, true)));
    m.poll_request();
    m.poll_done(Ok(unread_answer(2, "idle", 0, true)));
    assert!(m.unread("mac:s1:false"), "the turn finished while away");
    m.open("mac", "s1");
    assert!(!m.unread("mac:s1:false"));
    assert!(
        m.reads_request().is_none(),
        "the Mac is away: the read waits"
    );
    m.via = "mac".into();
    m.tick(m.now);
    assert!(m.reads_request().is_some(), "and goes when it is back");
}

#[test]
fn the_open_session_is_felt_when_its_turn_ends_or_stops_for_you() {
    let mut m = paired();
    m.poll_request();
    m.poll_done(Ok(unread_answer(1, "running", 0, false)));
    m.open("mac", "s1");
    m.haptic = None;
    m.poll_request();
    m.poll_done(Ok(unread_answer(2, "idle", 0, false)));
    assert_eq!(
        m.haptic_request().as_deref(),
        Some(r#"{"kind":"success","op":"haptic"}"#)
    );
    m.haptic_done();
    m.poll_request();
    m.poll_done(Ok(unread_answer(3, "blocked", 0, false)));
    assert!(m.haptic_request().unwrap().contains("warning"));
    m.haptic_done();
    // A send taps; a send that can't go buzzes an error.
    m.poll_request();
    m.poll_done(Ok(unread_answer(4, "idle", 0, false)));
    m.haptic = None;
    m.send_text("hi");
    assert!(m.haptic_request().unwrap().contains("light"));
}

#[test]
fn keyed_machines_are_reached_at_their_routes() {
    let mac = format!("k{}", "a".repeat(32));
    let redwood = format!("k{}", "b".repeat(32));
    let mut m = Model::default();
    let pairing = format!(
        r#"{{"relay":"https://fleet-relay.fly.dev","machine":"mac","token":"t","name":"Mac","route":"{mac}"}}"#
    );
    m.load(
        Some(&serde_json::to_string(&Connection::parse(&pairing).unwrap()).unwrap()),
        None,
        None,
    );
    m.tick(1_000.0);
    let (url, _) = m.poll_request().unwrap();
    assert!(
        url.starts_with(&format!("https://fleet-relay.fly.dev/m/{mac}/api/fleet")),
        "{url}"
    );
    let mut first = answer(1, true);
    first["peers"] =
        json!([{"machine_id": "redwood", "name": "redwood", "enabled": true, "route": redwood}]);
    m.poll_done(Ok(first));
    assert_eq!(m.route_of("redwood"), redwood);
    assert_eq!(m.route_of("studio"), "studio", "no route: its machine id");
    // Away from the Mac, the peer is probed at its route.
    for _ in 0..12 {
        m.tick(m.poll.next_at);
        m.poll_request();
        m.poll_done(Err((502, "machine is not connected".into())));
        if !m.probe_queue.is_empty() {
            break;
        }
    }
    assert!(m
        .probe_request()
        .unwrap()
        .contains(&format!("/m/{redwood}/api/health")));
}

#[test]
fn polls_keep_a_gap_however_fast_the_fleet_changes() {
    let mut m = paired();
    m.tick(1_000.0);
    m.poll_request();
    m.poll_done(Ok(answer(1, true)));
    let turn = m.poll.turn;
    m.tick(2_000.0);
    assert_eq!(m.poll.turn, turn, "not a second after the last answer");
    m.tick(1_000.0 + POLL_GAP_MS + 100.0);
    assert!(m.poll.turn > turn);
}

#[test]
fn coming_back_to_the_foreground_asks_again_at_once() {
    let mut m = paired();
    m.page(true);
    m.tick(1_000.0);
    assert!(m.poll_request().is_some());
    // Suspended with the poll out: iOS drops it and never calls back.
    m.page(false);
    m.page(true);
    assert!(!m.poll.inflight);
    assert!(m.poll_request().is_some(), "a fresh poll, without waiting");
}

#[test]
fn a_poll_that_never_answers_is_given_up_and_asked_again() {
    let mut m = paired();
    m.tick(1_000.0);
    m.poll_request();
    m.poll_done(Ok(answer(1, true)));
    m.tick(4_000.0);
    m.poll_request();
    let turn = m.poll.turn;
    m.tick(20_000.0);
    assert!(m.poll.inflight, "a long poll may hold 25 s");
    m.tick(45_000.0);
    assert!(!m.poll.inflight && m.poll.failures == 1, "given up");
    m.tick(m.poll.next_at);
    assert!(m.poll.turn > turn, "and asked again");
    let (url, _) = m.poll_request().unwrap();
    assert!(
        url.ends_with("&try=1"),
        "a new request, not the one that hung: {url}"
    );
}

#[test]
fn a_page_reported_hidden_or_offline_never_stops_polling() {
    let mut m = paired();
    m.page(false);
    m.network(false);
    m.tick(1_000.0);
    assert!(
        m.poll_request().is_some(),
        "iOS suspends a hidden app itself"
    );
    // A pull gives up a poll that never came back and asks again now.
    let turn = m.poll.turn;
    m.pull();
    assert!(m.poll.turn > turn && !m.poll.inflight);
}

#[test]
fn health_events_batch_up_and_name_no_content() {
    let mut m = paired();
    assert_eq!(m.telemetry.install.len(), 32);
    assert!(m
        .writes
        .iter()
        .any(|(k, v)| *k == KEY_INSTALL && v.is_some()));
    m.tick(1_000.0);
    m.poll_request();
    m.poll_done(Err((502, "machine is not connected to the relay".into())));
    m.tick(5_000.0);
    m.poll_request();
    m.poll_done(Ok(answer(1, true)));
    m.tick(40_000.0);
    let body = m.report_request().expect("a batch is due");
    for event in [
        "app.launch",
        "poll.failure",
        "not_connected",
        "connection.recovered",
    ] {
        assert!(body.contains(event), "{event} in {body}");
    }
    // No content: the session and machine names never appear.
    assert!(!body.contains("One") && !body.contains("\"Mac\""));
    m.report_done(false);
    assert!(m.telemetry.waiting() >= 3, "kept for the next batch");
}

#[test]
fn a_failed_poll_says_why() {
    let mut m = paired();
    m.poll_request();
    m.poll_done(Err((502, "machine is not connected to the relay".into())));
    assert!(m.poll_error.contains("isn't connected to the relay"));
    m.poll_request();
    m.poll_done(Err((
        0,
        "TypeError: Failed to fetch — The request timed out.".into(),
    )));
    assert_eq!(m.poll_error, "The relay didn't answer in time.");
    m.poll_request();
    m.poll_done(Ok(answer(1, true)));
    assert!(m.poll_error.is_empty());
}

#[test]
fn a_dropped_poll_does_not_grey_every_session() {
    let mut m = paired();
    m.tick(1_000.0);
    m.poll_request();
    m.poll_done(Ok(answer(1, true)));
    let key = ("mac".to_string(), "s1".to_string());
    m.tick(2_000.0);
    m.poll_request();
    m.poll_done(Err((0, "The request timed out.".into())));
    assert!(!m.fresh);
    assert!(m.live_session(&key).is_some(), "within the grace");
    let version = m.version;
    m.tick(1_000.0 + POLL_GRACE_MS);
    assert!(m.live_session(&key).is_none(), "the grace ran out");
    assert!(m.version > version, "and the view hears it");
}

#[test]
fn a_sent_message_waits_in_the_transcript_until_read_back() {
    let mut m = paired();
    m.poll_request();
    m.poll_done(Ok(answer(1, true)));
    m.open("mac", "s1");
    m.send_text("hello ");
    let (url, _, body) = m.send_request().unwrap();
    assert!(url.ends_with("/machines/mac/sessions/s1/input"));
    assert_eq!(body, r#"{"enter":true,"text":"hello"}"#);
    m.send_done(Ok(json!({"status": "ok"})));
    assert_eq!(m.pending.len(), 1);
    m.transcript_request();
    m.transcript_done(Ok(r#"{"entries":[{"kind":"user","text":"hello"}]}"#.into()));
    assert!(m.pending.is_empty());
}

fn codex_answer(state: &str) -> serde_json::Value {
    json!({
        "instance": "i", "version": 1, "machine": "mac",
        "fleet": {"machines": [{"id": "mac", "name": "Mac", "last": {"sessions": [
            {"id": "c1", "provider": "codex", "state": state,
             "codex_socket": "/tmp/c", "native_id": "thread-1"}
        ]}}]},
        "desktop": {"windows": []}
    })
}

#[test]
fn a_message_to_a_working_codex_session_is_queued_behind_its_turn() {
    let mut m = paired();
    m.poll_request();
    m.poll_done(Ok(codex_answer("running")));
    m.open("mac", "c1");
    m.send_text("next, run the tests");
    m.send_text("then commit");
    let op = |body: &str| serde_json::from_str::<serde_json::Value>(body).unwrap();
    let (url, _, body) = m.send_request().unwrap();
    assert!(url.ends_with("/machines/mac/sessions/c1/codex-client"));
    let body = op(&body);
    assert_eq!(body["operation"], "attach");
    assert_eq!(body["thread_id"], "thread-1");
    assert!(body["client_id"]
        .as_str()
        .unwrap()
        .starts_with("ocho-phone-"));
    m.send_done(Ok(json!({"thread_id": "thread-1", "turns": []})));
    let (_, _, body) = m.send_request().unwrap();
    let body = op(&body);
    assert_eq!(body["operation"], "send");
    assert_eq!(body["text"], "next, run the tests");
    m.send_done(Ok(json!({"turns": [], "receipt": {"status": "queued"}})));
    assert!(m.failed.is_none());
    // Every send is "queued" at first: a look a moment later says whether
    // it was held behind the turn or already taken.
    assert!(crate::view::session(&m)["queue"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(m.send_request().is_none(), "the look waits its moment");
    m.tick(m.now + 800.0);
    let look = op(&m.send_request().unwrap().2);
    assert_eq!(look["operation"], "poll");
    assert_eq!(look["request_id"], "00000000000000000000000000000001");
    m.send_done(Ok(json!({"turns": [], "receipt": {"status": "queued"}})));
    let view = crate::view::session(&m);
    assert_eq!(
        view["entries"].as_array().unwrap().len(),
        1,
        "the second, not yet sent, is a bubble"
    );
    let queue = view["queue"].as_array().unwrap();
    assert_eq!(queue.len(), 1, "the held one waits in the tray");
    assert_eq!(op(&m.send_request().unwrap().2)["operation"], "detach");
    m.send_done(Ok(json!({"turns": []})));
    // The second message goes the same way, in order.
    let body = op(&m.send_request().unwrap().2);
    assert_eq!(body["operation"], "attach");
    m.send_done(Ok(json!({"turns": []})));
    let body = op(&m.send_request().unwrap().2);
    assert_eq!(body["text"], "then commit");
    m.send_done(Ok(
        json!({"receipt": {"status": "not_submitted", "reason": "session queue is full"}}),
    ));
    assert_eq!(m.failed.as_ref().unwrap().2, "session queue is full");
    assert_eq!(m.pending.len(), 1);
    assert_eq!(
        op(&m.send_request().unwrap().2)["operation"],
        "detach",
        "and still detaches"
    );
}

#[test]
fn a_codex_thread_the_desktop_never_opened_cannot_be_sent_to() {
    let mut m = paired();
    m.poll_request();
    let mut a = codex_answer("idle");
    a["fleet"]["machines"][0]["last"]["sessions"][0]["native_id"] = json!("");
    m.poll_done(Ok(a));
    m.open("mac", "c1");
    m.send_text("hi");
    assert!(m.send_request().is_none());
    assert!(m
        .failed
        .as_ref()
        .unwrap()
        .2
        .contains("Open this conversation in Codex"));
}

#[test]
fn a_refused_message_comes_back_to_retry() {
    let mut m = paired();
    m.poll_request();
    m.poll_done(Ok(answer(1, true)));
    m.open("mac", "s1");
    m.send_text("hi");
    m.send_request();
    m.send_done(Err("502".into()));
    assert!(m.pending.is_empty());
    assert_eq!(m.failed.as_ref().unwrap().1, "hi");
    m.retry();
    assert!(m.send_request().is_some());
}

#[test]
fn a_scanned_code_pairs_or_says_what_it_was() {
    let mut m = Model::default();
    m.load(None, None, None);
    m.pair_scanned("https://example.com");
    assert!(m.conn.is_none() && m.pair_error.contains("isn't an Ocho pairing code"));
    m.pair_scanned(
        r#"{"machine":"mac","name":"Mac","relay":"https://fleet-relay.fly.dev","token":"t"}"#,
    );
    assert_eq!(m.conn.as_ref().unwrap().name, "Mac");
    assert!(m.pair_error.is_empty());
}

#[test]
fn unpair_forgets_and_turns_move_on() {
    let mut m = paired();
    let turn = m.poll.turn;
    m.unpair();
    assert!(m.conn.is_none());
    assert_eq!(m.poll.turn, turn);
    assert!(m.writes.contains(&(KEY_CONNECTION, None)));
}

#[test]
fn the_launch_poll_is_timed_from_launch_and_given_up_soon() {
    let mut m = paired();
    // Asked before any event said the time.
    assert!(m.poll_request().is_some());
    m.clock(1_790_000_000_000.0);
    m.tick(1_790_000_001_000.0);
    assert!(m.poll.inflight, "not overdue on the first tick");
    m.tick(1_790_000_011_500.0);
    assert!(
        !m.poll.inflight,
        "the first poll is answered at once, so 10 s is enough"
    );
    m.tick(1_790_000_014_000.0);
    assert!(m.poll_request().is_some());
    m.poll_done(Ok(answer(1, true)));
    let events = m.telemetry.take(m.now, "test").unwrap();
    assert!(events.contains("connect.first"));
    assert!(events.contains("14000"), "{events}");
}

/// `OCHO_FLEET=<file> cargo test --release -p ocho-mobile-data -- --ignored bench --nocapture`
#[test]
#[ignore]
fn bench_a_real_poll() {
    let Ok(path) = std::env::var("OCHO_FLEET") else {
        return;
    };
    let text = std::fs::read_to_string(path).unwrap();
    let mut m = paired();
    m.clock(1_790_000_000_000.0);
    let n = 20;
    let start = std::time::Instant::now();
    for _ in 0..n {
        let _: serde_json::Value = serde_json::from_str(&text).unwrap();
    }
    let parse = start.elapsed() / n;
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    let start = std::time::Instant::now();
    for _ in 0..n {
        m.poll_request();
        m.poll_done(Ok(json.clone()));
    }
    let apply = start.elapsed() / n;
    let start = std::time::Instant::now();
    for _ in 0..n {
        let _ = crate::view::render(&m);
    }
    let render = start.elapsed() / n;
    println!("parse {parse:?} · read+apply {apply:?} · render {render:?}");
}

/// `OCHO_TRANSCRIPT=<file> cargo test --release -p ocho-mobile-data -- --ignored bench_open --nocapture`
#[test]
#[ignore]
fn bench_open_a_thread() {
    let Ok(path) = std::env::var("OCHO_TRANSCRIPT") else {
        return;
    };
    let body = std::fs::read_to_string(path).unwrap();
    let mut m = paired();
    m.clock(1_790_000_000_000.0);
    m.poll_request();
    m.poll_done(Ok(answer(1, true)));
    m.open("mac", "s1");
    let n = 10;
    let start = std::time::Instant::now();
    for _ in 0..n {
        m.transcript_request();
        m.conversations.clear();
        m.transcript_done(Ok(body.clone()));
    }
    let decode = start.elapsed() / n;
    let start = std::time::Instant::now();
    for _ in 0..n {
        let _ = crate::view::render(&m);
    }
    let render = start.elapsed() / n;
    let entries = m
        .conversations
        .values()
        .next()
        .map(|c| c.transcript.entries.len())
        .unwrap_or(0);
    let view = crate::view::session(&m);
    let json = serde_json::to_string(&view).unwrap().len();
    println!(
        "entries {entries} · decode+apply {decode:?} · render {render:?} · view json {json} B"
    );
}

#[test]
fn a_long_thread_opens_on_its_last_page_and_shows_earlier_on_request() {
    let mut m = paired();
    m.poll_request();
    m.poll_done(Ok(answer(1, true)));
    m.open("mac", "s1");
    let entries: Vec<serde_json::Value> = (0..100)
        .map(|i| json!({"kind": if i % 2 == 0 { "user" } else { "assistant" }, "text": format!("message {i}")}))
        .collect();
    m.transcript_request();
    m.transcript_done(Ok(json!({ "entries": entries }).to_string()));
    let drawn = |m: &Model| crate::view::session(m)["entries"].as_array().unwrap().len();
    let earlier = |m: &Model| {
        crate::view::session(m)["earlier"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert_eq!(drawn(&m), 40);
    assert_eq!(earlier(&m), "Show 40 earlier messages");
    m.show_earlier();
    assert_eq!(drawn(&m), 80);
    assert_eq!(earlier(&m), "Show 20 earlier messages");
    m.show_earlier();
    assert_eq!(drawn(&m), 100);
    assert_eq!(earlier(&m), "");
}

#[test]
fn a_live_codex_thread_offers_voice_through_the_relay() {
    let mut m = paired();
    m.poll_request();
    m.poll_done(Ok(codex_answer("idle")));
    m.open("mac", "c1");
    let (url, auth) = m.voice().expect("a live Codex app-server thread");
    assert_eq!(
        url,
        "https://fleet-relay.fly.dev/m/mac/api/machines/mac/sessions/c1/voice?thread=thread-1"
    );
    assert_eq!(auth, "Bearer t", "the bearer goes in a header, not the URL");
    assert_eq!(crate::view::session(&m)["canTalk"], true);
    // A terminal session, or one Codex never opened, has no voice.
    m.open("mac", "s1");
    assert!(m.voice().is_none());
    let mut a = codex_answer("idle");
    a["fleet"]["machines"][0]["last"]["sessions"][0]["native_id"] = json!("");
    m.poll_request();
    m.poll_done(Ok(a));
    m.open("mac", "c1");
    assert!(m.voice().is_none());
}

#[test]
fn send_now_asks_for_the_same_message_to_interrupt_the_turn() {
    let mut m = paired();
    m.poll_request();
    m.poll_done(Ok(codex_answer("running")));
    m.open("mac", "c1");
    m.send_text("stop and do this instead");
    let op = |body: &str| serde_json::from_str::<serde_json::Value>(body).unwrap();
    m.send_request();
    m.send_done(Ok(json!({"turns": []})));
    let sent = op(&m.send_request().unwrap().2);
    assert_eq!(sent["operation"], "send");
    let id = sent["request_id"].as_str().unwrap().to_string();
    m.send_done(Ok(json!({"receipt": {"status": "queued"}})));
    m.tick(m.now + 800.0);
    assert_eq!(op(&m.send_request().unwrap().2)["operation"], "poll");
    m.send_done(Ok(json!({"receipt": {"status": "queued"}})));
    m.send_request();
    m.send_done(Ok(json!({"turns": []})));
    assert_eq!(crate::view::session(&m)["queue"][0]["id"], id.as_str());
    m.send_now(&id);
    assert_eq!(crate::view::session(&m)["queue"][0]["interrupting"], true);
    assert_eq!(op(&m.send_request().unwrap().2)["operation"], "attach");
    m.send_done(Ok(json!({"turns": []})));
    let again = op(&m.send_request().unwrap().2);
    assert_eq!(again["operation"], "interrupt-send");
    assert_eq!(
        again["request_id"],
        id.as_str(),
        "the same message, not a second one"
    );
    assert_eq!(again["text"], "stop and do this instead");
    // A second press while it interrupts asks nothing more.
    m.send_done(Ok(json!({"receipt": {"status": "submitted"}})));
    m.send_request();
    m.send_done(Ok(json!({"turns": []})));
    m.send_now(&id);
    assert!(m.send_request().is_none());
}

#[test]
fn attached_files_are_pasted_alone_before_the_words() {
    let mut m = paired();
    m.poll_request();
    m.poll_done(Ok(answer(1, true)));
    m.open("mac", "s1");
    let path = "/Users/me/.local/share/fleet/paste/1-IMG_1.jpg";
    m.send_text(&format!("what is this?\n\n{path}"));
    let (url, _, body) = m.send_request().unwrap();
    assert!(url.ends_with("/machines/mac/sessions/s1/input"));
    assert_eq!(body, format!(r#"{{"enter":false,"text":"{path} "}}"#));
    m.send_done(Ok(json!({"status": "ok"})));
    let (_, _, body) = m.send_request().expect("then the words");
    assert_eq!(body, r#"{"enter":true,"text":"what is this?"}"#);
    m.send_done(Ok(json!({"status": "ok"})));
    assert!(m.send_request().is_none());
    // Claude shows the image its own way; the words still match.
    m.transcript_request();
    m.transcript_done(Ok(
        r#"{"entries":[{"kind":"user","text":"[Image #1] what is this?"}]}"#.into(),
    ));
    assert!(m.pending.is_empty());
}

#[test]
fn only_paste_folder_paths_split_off() {
    use super::send::split_files;
    let p = "/Users/me/.local/share/fleet/paste/1-a.png";
    assert_eq!(
        split_files(&format!("hi\n\n{p} {p}")),
        ("hi".into(), format!("{p} {p}"))
    );
    assert_eq!(split_files(p), (String::new(), p.to_string()));
    assert_eq!(
        split_files("hi\n\nsee /tmp/x"),
        ("hi\n\nsee /tmp/x".into(), String::new())
    );
    assert_eq!(split_files("plain"), ("plain".into(), String::new()));
}

fn claude_gateway_answer(state: &str) -> serde_json::Value {
    json!({
        "instance": "i", "version": 1, "machine": "mac",
        "fleet": {"machines": [{"id": "mac", "name": "Mac", "last": {"sessions": [
            {"id": "k1", "provider": "claude", "state": state, "tmux_pane": "%1",
             "capabilities": ["attach"], "claude_socket": "/tmp/k", "native_id": "claude-1"}
        ]}}]},
    })
}

#[test]
fn claude_behind_its_gateway_queues_and_sends_now_as_codex_does() {
    let mut m = paired();
    m.poll_request();
    m.poll_done(Ok(claude_gateway_answer("running")));
    m.open("mac", "k1");
    m.send_text("then do this");
    let op = |body: &str| serde_json::from_str::<serde_json::Value>(body).unwrap();
    let (url, _, body) = m.send_request().unwrap();
    assert!(url.ends_with("/machines/mac/sessions/k1/session-client"));
    assert_eq!(op(&body)["operation"], "attach");
    assert_eq!(op(&body)["thread_id"], "claude-1");
    m.send_done(Ok(json!({"turns": []})));
    let sent = op(&m.send_request().unwrap().2);
    assert_eq!(sent["operation"], "send");
    let id = sent["request_id"].as_str().unwrap().to_string();
    m.send_done(Ok(json!({"receipt": {"status": "queued"}})));
    m.tick(m.now + 800.0);
    assert_eq!(op(&m.send_request().unwrap().2)["operation"], "poll");
    m.send_done(Ok(json!({"receipt": {"status": "queued"}})));
    m.send_request();
    m.send_done(Ok(json!({"turns": []})));
    assert_eq!(crate::view::session(&m)["queue"][0]["id"], id.as_str());
    m.send_now(&id);
    let (url, _, _) = m.send_request().unwrap();
    assert!(url.ends_with("/session-client"));
    m.send_done(Ok(json!({"turns": []})));
    let again = op(&m.send_request().unwrap().2);
    assert_eq!(again["operation"], "interrupt-send");
    assert_eq!(again["request_id"], id.as_str());
}

#[test]
fn a_claude_message_with_files_still_pastes_them_as_images() {
    let mut m = paired();
    m.poll_request();
    m.poll_done(Ok(claude_gateway_answer("idle")));
    m.open("mac", "k1");
    let path = "/Users/me/.local/share/fleet/paste/1-IMG_1.jpg";
    m.send_text(&format!("look\n\n{path}"));
    let (url, _, body) = m.send_request().unwrap();
    assert!(url.ends_with("/sessions/k1/input"));
    assert_eq!(body, format!(r#"{{"enter":false,"text":"{path} "}}"#));
}

#[test]
fn telemetry_keeps_sending_after_the_first_batch() {
    let mut m = paired();
    let t0 = 1_790_000_000_000.0;
    m.tick(t0);
    let turn = m.report.turn;
    assert!(turn > 0, "the launch batch is due");
    let first = m.report_request().expect("launch batch");
    assert!(first.contains("app.launch"));
    m.report_done(true);
    m.poll_request();
    m.poll_done(Ok(answer(1, true)));
    m.tick(t0 + 1_000.0);
    m.tick(t0 + 31_000.0);
    assert!(m.report.turn > turn, "a second batch is due");
    let second = m.report_request().expect("second batch");
    assert!(second.contains("connect.first"), "{second}");
}

#[test]
fn transcript_reads_ask_only_for_what_changed() {
    let mut m = paired();
    m.poll_request();
    m.poll_done(Ok(answer(1, true)));
    m.open("mac", "s1");
    let (url, _) = m.transcript_request().unwrap();
    assert!(
        url.ends_with("/transcript"),
        "a first read asks for everything"
    );
    m.transcript_done(Ok(r#"{"entries":[{"kind":"user","text":"hi"},{"kind":"assistant","text":"wor"}],"revision":"r1","base":"b1"}"#.into()));
    let key = ("mac".to_string(), "s1".to_string());
    let (url, _) = m.transcript_request().unwrap();
    assert!(
        url.ends_with("/transcript?revision=r1&have=2&base=b1"),
        "{url}"
    );
    // Unchanged: nothing redrawn, entries kept.
    let version = m.version;
    m.transcript_done(Ok(
        r#"{"entries":[],"revision":"r1","unchanged":true}"#.into()
    ));
    assert_eq!(m.version, version);
    assert_eq!(m.conversations[&key].transcript.entries.len(), 2);
    // A delta: the last entry grew, one more followed.
    m.transcript_request();
    m.transcript_done(Ok(r#"{"entries":[{"kind":"assistant","text":"working"},{"kind":"tools","count":2}],"from":1,"revision":"r2","base":"b2"}"#.into()));
    let entries = &m.conversations[&key].transcript.entries;
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].text, "hi");
    assert_eq!(entries[1].text, "working");
    assert_eq!(entries[2].count, 2);
    assert!(m.version > version);
    let (url, _) = m.transcript_request().unwrap();
    assert!(url.ends_with("?revision=r2&have=3&base=b2"), "{url}");
}

#[test]
fn a_network_blip_neither_drops_the_poll_nor_resets_its_failures() {
    let mut m = paired();
    m.page(true);
    m.network(true);
    m.tick(1_000.0);
    assert!(m.poll_request().is_some());
    m.poll.failures = 2;
    m.network(false);
    m.network(true);
    assert!(m.poll.inflight, "the poll out is still out");
    assert_eq!(m.poll.failures, 2, "and the count stands");
    // Failed while the path was down: tried again as soon as it is back.
    m.poll_done(Err((
        0,
        "TypeError: Failed to fetch — The Internet connection appears to be offline.".into(),
    )));
    assert!(m.poll.next_at > m.now);
    m.network(false);
    m.network(true);
    assert_eq!(m.poll.next_at, 0.0);
    m.page(false);
    m.page(true);
    assert_eq!(
        m.poll.failures, 0,
        "coming back to the foreground starts over"
    );
}

#[test]
fn a_lost_connection_is_not_called_offline() {
    let lost = (
        0,
        "TypeError: Failed to fetch — The network connection was lost.".to_string(),
    );
    assert_eq!(failure_kind(&lost), "lost");
    assert_eq!(poll_failure(&lost), "The network connection was lost.");
    let offline = (
        0,
        "TypeError: Failed to fetch — The Internet connection appears to be offline.".to_string(),
    );
    assert_eq!(failure_kind(&offline), "offline");
    assert_eq!(poll_failure(&offline), "This phone is offline.");
}

#[test]
fn a_send_caught_by_a_network_blip_goes_again_when_that_is_safe() {
    let offline = "TypeError: Failed to fetch — The Internet connection appears to be offline.";
    let lost = "TypeError: Failed to fetch — The network connection was lost.";
    // Typed input: tried again only when it never left the phone.
    let mut m = paired();
    m.poll_request();
    m.poll_done(Ok(answer(1, true)));
    m.open("mac", "s1");
    m.send_text("hello");
    m.send_request().unwrap();
    m.send_done(Err(offline.into()));
    assert!(m.failed.is_none(), "not given up");
    let turn = m.send.turn;
    m.tick(m.now + 500.0);
    assert_eq!(m.send.turn, turn, "waits out its backoff");
    m.tick(m.now + 2_500.0);
    assert!(m.send.turn > turn, "then goes again");
    let (_, _, body) = m.send_request().unwrap();
    assert_eq!(body, r#"{"enter":true,"text":"hello"}"#);
    m.send_done(Err(lost.into()));
    assert!(
        m.failed.is_some(),
        "it may have arrived: typing it again could double it"
    );
    // A queued message names itself: the same request again, until it lands.
    let mut m = paired();
    m.poll_request();
    m.poll_done(Ok(codex_answer("running")));
    m.open("mac", "c1");
    m.send_text("later");
    m.send_request();
    m.send_done(Ok(json!({"turns": []})));
    let op = |body: &str| serde_json::from_str::<serde_json::Value>(body).unwrap();
    let first = op(&m.send_request().unwrap().2);
    m.send_done(Err(lost.into()));
    m.tick(m.now + 2_500.0);
    let again = op(&m.send_request().unwrap().2);
    assert_eq!(again["operation"], "send");
    assert_eq!(again["request_id"], first["request_id"]);
    for _ in 0..4 {
        m.send_done(Err(lost.into()));
        m.tick(m.now + 20_000.0);
        m.send_request();
    }
    assert!(m.failed.is_some(), "and gives up after a few");
}

#[test]
fn a_message_codex_takes_at_once_is_never_shown_queued() {
    let mut m = paired();
    m.poll_request();
    m.poll_done(Ok(codex_answer("idle")));
    m.open("mac", "c1");
    m.send_text("go");
    let op = |body: &str| serde_json::from_str::<serde_json::Value>(body).unwrap();
    m.send_request();
    m.send_done(Ok(json!({"turns": []})));
    m.send_request();
    m.send_done(Ok(json!({"receipt": {"status": "queued"}})));
    // Codex starts on it: the session is working before the look.
    m.poll_request();
    m.poll_done(Ok(codex_answer("running")));
    assert!(crate::view::session(&m)["queue"]
        .as_array()
        .unwrap()
        .is_empty());
    m.tick(m.now + 800.0);
    assert_eq!(op(&m.send_request().unwrap().2)["operation"], "poll");
    m.send_done(Ok(json!({"receipt": {"status": "submitted"}})));
    let view = crate::view::session(&m);
    assert!(
        view["queue"].as_array().unwrap().is_empty(),
        "taken, not queued"
    );
    assert_eq!(
        view["entries"].as_array().unwrap().len(),
        1,
        "a plain bubble"
    );
    assert_eq!(op(&m.send_request().unwrap().2)["operation"], "detach");
}

#[test]
fn a_receipt_still_unknown_is_looked_at_a_few_times_then_left() {
    let mut m = paired();
    m.poll_request();
    m.poll_done(Ok(codex_answer("running")));
    m.open("mac", "c1");
    m.send_text("go");
    let op = |body: &str| serde_json::from_str::<serde_json::Value>(body).unwrap();
    m.send_request();
    m.send_done(Ok(json!({"turns": []})));
    m.send_request();
    m.send_done(Ok(json!({"receipt": {"status": "queued"}})));
    let mut looks = 0;
    loop {
        m.tick(m.now + 1_100.0);
        let body = op(&m.send_request().unwrap().2);
        if body["operation"] == "detach" {
            break;
        }
        looks += 1;
        m.send_done(Ok(json!({"receipt": {"status": "unknown"}})));
    }
    assert_eq!(looks, 3);
}

#[test]
fn an_older_server_without_poll_receipts_still_shows_the_queue() {
    let mut m = paired();
    m.poll_request();
    m.poll_done(Ok(codex_answer("running")));
    m.open("mac", "c1");
    m.send_text("later");
    m.send_request();
    m.send_done(Ok(json!({"turns": []})));
    m.send_request();
    m.send_done(Ok(json!({"receipt": {"status": "queued"}})));
    m.tick(m.now + 800.0);
    m.send_request();
    m.send_done(Ok(json!({"thread_id": "thread-1", "turns": []})));
    assert_eq!(
        crate::view::session(&m)["queue"].as_array().unwrap().len(),
        1
    );
}

#[test]
fn a_peer_is_asked_for_every_open_session() {
    let mut m = paired();
    let (url, _) = m.poll_request().unwrap();
    assert!(!url.contains("sessions=open"), "home keeps its tabs: {url}");
    m.via = "redwood".into();
    m.poll.inflight = false;
    let (url, _) = m.poll_request().unwrap();
    assert!(url.contains("&sessions=open"), "{url}");
}

#[test]
fn a_new_session_launches_with_its_choices_and_opens() {
    let mut m = paired();
    m.poll_request();
    m.poll_done(Ok(codex_answer("idle")));
    m.compose_open();
    assert_eq!(m.launcher.machine, "mac", "home, while it answers");
    assert_eq!(m.launcher.provider, "codex");
    let (url, _) = m.models_request().expect("the models are asked for");
    assert!(
        url.ends_with("/machines/mac/models?provider=codex"),
        "{url}"
    );
    m.models_done(Ok(json!([
        {"id": "sol", "name": "6.1 Sol", "default": true},
        {"id": "astra", "name": "6 Astra"}
    ])));
    assert_eq!(m.launch_model_name(), "6.1 Sol");
    m.compose_choose("model", "astra");
    m.compose_choose("effort", "high");
    m.compose_send("build the thing");
    // At once: the message as the user's, the session starting.
    let view = crate::view::session(&m);
    assert_eq!(view["title"], "New session");
    assert_eq!(view["working"], true);
    assert_eq!(view["status"], "Starting session…");
    assert_eq!(view["entries"][0]["kind"], "user");
    assert_eq!(view["entries"][0]["pending"], true);
    assert!(m.transcript_request().is_none(), "nothing to read yet");
    let (url, _, body) = m.launch_request().unwrap();
    assert!(url.ends_with("/machines/mac/launch"));
    let body: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(body["provider"], "codex");
    assert_eq!(body["model"], "astra");
    assert_eq!(body["effort"], "high");
    assert_eq!(body["prompt"], "build the thing");
    assert!(
        body.get("account").is_none(),
        "the provider's default account"
    );
    m.launch_done(Ok(json!({"request_id": "r", "session": {"id": "new1"}})));
    assert_eq!(m.launcher.goto, Some(("mac".into(), "new1".into())));
    let key = ("mac".to_string(), "new1".to_string());
    assert_eq!(
        m.pending_for(&key).count(),
        1,
        "the message moved to the named session"
    );
    m.goto_done();
    assert!(m.launcher.goto.is_none());
}

#[test]
fn a_failed_launch_says_why_and_can_go_again() {
    let mut m = paired();
    m.poll_request();
    m.poll_done(Ok(codex_answer("idle")));
    m.compose_open();
    m.compose_send("go");
    m.launch_request().unwrap();
    m.launch_done(Err("launch request x: no account".into()));
    assert!(m.launcher.error.contains("no account"));
    assert!(!m.launcher.launching);
    let view = crate::view::session(&m);
    assert!(view["failed"].as_str().unwrap().contains("no account"));
    assert_eq!(view["working"], false);
    // Retry launches the same message again.
    m.retry();
    assert!(m.launcher.launching);
    let (_, _, body) = m.launch_request().unwrap();
    assert!(body.contains("\"prompt\":\"go\""));
}

#[test]
fn a_draft_stays_with_its_conversation() {
    let mut m = paired();
    m.poll_request();
    m.poll_done(Ok(answer(1, true)));
    m.open("mac", "s1");
    m.draft_written("half a thought");
    m.close();
    m.open("mac", "s2");
    assert_eq!(crate::view::session(&m)["draft"], "");
    m.close();
    m.open("mac", "s1");
    assert_eq!(crate::view::session(&m)["draft"], "half a thought");
    assert!(m.writes.iter().any(
        |(k, v)| *k == KEY_DRAFTS && v.as_deref().is_some_and(|v| v.contains("half a thought"))
    ));
    // Kept across a relaunch, and gone once sent.
    let saved = m
        .writes
        .iter()
        .rev()
        .find(|(k, _)| *k == KEY_DRAFTS)
        .unwrap()
        .1
        .clone();
    let mut again = paired();
    again.load_drafts(saved.as_deref());
    assert_eq!(
        again.draft_for(&("mac".into(), "s1".into())),
        "half a thought"
    );
    m.send_text("half a thought");
    assert_eq!(m.draft_for(&("mac".into(), "s1".into())), "");
}

#[test]
fn a_working_codex_reply_streams_into_the_conversation() {
    let mut m = paired();
    m.poll_request();
    m.poll_done(Ok(codex_answer("running")));
    m.open("mac", "c1");
    let op = |body: &str| serde_json::from_str::<serde_json::Value>(body).unwrap();
    m.tick(m.now + 1_000.0);
    let (url, _, body) = m.stream_request().expect("attaches while it works");
    assert!(url.ends_with("/sessions/c1/codex-client"));
    assert_eq!(op(&body)["operation"], "attach");
    m.stream_done(Ok(
        json!({"turns": [{"id": "t1", "complete": false, "message": "Looking at"}]}),
    ));
    let entries = crate::view::session(&m)["entries"].clone();
    let last = entries.as_array().unwrap().last().unwrap().clone();
    assert_eq!(last["id"], "streaming");
    m.tick(m.now + 800.0);
    let (_, _, body) = m.stream_request().unwrap();
    assert_eq!(op(&body)["operation"], "poll");
    m.stream_done(Ok(
        json!({"turns": [{"id": "t1", "complete": false, "message": "Looking at the tests now"}]}),
    ));
    assert_eq!(m.streaming.text, "Looking at the tests now");
    // The turn ends: the session goes idle, the phone lets go.
    m.poll_request();
    m.poll_done(Ok(codex_answer("idle")));
    m.tick(m.now + 800.0);
    let (_, _, body) = m.stream_request().unwrap();
    assert_eq!(op(&body)["operation"], "detach");
    m.stream_done(Ok(json!({"turns": []})));
    assert!(m.streaming_text().is_none());
    m.tick(m.now + 800.0);
    assert!(m.stream_request().is_none(), "nothing more to stream");
}

#[test]
fn a_message_read_back_behind_a_stray_terminal_reply_still_echoes() {
    let sent = "Yes I am expecting it to be paid only, that’s fine.";
    assert!(send::echoes(sent, sent));
    assert!(
        send::echoes(&format!("2;35R{sent}"), sent),
        "cursor report leaked in front"
    );
    assert!(
        !send::echoes(&format!("earlier words {sent}"), sent),
        "another message"
    );
    assert!(!send::echoes("2;35R", ""), "nothing sent");
}

#[test]
fn a_failover_queued_in_the_background_is_dropped_once_home_answers() {
    let mut m = paired();
    m.page(true);
    m.tick(1_000.0);
    m.poll_request();
    m.poll_done(Ok(answer(1, true)));
    m.page(false);
    // Suspended: polls fail until a look for a peer is under way.
    while m.probe_queue.is_empty() && !m.probe.inflight {
        m.tick(m.poll.next_at);
        m.poll_request();
        m.poll_done(Err((0, "The network connection was lost.".into())));
    }
    let probing = m.probe_request().is_some();
    // Back in the foreground, home answers before the probe does.
    m.page(true);
    m.tick(m.now + 100.0);
    m.poll_request();
    m.poll_done(Ok(answer(2, true)));
    if probing {
        m.probe_done(true);
    }
    assert_eq!(m.via, "mac", "stays on the machine that answers");
    assert!(m.probe_request().is_none(), "nothing left to look for");
}

#[test]
fn a_relay_handover_neither_moves_the_phone_nor_shows() {
    let mut m = paired();
    m.tick(1_000.0);
    m.poll_request();
    m.poll_done(Ok(answer(1, true)));
    // The Mac's relay connection is replaced: one poll hears "not connected".
    m.tick(m.poll.next_at);
    m.poll_request();
    m.poll_done(Err((
        502,
        r#"{"error":"machine is not connected to the relay"}"#.into(),
    )));
    assert!(m.probe_request().is_none(), "no look elsewhere yet");
    assert!(!m.unreachable(), "and nothing said");
    assert_eq!(m.poll.next_at - m.now, FIRST_RETRY_MS, "retried quickly");
    m.tick(m.poll.next_at);
    m.poll_request();
    m.poll_done(Ok(answer(2, true)));
    assert_eq!(m.via, "mac");
    assert!(!m.unreachable());
}

#[test]
fn polls_failing_for_a_while_say_so() {
    let mut m = paired();
    m.tick(1_000.0);
    m.poll_request();
    m.poll_done(Ok(answer(1, true)));
    while m.now < 1_000.0 + UNREACHABLE_AFTER_MS + 2_000.0 {
        m.tick(m.poll.next_at.max(m.now + 1_000.0));
        if m.poll_request().is_some() {
            m.poll_done(Err((0, "The request timed out.".into())));
        }
    }
    assert!(m.unreachable());
}
