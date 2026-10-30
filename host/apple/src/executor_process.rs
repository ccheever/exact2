//! Desktop processes, natively (LLP 1133.007): the process requests an
//! `@exact/child-process` call makes, run by the executor core Apple and
//! Linux share. Each runs on a thread of its own, as a stream does: a
//! `spawn` is supervised there for as long as its resource holds it
//! (`supervise.rs`); `execFile`, `execPath` and the environment reads answer
//! once.
//!
//! Admission comes first, as for a fetch (D12): the program against the
//! `process.spawn` lines of the source's grant scope, a stdin file against
//! `fs.read`, the readiness URL against `net.fetch`; then the origin (D13:
//! only a person's turn starts, writes to, ends or signals a child); then,
//! under the agent, the drive's substitute (§7), without which nothing runs.
use super::{complete, failed, has_ready, wake, Completed, Core, Shared};
use exact_runner::{
    CallOrigin, FailureKind, Message, Outcome, ProcessOp, Request, Response, SocketOp,
};
use ibex2::stdlib::abort::AbortController;
use serde_json::{json, Value as Json};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Condvar, Mutex};

#[path = "executor_process_clock.rs"]
pub(crate) mod clock;
#[path = "executor_process_script.rs"]
mod script;
#[path = "executor_process_supervise.rs"]
mod supervise;

use clock::Clock;
use script::Script;

/// A child's stdin queue's bound (D2). Past 64 KiB its handle's `write`
/// answers `false` (js/src/process.js), and `drain` follows once it is empty.
const STDIN_BYTES: u64 = 1 << 20;
/// What a child's undelivered output may hold before the host stops
/// reading its pipes (D2).
const RECEIVE_CHUNKS: usize = 256;
const RECEIVE_BYTES: usize = 1 << 20;
/// The longest chunk one event carries.
pub(crate) const CHUNK: usize = 64 << 10;
/// How long a quit waits for children to end before `SIGKILL` (D2, D9).
const QUIT_GRACE_MS: f64 = 5000.0;
/// What one-shots (`execFile`, `execPath`, the environment reads) may hold
/// at once: 16 runs and 64 MiB of reply ceilings, one alone always fitting.
/// They hold no count or bytes of their lane, as an upload waiting does
/// not, so slow ones leave the app's requests their room (D3).
const ONE_SHOTS: usize = 16;
const ONE_SHOT_BYTES: usize = 64 << 20;

/// What the executor keeps for processes.
pub(crate) struct Processes {
    /// Each running child's control, by its stream's ticket.
    children: Mutex<HashMap<u64, Arc<ChildCtl>>>,
    /// Every child stream's ticket, whose outcomes the agent's clock owes.
    streams: Mutex<std::collections::HashSet<u64>>,
    /// The runner took a completion: output waiting for room goes on.
    pub(crate) drained: Condvar,
    /// The supervisors' clock: the agent's under the agent.
    clock: Arc<Clock>,
    /// The drive's launch facts, under the agent.
    agent: Option<AgentFacts>,
    /// The host starts programs: macOS and Linux (§8). Off, every process
    /// request answers `Unsupported` (iOS, the render host).
    enabled: AtomicBool,
    /// The login variables captured for the app's origin, which later
    /// reads answer for any origin (D13).
    login_captured: Mutex<std::collections::HashSet<String>>,
    /// The one-shots running: each ticket's reply ceiling.
    one_shots: Mutex<HashMap<u64, usize>>,
}

impl Processes {
    /// Whether `origin` may read login variables `names` now: the app's
    /// always, any other once the app's origin has captured them (D13).
    fn login_readable(&self, origin: CallOrigin, names: &[String]) -> bool {
        let captured = self.login_captured.lock().unwrap();
        origin == CallOrigin::App || names.iter().all(|n| captured.contains(n))
    }

    pub(crate) fn new() -> Processes {
        let agent = AgentFacts::from_env();
        #[cfg(test)]
        let agent = UNDER_AGENT.take().or(agent);
        let clock = if agent.is_some() {
            Clock::agent()
        } else {
            Clock::wall()
        };
        Processes {
            children: Mutex::default(),
            streams: Mutex::default(),
            drained: Condvar::new(),
            clock: Arc::new(clock),
            agent,
            enabled: AtomicBool::new(false),
            login_captured: Mutex::default(),
            one_shots: Mutex::default(),
        }
    }
}

/// Take room for one-shot `ticket`, whose reply may hold `ceiling` bytes.
pub(crate) fn admit_one_shot(
    shared: &Shared,
    ticket: u64,
    ceiling: usize,
) -> Result<(), &'static str> {
    let mut running = shared.processes.one_shots.lock().unwrap();
    let held: usize = running.values().sum();
    if running.len() >= ONE_SHOTS || (!running.is_empty() && held + ceiling > ONE_SHOT_BYTES) {
        return Err("native executor process limit reached");
    }
    running.insert(ticket, ceiling);
    Ok(())
}

