//! Child processes under the agent's clock, answered by scripts (LLP
//! 1133.007 §7): the events, restarts, stdin's queue and `drain`, the
//! origin rule, release and its escalation, and the refusals that come
//! before anything runs.
use super::super::Core;
use exact_runner::{
    CallOrigin, FailureKind, Outcome, ProcessOp, Request, RequestOut, SocketOp, SocketOrigin,
    SocketSend, WorkClock,
};
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

const GRANTS: &str = "process.spawn path:t3\nprocess.spawn /usr/bin/tar\nenv.read login:PATH\nenv.read HOME\nnet.fetch http://127.0.0.1";

/// A wall-clock wait's limit, which only stops a hanging test
/// (docs/testing.md, "Waiting").
const BOUND: Duration = Duration::from_secs(30);

fn script(json: &str) -> String {
    let dir = std::env::temp_dir().join(format!("exact-process-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join(format!("{:x}.json", fnv(json)));
    std::fs::write(&file, json).unwrap();
    file.display().to_string()
}

fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf29ce484222325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
    })
}

fn setup(processes: &[(&str, &str)]) -> (Core, Receiver<()>) {
    let (wake, woke) = channel();
    let core = Core::under_agent(processes, || {
        Core::start(
            None,
            GRANTS,
            Box::new(move || {
                let _ = wake.send(());
            }),
        )
    });
    (core, woke)
}

fn request(ticket: u64, body: &str, stream: bool) -> RequestOut {
    RequestOut {
        ticket,
        target: "server".into(),
        request: Request::process(body.as_bytes().to_vec(), stream, 1 << 20),
        forced: false,
    }
}

/// Every outcome queued now, in order.
fn taken(core: &Core) -> Vec<Outcome> {
    let mut out = Vec::new();
    loop {
        core.begin_pump();
        match core.drain().pop() {
            Some(done) => out.push(done.1),
            None => return out,
        }
    }
}

/// The next outcome, waiting for it on the wall (a one-shot's thread).
fn next(core: &Core, woke: &Receiver<()>) -> Outcome {
    let deadline = Instant::now() + BOUND;
    loop {
        core.begin_pump();
        if let Some(done) = core.drain().pop() {
            return done.1;
        }
        woke.recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("an outcome within five seconds");
    }
}

/// Each event as `name data`, data trimmed of the incarnation's details.
fn events(outcomes: &[Outcome]) -> Vec<String> {
    outcomes
        .iter()
        .map(|o| match o {
            Outcome::Message(m) if m.event == "stdout" || m.event == "stderr" => {
                format!("{} {}", m.event, m.data)
            }
            Outcome::Message(m) => {
                let data: serde_json::Value = serde_json::from_str(&m.data).unwrap_or_default();
                match m.event.as_str() {
                    "spawn" => format!("spawn {} {}", data["pid"], data["restarts"]),
                    "exit" => format!("exit {} {}", data["code"], data["signal"]),
                    "close" => format!("close {} {}", data["code"], data["drained"]),
                    other => other.to_string(),
                }
            }
            Outcome::Closed {
                reason, was_clean, ..
            } => {
                let data: serde_json::Value = serde_json::from_str(reason).unwrap();
                format!("end {} {} {was_clean}", data["code"], data["signal"])
            }
            Outcome::Failed { kind, message } => format!("failed {kind:?} {message}"),
            other => format!("{other:?}"),
        })
        .collect()
}

/// The core's clock, moved as the runner moves it.
fn clock(core: &Core) -> std::sync::Arc<dyn WorkClock> {
    core.work_clock().expect("under the agent")
}

