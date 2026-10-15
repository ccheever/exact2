//! `contract tamber serve --stdio` (LLP 1121 stage 3, commit 3): reads, a
//! transaction with its digests, its inverse, a conflict, `propose`, the
//! lock, and a commit killed between its renames completed on restart.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

const CLONE: &str = include_str!("../../../../tamber/tests/fmt/bluesky-clone.tamber");

fn app(tag: &str, design: &str) -> std::path::PathBuf {
    let dir =
        std::env::temp_dir().join(format!("exact2-tamber-serve-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "component App\n  view\n    text \"a\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.tamber"),
        exact_tamber::format(design).unwrap(),
    )
    .unwrap();
    dir
}

struct Serve {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    next: u64,
}

impl Serve {
    fn start(dir: &std::path::Path, env: &[(&str, &str)]) -> Serve {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_contract"));
        cmd.args(["tamber", "serve", "--stdio", dir.to_str().unwrap()])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (k, v) in env {
            cmd.env(k, v);
        }
        let mut child = cmd.spawn().unwrap();
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        Serve {
            child,
            input,
            output,
            next: 0,
        }
    }

    /// One request; its reply, and the event lines that followed it.
    fn ask(&mut self, mut request: Value) -> (Value, Vec<Value>) {
        self.next += 1;
        request["id"] = json!(self.next);
        writeln!(self.input, "{request}").unwrap();
        self.input.flush().unwrap();
        let mut events = Vec::new();
        loop {
            let mut line = String::new();
            if self.output.read_line(&mut line).unwrap() == 0 {
                return (Value::Null, events);
            }
            let v: Value = serde_json::from_str(&line).unwrap();
            if v.get("event").is_some() {
                events.push(v);
                continue;
            }
            // A write's event follows its reply.
            if v["ok"]["inverse"].is_string() {
                let mut e = String::new();
                self.output.read_line(&mut e).unwrap();
                events.push(serde_json::from_str(&e).unwrap());
            }
            return (v, events);
        }
    }
}