/// One-shot `ticket` ended: its room goes back.
pub(crate) fn release_one_shot(shared: &Shared, ticket: u64) {
    shared.processes.one_shots.lock().unwrap().remove(&ticket);
}

/// The drive's launch facts (§7): `--process <target>=<file>|script:<file>|real`,
/// `--login-env NAME=value` and `--home <dir>`, as `EXACT_AGENT_PROCESS`,
/// `EXACT_AGENT_LOGIN_ENV` and `EXACT_AGENT_HOME`, one per line, read only
/// under the agent (`EXACT_AGENT=1`).
pub(crate) struct AgentFacts {
    processes: Vec<(String, String)>,
    login_env: Vec<(String, String)>,
    #[allow(dead_code)] // `home:` paths are the storage work's (stage 1)
    pub(crate) home: Option<std::path::PathBuf>,
}

impl AgentFacts {
    fn from_env() -> Option<AgentFacts> {
        if !std::env::var("EXACT_AGENT").is_ok_and(|v| v == "1") {
            return None;
        }
        let pairs = |name: &str| -> Vec<(String, String)> {
            (std::env::var(name).unwrap_or_default().lines())
                .filter_map(|line| line.split_once('='))
                .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
                .filter(|(k, _)| !k.is_empty())
                .collect()
        };
        Some(AgentFacts {
            processes: pairs("EXACT_AGENT_PROCESS"),
            login_env: pairs("EXACT_AGENT_LOGIN_ENV"),
            home: std::env::var_os("EXACT_AGENT_HOME").map(Into::into),
        })
    }

    /// What the drive says runs for `program`: its own line, else `all`'s.
    fn substitute(&self, program: &str) -> Option<&str> {
        let named = |target: &str| self.processes.iter().find(|(t, _)| t == target);
        (named(program).or_else(|| named("all"))).map(|(_, spec)| spec.as_str())
    }
}

/// What runs for a program: itself, another executable, or a script.
pub(crate) enum Ran {
    Real,
    File(String),
    Script(Script),
}

impl Ran {
    /// Its words in `state.streams`.
    fn words(&self) -> String {
        match self {
            Ran::Real => "real".into(),
            Ran::File(file) => file.clone(),
            Ran::Script(script) => format!("script:{}", script.file),
        }
    }
}

/// A call's options, as the binding wrote them (js/src/process.js).
#[allow(dead_code)] // `args`, `cwd` and `input` are the real transport's (stage 1)
pub(crate) struct Spec {
    pub(crate) kind: String,
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
    pub(crate) env: Option<Vec<(String, String)>>,
    pub(crate) cwd: Option<String>,
    /// Each of stdin, stdout and stderr: a pipe, or ignored.
    pub(crate) pipes: [bool; 3],
    pub(crate) stdin_from: Option<String>,
    pub(crate) input: Option<Vec<u8>>,
    pub(crate) utf8: bool,
    pub(crate) kill_signal: String,
    pub(crate) force_kill_after: f64,
    pub(crate) drain_timeout: f64,
    pub(crate) restart: Option<Restart>,
    pub(crate) ready: Option<Ready>,
    pub(crate) timeout: Option<f64>,
    /// An `AbortSignal.timeout`'s time left (the request's `timeout_ms`):
    /// a one-shot still running then ends as `Timeout`.
    pub(crate) deadline: Option<f64>,
    pub(crate) max_buffer: usize,
}

/// `exactRestart` (D4).
pub(crate) struct Restart {
    pub(crate) always: bool,
    pub(crate) delay: f64,
    pub(crate) max_delay: f64,
    pub(crate) prevent: Vec<i64>,
}

/// `exactReady` (D4).
pub(crate) struct Ready {
    pub(crate) url: String,
    pub(crate) interval: f64,
    pub(crate) timeout: f64,
    pub(crate) accept: (u16, u16),
    pub(crate) fail: bool,
}

/// The signals `kill` and `killSignal` take (D2).
const SIGNALS: [&str; 9] = [
    "SIGTERM", "SIGINT", "SIGHUP", "SIGQUIT", "SIGUSR1", "SIGUSR2", "SIGSTOP", "SIGCONT", "SIGKILL",
];

