//! Real children (LLP 1133.007 D9), over ibex2's process transport: a
//! process group of the child's own, its output read by a thread per pipe
//! and handed to the supervisor a few chunks ahead at most (so a full queue
//! stops the reading and the child waits on its pipe), its stdin written by
//! a thread from a queue, and its exit reaped by another. Its group is
//! recorded for the quit and for the next launch after a crash
//! (`groups.rs`).
use super::supervise::{exec_reply, Child, Seen};
use super::{failed, ChildCtl, Input, Ran, Shared, Spec, CHUNK};
use exact_runner::{FailureKind, Outcome};
use ibex2::process as os;
use ibex2::stdlib::abort::AbortController;
use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Chunks a reader hands the supervisor before it waits for one to be
/// delivered.
const AHEAD: usize = 4;
/// The login shell's limits (D5), and `launchctl`'s.
const SHELL_LIMIT: Duration = Duration::from_secs(5);
const LAUNCHCTL_LIMIT: Duration = Duration::from_secs(2);

/// A namespace path's native path: `home:/` the person's home directory
/// (under the agent, the drive's `--home`, or an empty scratch directory),
/// `app:/data`, `app:/cache` and `app:/tmp` the app's directories,
/// `bundle:/` the app bundle's `Contents` on macOS; an absolute path itself.
pub(crate) fn native(shared: &Shared, path: &str) -> Result<PathBuf, String> {
    if let Some(rest) = path.strip_prefix("home:/") {
        return Ok(home(shared)?.join(rest));
    }
    if let Some(rest) = path.strip_prefix("app:/") {
        let roots = shared
            .root_paths
            .get()
            .ok_or("this host names no app directories")?;
        let (dir, rest) = rest.split_once('/').unwrap_or((rest, ""));
        let root = match dir {
            "data" => &roots[0],
            "cache" => &roots[1],
            "tmp" => &roots[2],
            _ => {
                return Err(format!(
                    "{path}: an app path is under app:/data, app:/cache or app:/tmp"
                ))
            }
        };
        return Ok(root.join(rest));
    }
    if let Some(rest) = path.strip_prefix("bundle:/") {
        return Ok(bundle()?.join(rest));
    }
    if path.starts_with('/') {
        return Ok(PathBuf::from(path));
    }
    Err(format!("{path}: not a path a process takes"))
}

/// The person's home directory, as this launch sees it.
pub(crate) fn home(shared: &Shared) -> Result<PathBuf, String> {
    super::home(&shared.processes).ok_or_else(|| "the home directory is unknown".to_string())
}

/// The password database's home directory.
pub(super) fn home_dir() -> Option<PathBuf> {
    os::home_dir()
}