impl Drop for Serve {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn digest(path: &std::path::Path) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(std::fs::read(path).unwrap())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[test]
fn reads_answer_on_the_clones_design() {
    let dir = app("reads", CLONE);
    let mut s = Serve::start(&dir, &[]);
    let (hello, _) = s.ask(json!({"method": "hello", "client": "test"}));
    assert_eq!(hello["ok"]["protocol"], "tamber/1.0");
    assert_eq!(hello["ok"]["builds"], json!([]));
    let (describe, _) = s.ask(json!({"method": "describe"}));
    assert_eq!(describe["ok"]["ir"]["name"], "bluesky");
    assert_eq!(
        describe["ok"]["files"]["app.tamber"],
        json!(digest(&dir.join("app.tamber")))
    );
    assert_eq!(
        describe["ok"]["meta"]["tokens"]["white_100"],
        json!([["scheme", "fixed"]])
    );
    let (resolve, _) = s.ask(json!({"method": "resolve", "token": "contrast_0"}));
    let tuples = resolve["ok"]["tuples"].as_array().unwrap();
    assert_eq!(tuples.len(), 2);
    assert_eq!(tuples[0]["modes"], json!({"dark-style": "dim"}));
    assert!(
        tuples[0]["light"].is_string() && tuples[0]["dark"].is_string(),
        "{resolve}"
    );
    let (explain, _) = s.ask(json!({"method": "explain", "token": "contrast_0"}));
    assert_eq!(explain["ok"]["token"], "contrast_0");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_transaction_writes_a_line_its_inverse_undoes_and_a_stale_digest_conflicts() {
    let dir = app("write", CLONE);
    let file = dir.join("app.tamber");
    let before = std::fs::read_to_string(&file).unwrap();
    let mut s = Serve::start(&dir, &[]);
    let op =
        json!({"op": "set-token", "name": "contrast_0", "value": "light-dark(#fefefe, #151d28)"});
    // `propose` changes nothing on disk.
    let (proposed, events) = s.ask(json!({"method": "propose", "ops": [op]}));
    assert_eq!(proposed["ok"]["class"], "live");
    assert_eq!(proposed["ok"]["changed"], json!(["contrast_0"]));
    assert!(proposed["ok"]["diff"]
        .as_str()
        .unwrap()
        .contains("+  contrast_0 = light-dark(#fefefe, #151d28)"));
    assert!(events.is_empty());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
    let seen = digest(&file);
    let (done, events) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.tamber": seen}, "ops": [op]}));
    let after = std::fs::read_to_string(&file).unwrap();
    assert_eq!(
        after
            .lines()
            .zip(before.lines())
            .filter(|(a, b)| a != b)
            .count(),
        1
    );
    assert_eq!(done["ok"]["files"]["app.tamber"], json!(digest(&file)));
    assert_eq!(
        events,
        [json!({"event": "changed", "files": done["ok"]["files"], "client": "one"})]
    );
    // A second client that saw the old file conflicts instead of overwriting.
    let (conflict, _) = s.ask(json!({"method": "transaction", "files": {"app.tamber": seen}, "ops": [{"op": "set-token", "name": "contrast_0", "value": "#000000"}]}));
    assert_eq!(conflict["error"]["conflict"], true);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), after);
    // A structural edit is a rebuild; the inverse gives the bytes back.
    let (rename, _) = s.ask(json!({"method": "propose", "ops": [{"op": "rename-token", "from": "mute_banner", "to": "banner.mute"}]}));
    assert_eq!(rename["ok"]["class"], "rebuild");
    let (undone, _) = s.ask(json!({"method": "undo", "inverse": done["ok"]["inverse"]}));
    assert!(undone["ok"]["inverse"].is_string(), "{undone}");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
    // A restore is no client's write, and an inverse applies once.
    let (forged, _) = s.ask(json!({"method": "transaction", "files": {"app.tamber": digest(&file)}, "ops": [{"op": "restore", "line": 1, "remove": 1, "lines": ["tamber x"], "newline": true}]}));
    assert!(forged["error"].is_object());
    let (again, _) = s.ask(json!({"method": "undo", "inverse": done["ok"]["inverse"]}));
    assert!(again["error"].is_object());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_second_server_for_the_app_refuses_to_start() {
    let dir = app("lock", CLONE);
    let mut first = Serve::start(&dir, &[]);
    first.ask(json!({"method": "hello"}));
    let second = Command::new(env!("CARGO_BIN_EXE_contract"))
        .args(["tamber", "serve", "--stdio", dir.to_str().unwrap()])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!second.status.success());
    assert!(String::from_utf8_lossy(&second.stderr).contains("another tamber server"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_commit_killed_between_its_renames_is_completed_on_restart() {
    let dir = app("crash", CLONE);
    std::fs::write(
        dir.join("brand.tamber"),
        "tamber brand\n\ntheme\n  brand.ink = #101010\n",
    )
    .unwrap();
    let ops = json!([
        {"op": "set-token", "name": "contrast_0", "value": "light-dark(#fefefe, #151d28)"},
        {"op": "set-token", "file": "brand.tamber", "name": "brand.ink", "value": "#202020"}
    ]);
    let files = json!({"app.tamber": digest(&dir.join("app.tamber")), "brand.tamber": digest(&dir.join("brand.tamber"))});
    let mut s = Serve::start(&dir, &[("EXACT_TAMBER_CRASH_AFTER_RENAME", "1")]);
    let (reply, _) = s.ask(json!({"method": "transaction", "files": files, "ops": ops}));
    assert!(
        reply.is_null(),
        "the server died between its renames: {reply}"
    );
    drop(s);
    // One file renamed, one not: the journal says which.
    let renamed = std::fs::read_to_string(dir.join("app.tamber"))
        .unwrap()
        .contains("#fefefe")
        ^ std::fs::read_to_string(dir.join("brand.tamber"))
            .unwrap()
            .contains("#202020");
    assert!(renamed, "exactly one file was renamed before the crash");
    assert!(dir.join(".exact/tamber.journal").exists());
    let mut s = Serve::start(&dir, &[]);
    s.ask(json!({"method": "hello"}));
    assert!(std::fs::read_to_string(dir.join("app.tamber"))
        .unwrap()
        .contains("#fefefe"));
    assert!(std::fs::read_to_string(dir.join("brand.tamber"))
        .unwrap()
        .contains("#202020"));
    assert!(!dir.join(".exact/tamber.journal").exists());
    let _ = std::fs::remove_dir_all(&dir);
}