impl Spec {
    /// The call, held to what the binding writes (js/src/process.js), since
    /// data code can write a request's body itself: a key, a type or a bound
    /// the binding would not send is refused (D12). `deadline`: the
    /// request's `timeout_ms`, an `AbortSignal.timeout` on a one-shot.
    fn parse(body: &[u8], deadline: Option<u32>) -> Result<Spec, String> {
        let j: Json = serde_json::from_slice(body).map_err(|e| format!("process: {e}"))?;
        let o = j.as_object().ok_or("process: the call is a JSON object")?;
        let kind = o
            .get("kind")
            .and_then(Json::as_str)
            .unwrap_or("")
            .to_string();
        const COMMON: [&str; 8] = [
            "kind",
            "program",
            "args",
            "cwd",
            "env",
            "encoding",
            "killSignal",
            "stdinFrom",
        ];
        let keys: Vec<&str> = match kind.as_str() {
            "spawn" => [
                &COMMON[..],
                &[
                    "stdio",
                    "forceKillAfter",
                    "drainTimeout",
                    "restart",
                    "ready",
                ],
            ]
            .concat(),
            "execFile" => [&COMMON[..], &["timeout", "maxBuffer", "input"]].concat(),
            "execPath" => vec!["kind", "program"],
            "loginEnv" | "appEnv" => vec!["kind"],
            other => return Err(format!("process: no call `{other}`")),
        };
        let bad = |what: &str| format!("process: {kind}'s {what}");
        if let Some(key) = o.keys().find(|k| !keys.contains(&k.as_str())) {
            return Err(bad(&format!("`{key}` is not an option it takes")));
        }
        let ms = |v: Option<&Json>, name: &str, max: u64| -> Result<Option<f64>, String> {
            match v {
                None | Some(Json::Null) => Ok(None),
                Some(v) => match v.as_u64().filter(|n| *n <= max) {
                    Some(n) => Ok(Some(n as f64)),
                    None => Err(bad(&format!("{name} is milliseconds from 0 to {max}"))),
                },
            }
        };
        let string = |key: &str| -> Result<Option<String>, String> {
            match o.get(key) {
                None => Ok(None),
                Some(Json::String(s)) => Ok(Some(s.clone())),
                Some(_) => Err(bad(&format!("{key} is a string"))),
            }
        };
        let program = string("program")?.unwrap_or_default();
        if program.is_empty() && !matches!(kind.as_str(), "loginEnv" | "appEnv") {
            return Err(bad("program is its grant line's name"));
        }
        let args = match o.get("args") {
            None => Vec::new(),
            Some(Json::Array(a)) if a.iter().all(Json::is_string) => a
                .iter()
                .filter_map(|s| s.as_str().map(str::to_string))
                .collect(),
            Some(_) => return Err(bad("args are strings")),
        };
        let env = match o.get("env") {
            None => None,
            Some(Json::Array(pairs)) => Some(
                (pairs.iter())
                    .map(|p| match p.as_array().map(Vec::as_slice) {
                        Some([Json::String(k), Json::String(v)]) => Ok((k.clone(), v.clone())),
                        _ => Err(bad("env is pairs of strings")),
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            Some(_) => return Err(bad("env is pairs of strings")),
        };
        let encoding = string("encoding")?;
        if encoding
            .as_deref()
            .is_some_and(|e| e != "utf8" && e != "buffer")
        {
            return Err(bad("encoding is utf8 or buffer"));
        }
        let kill_signal = string("killSignal")?.unwrap_or_else(|| "SIGTERM".into());
        if !SIGNALS.contains(&kill_signal.as_str()) {
            return Err(bad("killSignal is a signal kill takes"));
        }
        let pipes = match o.get("stdio") {
            None => [true; 3],
            Some(Json::Array(a))
                if a.len() <= 3 && a.iter().all(|s| s == "pipe" || s == "ignore") =>
            {
                let pipe = |i: usize| a.get(i).is_none_or(|s| s == "pipe");
                [pipe(0), pipe(1), pipe(2)]
            }
            Some(_) => return Err(bad("stdio entries are pipe or ignore")),
        };
        let restart = match o.get("restart") {
            None => None,
            Some(Json::Object(r)) => {
                if let Some(key) = r.keys().find(|k| {
                    !["when", "delay", "maxDelay", "preventExitStatus"].contains(&k.as_str())
                }) {
                    return Err(bad(&format!(
                        "exactRestart's `{key}` is not an option it takes"
                    )));
                }
                let always = match r.get("when").and_then(Json::as_str) {
                    Some("always") => true,
                    Some("on-failure") => false,
                    _ => return Err(bad("exactRestart.when is always or on-failure")),
                };
                let prevent = match r.get("preventExitStatus") {
                    None => Vec::new(),
                    Some(Json::Array(c)) if c.iter().all(Json::is_i64) => {
                        c.iter().filter_map(Json::as_i64).collect()
                    }
                    Some(_) => return Err(bad("exactRestart.preventExitStatus is exit codes")),
                };
                Some(Restart {
                    always,
                    delay: ms(r.get("delay"), "exactRestart.delay", 3_600_000)?.unwrap_or(500.0),
                    max_delay: ms(r.get("maxDelay"), "exactRestart.maxDelay", 3_600_000)?
                        .unwrap_or(10_000.0),
                    prevent,
                })
            }
            Some(_) => return Err(bad("exactRestart is an object")),
        };
        let ready = match o.get("ready") {
            None => None,
            Some(Json::Object(r)) => {
                if let Some(key) = r.keys().find(|k| {
                    !["url", "interval", "timeout", "accept", "onTimeout"].contains(&k.as_str())
                }) {
                    return Err(bad(&format!(
                        "exactReady's `{key}` is not an option it takes"
                    )));
                }
                let url = r
                    .get("url")
                    .and_then(Json::as_str)
                    .ok_or_else(|| bad("exactReady needs a url"))?;
                let accept = match r.get("accept") {
                    None | Some(Json::Null) => (200, 299),
                    Some(Json::Array(a)) => match (
                        a.as_slice(),
                        a.iter().all(|c| c.as_u64().is_some_and(|c| c <= 999)),
                    ) {
                        ([lo, hi], true) => (
                            lo.as_u64().unwrap_or(200) as u16,
                            hi.as_u64().unwrap_or(299) as u16,
                        ),
                        _ => return Err(bad("exactReady.accept is [lowest, highest] status")),
                    },
                    Some(_) => return Err(bad("exactReady.accept is [lowest, highest] status")),
                };
                let fail = match r.get("onTimeout").and_then(Json::as_str) {
                    None | Some("retry") => false,
                    Some("fail") => true,
                    Some(_) => return Err(bad("exactReady.onTimeout is retry or fail")),
                };
                Some(Ready {
                    url: url.to_string(),
                    interval: ms(r.get("interval"), "exactReady.interval", 3_600_000)?
                        .unwrap_or(100.0)
                        .max(1.0),
                    timeout: ms(r.get("timeout"), "exactReady.timeout", 3_600_000)?
                        .unwrap_or(60_000.0),
                    accept,
                    fail,
                })
            }
            Some(_) => return Err(bad("exactReady is an object")),
        };
        let max_buffer = match o.get("maxBuffer") {
            None => 1 << 20,
            Some(v) => match v.as_u64().filter(|n| (1..=1 << 24).contains(n)) {
                Some(n) => n as usize,
                None => return Err(bad("maxBuffer is 1 to 16777216 bytes")),
            },
        };
        let input = match string("input")? {
            Some(b64) => {
                let bytes =
                    exact_runner::agent::unbase64(&b64).ok_or_else(|| bad("input is base64"))?;
                if bytes.len() > 1 << 24 {
                    return Err(bad("input is at most 16 MiB"));
                }
                Some(bytes)
            }
            None => None,
        };
        Ok(Spec {
            program,
            args,
            env,
            cwd: string("cwd")?,
            pipes,
            stdin_from: string("stdinFrom")?,
            input,
            utf8: encoding.as_deref() == Some("utf8"),
            kill_signal,
            force_kill_after: ms(o.get("forceKillAfter"), "exactForceKillAfter", 600_000)?
                .unwrap_or(5000.0),
            drain_timeout: ms(o.get("drainTimeout"), "exactDrainTimeout", 600_000)?
                .unwrap_or(5000.0),
            restart,
            ready,
            timeout: ms(o.get("timeout"), "timeout", 3_600_000)?.filter(|t| *t > 0.0),
            deadline: deadline.map(f64::from),
            max_buffer,
            kind,
        })
    }
}

/// How a child's staged operations reach its supervisor, and what
/// [`Core::socket`] checks them against.
pub(crate) struct ChildCtl {
    tx: Mutex<mpsc::Sender<Input>>,
    /// Held while the incarnation changes and while [`send`] hands an
    /// operation over, so an operation never reaches the next incarnation's
    /// stdin state.
    turn: Mutex<()>,
    /// The incarnation running now; 0 between incarnations.
    incarnation: AtomicU32,
    /// Its stdin was ended, or is ignored.
    ended: AtomicBool,
    /// Bytes waiting in its stdin.
    pub(crate) queued: AtomicU64,
}

impl ChildCtl {
    fn send(&self, input: Input) {
        let _ = self.tx.lock().unwrap().send(input);
    }

    /// Incarnation `n` runs now, its stdin open unless ignored.
    pub(crate) fn begin(&self, n: u32, stdin: bool) {
        let _turn = self.turn.lock().unwrap();
        self.queued.store(0, Ordering::Release);
        self.ended.store(!stdin, Ordering::Release);
        self.incarnation.store(n, Ordering::Release);
    }

    /// The incarnation has ended: its handle's operations are refused.
    pub(crate) fn end(&self) {
        let _turn = self.turn.lock().unwrap();
        self.incarnation.store(0, Ordering::Release);
    }

    /// The child took `bytes` of its stdin queue.
    pub(crate) fn consumed(&self, bytes: u64) {
        if bytes > 0 {
            let _ = (self.queued).fetch_update(Ordering::AcqRel, Ordering::Acquire, |q| {
                Some(q.saturating_sub(bytes))
            });
        }
    }
}

/// What reaches a supervisor.
pub(crate) enum Input {
    /// The clock moved, or a wait should look again.
    Tick,
    /// A staged operation whose commit stood, for incarnation `n`.
    Op(u32, ProcessOp),
    /// The resource let the child go, or the app quits.
    Release,
    /// What a real child's reader, writer or waiter saw, for incarnation `n`.
    #[allow(dead_code)] // the real transport's (stage 1)
    Child(u32, supervise::Seen),
    /// A readiness probe's answer, for incarnation `n`.
    Probe(u32, bool),
}

/// Run process request `ticket` to its end, on its own thread.
pub(super) fn run(
    shared: &Arc<Shared>,
    ticket: u64,
    request: Request,
    grants: &str,
    host: &dyn Fn() -> ibex2::host::Host,
    abort: AbortController,
) {
    let _retiring = {
        let abort = abort.clone();
        shared.abort.signal().register(move || abort.abort())
    };
    if shared.abort.signal().aborted() {
        abort.abort();
    }
    let mut leave = None;
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        answer(shared, ticket, &request, grants, host, &abort, &mut leave)
    }))
    .unwrap_or_else(|_| failed(FailureKind::Aborted, "the process work panicked"));
    // Completed before it leaves the children, so the agent's clock never
    // sees it as neither running nor owed.
    complete(shared, ticket, outcome);
    shared.processes.children.lock().unwrap().remove(&ticket);
    release_one_shot(shared, ticket);
    // The agent's advance that woke the stream waits until its end is queued.
    drop(leave);
}

