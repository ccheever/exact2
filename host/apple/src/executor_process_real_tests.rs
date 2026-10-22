//! Real children (LLP 1133.007 D9): `/bin/sh` started in a group of its
//! own, its stdin and output, a grandchild that the release and the quit
//! end with it, a leader that exits first, the record a crash leaves and the
//! next launch's reaping, `execFile`'s limits, and a program's grant checked
//! once its links are followed.
use super::super::super::Core;
use exact_runner::{
    Outcome, ProcessOp, Request, RequestOut, SocketOp, SocketOrigin, SocketSend, TurnOrigin,
};
use ibex2::process as os;
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

fn core(grants: &str) -> (Core, Receiver<()>, std::path::PathBuf) {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    core_in(
        grants,
        std::env::temp_dir().join(format!("exact-real-{}-{n}", std::process::id())),
    )
}

/// A core whose app directories are under `dir`: a relaunch names the same.
fn core_in(grants: &str, dir: std::path::PathBuf) -> (Core, Receiver<()>, std::path::PathBuf) {
    let (wake, woke) = channel();
    let core = Core::start(
        None,
        grants,
        Box::new(move || {
            let _ = wake.send(());
        }),
    );
    core.allow_processes();
    let roots = ["data", "cache", "tmp"].map(|d| dir.join(d));
    for root in &roots {
        std::fs::create_dir_all(root).unwrap();
    }
    core.set_app_roots(roots);
    (core, woke, dir)
}

fn request(ticket: u64, body: serde_json::Value, stream: bool) -> RequestOut {
    RequestOut {
        ticket,
        target: "server".into(),
        request: Request::process(body.to_string().into_bytes(), stream, 1 << 20),
        forced: false,
    }
}

/// Outcomes, on the wall, until `until` says they are enough (or 10 s).
fn collect(core: &Core, woke: &Receiver<()>, until: impl Fn(&[Outcome]) -> bool) -> Vec<Outcome> {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut out = Vec::new();
    while !until(&out) {
        core.begin_pump();
        match core.drain().pop() {
            Some(done) => out.push(done.1),
            None => {
                let left = deadline.saturating_duration_since(Instant::now());
                assert!(!left.is_zero(), "outcomes so far: {out:?}");
                let _ = woke.recv_timeout(left.min(Duration::from_millis(50)));
            }
        }
    }
    out
}

fn event(o: &Outcome) -> String {
    match o {
        Outcome::Message(m) if m.event == "stdout" || m.event == "stderr" => {
            format!("{} {}", m.event, m.data)
        }
        Outcome::Message(m) => {
            let d: serde_json::Value = serde_json::from_str(&m.data).unwrap_or_default();
            match m.event.as_str() {
                "exit" => format!("exit {} {}", d["code"], d["signal"]),
                "close" => format!("close {} {}", d["code"], d["drained"]),
                other => other.to_string(),
            }
        }
        Outcome::Closed { reason, .. } => {
            let d: serde_json::Value = serde_json::from_str(reason).unwrap();
            format!("end {} {} {}", d["code"], d["signal"], d["drained"])
        }
        Outcome::Failed { kind, message } => format!("failed {kind:?} {message}"),
        other => format!("{other:?}"),
    }
}

fn ended(outcomes: &[Outcome]) -> bool {
    outcomes.iter().any(|o| !matches!(o, Outcome::Message(_)))
}

fn spawned_pid(outcomes: &[Outcome]) -> i32 {
    let Outcome::Message(m) = &outcomes[0] else {
        panic!("{outcomes:?}")
    };
    assert_eq!(m.event, "spawn");
    let d: serde_json::Value = serde_json::from_str(&m.data).unwrap();
    assert_eq!(d["ran"], "real");
    d["pid"].as_i64().unwrap() as i32
}

fn write(core: &Core, op: ProcessOp) {
    let batch = SocketSend {
        origin: SocketOrigin::Mutation("send".into()),
        turn: TurnOrigin::Person,
        ops: vec![(1, "server".into(), SocketOp::Process { incarnation: 1, op })],
    };
    core.socket(&batch).unwrap();
}