/// The app bundle's `Contents`, read only (D6): where `host.macos.resources`
/// copied its trees. Linux bundles none yet.
pub(super) fn bundle() -> Result<PathBuf, String> {
    if !cfg!(target_vendor = "apple") {
        return Err(
            "bundle: paths wait for the Linux host to bundle resources (LLP 1133.007 D6)".into(),
        );
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let contents = exe
        .parent()
        .and_then(Path::parent)
        .filter(|c| c.ends_with("Contents"));
    contents.map(Path::to_path_buf).ok_or_else(|| {
        "bundle: needs the app as a bundle (a development executable has none)".into()
    })
}

/// The program a spawn names, as a native path: a `path:` name looked up on
/// the login shell's `PATH`, never on one the call writes (D7), and refused
/// where one of the app's `fs.write` lines (`app`) lets it write (§9); a
/// namespace path resolved; then checked again against the line that
/// admitted it, by whole names after realization, so a symlink under a
/// granted directory that leads outside it is outside it (D6, D7).
pub(crate) fn program(
    shared: &Shared,
    grants: &str,
    app: &str,
    spec: &Spec,
    ran: &Ran,
) -> Result<PathBuf, Outcome> {
    let refused = |why: String| failed(FailureKind::Refused, why);
    if let Ran::File(file) = ran {
        return Ok(PathBuf::from(file));
    }
    if let Some(name) = spec.program.strip_prefix("path:") {
        // A `PATH` data code writes could name a program the app itself
        // wrote under `app:/`; the login shell's is the person's.
        let path = super::login_variables(shared, &["PATH"])
            .remove("PATH")
            .unwrap_or_default();
        return on_path(shared, app, name, &path);
    }
    let path = native(shared, &spec.program).map_err(refused)?;
    let real = std::fs::canonicalize(&path).map_err(|e| {
        failed(
            FailureKind::NotFound,
            format!("spawn {}: {e}", spec.program),
        )
    })?;
    use exact_grants::process::{validate, SpawnTarget, SPAWN};
    let targets = (grants.lines().map(str::trim))
        .filter(|line| line.split_whitespace().next() == Some(SPAWN) && validate(line).is_ok())
        .filter_map(|line| SpawnTarget::parse(line.split_whitespace().nth(1)?).ok());
    let covered = targets.into_iter().any(|target| match target {
        SpawnTarget::Path(prefix) => {
            let spelt = prefix.to_string();
            prefix.covers(&spec.program)
                && native(shared, &spelt)
                    .and_then(|p| std::fs::canonicalize(p).map_err(|e| e.to_string()))
                    .is_ok_and(|granted| real.starts_with(granted))
        }
        SpawnTarget::Name(_) => false,
    });
    if !covered {
        return Err(refused(format!(
            "spawn {}: {} is outside its grant once its links are followed",
            spec.program,
            real.display()
        )));
    }
    Ok(real)
}

/// A `path:` name's program on `path`, as realized, refused where one of
/// the app's `fs.write` lines (`app`) lets it write (§9). What runs is the
/// file checked, as realized: a link on the way to it that the app could
/// replace later (a `PATH` entry through a directory it writes) is not read
/// again, at this start or a keep-alive restart.
fn on_path(shared: &Shared, app: &str, name: &str, path: &str) -> Result<PathBuf, Outcome> {
    let found = os::find_on_path(name, path).ok_or_else(|| {
        failed(
            FailureKind::NotFound,
            format!("spawn path:{name}: no executable `{name}` on the PATH"),
        )
    })?;
    let real = std::fs::canonicalize(&found)
        .map_err(|e| failed(FailureKind::NotFound, format!("spawn path:{name}: {e}")))?;
    match writable(shared, app, &found, &real) {
        Some(line) => Err(failed(
            FailureKind::Refused,
            format!(
                "spawn path:{name}: {} is where the app may write (`{line}`): a program there runs only by a `process.spawn` line that names its path (LLP 1133.007 §9)",
                found.display()
            ),
        )),
        None => Ok(real),
    }
}

/// Lets a reader run `AHEAD` chunks ahead of their delivery.
struct Credit {
    taken: Mutex<usize>,
    freed: Condvar,
}

impl Credit {
    fn take(&self, cancel: &AtomicBool) -> bool {
        let mut taken = self.taken.lock().unwrap();
        while *taken >= AHEAD {
            if cancel.load(Ordering::Acquire) {
                return false;
            }
            taken = self
                .freed
                .wait_timeout(taken, Duration::from_millis(50))
                .unwrap()
                .0;
        }
        *taken += 1;
        true
    }

    fn give(&self) {
        let mut taken = self.taken.lock().unwrap();
        *taken = taken.saturating_sub(1);
        self.freed.notify_all();
    }
}

/// A child's stdin: a queue a thread writes out.
struct Stdin {
    queue: Mutex<(VecDeque<Vec<u8>>, bool)>,
    more: Condvar,
    queued: AtomicU64,
}

/// One running incarnation.
struct RealChild {
    pid: i32,
    stdin: Option<Arc<Stdin>>,
    cancel: Arc<AtomicBool>,
    credit: Arc<Credit>,
    shared: Arc<Shared>,
}

impl Child for RealChild {
    fn pid(&self) -> u64 {
        self.pid as u64
    }
    fn write(&mut self, bytes: Vec<u8>) {
        if let Some(stdin) = &self.stdin {
            stdin.queued.fetch_add(bytes.len() as u64, Ordering::AcqRel);
            stdin.queue.lock().unwrap().0.push_back(bytes);
            stdin.more.notify_all();
        }
    }
    fn end(&mut self) {
        if let Some(stdin) = &self.stdin {
            stdin.queue.lock().unwrap().1 = true;
            stdin.more.notify_all();
        }
    }
    fn signal(&mut self, signal: &str) -> Vec<Seen> {
        if let Some(n) = os::signal_number(signal) {
            let _ = os::signal_group(self.pid, n);
        }
        Vec::new()
    }
    fn queued(&self) -> u64 {
        self.stdin
            .as_ref()
            .map_or(0, |s| s.queued.load(Ordering::Acquire))
    }
    fn group_alive(&self) -> bool {
        os::group_alive(self.pid)
    }
    fn close_pipes(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
    fn taken(&self) {
        self.credit.give();
    }
}

impl Drop for RealChild {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
        self.end();
        if !os::group_alive(self.pid) {
            super::groups::forget(&self.shared, self.pid);
        }
    }
}

/// The environment a child gets: its `env`, else the app's own.
fn environment(spec: &Spec) -> Vec<(String, String)> {
    spec.env.clone().unwrap_or_else(app_environment)
}

/// The app's own environment, less what is not UTF-8 (`std::env::vars`
/// would panic on it).
fn app_environment() -> Vec<(String, String)> {
    std::env::vars_os()
        .filter_map(|(k, v)| Some((k.into_string().ok()?, v.into_string().ok()?)))
        .collect()
}

/// The child's working directory: `cwd`, else `home:/`.
fn cwd(shared: &Shared, spec: &Spec) -> Result<PathBuf, Outcome> {
    let spelt = spec.cwd.as_deref().unwrap_or("home:/");
    native(shared, spelt).map_err(|why| failed(FailureKind::Refused, why))
}

/// Start the program as incarnation `n` of stream `ticket`, its readers,
/// writer and waiter reporting to its supervisor through `ctl`.
pub(super) fn spawn(
    shared: &Arc<Shared>,
    _ticket: u64,
    n: u32,
    spec: &Spec,
    _ran: &Ran,
    ctl: &Arc<ChildCtl>,
) -> Result<Box<dyn Child>, Outcome> {
    let program = spec
        .native
        .clone()
        .ok_or_else(|| failed(FailureKind::Refused, "spawn: no program"))?;
    let env = environment(spec);
    let cwd = cwd(shared, spec)?;
    let stdin = match (&spec.stdin_file, spec.pipes[0]) {
        // Admitted and opened with the call; each incarnation reads it from its start.
        (Some(file), _) => {
            use std::io::Seek;
            let mut file = file
                .try_clone()
                .map_err(|e| failed(FailureKind::Refused, format!("exactStdinFrom: {e}")))?;
            file.rewind()
                .map_err(|e| failed(FailureKind::Refused, format!("exactStdinFrom: {e}")))?;
            os::Stdio::File(file)
        }
        (None, true) => os::Stdio::Pipe,
        (None, false) => os::Stdio::Null,
    };
    let pipe = |on: bool| if on { os::Stdio::Pipe } else { os::Stdio::Null };
    // `argv[0]` is the program as named, as Node passes it: a `path:` name's
    // name, a path before its links are followed.
    let arg0 = match spec.program.strip_prefix("path:") {
        Some(name) => name.to_string(),
        None => native(shared, &spec.program)
            .unwrap_or_else(|_| program.clone())
            .display()
            .to_string(),
    };
    let child = os::spawn(os::Command {
        arg0: Some(&arg0),
        program: &program,
        args: &spec.args,
        env: &env,
        cwd: &cwd,
        stdin,
        stdout: pipe(spec.pipes[1]),
        stderr: pipe(spec.pipes[2]),
        session: false,
    })
    .map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => failed(
            FailureKind::NotFound,
            format!("spawn {}: {e}", spec.program),
        ),
        _ => failed(FailureKind::Refused, format!("spawn {}: {e}", spec.program)),
    })?;
    let pid = child.pid;
    super::groups::remember(shared, pid, &spec.kill_signal, spec.force_kill_after);
    let cancel = Arc::new(AtomicBool::new(false));
    let credit = Arc::new(Credit {
        taken: Mutex::new(0),
        freed: Condvar::new(),
    });
    for (fd, stderr) in [(child.stdout, false), (child.stderr, true)] {
        match fd {
            Some(fd) => read(fd, stderr, n, ctl.clone(), cancel.clone(), credit.clone()),
            None => ctl.send(Input::Child(n, Seen::Eof(Some(stderr)))),
        }
    }
    let stdin = child.stdin.map(|fd| write(fd, n, ctl.clone()));
    let waiter = ctl.clone();
    let _ = std::thread::Builder::new()
        .name(format!("exact-child-{pid}"))
        .spawn(move || {
            let seen = match os::wait(pid) {
                Ok(os::Exit::Code(code)) => Seen::Exit(Some(i64::from(code)), None),
                Ok(os::Exit::Signal(signal)) => Seen::Exit(None, Some(os::signal_name(signal))),
                Err(_) => Seen::Exit(None, None),
            };
            waiter.send(Input::Child(n, seen));
        });
    Ok(Box::new(RealChild {
        pid,
        stdin,
        cancel,
        credit,
        shared: shared.clone(),
    }))
}