fn answer(
    shared: &Arc<Shared>,
    ticket: u64,
    request: &Request,
    grants: &str,
    host: &dyn Fn() -> ibex2::host::Host,
    abort: &AbortController,
    leave: &mut Option<clock::Leave>,
) -> Outcome {
    if !shared.processes.enabled.load(Ordering::Acquire) {
        return failed(
            FailureKind::Unsupported,
            "this host starts no programs (LLP 1133.007 §8)",
        );
    }
    let spec = match Spec::parse(&request.body, request.timeout_ms) {
        Ok(spec) => spec,
        Err(why) => return failed(FailureKind::Refused, why),
    };
    // A spawn is a two-way stream, and nothing else is: one without its
    // place among the streams is refused.
    if (spec.kind == "spawn") != (request.stream && request.socket) {
        let why = format!(
            "process: {} is not a {}",
            spec.kind,
            ["one-shot", "stream"][usize::from(request.stream)]
        );
        return failed(FailureKind::Refused, why);
    }
    // A source's scope narrows the app's grants, as on the workers. The
    // process lines are exact2's own (`exact_grants::process`); the file
    // ones the I/O grammar reads.
    let scoped = match exact_data::storage::scope(grants, request.grants.as_deref()) {
        Ok(scoped) => scoped,
        Err(why) => return failed(FailureKind::Refused, why),
    };
    let set = match ibex2::grant::GrantSet::parse(&exact_runner::io_grants(scoped)) {
        Ok(set) => set,
        Err(e) => {
            return failed(
                FailureKind::Refused,
                format!("the app's grants did not parse: {e}"),
            )
        }
    };
    // The runner admitted the kind and a start by its origin (D13); a
    // first capture of the login environment is the host's to know, so it
    // is held here.
    if exact_runner::process::declared_name(&spec.kind).is_none() {
        let why = format!("process: no call `{}`", spec.kind);
        return failed(FailureKind::Refused, why);
    }
    let origin = request.reach.origin;
    match spec.kind.as_str() {
        "appEnv" => return environment(scoped, &set, false, &shared.processes),
        "loginEnv" => {
            let names: Vec<String> = (exact_grants::process::login_variables(scoped).into_iter())
                .map(str::to_string)
                .collect();
            if !shared.processes.login_readable(origin, &names) {
                return person_required("loginShellEnvironment", origin);
            }
            let outcome = environment(scoped, &set, true, &shared.processes);
            if matches!(outcome, Outcome::Response(_)) && origin == CallOrigin::App {
                (shared.processes.login_captured.lock().unwrap()).extend(names);
            }
            return outcome;
        }
        "spawn" | "execFile" | "execPath" => {}
        other => return failed(FailureKind::Refused, format!("process: no call `{other}`")),
    }
    if exact_grants::process::admitting(scoped, &spec.program).is_none() {
        let why = format!("outside the app's grants (process.spawn {})", spec.program);
        return failed(FailureKind::Refused, why);
    }
    if let Some(path) = &spec.stdin_from {
        if !set.permits(&ibex2::grant::Operation::FsRead { path: path.clone() }) {
            return failed(
                FailureKind::Refused,
                format!("outside the app's grants (fs.read {path})"),
            );
        }
    }
    if let Some(ready) = &spec.ready {
        let admitted = ibex2::stdlib::fetch::origin_of(&ready.url)
            .is_ok_and(|origin| set.permits(&ibex2::grant::Operation::Fetch { origin }));
        if !admitted {
            return failed(
                FailureKind::Refused,
                format!("outside the app's grants (net.fetch {})", ready.url),
            );
        }
    }
    let ran = match &shared.processes.agent {
        None => Ran::Real,
        Some(facts) => match facts.substitute(&spec.program) {
            None => {
                return failed(
                    FailureKind::Refused,
                    format!(
                        "under the agent, `{p}` runs only when the drive names it: --process {p}=<file>|script:<file.json>|real",
                        p = spec.program
                    ),
                )
            }
            Some("real") => Ran::Real,
            Some(spec) => match spec.strip_prefix("script:") {
                Some(file) => match Script::read(file) {
                    Ok(script) => Ran::Script(script),
                    Err(why) => return failed(FailureKind::Refused, why),
                },
                None => Ran::File(spec.to_string()),
            },
        },
    };
    match spec.kind.as_str() {
        "execPath" => exec_path(&spec, &ran),
        "execFile" => supervise::exec_file(shared, &spec, ran, abort),
        _ => {
            let (tx, rx) = mpsc::channel();
            let ctl = Arc::new(ChildCtl {
                tx: Mutex::new(tx),
                turn: Mutex::new(()),
                incarnation: AtomicU32::new(0),
                ended: AtomicBool::new(!spec.pipes[0]),
                queued: AtomicU64::new(0),
            });
            shared
                .processes
                .children
                .lock()
                .unwrap()
                .insert(ticket, ctl.clone());
            let release = {
                let ctl = ctl.clone();
                abort.signal().register(move || ctl.send(Input::Release))
            };
            if abort.signal().aborted() {
                ctl.send(Input::Release);
            }
            let probe_host = host();
            shared.processes.streams.lock().unwrap().insert(ticket);
            let (outcome, place) =
                supervise::Supervisor::new(shared, ticket, spec, ran, ctl, rx, probe_host, &set)
                    .run();
            *leave = place;
            drop(release);
            outcome
        }
    }
}