fn sh(script: &str) -> serde_json::Value {
    serde_json::json!({ "kind": "spawn", "program": "/bin/sh", "args": ["-c", script], "encoding": "utf8", "forceKillAfter": 500, "drainTimeout": 300 })
}

#[test]
fn a_real_child_reads_its_stdin_and_streams_its_output_in_order() {
    let (core, woke, _) = core("process.spawn /bin/sh");
    core.run(
        request(
            1,
            sh("read line; echo \"got $line\"; echo oops >&2; exit 3"),
            true,
        ),
        None,
    )
    .unwrap();
    let first = collect(&core, &woke, |o| !o.is_empty());
    assert!(spawned_pid(&first) > 1);
    write(&core, ProcessOp::Write(b"ping\n".to_vec()));
    write(&core, ProcessOp::End);
    let rest = collect(&core, &woke, ended);
    let seen: Vec<String> = rest.iter().map(event).collect();
    assert!(seen.contains(&"stdout got ping\n".to_string()), "{seen:?}");
    assert!(seen.contains(&"stderr oops\n".to_string()), "{seen:?}");
    assert!(seen.contains(&"exit 3 null".to_string()), "{seen:?}");
    assert_eq!(seen.last().unwrap(), "end 3 null true");
}

#[test]
fn release_ends_the_group_a_grandchild_included() {
    let (core, woke, _) = core("process.spawn /bin/sh");
    core.run(request(1, sh("sleep 30 & echo $!; wait"), true), None)
        .unwrap();
    let out = collect(&core, &woke, |o| o.len() >= 2);
    let leader = spawned_pid(&out);
    let grandchild: i32 = event(&out[1])
        .trim_start_matches("stdout ")
        .trim()
        .parse()
        .unwrap();
    assert!(os::group_members(leader).contains(&grandchild));
    core.forget(|_| false);
    let deadline = Instant::now() + Duration::from_secs(5);
    while os::group_alive(leader) {
        assert!(Instant::now() < deadline, "the group outlived its release");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn a_leader_that_exits_first_leaves_no_group_behind() {
    let (core, woke, _) = core("process.spawn /bin/sh");
    // The grandchild keeps the pipe after its leader exits: the drain runs
    // out, the incarnation closes undrained, and the group is ended.
    core.run(request(1, sh("sleep 30 & echo $!; exit 0"), true), None)
        .unwrap();
    let out = collect(&core, &woke, ended);
    let leader = spawned_pid(&out);
    let seen: Vec<String> = out.iter().map(event).collect();
    assert!(seen.contains(&"exit 0 null".to_string()), "{seen:?}");
    assert_eq!(seen.last().unwrap(), "end 0 null false", "{seen:?}");
    assert!(
        !os::group_alive(leader),
        "its grandchild was ended with the group"
    );
}

#[test]
fn a_crash_leaves_a_record_that_the_next_launch_reaps() {
    let (core, woke, dir) = core("process.spawn /bin/sh");
    core.run(request(1, sh("sleep 30 & wait"), true), None)
        .unwrap();
    let out = collect(&core, &woke, |o| !o.is_empty());
    let leader = spawned_pid(&out);
    let record = dir.join("cache/exact-children.json");
    let text = std::fs::read_to_string(&record).unwrap();
    assert!(text.contains(&format!("\"pgid\":{leader}")), "{text}");
    // A crash: the core is gone without its quit, the group left running.
    std::mem::forget(core);
    assert!(os::group_alive(leader));
    let (next, _woke, _) = core_in("process.spawn /bin/sh", dir.clone());
    let deadline = Instant::now() + Duration::from_secs(5);
    while os::group_alive(leader) {
        assert!(Instant::now() < deadline, "the next launch left the group");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(std::fs::read_to_string(&record).unwrap(), "[]");
    drop(next);
}

#[test]
fn a_quit_ends_every_group() {
    let (core, woke, _) = core("process.spawn /bin/sh\nfs.read /");
    core.run(request(1, sh("trap '' TERM; sleep 30 & wait"), true), None)
        .unwrap();
    let out = collect(&core, &woke, |o| !o.is_empty());
    let leader = spawned_pid(&out);
    let quit = Instant::now();
    drop(core);
    assert!(!os::group_alive(leader), "the quit ended the group");
    assert!(
        quit.elapsed() < Duration::from_secs(6),
        "a quit waits at most 5 s"
    );
}

#[test]
fn exec_file_keeps_its_limits() {
    let (core, woke, _) = core("process.spawn /bin/sh\nprocess.spawn path:sh");
    let body = |r: &Outcome| match r {
        Outcome::Response(r) => serde_json::from_slice::<serde_json::Value>(&r.body).unwrap(),
        other => panic!("{other:?}"),
    };
    let exec = |ticket, script: &str, more: serde_json::Value| {
        let mut j = serde_json::json!({ "kind": "execFile", "program": "/bin/sh", "args": ["-c", script], "encoding": "utf8" });
        j.as_object_mut()
            .unwrap()
            .extend(more.as_object().unwrap().clone());
        request(ticket, j, false)
    };
    core.run(
        exec(
            1,
            "cat; echo err >&2; exit 2",
            serde_json::json!({ "input": exact_runner::agent::base64(b"in\n") }),
        ),
        None,
    )
    .unwrap();
    let r = body(&collect(&core, &woke, |o| !o.is_empty())[0]);
    assert_eq!(
        (
            r["stdout"].as_str(),
            r["stderr"].as_str(),
            r["code"].as_i64()
        ),
        (Some("in\n"), Some("err\n"), Some(2))
    );
    core.run(
        exec(2, "sleep 5", serde_json::json!({ "timeout": 200 })),
        None,
    )
    .unwrap();
    let r = body(&collect(&core, &woke, |o| !o.is_empty())[0]);
    assert_eq!(
        (r["timedOut"].as_bool(), r["signal"].as_str()),
        (Some(true), Some("SIGTERM"))
    );
    core.run(
        exec(
            3,
            "yes | head -c 100000",
            serde_json::json!({ "maxBuffer": 1000 }),
        ),
        None,
    )
    .unwrap();
    let r = body(&collect(&core, &woke, |o| !o.is_empty())[0]);
    assert_eq!(r["overflow"].as_str(), Some("stdout"));
    let path = |ticket, program: &str| {
        request(
            ticket,
            serde_json::json!({ "kind": "execPath", "program": program, "env": [["PATH", "/nonexistent:/bin"]] }),
            false,
        )
    };
    core.run(path(4, "path:sh"), None).unwrap();
    // A `path:` name is looked up on the call's own `env.PATH` when it gives one.
    assert_eq!(
        body(&collect(&core, &woke, |o| !o.is_empty())[0])["path"],
        "/bin/sh"
    );
}

#[test]
fn a_link_out_of_a_granted_directory_is_outside_its_grant() {
    let dir = std::env::temp_dir().join(format!("exact-real-link-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let link = dir.join("sh");
    let _ = std::fs::remove_file(&link);
    std::os::unix::fs::symlink("/bin/sh", &link).unwrap();
    let real = std::fs::canonicalize(&dir).unwrap();
    let (core, woke, _) = core(&format!("process.spawn {}", real.display()));
    let body = serde_json::json!({ "kind": "spawn", "program": format!("{}/sh", real.display()), "args": ["-c", "true"] });
    core.run(request(1, body, true), None).unwrap();
    let out = collect(&core, &woke, ended);
    assert!(
        event(&out[0]).contains("outside its grant once its links are followed"),
        "{out:?}"
    );
}

#[test]
fn the_login_shell_answers_the_named_variables_between_markers() {
    let shell = if cfg!(target_vendor = "apple") {
        "/bin/zsh"
    } else {
        "/bin/sh"
    };
    let found = super::capture_with(
        &[(std::path::PathBuf::from(shell), "-lc")],
        &["PATH", "EXACT_NOT_SET_ANYWHERE"],
        std::path::Path::new("/"),
    );
    assert!(
        found.get("PATH").is_some_and(|p| p.contains('/')),
        "{found:?}"
    );
    assert!(!found.contains_key("EXACT_NOT_SET_ANYWHERE"));
}