/// Read one pipe to its end (or until cancelled): each chunk to the
/// supervisor, at most `AHEAD` undelivered, then its end of file.
fn read(
    fd: OwnedFd,
    stderr: bool,
    n: u32,
    ctl: Arc<ChildCtl>,
    cancel: Arc<AtomicBool>,
    credit: Arc<Credit>,
) {
    let _ = std::thread::Builder::new()
        .name("exact-child-pipe".into())
        .spawn(move || {
            let _ = os::set_nonblocking(fd.as_raw_fd());
            let mut file = std::fs::File::from(fd);
            let mut buf = vec![0u8; CHUNK];
            loop {
                if cancel.load(Ordering::Acquire) {
                    break;
                }
                if !os::readable(file.as_raw_fd(), 100) {
                    continue;
                }
                match file.read(&mut buf) {
                    Ok(0) => break,
                    Ok(len) => {
                        if !credit.take(&cancel) {
                            break;
                        }
                        ctl.send(Input::Child(n, Seen::Out(stderr, buf[..len].to_vec())));
                    }
                    Err(e)
                        if matches!(
                            e.kind(),
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                        ) => {}
                    Err(_) => break,
                }
            }
            ctl.send(Input::Child(n, Seen::Eof(Some(stderr))));
        });
}

/// Write a child's stdin from its queue; close it once ended and empty.
fn write(fd: OwnedFd, n: u32, ctl: Arc<ChildCtl>) -> Arc<Stdin> {
    let stdin = Arc::new(Stdin {
        queue: Mutex::new((VecDeque::new(), false)),
        more: Condvar::new(),
        queued: AtomicU64::new(0),
    });
    let queue = stdin.clone();
    let _ = std::thread::Builder::new()
        .name("exact-child-stdin".into())
        .spawn(move || {
            let mut file = std::fs::File::from(fd);
            loop {
                let next = {
                    let mut q = queue.queue.lock().unwrap();
                    loop {
                        if let Some(bytes) = q.0.pop_front() {
                            break Some(bytes);
                        }
                        if q.1 {
                            break None;
                        }
                        q = queue.more.wait(q).unwrap();
                    }
                };
                let Some(bytes) = next else {
                    break;
                };
                if file.write_all(&bytes).is_err() {
                    // The child closed its stdin: what is queued is dropped.
                    queue.queue.lock().unwrap().0.clear();
                    queue.queued.store(0, Ordering::Release);
                    ctl.send(Input::Child(n, Seen::Drained));
                    break;
                }
                let after = queue.queued.fetch_sub(bytes.len() as u64, Ordering::AcqRel)
                    - bytes.len() as u64;
                // Empty: the supervisor looks, and a write that answered
                // `false` hears `drain` (D2, as Node's Writable).
                if after == 0 {
                    ctl.send(Input::Child(n, Seen::Drained));
                }
            }
        });
    stdin
}