fn person_required(api: &str, origin: CallOrigin) -> Outcome {
    failed(
        FailureKind::Refused,
        format!(
            "person-required: {api} from a {} origin: only a person's input or the app's own code captures the login environment first (LLP 1133.007 D13)",
            origin.word()
        ),
    )
}

/// One answer, as JSON.
fn reply(body: Json) -> Outcome {
    Outcome::Response(Response {
        status: 200,
        headers: Vec::new(),
        body: body.to_string().into_bytes(),
    })
}

/// `execPath`: the native path a spawn of the target would run, or `null`.
fn exec_path(spec: &Spec, ran: &Ran) -> Outcome {
    match ran {
        Ran::Script(script) => reply(json!({ "path": script.path() })),
        Ran::File(file) => reply(json!({ "path": file })),
        Ran::Real => match real::resolve(&spec.program, spec.env.as_deref()) {
            Ok(path) => reply(json!({ "path": path })),
            Err(outcome) => outcome,
        },
    }
}

/// `appEnvironment()` or `loginShellEnvironment()`: the variables the
/// grants name, `env.read NAME` or `env.read login:NAME` (D5).
fn environment(
    scoped: &str,
    set: &ibex2::grant::GrantSet,
    login: bool,
    processes: &Processes,
) -> Outcome {
    let names: Vec<&str> = match login {
        true => exact_grants::process::login_variables(scoped),
        false => set.readable_env(),
    };
    let mut out = serde_json::Map::new();
    // Outside the agent the login shell is the person's, captured once.
    let captured = match login && processes.agent.is_none() {
        true => match real::login_variables(&names) {
            Ok(found) => found,
            Err(outcome) => return outcome,
        },
        false => HashMap::new(),
    };
    let given = (processes.agent.as_ref()).filter(|facts| login && !facts.login_env.is_empty());
    for name in names {
        let value = match given {
            // A drive never runs the person's shell (§7).
            Some(facts) => (facts.login_env.iter())
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.clone()),
            None if login && processes.agent.is_none() => captured.get(name).cloned(),
            None => std::env::var(name).ok(),
        };
        if let Some(value) = value {
            out.insert(name.to_string(), Json::String(value));
        }
    }
    reply(Json::Object(out))
}