/// Wait for the supervisor to start the child: its `spawn` is queued. The
/// next advance waits for the rest of its first turn (the clock counts it
/// as running until it first waits).
fn waiting(core: &Core) {
    let deadline = Instant::now() + BOUND;
    loop {
        let spawned =
            (core.shared.state.lock().unwrap().completed.iter().flatten()).any(|done| done.message);
        if spawned {
            break;
        }
        assert!(Instant::now() < deadline, "the supervisor never started");
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn ops(ops: Vec<(u32, ProcessOp)>) -> SocketSend {
    SocketSend {
        origin: SocketOrigin::Mutation("send".into()),
        ops: (ops.into_iter())
            .map(|(incarnation, op)| (1, "server".into(), SocketOp::Process { incarnation, op }))
            .collect(),
    }
}

#[test]
fn a_scripted_child_streams_its_events_and_restarts_on_the_agents_clock() {
    let file = script(
        r#"{"incarnations":[
            {"pid":11,"steps":[{"at":0,"stdout":"hello\n"},{"at":100,"exit":0}]},
            {"pid":12,"steps":[{"at":0,"stderr":"again"},{"at":50,"exit":78}]}]}"#,
    );
    let (core, _woke) = setup(&[("path:t3", &format!("script:{file}"))]);
    let body = r#"{"kind":"spawn","program":"path:t3","args":["--bootstrap-fd","0"],"encoding":"utf8",
        "restart":{"when":"always","delay":500,"maxDelay":10000,"preventExitStatus":[78]}}"#;
    core.run(request(1, body, true), None).unwrap();
    waiting(&core);
    let clock = clock(&core);
    clock.advance(0.0);
    assert_eq!(events(&taken(&core)), ["spawn 11 0", "stdout hello\n"]);
    assert_eq!(clock.due(), Some(100.0));
    clock.advance(100.0);
    assert_eq!(events(&taken(&core)), ["exit 0 null", "close 0 true"]);
    assert_eq!(clock.due(), Some(600.0), "500 ms after the close");
    clock.advance(599.0);
    assert!(taken(&core).is_empty());
    clock.advance(600.0);
    assert_eq!(events(&taken(&core)), ["spawn 12 1", "stderr again"]);
    clock.advance(650.0);
    // 78 is prevented: the stream ends with the last close.
    let end = Instant::now() + BOUND;
    let mut seen = taken(&core);
    while seen.len() < 2 && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(5));
        seen.extend(taken(&core));
    }
    assert_eq!(events(&seen), ["exit 78 null", "end 78 null true"]);
}

