//! Real children (LLP 1133.007 D9): `/bin/sh` started in a group of its
//! own, its stdin and output, a grandchild that the release and the quit
//! end with it, a leader that exits first, the record a crash leaves and the
//! next launch's reaping, `execFile`'s limits, and a program's grant checked
//! once its links are followed.
use super::super::super::Core;
use exact_runner::{Outcome, ProcessOp, Request, RequestOut, SocketOp, SocketOrigin, SocketSend};
use ibex2::process as os;
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

/// A wall-clock wait's limit, which only stops a hanging test (docs/testing.md,
/// "Waiting"). A child a test waits to see ended sleeps longer than this, so
/// one that is never ended fails the wait.
const BOUND: Duration = Duration::from_secs(30);

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

/// Outcomes, on the wall, until `until` says they are enough.
fn collect(core: &Core, woke: &Receiver<()>, until: impl Fn(&[Outcome]) -> bool) -> Vec<Outcome> {
    let deadline = Instant::now() + BOUND;
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
    write(
        &core,
        ProcessOp::Write {
            data: b"ping\n".to_vec(),
            drain: false,
        },
    );
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
    core.run(request(1, sh("sleep 120 & echo $!; wait"), true), None)
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
    until_gone(leader, "the group outlived its release");
}

#[test]
fn a_leader_that_exits_first_leaves_no_group_behind() {
    let (core, woke, _) = core("process.spawn /bin/sh");
    // The grandchild keeps the pipe after its leader exits: the drain runs
    // out, the incarnation closes undrained, and the group is ended.
    core.run(request(1, sh("sleep 120 & echo $!; exit 0"), true), None)
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

/// Rewrite every entry of the record at `path` with `change`.
fn edit_record(path: &std::path::Path, change: impl Fn(&mut serde_json::Value)) {
    let mut entries: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    entries.iter_mut().for_each(change);
    std::fs::write(path, serde_json::Value::Array(entries).to_string()).unwrap();
}

/// A pid that no process has now: a child already reaped.
fn dead_pid() -> i64 {
    let mut child = std::process::Command::new("/usr/bin/true").spawn().unwrap();
    let pid = child.id();
    child.wait().unwrap();
    i64::from(pid)
}

fn until_gone(pgid: i32, what: &str) {
    let deadline = Instant::now() + BOUND;
    while os::group_alive(pgid) {
        assert!(Instant::now() < deadline, "{what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn a_crash_leaves_a_record_that_the_next_launch_reaps() {
    let (core, woke, dir) = core("process.spawn /bin/sh");
    core.run(request(1, sh("sleep 120 & wait"), true), None)
        .unwrap();
    let out = collect(&core, &woke, |o| !o.is_empty());
    let leader = spawned_pid(&out);
    let record = dir.join("cache/exact-children.json");
    let text = std::fs::read_to_string(&record).unwrap();
    assert!(text.contains(&format!("\"pgid\":{leader}")), "{text}");
    // Another executor of the app in this live process (a plan swap, a
    // second window) keeps the first one's child.
    let (other, _woke, _) = core_in("process.spawn /bin/sh", dir.clone());
    super::super::groups::reap(&other.shared);
    assert!(
        os::group_alive(leader),
        "a live recorder's group is its own"
    );
    // A crash: the recorder is gone, the group left running.
    std::mem::forget(core);
    let dead = dead_pid();
    edit_record(&record, |e| e["recorder"] = dead.into());
    super::super::groups::reap(&other.shared);
    until_gone(leader, "the next launch left the group");
    assert_eq!(std::fs::read_to_string(&record).unwrap(), "[]");
    drop(other);
}

#[test]
fn a_stale_record_ends_no_group_that_is_not_the_apps() {
    let (core, _woke, dir) = core("process.spawn /bin/sh");
    // Groups this test starts, alive and nobody's child of the app: one
    // whose leader lives, one whose leader has exited and left a member.
    let start = |script: &str| {
        let args = vec!["-c".to_string(), script.to_string()];
        let child = os::spawn(os::Command {
            arg0: None,
            program: std::path::Path::new("/bin/sh"),
            args: &args,
            env: &[],
            cwd: std::path::Path::new("/"),
            stdin: os::Stdio::Null,
            stdout: os::Stdio::Null,
            stderr: os::Stdio::Null,
            session: false,
        })
        .unwrap();
        child.pid
    };
    let led = start("sleep 30 & wait");
    let orphaned = start("sleep 30 & exit 0");
    let _ = os::wait(orphaned);
    std::thread::sleep(Duration::from_millis(50));
    assert!(os::group_alive(orphaned), "the member outlives its leader");
    let record = dir.join("cache/exact-children.json");
    let dead = dead_pid();
    let reap = |pgid: i32, start: Option<u64>, session: i64| {
        let entry = serde_json::json!({ "pgid": pgid, "start": start, "boot": os::boot_id(),
            "session": session, "recorder": dead, "recorderStart": 1 });
        std::fs::write(&record, serde_json::json!([entry]).to_string()).unwrap();
        super::super::groups::reap(&core.shared);
        assert_eq!(std::fs::read_to_string(&record).unwrap(), "[]");
    };
    let session = i64::from(os::session_of(led).unwrap());
    // A live leader that started at another time: a reused id.
    reap(led, Some(1), session);
    assert!(
        os::group_alive(led),
        "a reused group id's group is left alone"
    );
    // The leader gone: a session the recorder did not lead tells nothing,
    // nor does launchd's session 1, where GUI apps run.
    reap(orphaned, Some(0), session);
    assert!(
        os::group_alive(orphaned),
        "a shared session's group is left alone"
    );
    reap(orphaned, Some(0), 1);
    assert!(os::group_alive(orphaned), "session 1's group is left alone");
    for pgid in [led, orphaned] {
        os::signal_group(pgid, os::SIGKILL).unwrap();
    }
    let _ = os::wait(led);
}

#[test]
fn a_quit_ends_every_group() {
    let (core, woke, _) = core("process.spawn /bin/sh\nfs.read /");
    // A grace past the quit's 5 s: the quit's limit is what ends it.
    let mut body = sh("trap '' TERM; sleep 120 & wait");
    body["forceKillAfter"] = 60_000.into();
    core.run(request(1, body, true), None).unwrap();
    let out = collect(&core, &woke, |o| !o.is_empty());
    let leader = spawned_pid(&out);
    drop(core);
    // The caller's thread does not wait: the escalation runs off it, and
    // the group, which ignores the quit's signal, is alive when it returns.
    assert!(os::group_alive(leader), "the quit waited out the grace");
    until_gone(leader, "the quit left the group past its 5 s limit");
}

#[test]
fn exec_file_ends_what_its_child_left_running() {
    let (core, woke, _) = core("process.spawn /bin/sh");
    let body = serde_json::json!({ "kind": "execFile", "program": "/bin/sh",
        "args": ["-c", "sleep 30 >/dev/null 2>&1 & echo $!"], "encoding": "utf8" });
    core.run(request(1, body, false), None).unwrap();
    let out = collect(&core, &woke, |o| !o.is_empty());
    let Outcome::Response(r) = &out[0] else {
        panic!("{out:?}")
    };
    let reply: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
    let grandchild: i32 = reply["stdout"].as_str().unwrap().trim().parse().unwrap();
    assert!(
        os::start_time(grandchild).is_none(),
        "its group ended before the answer"
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
}

#[test]
fn a_program_the_app_wrote_does_not_run_by_its_name() {
    let (core, woke, dir) = core("process.spawn path:sh");
    // The app writes its own `sh` under `app:/data/bin` and puts that
    // directory first on its child's `PATH`: `path:sh` is still the login
    // shell's `PATH`'s (D7), so `exit 7` runs, not the app's `exit 42`.
    let bin = dir.join("data/bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::write(bin.join("sh"), "#!/bin/sh\nexit 42\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(bin.join("sh"), std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!("{}:/usr/bin:/bin", bin.display());
    let call = serde_json::json!({ "kind": "execFile", "program": "path:sh", "args": ["-c", "exit 7"],
        "env": [["PATH", path]] });
    core.run(request(1, call, false), None).unwrap();
    let out = collect(&core, &woke, |o| !o.is_empty());
    let Outcome::Response(r) = &out[0] else {
        panic!("{out:?}")
    };
    let reply: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
    assert_eq!(reply["code"], 7, "{reply}");
}

#[test]
fn a_path_name_found_where_the_app_may_write_does_not_run() {
    // Where `path:sh` resolves on the login shell's `PATH`.
    let (probe, woke, _) = core("process.spawn path:sh");
    let ask = serde_json::json!({ "kind": "execPath", "program": "path:sh" });
    probe.run(request(1, ask.clone(), false), None).unwrap();
    let path = |out: &[Outcome]| match &out[0] {
        Outcome::Response(r) => serde_json::from_slice::<serde_json::Value>(&r.body).unwrap(),
        other => panic!("{other:?}"),
    };
    let found = path(&collect(&probe, &woke, |o| !o.is_empty()))["path"].clone();
    let dir = std::path::Path::new(found.as_str().unwrap())
        .parent()
        .unwrap()
        .to_owned();
    // An `fs.write` line over that directory lets the app write what the
    // name finds, so the name runs nothing there and resolves to nothing.
    let grants = format!("process.spawn path:sh\nfs.write {}", dir.display());
    let (covered, woke, _) = core(&grants);
    let call =
        serde_json::json!({ "kind": "execFile", "program": "path:sh", "args": ["-c", "exit 7"] });
    covered.run(request(1, call, false), None).unwrap();
    let out = collect(&covered, &woke, |o| !o.is_empty());
    assert!(
        matches!(&out[0], Outcome::Failed { message, .. } if message.contains("where the app may write")),
        "{out:?}"
    );
    covered.run(request(2, ask.clone(), false), None).unwrap();
    let out = collect(&covered, &woke, |o| !o.is_empty());
    assert_eq!(path(&out)["path"], serde_json::Value::Null);
    // A quoted line, its path with a space: here a link to that directory.
    let spaced = std::env::temp_dir().join(format!("exact real {}", std::process::id()));
    std::fs::create_dir_all(&spaced).unwrap();
    let link = spaced.join("bin dir");
    let _ = std::fs::remove_file(&link);
    std::os::unix::fs::symlink(&dir, &link).unwrap();
    let grants = format!("process.spawn path:sh\nfs.write \"{}\"", link.display());
    let (quoted, woke, _) = core(&grants);
    quoted.run(request(1, ask, false), None).unwrap();
    let out = collect(&quoted, &woke, |o| !o.is_empty());
    assert_eq!(path(&out)["path"], serde_json::Value::Null);
    std::fs::remove_dir_all(&spaced).unwrap();
}

#[test]
fn a_path_name_runs_the_file_it_checked_not_a_link_the_app_may_replace() {
    // A `PATH` entry through a link in a directory the app writes: the link
    // leads out of it now, but the app may point it at its own file later.
    let base = std::env::temp_dir().join(format!("exact-real-on-path-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (writes, person) = (base.join("writes"), base.join("person/bin"));
    std::fs::create_dir_all(&writes).unwrap();
    std::fs::create_dir_all(&person).unwrap();
    let prog = person.join("prog");
    std::fs::write(&prog, "#!/bin/sh\nexit 0\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&prog, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::os::unix::fs::symlink(base.join("person"), writes.join("link")).unwrap();
    let (core, _, _) = core("");
    let app = format!("fs.write {}", writes.display());
    let path = writes.join("link/bin").display().to_string();
    let ran = super::on_path(&core.shared, &app, "prog", &path).unwrap();
    assert_eq!(ran, std::fs::canonicalize(&prog).unwrap());
    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn an_fs_write_line_that_names_no_directory_here_refuses_a_path_name() {
    let (core, _, _) = core("");
    let out = super::on_path(&core.shared, "fs.write doc:/notes", "sh", "/bin");
    assert!(
        matches!(&out, Err(Outcome::Failed { message, .. }) if message.contains("names no directory here")),
        "{out:?}"
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
    // A name that is not a variable's never reaches the shell (D5).
    let marker = std::env::temp_dir().join(format!("exact-injected-{}", std::process::id()));
    let bad = format!("X;touch {}", marker.display());
    let found = super::capture_with(
        &[(std::path::PathBuf::from("/bin/sh"), "-lc")],
        &["PATH", &bad],
        std::path::Path::new("/"),
    );
    assert!(found.contains_key("PATH") && !found.contains_key(bad.as_str()));
    assert!(!marker.exists(), "the name ran as a command");
    // A background job holding the pipe: the answer comes at the exit,
    // not at the job's end or the limit.
    let started = Instant::now();
    let out = super::run(
        std::path::Path::new("/bin/sh"),
        &["-c".into(), "echo hi; sleep 120 &".into()],
        std::path::Path::new("/"),
        Duration::from_secs(60),
    );
    assert_eq!(out.as_deref(), Some("hi\n"));
    assert!(started.elapsed() < BOUND, "{:?}", started.elapsed());
}