/// Queue one event of stream `ticket` behind the ones the runner has not
/// taken, never coalesced. Output counts against the receive bound: while
/// the queue is full, `wait` holds the caller until the runner takes some,
/// so a child's pipe fills and the child waits (D2). False once nobody
/// wants more.
pub(crate) fn deliver(shared: &Shared, ticket: u64, message: Message, wait: bool) -> bool {
    let output = matches!(message.event.as_str(), "stdout" | "stderr");
    let size = |m: &Message| m.binary.as_ref().map_or(m.data.len(), Vec::len);
    let mut state = shared.state.lock().unwrap();
    loop {
        if state.retired {
            return false;
        }
        let Some((lane, elapsed_ms)) = (state.running.iter())
            .find(|run| run.ticket == ticket && !run.forgotten)
            .map(|run| {
                (
                    run.lane,
                    run.started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
                )
            })
        else {
            return false;
        };
        let (chunks, bytes) = (state.completed[lane].iter())
            .filter(|done| done.ticket == ticket && done.message)
            .fold((0, 0), |(n, b), done| match &done.outcome {
                Outcome::Message(m) if matches!(m.event.as_str(), "stdout" | "stderr") => {
                    (n + 1, b + size(m))
                }
                _ => (n, b),
            });
        let full = chunks >= RECEIVE_CHUNKS || bytes + size(&message) > RECEIVE_BYTES;
        if output && full && wait {
            state = (shared.processes.drained)
                .wait_timeout(state, std::time::Duration::from_millis(50))
                .unwrap()
                .0;
            continue;
        }
        state.completed[lane].push_back(Completed {
            elapsed_ms,
            ticket,
            outcome: Outcome::Message(message),
            bytes: 0,
            message: true,
            stream: true,
            light: false,
            again: false,
            reply: false,
        });
        if has_ready(&state) {
            wake(&mut state);
        }
        return true;
    }
}