/// `execFile` on a real program (D3): to `close`, with `input` on stdin,
/// output kept to `maxBuffer` (past it the group is ended, as Node's is),
/// and the timeout on the supervision clock.
pub(super) fn exec(
    shared: &Arc<Shared>,
    spec: &Spec,
    ran: &Ran,
    abort: &AbortController,
) -> Outcome {
    let _ = ran;
    let (tx, rx) = std::sync::mpsc::channel();
    let ctl = Arc::new(ChildCtl::detached(tx));
    let mut child = match spawn(shared, 0, 1, spec, ran, &ctl) {
        Ok(child) => child,
        Err(outcome) => return outcome,
    };
    if let Some(input) = &spec.input {
        child.write(input.clone());
    }
    child.end();
    let clock = shared.processes.clock.clone();
    let tick = ctl.clone();
    let waiter = clock.join(Box::new(move || tick.send(Input::Tick)));
    let _leave = super::clock::Leave(clock.clone(), waiter);
    let start = clock.now();
    let deadline = spec.timeout.map(|t| start + t);
    // Its signal's deadline (an `AbortSignal.timeout`) ends it as an abort does.
    let cut = spec.deadline.map(|d| start + d);
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let (mut eof, mut exit) = ([false, false], None);
    let (mut timed_out, mut overflow, mut escalate) = (false, false, None::<f64>);
    let (mut exited_at, mut cut_off) = (None::<Instant>, false);
    loop {
        cut_off |= clock.passed(cut);
        if (abort.signal().aborted() || cut_off) && escalate.is_none() {
            child.signal(&spec.kill_signal);
            escalate = Some(clock.now() + 5000.0);
        }
        if !timed_out && clock.passed(deadline) {
            timed_out = true;
            child.signal(&spec.kill_signal);
            escalate = Some(clock.now() + 5000.0);
        }
        if escalate.is_some_and(|at| clock.passed(Some(at))) && child.group_alive() {
            child.signal("SIGKILL");
            escalate = None;
        }
        // Output past the drain after its exit is not waited for (D2).
        if exited_at
            .is_some_and(|at| at.elapsed() > Duration::from_millis(spec.drain_timeout as u64))
        {
            child.close_pipes();
            eof = [true, true];
        }
        if exit.is_some() && eof[0] && eof[1] {
            break;
        }
        let next = [
            deadline.filter(|_| !timed_out),
            cut.filter(|_| !cut_off),
            escalate,
        ]
        .into_iter()
        .flatten()
        .reduce(f64::min);
        clock.wait_for(waiter, next);
        let wait = clock.timeout(next).min(Duration::from_millis(250));
        match rx.recv_timeout(wait) {
            Ok(Input::Child(_, Seen::Out(err, bytes))) => {
                child.taken();
                let into = if err { &mut stderr } else { &mut stdout };
                into.extend_from_slice(&bytes);
                if into.len() > spec.max_buffer && !overflow {
                    overflow = true;
                    child.signal(&spec.kill_signal);
                    escalate = Some(clock.now() + 5000.0);
                }
            }
            Ok(Input::Child(_, Seen::Eof(Some(err)))) => eof[usize::from(err)] = true,
            Ok(Input::Child(_, Seen::Eof(None))) => eof = [true, true],
            Ok(Input::Child(_, Seen::Exit(code, signal))) => {
                exit = Some((code, signal));
                exited_at = Some(Instant::now());
            }
            _ => {}
        }
    }
    // Its leader has closed; what is left of its group ends before the answer.
    super::groups::end(child.pid() as i32, &spec.kill_signal, spec.force_kill_after);
    drop(child);
    if abort.signal().aborted() {
        return failed(FailureKind::Aborted, "the execFile was let go");
    }
    if let (true, Some(ms)) = (cut_off, spec.deadline) {
        return failed(
            FailureKind::Timeout,
            format!("the execFile timed out after {ms} ms"),
        );
    }
    let (code, signal) = exit.unwrap_or((None, None));
    exec_reply(spec, &stdout, &stderr, code, signal, timed_out)
}

