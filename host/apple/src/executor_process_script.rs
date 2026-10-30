//! A scripted child (LLP 1133.007 §7): `--process <target>=script:<file>`
//! answers a spawn with no process at all. The script says what each
//! incarnation writes to stdout and stderr, how much of its stdin it reads
//! and when, how it exits (a code or a signal), whether its pipes stay open
//! after it exits, which signals it ignores and when it is ready, at times
//! on the agent's clock from the incarnation's start. The last incarnation
//! listed repeats for every later restart.
//!
//! ```json
//! { "incarnations": [ { "pid": 4100, "ready": 300, "ignore": ["SIGTERM"], "openAfterExit": false,
//!     "steps": [ { "at": 0, "stdout": "listening\n" }, { "at": 50, "read": 65536 },
//!                { "readUntil": 5 }, { "at": 100, "exit": 0 } ] } ],
//!   "execFile": { "stdout": "1.2.0\n", "stderr": "", "code": 0, "after": 10 },
//!   "path": "/usr/local/bin/t3" }
//! ```
use serde_json::Value as Json;

/// A script, read when the drive names it.
#[derive(Clone, Debug)]
pub(crate) struct Script {
    json: Json,
    /// The file it came from, which `state.streams` names.
    pub(crate) file: String,
}

impl Script {
    /// The script `file` holds.
    pub(crate) fn read(file: &str) -> Result<Script, String> {
        let text =
            std::fs::read_to_string(file).map_err(|e| format!("--process script {file}: {e}"))?;
        let json =
            serde_json::from_str(&text).map_err(|e| format!("--process script {file}: {e}"))?;
        Ok(Script {
            json,
            file: file.to_string(),
        })
    }

    /// Incarnation `n` (from 1): its entry, or the last one's.
    pub(crate) fn incarnation(&self, n: u32) -> Scripted {
        let list = self.json["incarnations"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let entry = (list.get(n.saturating_sub(1) as usize))
            .or(list.last())
            .cloned()
            .unwrap_or(Json::Null);
        Scripted::new(entry, n)
    }

    /// What `execFile` answers: stdout, stderr, the code or signal, and
    /// after how long.
    pub(crate) fn exec(&self) -> (Vec<u8>, Vec<u8>, Option<i64>, Option<String>, f64) {
        let e = &self.json["execFile"];
        let text = |k: &str| e[k].as_str().unwrap_or("").as_bytes().to_vec();
        let signal = e["signal"].as_str().map(str::to_string);
        let code = if signal.is_some() {
            None
        } else {
            Some(e["code"].as_i64().unwrap_or(0))
        };
        (
            text("stdout"),
            text("stderr"),
            code,
            signal,
            e["after"].as_f64().unwrap_or(0.0),
        )
    }

    /// What `execPath` answers: the script's `path`, else the script itself.
    pub(crate) fn path(&self) -> String {
        self.json["path"]
            .as_str()
            .map_or_else(|| self.file.clone(), str::to_string)
    }
}

/// What a scripted incarnation does next.
#[derive(Debug, PartialEq)]
pub(crate) enum Step {
    /// A chunk on stdout (`false`) or stderr (`true`).
    Out(bool, Vec<u8>),
    /// It exits, with a code or the signal that ended it.
    Exit(Option<i64>, Option<String>),
    /// Its pipes reach end of file.
    Close,
}

/// One scripted incarnation, run by its supervisor on the clock.
pub(crate) struct Scripted {
    entry: Json,
    next: usize,
    /// Its pid, as the script gives it or made up.
    pub(crate) pid: u64,
    /// Bytes waiting in its stdin, which steps read.
    pub(crate) stdin: u64,
    /// Bytes it has read.
    read: u64,
    /// Its stdin was ended.
    ended: bool,
    exited: bool,
    closed: bool,
}

impl Scripted {
    fn new(entry: Json, n: u32) -> Scripted {
        let pid = entry["pid"].as_u64().unwrap_or(40_000 + u64::from(n));
        Scripted {
            entry,
            next: 0,
            pid,
            stdin: 0,
            read: 0,
            ended: false,
            exited: false,
            closed: false,
        }
    }

    fn steps(&self) -> &[Json] {
        self.entry["steps"].as_array().map_or(&[], Vec::as_slice)
    }

    /// When, after its start, it is ready, if ever.
    pub(crate) fn ready_after(&self) -> Option<f64> {
        self.entry["ready"].as_f64()
    }

    /// Whether `signal` leaves it running.
    pub(crate) fn ignores(&self, signal: &str) -> bool {
        signal != "SIGKILL"
            && (self.entry["ignore"].as_array())
                .is_some_and(|list| list.iter().any(|s| s.as_str() == Some(signal)))
    }

    /// Bytes written to its stdin.
    pub(crate) fn write(&mut self, bytes: u64) {
        self.stdin += bytes;
    }

    /// Its stdin was ended.
    pub(crate) fn end(&mut self) {
        self.ended = true;
    }