#[test]
fn stdin_queues_and_drain_follows_once_it_is_empty() {
    let file = script(
        r#"{"incarnations":[{"pid":21,"steps":[{"at":10,"read":70000},{"at":20,"read":70000},{"at":30,"exit":0}]}]}"#,
    );
    let (core, _woke) = setup(&[("path:t3", &format!("script:{file}"))]);
    core.run(
        request(1, r#"{"kind":"spawn","program":"path:t3"}"#, true),
        None,
    )
    .unwrap();
    waiting(&core);
    let clock = clock(&core);
    clock.advance(0.0);
    assert_eq!(events(&taken(&core)), ["spawn 21 0"]);
    // The handle answered `false`: past 64 KiB.
    let write = |n: usize| {
        ops(vec![(
            1,
            ProcessOp::Write {
                data: vec![b'x'; n],
                drain: true,
            },
        )])
    };
    let sent = core.socket(&write(100_000)).unwrap();
    assert_eq!(sent, vec![(1, 1, 100_000)]);
    assert_eq!(
        core.socket_reader()(1),
        Some(100_000),
        "writableLength's start"
    );
    // Past 1 MiB is refused whole.
    let over = core.socket(&write(1 << 20)).unwrap_err();
    assert!(over.contains("QuotaExceededError"), "{over}");
    // 30,000 bytes left: below 64 KiB, and no drain until it is empty (Node's).
    clock.advance(10.0);
    assert!(events(&taken(&core)).is_empty());
    assert_eq!(core.socket_reader()(1), Some(30_000));
    clock.advance(20.0);
    assert_eq!(events(&taken(&core)), ["drain"]);
    assert_eq!(core.socket_reader()(1), Some(0));
    // Stale incarnation, then end and a write after it.
    assert!(core
        .socket(&ops(vec![(2, ProcessOp::End)]))
        .unwrap_err()
        .contains("incarnation"));
    core.socket(&ops(vec![(1, ProcessOp::End)])).unwrap();
    assert!(core.socket(&write(1)).unwrap_err().contains("ended"));
    clock.advance(30.0);
    let seen = events(&taken(&core));
    assert!(seen.contains(&"exit 0 null".to_string()), "{seen:?}");
}

#[test]
fn only_a_write_that_answered_false_hears_drain() {
    let file = script(
        r#"{"incarnations":[{"pid":22,"steps":[{"at":10,"read":1000},{"at":20,"read":70000},{"at":30,"exit":0}]}]}"#,
    );
    let (core, _woke) = setup(&[("path:t3", &format!("script:{file}"))]);
    core.run(
        request(1, r#"{"kind":"spawn","program":"path:t3"}"#, true),
        None,
    )
    .unwrap();
    waiting(&core);
    let clock = clock(&core);
    clock.advance(0.0);
    taken(&core);
    let write = |n: usize, drain: bool| {
        ops(vec![(
            1,
            ProcessOp::Write {
                data: vec![0; n],
                drain,
            },
        )])
    };
    core.socket(&write(1000, false)).unwrap();
    // The supervisor sees the write on its next turn: move the clock by nothing.
    clock.advance(1.0);
    clock.advance(10.0);
    assert!(events(&taken(&core)).is_empty(), "it answered true");
    core.socket(&write(70_000, true)).unwrap();
    clock.advance(11.0);
    clock.advance(20.0);
    assert_eq!(events(&taken(&core)), ["drain"]);
}

#[test]
fn a_first_login_capture_needs_the_apps_origin() {
    let (core, woke) = setup(&[]);
    let login = |ticket, origin| {
        let mut out = request(ticket, r#"{"kind":"loginEnv"}"#, false);
        out.request.reach.origin = origin;
        core.run(out, None).unwrap();
        next(&core, &woke)
    };
    match login(1, CallOrigin::Tool) {
        Outcome::Failed { message, .. } => {
            assert!(message.starts_with("person-required"), "{message}")
        }
        other => panic!("{other:?}"),
    }
    // Once the app's origin has captured it, any origin reads it.
    assert!(matches!(login(2, CallOrigin::App), Outcome::Response(_)));
    assert!(matches!(login(3, CallOrigin::Tool), Outcome::Response(_)));
}

#[test]
fn a_start_from_another_origin_is_refused_by_the_host_too() {
    let file = script(r#"{"incarnations":[{"pid":51,"steps":[]}]}"#);
    let (core, woke) = setup(&[("path:t3", &format!("script:{file}"))]);
    for (ticket, kind) in [(1, "spawn"), (2, "execFile")] {
        let body = format!(r#"{{"kind":"{kind}","program":"path:t3"}}"#);
        let mut out = request(ticket, &body, kind == "spawn");
        out.request.reach.origin = CallOrigin::Tool;
        core.run(out, None).unwrap();
        match next(&core, &woke) {
            Outcome::Failed { message, .. } => {
                assert!(message.starts_with("person-required"), "{kind}: {message}")
            }
            other => panic!("{kind}: {other:?}"),
        }
    }
}

#[test]
fn every_kind_the_runner_declares_is_one_the_host_runs() {
    let (core, woke) = setup(&[]);
    for (ticket, (kind, _)) in (1..).zip(exact_runner::process::KINDS) {
        let body = format!(r#"{{"kind":"{kind}","program":"path:t3"}}"#);
        core.run(request(ticket, &body, kind == "spawn"), None)
            .unwrap();
        if let Outcome::Failed { message, .. } = next(&core, &woke) {
            assert!(!message.contains("no call"), "{kind}: {message}");
        }
    }
    core.run(request(9, r#"{"kind":"shell"}"#, false), None)
        .unwrap();
    match next(&core, &woke) {
        Outcome::Failed { message, .. } => assert!(message.contains("no call"), "{message}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn release_signals_the_group_and_escalates_after_the_grace() {
    let file = script(
        r#"{"incarnations":[{"pid":41,"ignore":["SIGTERM"],"steps":[{"at":0,"stdout":"up"}]}]}"#,
    );
    let (core, _woke) = setup(&[("path:t3", &format!("script:{file}"))]);
    let body = r#"{"kind":"spawn","program":"path:t3","encoding":"utf8","forceKillAfter":2000}"#;
    core.run(request(1, body, true), None).unwrap();
    waiting(&core);
    let clock = clock(&core);
    clock.advance(0.0);
    assert_eq!(events(&taken(&core)), ["spawn 41 0", "stdout up"]);
    core.forget(|_| false);
    // The release reaches the supervisor on the wall; its escalation then
    // waits out the grace.
    let deadline = Instant::now() + BOUND;
    while clock.due() != Some(2000.0) {
        assert!(Instant::now() < deadline, "{:?}", clock.due());
        std::thread::sleep(Duration::from_millis(2));
    }
    clock.advance(2000.0);
    let deadline = Instant::now() + BOUND;
    while !core.shared.state.lock().unwrap().running.is_empty() {
        assert!(Instant::now() < deadline, "the stream never ended");
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(
        taken(&core).is_empty(),
        "a released child's later events are dropped"
    );
}

#[test]
fn nothing_runs_under_the_agent_unless_the_drive_names_it() {
    let (core, woke) = setup(&[]);
    core.run(
        request(1, r#"{"kind":"spawn","program":"path:t3"}"#, true),
        None,
    )
    .unwrap();
    match next(&core, &woke) {
        Outcome::Failed { message, .. } => {
            assert!(message.contains("--process path:t3="), "{message}")
        }
        other => panic!("{other:?}"),
    }
    core.run(
        request(2, r#"{"kind":"spawn","program":"path:ssh"}"#, true),
        None,
    )
    .unwrap();
    match next(&core, &woke) {
        Outcome::Failed { message, .. } => {
            assert_eq!(message, "outside the app's grants (process.spawn path:ssh)")
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn one_shots_answer_from_the_script_and_the_drive() {
    let file = script(
        r#"{"execFile":{"stdout":"1.2.0\n","stderr":"","code":0,"after":0},"path":"/drive/t3"}"#,
    );
    let (core, woke) = setup(&[("path:t3", &format!("script:{file}"))]);
    let body = |r: &Outcome| match r {
        Outcome::Response(r) => serde_json::from_slice::<serde_json::Value>(&r.body).unwrap(),
        other => panic!("{other:?}"),
    };
    core.run(
        request(
            1,
            r#"{"kind":"execFile","program":"path:t3","encoding":"utf8"}"#,
            false,
        ),
        None,
    )
    .unwrap();
    let r = body(&next(&core, &woke));
    assert_eq!(
        (r["stdout"].as_str(), r["code"].as_i64()),
        (Some("1.2.0\n"), Some(0))
    );
    core.run(
        request(2, r#"{"kind":"execPath","program":"path:t3"}"#, false),
        None,
    )
    .unwrap();
    assert_eq!(body(&next(&core, &woke))["path"], "/drive/t3");
    core.run(request(3, r#"{"kind":"loginEnv"}"#, false), None)
        .unwrap();
    assert_eq!(
        body(&next(&core, &woke)),
        serde_json::json!({ "PATH": "/drive/bin" })
    );
}

/// Wait until no stream runs.
fn ended(core: &Core) {
    let deadline = Instant::now() + BOUND;
    while !core.shared.state.lock().unwrap().running.is_empty()
        || !core.shared.processes.children.lock().unwrap().is_empty()
    {
        assert!(Instant::now() < deadline, "the stream never ended");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn a_child_let_go_before_its_first_event_is_released() {
    let file = script(
        r#"{"incarnations":[{"pid":51,"ignore":["SIGTERM"],"steps":[{"at":0,"stdout":"up"}]}]}"#,
    );
    let (core, _woke) = setup(&[("path:t3", &format!("script:{file}"))]);
    let body = r#"{"kind":"spawn","program":"path:t3","forceKillAfter":2000}"#;
    core.run(request(1, body, true), None).unwrap();
    // Forgotten at once: its `spawn` finds nobody to take it.
    core.forget(|_| false);
    let clock = clock(&core);
    let deadline = Instant::now() + BOUND;
    while clock.due() != Some(2000.0) {
        assert!(Instant::now() < deadline, "the group was never signalled");
        std::thread::sleep(Duration::from_millis(2));
    }
    clock.advance(2000.0);
    ended(&core);
}

#[test]
fn an_apps_abort_ends_a_one_shots_child() {
    let file = script(r#"{"execFile":{"stdout":"","stderr":"","code":0,"after":5000}}"#);
    let (core, _woke) = setup(&[("path:t3", &format!("script:{file}"))]);
    let body = r#"{"kind":"execFile","program":"path:t3"}"#;
    core.run(request(1, body, false), None).unwrap();
    // Its `signal` (`Core::cancel`), with the agent's clock still at 0.
    core.cancel(&[1]);
    ended(&core);
}

#[test]
fn on_failure_restarts_no_child_the_app_signalled() {
    let body = r#"{"kind":"spawn","program":"path:t3","encoding":"utf8",
        "restart":{"when":"on-failure","delay":100}}"#;
    let run = |json: &str, kill: bool| {
        let file = script(json);
        let (core, _woke) = setup(&[("path:t3", &format!("script:{file}"))]);
        core.run(request(1, body, true), None).unwrap();
        waiting(&core);
        let clock = clock(&core);
        clock.advance(0.0);
        let mut seen = events(&taken(&core));
        if kill {
            let sigint = ProcessOp::Kill("SIGINT".into());
            core.socket(&ops(vec![(1, sigint)])).unwrap();
        }
        for t in [1.0, 10.0, 110.0, 111.0] {
            clock.advance(t);
            seen.extend(events(&taken(&core)));
        }
        // The app's `kill` reaches the supervisor on the wall, not at an
        // advance: wait for the stream's end.
        let deadline = Instant::now() + BOUND;
        while kill && !seen.iter().any(|e| e.starts_with("end ")) {
            assert!(Instant::now() < deadline, "{seen:?}");
            std::thread::sleep(Duration::from_millis(5));
            seen.extend(events(&taken(&core)));
        }
        seen
    };
    // The app's own `kill`: the stream ends at the close.
    let seen = run(
        r#"{"incarnations":[{"pid":61,"steps":[]},{"pid":62,"steps":[]}]}"#,
        true,
    );
    assert_eq!(
        seen,
        [
            "spawn 61 0",
            "exit null \"SIGINT\"",
            "end null \"SIGINT\" true"
        ]
    );
    // A signal it did not send is a failure: it restarts.
    let seen = run(
        r#"{"incarnations":[{"pid":63,"steps":[{"at":10,"signal":"SIGSEGV"}]},{"pid":64,"steps":[]}]}"#,
        false,
    );
    assert_eq!(
        seen,
        [
            "spawn 63 0",
            "exit null \"SIGSEGV\"",
            "close null true",
            "spawn 64 1"
        ]
    );
}

#[test]
fn an_operation_for_an_ended_incarnation_reaches_no_later_one() {
    let file = script(
        r#"{"incarnations":[
            {"pid":71,"steps":[{"at":0,"exit":1}]},
            {"pid":72,"steps":[{"at":0,"stdout":"two"},{"at":10,"read":70000}]}]}"#,
    );
    let (core, _woke) = setup(&[("path:t3", &format!("script:{file}"))]);
    let body = r#"{"kind":"spawn","program":"path:t3","encoding":"utf8",
        "restart":{"when":"always","delay":0}}"#;
    core.run(request(1, body, true), None).unwrap();
    waiting(&core);
    let clock = clock(&core);
    clock.advance(0.0);
    let deadline = Instant::now() + BOUND;
    let mut seen = Vec::new();
    // The restart waits the 100 ms floor, though the delay says 0.
    clock.advance(99.0);
    assert!(!events(&taken(&core)).contains(&"spawn 72 1".to_string()));
    while !seen.iter().any(|e: &String| e == "stdout two") {
        assert!(Instant::now() < deadline, "{seen:?}");
        clock.advance(100.0);
        seen.extend(events(&taken(&core)));
    }
    // Checked while the first ran, sent after the second began: dropped.
    let stale = [
        SocketOp::Process {
            incarnation: 1,
            op: ProcessOp::End,
        },
        SocketOp::Process {
            incarnation: 1,
            op: ProcessOp::Write {
                data: vec![0; 500],
                drain: false,
            },
        },
    ];
    let sent = super::send(&core.shared, &[(1, &stale[0]), (1, &stale[1])]);
    assert!(sent.is_empty(), "{sent:?}");
    assert_eq!(core.socket_reader()(1), Some(0));
    // The second's stdin is open, and counts what it was sent until read.
    let write = |n: usize| {
        ops(vec![(
            2,
            ProcessOp::Write {
                data: vec![b'x'; n],
                drain: false,
            },
        )])
    };
    core.socket(&write(100_000)).unwrap();
    for t in [101.0, 102.0, 103.0] {
        clock.advance(t);
        assert_eq!(core.socket_reader()(1), Some(100_000));
    }
    core.socket(&write(50_000)).unwrap();
    assert_eq!(core.socket_reader()(1), Some(150_000));
    clock.advance(110.0);
    let deadline = Instant::now() + BOUND;
    while core.socket_reader()(1) != Some(80_000) {
        assert!(Instant::now() < deadline, "{:?}", core.socket_reader()(1));
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Outcomes, on the wall, until one is a stream's or a one-shot's last.
fn to_the_end(core: &Core) -> Vec<Outcome> {
    let deadline = Instant::now() + BOUND;
    let mut seen = taken(core);
    while !seen.iter().any(|o| !matches!(o, Outcome::Message(_))) {
        assert!(Instant::now() < deadline, "{:?}", events(&seen));
        std::thread::sleep(Duration::from_millis(2));
        seen.extend(taken(core));
    }
    seen
}

#[test]
fn a_release_ends_a_child_its_signal_ends_without_waiting_out_the_grace() {
    // No `ignore`: the release's SIGTERM ends the script at once.
    let file = script(
        r#"{"incarnations":[{"pid":81,"steps":[{"at":0,"stdout":"up"},{"at":10000,"exit":0}]}]}"#,
    );
    let (core, _woke) = setup(&[("path:t3", &format!("script:{file}"))]);
    let body = r#"{"kind":"spawn","program":"path:t3","forceKillAfter":5000}"#;
    core.run(request(1, body, true), None).unwrap();
    waiting(&core);
    clock(&core).advance(0.0);
    taken(&core);
    core.forget(|_| false);
    // The agent's clock stays at 0: neither the script's exit nor the grace comes.
    ended(&core);
}

#[test]
fn readiness_that_fails_ends_the_stream_as_a_timeout() {
    let file = script(r#"{"incarnations":[{"pid":82,"steps":[{"at":0,"stdout":"up"}]}]}"#);
    let (core, _woke) = setup(&[("path:t3", &format!("script:{file}"))]);
    let body = r#"{"kind":"spawn","program":"path:t3","encoding":"utf8",
        "ready":{"url":"http://127.0.0.1/ready","interval":50,"timeout":100,"onTimeout":"fail"}}"#;
    core.run(request(1, body, true), None).unwrap();
    waiting(&core);
    let clock = clock(&core);
    clock.advance(0.0);
    assert_eq!(events(&taken(&core)), ["spawn 82 0", "stdout up"]);
    clock.advance(50.0);
    clock.advance(100.0);
    // `ready-timeout`, then the release (whose events a stream never hears,
    // D2) and the stream's end as D4 says, with no wait for the grace.
    let seen = events(&to_the_end(&core));
    assert_eq!(seen.len(), 2, "{seen:?}");
    assert_eq!(seen[0], "ready-timeout");
    assert!(seen[1].starts_with("failed Timeout"), "{seen:?}");
    ended(&core);
}

#[test]
fn readiness_that_retries_says_so_and_goes_on() {
    let file =
        script(r#"{"incarnations":[{"pid":83,"ready":250,"steps":[{"at":0,"stdout":"up"}]}]}"#);
    let (core, _woke) = setup(&[("path:t3", &format!("script:{file}"))]);
    let body = r#"{"kind":"spawn","program":"path:t3","encoding":"utf8",
        "ready":{"url":"http://127.0.0.1/ready","interval":50,"timeout":100}}"#;
    core.run(request(1, body, true), None).unwrap();
    waiting(&core);
    let clock = clock(&core);
    clock.advance(0.0);
    taken(&core);
    clock.advance(100.0);
    assert_eq!(events(&taken(&core)), ["ready-timeout"]);
    clock.advance(250.0);
    assert_eq!(events(&taken(&core)), ["ready"]);
}

#[test]
fn an_exec_files_signal_deadline_ends_it_as_a_timeout() {
    let file = script(r#"{"execFile":{"stdout":"","stderr":"","code":0,"after":5000}}"#);
    let (core, woke) = setup(&[("path:t3", &format!("script:{file}"))]);
    let mut out = request(1, r#"{"kind":"execFile","program":"path:t3"}"#, false);
    // What `AbortSignal.timeout(100)` gives a request (js/src/prelude.js).
    out.request.timeout_ms = Some(100);
    core.run(out, None).unwrap();
    let clock = clock(&core);
    let deadline = Instant::now() + BOUND;
    while clock.due() != Some(100.0) {
        assert!(Instant::now() < deadline, "the deadline was never armed");
        std::thread::sleep(Duration::from_millis(2));
    }
    clock.advance(100.0);
    match next(&core, &woke) {
        Outcome::Failed { kind, .. } => assert_eq!(kind, FailureKind::Timeout),
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_host_refuses_what_the_binding_never_sends() {
    let (core, woke) = setup(&[("path:t3", "script:/nowhere.json")]);
    let refused = [
        (r#"{"kind":"spawn","program":"path:t3","shell":true}"#, true),
        (
            r#"{"kind":"execFile","program":"path:t3","maxBuffer":0}"#,
            false,
        ),
        (
            r#"{"kind":"execFile","program":"path:t3","input":"!!"}"#,
            false,
        ),
        (
            r#"{"kind":"execFile","program":"path:t3","timeout":-1}"#,
            false,
        ),
        (
            r#"{"kind":"spawn","program":"path:t3","killSignal":"SIGSEGV"}"#,
            true,
        ),
        (
            r#"{"kind":"spawn","program":"path:t3","restart":{"when":"sometimes"}}"#,
            true,
        ),
        (
            r#"{"kind":"spawn","program":"path:t3","stdio":["inherit"]}"#,
            true,
        ),
        // A spawn with no stream's place, and a one-shot as a stream.
        (r#"{"kind":"spawn","program":"path:t3"}"#, false),
        (r#"{"kind":"execFile","program":"path:t3"}"#, true),
    ];
    for (ticket, (body, stream)) in (1..).zip(refused) {
        core.run(request(ticket, body, stream), None).unwrap();
        match next(&core, &woke) {
            Outcome::Failed { kind, message } => {
                assert_eq!(kind, FailureKind::Refused, "{body}");
                assert!(message.starts_with("process"), "{body}: {message}");
            }
            other => panic!("{body}: {other:?}"),
        }
    }
}

#[test]
fn one_shots_have_room_of_their_own() {
    let file = script(r#"{"execFile":{"stdout":"","stderr":"","code":0,"after":5000}}"#);
    let (core, _woke) = setup(&[("path:t3", &format!("script:{file}"))]);
    let body = r#"{"kind":"execFile","program":"path:t3"}"#;
    for ticket in 1..=16 {
        core.run(request(ticket, body, false), None).unwrap();
    }
    // None holds a count or bytes of the lane the app's other requests use.
    let lane = |core: &Core| {
        let state = core.shared.state.lock().unwrap();
        (state.counts[1], state.bytes[1])
    };
    assert_eq!(lane(&core), (0, 0));
    let refused = core.run(request(17, body, false), None).unwrap_err();
    assert!(refused.contains("process limit"), "{refused}");
    core.forget(|_| false);
    ended(&core);
    assert_eq!(lane(&core), (0, 0));
    core.run(request(18, body, false), None).unwrap();
}

#[test]
fn a_spawn_without_readiness_makes_no_network_host() {
    // Only `exactReady`'s probes use one. Counted on the stream's own
    // thread: the lane's workers, which start with the first request, make
    // theirs.
    let file = script(r#"{"incarnations":[{"pid":91,"steps":[]}]}"#);
    let made = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let host: super::super::StreamHost = {
        let made = made.clone();
        std::sync::Arc::new(move || {
            let name = std::thread::current().name().map(str::to_string);
            if name.is_some_and(|n| n.starts_with("exact-stream-")) {
                made.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            ibex2::host::Host::new()
        })
    };
    let (wake, _woke) = channel::<()>();
    let wake = Box::new(move || {
        let _ = wake.send(());
    });
    let core = Core::under_agent(&[("path:t3", &format!("script:{file}"))], || {
        Core::start_on(None, GRANTS, wake, host)
    });
    let body = r#"{"kind":"spawn","program":"path:t3"}"#;
    core.run(request(1, body, true), None).unwrap();
    waiting(&core);
    assert_eq!(made.load(std::sync::atomic::Ordering::Relaxed), 0);
}