/// The `fs.write` line of the app's grants `app` under which it may write
/// `path`'s directory entry or `real`, what it names, compared by realized
/// whole names: a native line, or a `home:` or `app:` one. Where the grants
/// do not parse, or a line names no directory here, the answer is the parse
/// error or that line, and `path` is refused as if a line covered it.
fn writable(shared: &Shared, app: &str, path: &Path, real: &Path) -> Option<String> {
    let realize = ibex2::stdlib::fs::realize;
    let entry = match (path.parent(), path.file_name()) {
        (Some(parent), Some(leaf)) => realize(parent).join(leaf),
        _ => path.to_path_buf(),
    };
    let reached = [entry, real.to_path_buf()];
    let set = match ibex2::grant::GrantSet::parse(&exact_runner::io_grants(app)) {
        Ok(set) => set,
        Err(e) => return Some(format!("the app's grants did not parse: {e}")),
    };
    let line = set.iter().find_map(|grant| {
        let ibex2::grant::Grant::FsWrite(prefix) = grant else {
            return None;
        };
        // Parsed, so a quoted path with spaces is one path.
        let (namespace, parts) = prefix.components().split_first()?;
        let spelt = format!("{namespace}/{}", parts.join("/"));
        match native(shared, &spelt) {
            Ok(dir) => (reached.iter().any(|p| p.starts_with(realize(&dir))))
                .then(|| format!("fs.write {prefix}")),
            Err(e) => Some(format!(
                "fs.write {prefix}, which names no directory here: {e}"
            )),
        }
    });
    line
}