/// A child's event with JSON details.
pub(crate) fn event(name: &str, data: Json) -> Message {
    Message {
        event: name.into(),
        data: data.to_string(),
        ..Message::default()
    }
}

/// Check a batch's child operations whole (D2): each child still running
/// the incarnation the handle named, and a write before its stdin's end
/// within its 1 MiB queue. The runner admitted their origin (D13).
pub(crate) fn check(shared: &Shared, ops: &[(u64, &SocketOp)]) -> Result<(), String> {
    if ops.is_empty() {
        return Ok(());
    }
    let children = shared.processes.children.lock().unwrap();
    let mut queued: HashMap<u64, u64> = HashMap::new();
    let mut ended: HashMap<u64, bool> = HashMap::new();
    for (ticket, op) in ops {
        let SocketOp::Process { incarnation, op } = op else {
            continue;
        };
        let child = children
            .get(ticket)
            .filter(|c| c.incarnation.load(Ordering::Acquire) == *incarnation)
            .ok_or("InvalidStateError: the child's incarnation has ended")?;
        let end = ended
            .entry(*ticket)
            .or_insert_with(|| child.ended.load(Ordering::Acquire));
        let bytes = queued
            .entry(*ticket)
            .or_insert_with(|| child.queued.load(Ordering::Acquire));
        match op {
            ProcessOp::Write { .. } if *end => {
                return Err("InvalidStateError: the child's stdin is ended".into());
            }
            ProcessOp::Write { data, .. } => {
                *bytes += data.len() as u64;
                if *bytes > STDIN_BYTES {
                    return Err("QuotaExceededError: the child's stdin queue exceeds 1 MiB".into());
                }
            }
            ProcessOp::End if *end => {
                return Err("InvalidStateError: the child's stdin is ended".into())
            }
            ProcessOp::End => *end = true,
            ProcessOp::Kill(_) => {}
        }
    }
    Ok(())
}