    /// When its next step is due, after its start; `None` while it waits on
    /// its stdin or has no step left.
    pub(crate) fn due(&self) -> Option<f64> {
        if self.closed {
            return None;
        }
        let step = self.steps().get(self.next)?;
        if let Some(until) = step["readUntil"].as_u64() {
            // It waits for bytes, not time: due as soon as they are there.
            return (self.read + self.stdin >= until || self.ended).then_some(0.0);
        }
        Some(step["at"].as_f64().unwrap_or(0.0))
    }

    /// Every step due by `elapsed` milliseconds after its start, in order.
    pub(crate) fn run(&mut self, elapsed: f64) -> Vec<Step> {
        let mut out = Vec::new();
        while let Some(at) = self.due() {
            if at > elapsed {
                break;
            }
            let step = self.steps()[self.next].clone();
            self.next += 1;
            for (key, stderr) in [("stdout", false), ("stderr", true)] {
                if let Some(text) = step[key].as_str() {
                    out.push(Step::Out(stderr, text.as_bytes().to_vec()));
                }
            }
            if let Some(n) = step["read"].as_u64() {
                let taken = n.min(self.stdin);
                self.stdin -= taken;
                self.read += taken;
            }
            if let Some(until) = step["readUntil"].as_u64() {
                let taken = until.saturating_sub(self.read).min(self.stdin);
                self.stdin -= taken;
                self.read += taken;
            }
            if step.get("exit").is_some() || step.get("signal").is_some() {
                out.extend(self.exit(step["exit"].as_i64(), step["signal"].as_str()));
            }
            if step["close"].as_bool() == Some(true) && !self.closed {
                self.closed = true;
                out.push(Step::Close);
            }
        }
        out
    }

    /// It exits now: by `signal`, or with `code`. Its pipes close with it
    /// unless the script keeps them open.
    pub(crate) fn exit(&mut self, code: Option<i64>, signal: Option<&str>) -> Vec<Step> {
        if self.exited {
            return Vec::new();
        }
        self.exited = true;
        let code = if signal.is_some() {
            None
        } else {
            code.or(Some(0))
        };
        let mut out = vec![Step::Exit(code, signal.map(str::to_string))];
        let open = self.entry["openAfterExit"].as_bool() == Some(true);
        if !open && !self.closed {
            self.closed = true;
            out.push(Step::Close);
        }
        out
    }

    /// A signal to its group: it ends by it unless it ignores it; a
    /// group's `SIGKILL` closes pipes held open too.
    pub(crate) fn signal(&mut self, signal: &str) -> Vec<Step> {
        if self.ignores(signal) || matches!(signal, "SIGSTOP" | "SIGCONT") {
            return Vec::new();
        }
        let mut out = self.exit(None, Some(signal));
        if signal == "SIGKILL" && !self.closed {
            self.closed = true;
            out.push(Step::Close);
        }
        out
    }

    /// Whether it has exited.
    pub(crate) fn exited(&self) -> bool {
        self.exited
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script(json: &str) -> Script {
        Script {
            json: serde_json::from_str(json).unwrap(),
            file: "s.json".into(),
        }
    }

    #[test]
    fn steps_run_on_time_reads_wait_for_bytes_and_the_last_incarnation_repeats() {
        let s = script(
            r#"{"incarnations":[{"pid":7,"steps":[{"at":0,"stdout":"hi"},{"readUntil":3},{"at":10,"stderr":"x"},{"at":20,"exit":3}]},{"steps":[{"at":5,"exit":0}]}]}"#,
        );
        let mut one = s.incarnation(1);
        assert_eq!(one.pid, 7);
        assert_eq!(one.run(0.0), vec![Step::Out(false, b"hi".to_vec())]);
        assert_eq!(one.due(), None, "waits on its stdin");
        one.write(2);
        assert_eq!(one.run(100.0), vec![]);
        one.write(2);
        assert_eq!(
            one.run(100.0),
            vec![
                Step::Out(true, b"x".to_vec()),
                Step::Exit(Some(3), None),
                Step::Close
            ]
        );
        assert_eq!(one.stdin, 1, "it read three of four bytes");
        let mut third = s.incarnation(3);
        assert_eq!(third.run(5.0), vec![Step::Exit(Some(0), None), Step::Close]);
    }

    #[test]
    fn a_signal_ends_it_unless_ignored_and_kill_closes_held_pipes() {
        let s =
            script(r#"{"incarnations":[{"ignore":["SIGTERM"],"openAfterExit":true,"steps":[]}]}"#);
        let mut c = s.incarnation(1);
        assert!(c.signal("SIGTERM").is_empty());
        assert_eq!(
            c.signal("SIGINT"),
            vec![Step::Exit(None, Some("SIGINT".into()))]
        );
        assert_eq!(
            c.signal("SIGKILL"),
            vec![Step::Close],
            "its pipes were held open"
        );
        assert_eq!(s.exec().2, Some(0));
    }
}