/// `execPath` (D7): the native path a spawn of `program` would run, or `None`.
pub(super) fn resolve(shared: &Shared, grants: &str, app: &str, spec: &Spec) -> Option<String> {
    program(shared, grants, app, spec, &Ran::Real)
        .ok()
        .filter(|path| os::executable(path))
        .map(|path| path.display().to_string())
}

/// The login shell's variables `names`, captured once a launch (D5).
pub(super) fn login_environment(shared: &Shared, names: &[&str]) -> HashMap<String, String> {
    static CAPTURED: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
    let captured = CAPTURED.get_or_init(Mutex::default);
    let mut known = captured.lock().unwrap();
    let missing: Vec<&str> = names
        .iter()
        .copied()
        .filter(|n| !known.contains_key(*n))
        .collect();
    if !missing.is_empty() {
        let found = capture(shared, &missing);
        for name in missing {
            known.insert(name.to_string(), found.get(name).cloned());
        }
    }
    (names.iter())
        .filter_map(|n| Some((n.to_string(), known.get(*n)?.clone()?)))
        .collect()
}

/// D5's algorithm: `$SHELL -ilc` printing each variable between markers,
/// in a session of its own with stdin `/dev/null` and a 5 s limit; then
/// `/bin/zsh` on macOS, or `/bin/sh -l` elsewhere, if that gave no `PATH`;
/// on macOS then `launchctl getenv PATH`; failing all, the app's own.
fn capture(shared: &Shared, names: &[&str]) -> HashMap<String, String> {
    let home = home(shared).unwrap_or_else(|_| PathBuf::from("/"));
    let shells: Vec<(PathBuf, &str)> = (os::login_shell().into_iter().map(|s| (s, "-ilc")))
        .chain(if cfg!(target_vendor = "apple") {
            [(PathBuf::from("/bin/zsh"), "-ilc")]
        } else {
            [(PathBuf::from("/bin/sh"), "-lc")]
        })
        .collect();
    capture_with(&shells, names, &home)
}

