//! `@exact/child-process` for a Rust source (LLP 1133.007 D13): `execFile`,
//! `execPath` and the environment reads as typed requests, as `fetch` has
//! them, and `spawn` as a stream that only listens until Rust sources get
//! LLP 1016.000 D7's send path. Each builds the process request the
//! TypeScript binding builds (js/src/process.js), so the host admits and
//! runs both alike; each reply is read here.
use exact_runner::{Answer, Outcome, Request};
use serde_json::{json, Value as Json};

/// The ceiling of a one-shot answer's JSON: twice `maxBuffer` in base64
/// beside its other fields.
fn ceiling(max_buffer: usize) -> u32 {
    (2 * (4 * max_buffer.div_ceil(3)) + 65_536).min(64 << 20) as u32
}

/// `execFile`'s options (D3).
#[derive(Debug, Clone, Default)]
pub struct ExecFile {
    /// The child's whole environment; `None`, the app's own.
    pub env: Option<Vec<(String, String)>>,
    /// A `home:`, `app:` or absolute path; `None`, `home:/`.
    pub cwd: Option<String>,
    /// Written to stdin, which is then closed.
    pub input: Option<Vec<u8>>,
    /// Milliseconds before the group is signalled.
    pub timeout: Option<u32>,
    /// The most bytes of stdout or stderr kept (default 1 MiB).
    pub max_buffer: Option<usize>,
    /// Decode output as UTF-8 (otherwise base64 in the reply).
    pub utf8: bool,
}

/// What `execFile` answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecResult {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub code: Option<i64>,
    pub signal: Option<String>,
    pub timed_out: bool,
    /// `stdout` or `stderr`, the one past `maxBuffer`.
    pub overflow: Option<String>,
}

impl ExecResult {
    /// Whether it succeeded as Node's `execFile` resolves: code 0, no
    /// signal, in time, within `maxBuffer`.
    pub fn ok(&self) -> bool {
        self.code == Some(0) && self.signal.is_none() && !self.timed_out && self.overflow.is_none()
    }
}

/// `execFile(program, args, options)`: a step an answer awaits.
pub fn exec_file(program: &str, args: &[&str], options: &ExecFile) -> Request {
    let max = options.max_buffer.unwrap_or(1 << 20);
    let body = json!({
        "kind": "execFile",
        "program": program,
        "args": args,
        "env": options.env,
        "cwd": options.cwd,
        "input": options.input.as_ref().map(|b| exact_runner::agent::base64(b)),
        "timeout": options.timeout,
        "maxBuffer": max,
        "encoding": if options.utf8 { "utf8" } else { "buffer" },
    });
    Request::process(body.to_string().into_bytes(), false, ceiling(max))
}

/// `execPath(target)`.
pub fn exec_path(program: &str) -> Request {
    let body = json!({ "kind": "execPath", "program": program });
    Request::process(body.to_string().into_bytes(), false, 64 << 10)
}

/// `loginShellEnvironment()`: the variables `env.read login:<NAME>` lines name.
pub fn login_environment() -> Request {
    Request::process(
        json!({ "kind": "loginEnv" }).to_string().into_bytes(),
        false,
        1 << 20,
    )
}

/// `appEnvironment()`: the variables `env.read <NAME>` lines name.
pub fn app_environment() -> Request {
    Request::process(
        json!({ "kind": "appEnv" }).to_string().into_bytes(),
        false,
        1 << 20,
    )
}

/// `spawn(program, args)` as a stream that listens (D13): its events reach
/// the source's `parse` as messages, `utf8` output in `data`.
pub fn spawn(program: &str, args: &[&str], utf8: bool) -> Answer {
    let body = json!({
        "kind": "spawn",
        "program": program,
        "args": args,
        "stdio": ["ignore", "pipe", "pipe"],
        "encoding": if utf8 { "utf8" } else { "buffer" },
    });
    Answer::stream(Request::process(
        body.to_string().into_bytes(),
        true,
        1 << 20,
    ))
}

/// A one-shot reply's JSON, or why there is none.
fn body(outcome: &Outcome) -> Result<Json, String> {
    match outcome {
        Outcome::Response(r) if r.status == 200 => serde_json::from_slice(&r.body)
            .map_err(|e| format!("process: the reply is not JSON: {e}")),
        Outcome::Failed { kind, message } => Err(format!("{kind:?}: {message}")),
        other => Err(format!("process: {}", other.summary())),
    }
}

/// What `exec_file`'s request came back with.
pub fn exec_reply(outcome: &Outcome, utf8: bool) -> Result<ExecResult, String> {
    let j = body(outcome)?;
    let bytes = |key: &str| -> Vec<u8> {
        let text = j[key].as_str().unwrap_or("");
        if utf8 {
            text.as_bytes().to_vec()
        } else {
            exact_runner::agent::unbase64(text).unwrap_or_default()
        }
    };
    Ok(ExecResult {
        stdout: bytes("stdout"),
        stderr: bytes("stderr"),
        code: j["code"].as_i64(),
        signal: j["signal"].as_str().map(str::to_string),
        timed_out: j["timedOut"].as_bool().unwrap_or(false),
        overflow: j["overflow"].as_str().map(str::to_string),
    })
}

/// What `exec_path`'s request came back with.
pub fn path_reply(outcome: &Outcome) -> Result<Option<String>, String> {
    Ok(body(outcome)?["path"].as_str().map(str::to_string))
}

/// What an environment read came back with.
pub fn environment_reply(outcome: &Outcome) -> Result<Vec<(String, String)>, String> {
    let j = body(outcome)?;
    Ok((j.as_object().into_iter().flatten())
        .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string())))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_are_the_bindings_and_replies_read_back() {
        let r = exec_file(
            "path:tailscale",
            &["status", "--json"],
            &ExecFile {
                timeout: Some(1500),
                utf8: true,
                ..ExecFile::default()
            },
        );
        assert!(r.is_process() && !r.stream);
        let j: Json = serde_json::from_slice(&r.body).unwrap();
        assert_eq!(
            (j["kind"].as_str(), j["timeout"].as_u64()),
            (Some("execFile"), Some(1500))
        );
        let Answer::Later(s) = spawn("path:t3", &["--serve"], true) else {
            panic!()
        };
        assert!(s.stream && s.socket && s.is_process());
        let reply = Outcome::Response(exact_runner::Response {
            status: 200,
            headers: vec![],
            body: br#"{"stdout":"1.2.0\n","stderr":"","code":0,"signal":null,"timedOut":false,"overflow":null}"#.to_vec(),
        });
        let result = exec_reply(&reply, true).unwrap();
        assert!(result.ok() && result.stdout == b"1.2.0\n");
        let failed = Outcome::Failed {
            kind: exact_runner::FailureKind::NotFound,
            message: "no".into(),
        };
        assert_eq!(path_reply(&failed).unwrap_err(), "NotFound: no");
    }
}