/// Hand a checked batch's child operations to their supervisors, in order:
/// for each child, the operations taken and its stdin queue now.
pub(crate) fn send(shared: &Shared, ops: &[(u64, &SocketOp)]) -> Vec<(u64, u64, u64)> {
    let children = shared.processes.children.lock().unwrap();
    let mut totals: Vec<(u64, u64, u64)> = Vec::new();
    for (ticket, op) in ops {
        let (Some(child), SocketOp::Process { incarnation, op }) = (children.get(ticket), op)
        else {
            continue;
        };
        // Checked whole, then the incarnation ended: it goes nowhere.
        let _turn = child.turn.lock().unwrap();
        if child.incarnation.load(Ordering::Acquire) != *incarnation {
            continue;
        }
        match op {
            ProcessOp::Write { data, .. } => {
                child.queued.fetch_add(data.len() as u64, Ordering::AcqRel);
            }
            ProcessOp::End => child.ended.store(true, Ordering::Release),
            ProcessOp::Kill(_) => {}
        }
        child.send(Input::Op(*incarnation, op.clone()));
        match totals.iter_mut().find(|(t, ..)| t == ticket) {
            Some(total) => total.1 += 1,
            None => totals.push((*ticket, 1, 0)),
        }
    }
    for total in &mut totals {
        total.2 = children
            .get(&total.0)
            .map_or(0, |c| c.queued.load(Ordering::Acquire));
    }
    totals
}

/// Bytes waiting in child `ticket`'s stdin, if it is one: what its handle's
/// `writableLength` starts from.
pub(crate) fn queued(shared: &Shared, ticket: u64) -> Option<u64> {
    let children = shared.processes.children.lock().unwrap();
    children
        .get(&ticket)
        .map(|c| c.queued.load(Ordering::Acquire))
}

/// The app quits (D2, D9): every child is released at once, each group's
/// escalation at its grace or 5 s, whichever is shorter, so a quit waits at
/// most 5 s.
pub(crate) fn quit(shared: &Shared) {
    let children: Vec<Arc<ChildCtl>> = (shared.processes.children.lock().unwrap().values())
        .cloned()
        .collect();
    for child in &children {
        child.send(Input::Release);
    }
    real::quit(QUIT_GRACE_MS);
}

#[cfg(test)]
thread_local! {
    /// Tests: the facts the next core this thread starts supervises under
    /// ([`Core::under_agent`]). `Shared` refers to itself, so a test cannot
    /// replace its processes once it is made.
    static UNDER_AGENT: std::cell::Cell<Option<AgentFacts>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
impl Core {
    /// The core `start` makes, supervising under the agent with the
    /// drive's `--process` lines.
    fn under_agent(processes: &[(&str, &str)], start: impl FnOnce() -> Core) -> Core {
        UNDER_AGENT.set(Some(AgentFacts {
            processes: (processes.iter())
                .map(|(t, s)| (t.to_string(), s.to_string()))
                .collect(),
            login_env: vec![("PATH".into(), "/drive/bin".into())],
            home: None,
        }));
        let core = start();
        UNDER_AGENT.take();
        core.shared.processes.enabled.store(true, Ordering::Release);
        core
    }
}

impl Core {
    /// Start programs for data code: a desktop host's executor (§8).
    pub(crate) fn allow_processes(&self) {
        self.shared.processes.enabled.store(true, Ordering::Release);
    }

    /// The clock the host's process work keeps, for the runner under the
    /// agent (LLP 1133.007 §7); `None` on the wall.
    pub(crate) fn work_clock(&self) -> Option<Arc<dyn exact_runner::WorkClock>> {
        let processes = &self.shared.processes;
        processes.clock.is_agent().then(|| {
            Arc::new(Work {
                shared: self.shared.clone(),
            }) as Arc<dyn exact_runner::WorkClock>
        })
    }
}

/// The agent's clock, as the runner moves it.
struct Work {
    shared: Arc<Shared>,
}

impl exact_runner::WorkClock for Work {
    fn advance(&self, now_ms: f64) {
        self.shared.processes.clock.advance(now_ms);
    }

    fn due(&self) -> Option<f64> {
        self.shared.processes.clock.due()
    }

    /// Whether a child stream's outcome waits for the runner. A stream
    /// neither running nor waiting is dropped from the set here, so it stays
    /// as small as the children alive.
    fn owed(&self) -> bool {
        let processes = &self.shared.processes;
        let live: Vec<u64> = processes.children.lock().unwrap().keys().copied().collect();
        let waiting: Vec<u64> = (self.shared.state.lock().unwrap().completed.iter().flatten())
            .map(|done| done.ticket)
            .collect();
        let mut streams = processes.streams.lock().unwrap();
        streams.retain(|t| live.contains(t) || waiting.contains(t));
        waiting.iter().any(|t| streams.contains(t))
    }
}

#[path = "executor_process_none.rs"]
mod real;

#[cfg(test)]
#[path = "executor_process_tests.rs"]
mod tests;