/// [`capture`] through `shells`, in order, until one gives a `PATH`.
fn capture_with(
    shells: &[(PathBuf, &str)],
    names: &[&str],
    home: &Path,
) -> HashMap<String, String> {
    let mut found = HashMap::new();
    // Each name goes into the shell's command line: only a variable's name
    // may (the grammars refuse any other, LLP 1133.007 D5).
    let names: Vec<&str> = (names.iter().copied()).filter(|n| shell_name(n)).collect();
    let names = names.as_slice();
    for (shell, flags) in shells {
        let flags = *flags;
        let nu = shell.file_name().is_some_and(|n| n == "nu");
        let script: String = (names.iter().enumerate())
            .map(|(i, name)| {
                if nu {
                    format!("print \"__EXACT_{i}_START__\"; print ($env | get -i {name} | default \"\" | if ($in | describe | str starts-with \"list\") {{ str join (char esep) }} else {{ $in }}); print \"__EXACT_{i}_END__\"; ")
                } else {
                    format!("printf '\\n__EXACT_{i}_START__\\n'; printenv {name}; printf '\\n__EXACT_{i}_END__\\n'; ")
                }
            })
            .collect();
        if let Some(out) = run(shell, &[flags.to_string(), script], home, SHELL_LIMIT) {
            for (i, name) in names.iter().enumerate() {
                let (start, end) = (
                    format!("__EXACT_{i}_START__\n"),
                    format!("\n__EXACT_{i}_END__"),
                );
                let value = out
                    .split_once(&start)
                    .and_then(|(_, rest)| rest.split_once(&end))
                    .map(|(v, _)| v.trim_end_matches('\n'));
                if let Some(value) = value.filter(|v| !v.is_empty()) {
                    found
                        .entry(name.to_string())
                        .or_insert_with(|| value.to_string());
                }
            }
        }
        if found.contains_key("PATH") || !names.contains(&"PATH") {
            break;
        }
    }
    if cfg!(target_vendor = "apple") && names.contains(&"PATH") && !found.contains_key("PATH") {
        if let Some(path) = run(
            Path::new("/bin/launchctl"),
            &["getenv".into(), "PATH".into()],
            home,
            LAUNCHCTL_LIMIT,
        ) {
            let path = path.trim();
            if !path.is_empty() {
                found.insert("PATH".into(), path.to_string());
            }
        }
    }
    for name in names {
        if !found.contains_key(*name) {
            if let Ok(value) = std::env::var(name) {
                found.insert(name.to_string(), value);
            }
        }
    }
    found
}

/// A shell variable's name: `[A-Za-z_][A-Za-z0-9_]*`.
fn shell_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c == '_' || c.is_ascii_alphabetic())
        && chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

/// After the program exits, how long its output may still arrive: a
/// background job its rc files started can hold the pipe open forever.
const AFTER_EXIT: Duration = Duration::from_millis(200);

/// Run a fixed program in a session of its own with no controlling
/// terminal: its stdout (lossily UTF-8) once it exits and its output stops,
/// or `None` past `limit`. Its group is ended either way.
fn run(program: &Path, args: &[String], home: &Path, limit: Duration) -> Option<String> {
    let env = app_environment();
    let child = os::spawn(os::Command {
        arg0: None,
        program,
        args,
        env: &env,
        cwd: home,
        stdin: os::Stdio::Null,
        stdout: os::Stdio::Pipe,
        stderr: os::Stdio::Null,
        session: true,
    })
    .ok()?;
    let pid = child.pid;
    let mut out = std::fs::File::from(child.stdout?);
    enum Got {
        Bytes(Vec<u8>),
        End,
        Exit,
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let reader = tx.clone();
    let _ = std::thread::Builder::new()
        .name("exact-login-shell".into())
        .spawn(move || {
            let mut buf = vec![0u8; 16 << 10];
            loop {
                match out.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if reader.send(Got::Bytes(buf[..n].to_vec())).is_err() {
                            return;
                        }
                    }
                }
            }
            let _ = reader.send(Got::End);
        });
    let _ = std::thread::Builder::new()
        .name("exact-login-shell-wait".into())
        .spawn(move || {
            let _ = os::wait(pid);
            let _ = tx.send(Got::Exit);
        });
    let deadline = Instant::now() + limit;
    let (mut bytes, mut exited_at, mut ended) = (Vec::new(), None::<Instant>, false);
    while !(ended && exited_at.is_some()) {
        let until = match (exited_at, ended) {
            (Some(at), false) => (at + AFTER_EXIT).min(deadline),
            _ => deadline,
        };
        match rx.recv_timeout(until.saturating_duration_since(Instant::now())) {
            Ok(Got::Bytes(b)) => bytes.extend_from_slice(&b),
            Ok(Got::End) => ended = true,
            Ok(Got::Exit) => exited_at = Some(Instant::now()),
            Err(_) => break,
        }
    }
    let finished = exited_at.is_some();
    // Past the limit, or an rc file's background job holds the pipe: its
    // session's group ends.
    if !(finished && ended) {
        let _ = os::signal_group(pid, os::SIGKILL);
    }
    finished.then(|| String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
#[path = "executor_process_real_tests.rs"]
mod tests;
