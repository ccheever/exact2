//! `contract tamber serve --stdio` (LLP 1121 stage 3, commit 3): reads, a
//! transaction with its digests, its inverse, a conflict, `propose`, the
//! lock, a commit killed between its renames completed on restart, and a
//! journal that cannot be trusted refused. The server runs on macOS and
//! Linux only.
#![cfg(unix)]

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
    assert_eq!(hello["ok"]["protocol"], "tamber/1.1");
    assert_eq!(hello["ok"]["builds"], json!([]));
    assert_eq!(
        hello["ok"]["files"]["app.tamber"],
        json!(digest(&dir.join("app.tamber")))
    );
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
    // Each side's sRGB clip, for a contrast ratio.
    assert!(
        tuples[0]["srgb"]["light"]
            .as_str()
            .is_some_and(|h| h.starts_with('#') && h.len() == 9),
        "{resolve}"
    );
    assert!(
        tuples[0]["light"].is_string() && tuples[0]["dark"].is_string(),
        "{resolve}"
    );
    let (explain, _) = s.ask(json!({"method": "explain", "token": "contrast_0"}));
    assert_eq!(explain["ok"]["token"], "contrast_0");
    // Each read names the digests it read, as describe does.
    assert_eq!(resolve["ok"]["files"], describe["ok"]["files"], "{resolve}");
    assert_eq!(explain["ok"]["files"], describe["ok"]["files"], "{explain}");
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
    assert_eq!(
        proposed["ok"]["changed"],
        json!([{"file": "app.tamber", "token": "contrast_0"}])
    );
    assert_eq!(proposed["ok"]["files"]["app.tamber"], json!(digest(&file)));
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
        [
            json!({"event": "changed", "files": done["ok"]["files"], "changed": done["ok"]["changed"], "client": "one"})
        ]
    );
    // A second client that saw the old file conflicts instead of overwriting.
    let (conflict, _) = s.ask(json!({"method": "transaction", "client": "two", "files": {"app.tamber": seen}, "ops": [{"op": "set-token", "name": "contrast_0", "value": "#000000"}]}));
    assert_eq!(conflict["error"]["conflict"], true);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), after);
    // A structural edit is a rebuild; the inverse gives the bytes back.
    let (rename, _) = s.ask(json!({"method": "propose", "ops": [{"op": "rename-token", "from": "mute_banner", "to": "banner.mute"}]}));
    assert_eq!(rename["ok"]["class"], "rebuild");
    let (undone, _) =
        s.ask(json!({"method": "undo", "client": "one", "inverse": done["ok"]["inverse"]}));
    assert!(undone["ok"]["inverse"].is_string(), "{undone}");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
    // A restore is no client's write, and an inverse applies once.
    let (forged, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.tamber": digest(&file)}, "ops": [{"op": "restore", "line": 1, "remove": 1, "lines": ["tamber x"], "newline": true}]}));
    assert!(forged["error"].is_object());
    let (again, _) =
        s.ask(json!({"method": "undo", "client": "one", "inverse": done["ok"]["inverse"]}));
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
    let (reply, _) =
        s.ask(json!({"method": "transaction", "client": "one", "files": files, "ops": ops}));
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

const SMALL: &str = "tamber small

axes
  mode = plain | loud

theme
  label = #0b0f14 scheme=fixed
  base = #102030
  ink = #000000
  edge = base
    when mode=loud: color-mix(in srgb, ink 50%, white)
  text.link = LinkText
  platform = -exact-platform-color(ios systemTealColor, light-dark(#30b0c7, #40c8e0))
";

#[test]
fn reads_name_the_chain_the_checker_resolved_and_a_residuals_fallback() {
    let dir = app("chain", SMALL);
    let mut s = Serve::start(&dir, &[]);
    let (explain, _) = s.ask(json!({"method": "explain", "token": "edge"}));
    assert_eq!(explain["ok"]["names"], json!(["base", "ink"]), "{explain}");
    assert_eq!(
        explain["ok"]["whens"],
        json!([{"when": [{"axis": "mode", "value": "loud"}], "value": "color-mix(in srgb, ink 50%, white)"}])
    );
    let (resolve, _) = s.ask(json!({"method": "resolve", "token": "platform"}));
    let tuple = &resolve["ok"]["tuples"][0];
    assert_eq!(tuple["residual"], true);
    assert_eq!(
        tuple["fallback"],
        json!({"light": "#30b0c7ff", "dark": "#40c8e0ff"}),
        "{resolve}"
    );
    let (link, _) = s.ask(json!({"method": "resolve", "token": "text.link"}));
    assert!(
        link["ok"]["tuples"][0]["fallback"]["light"].is_string(),
        "{link}"
    );
    // Metadata by the IR's name: the role's row spelling.
    let (describe, _) = s.ask(json!({"method": "describe"}));
    assert_eq!(
        describe["ok"]["meta"]["tokens"]["-exact-label"],
        json!([["scheme", "fixed"]])
    );
    // A read of a file that is not the design's is refused.
    let (other, _) = s.ask(json!({"method": "describe", "file": "../app.tamber"}));
    assert!(other["error"].is_object());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn requests_are_typed_bounded_and_attributed() {
    let dir = app("strict", SMALL);
    let file = dir.join("app.tamber");
    let mut s = Serve::start(&dir, &[]);
    let seen = digest(&file);
    // A `when` that is not a list is an error, not the default line.
    let (bad, _) = s.ask(json!({"method": "transaction", "client": "one", "files": {"app.tamber": seen}, "ops": [{"op": "set-token", "name": "edge", "when": "mode=loud", "value": "#ffffff"}]}));
    assert!(bad["error"].is_object(), "{bad}");
    let (meta, _) = s.ask(json!({"method": "propose", "ops": [{"op": "add-token", "name": "x", "value": "#ffffff", "meta": [["scheme", "fixed"]]}]}));
    assert!(meta["error"].is_object(), "{meta}");
    // A write names its client; a hello without one keeps the last.
    let op = json!({"op": "set-token", "name": "base", "value": "#203040"});
    let (anonymous, _) =
        s.ask(json!({"method": "transaction", "files": {"app.tamber": seen}, "ops": [op]}));
    assert!(anonymous["error"].is_object(), "{anonymous}");
    s.ask(json!({"method": "hello", "client": "editor"}));
    s.ask(json!({"method": "hello"}));
    // A propose with a stale digest conflicts, as its transaction would.
    let (stale, _) = s.ask(json!({"method": "propose", "files": {"app.tamber": "0"}, "ops": [op]}));
    assert_eq!(stale["error"]["conflict"], true);
    let (done, events) =
        s.ask(json!({"method": "transaction", "files": {"app.tamber": seen}, "ops": [op]}));
    assert_eq!(events[0]["client"], "editor", "{done}");
    assert_eq!(
        std::fs::read_to_string(&file)
            .unwrap()
            .matches("#203040")
            .count(),
        1
    );
    // A line over the limit, and one that is not UTF-8, are answered and
    // the server reads on.
    writeln!(s.input, "{}", "x".repeat(1 << 21)).unwrap();
    s.input.write_all(b"\xff\xfe\n").unwrap();
    for _ in 0..2 {
        let mut line = String::new();
        s.output.read_line(&mut line).unwrap();
        let v: Value = serde_json::from_str(&line).unwrap();
        assert!(v["error"].is_object(), "{v}");
    }
    let (hello, _) = s.ask(json!({"method": "hello"}));
    assert_eq!(hello["ok"]["protocol"], "tamber/1.1");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_undo_holds_while_its_own_file_does_and_dies_with_its_server() {
    let dir = app("undo", SMALL);
    std::fs::write(
        dir.join("brand.tamber"),
        "tamber brand\n\ntheme\n  brand.ink = #101010\n",
    )
    .unwrap();
    let (app_file, brand) = (dir.join("app.tamber"), dir.join("brand.tamber"));
    let before = std::fs::read_to_string(&app_file).unwrap();
    let mut s = Serve::start(&dir, &[]);
    s.ask(json!({"method": "hello", "client": "one"}));
    let (first, _) = s.ask(json!({"method": "transaction", "files": {"app.tamber": digest(&app_file)}, "ops": [{"op": "set-token", "name": "base", "value": "#203040"}]}));
    let (second, _) = s.ask(json!({"method": "transaction", "files": {"brand.tamber": digest(&brand)}, "ops": [{"op": "set-token", "file": "brand.tamber", "name": "brand.ink", "value": "#202020"}]}));
    assert!(second["ok"]["inverse"].is_string(), "{second}");
    // brand.tamber moved; app.tamber is as the first write left it.
    let (undone, _) = s.ask(json!({"method": "undo", "inverse": first["ok"]["inverse"]}));
    assert!(undone["ok"].is_object(), "{undone}");
    assert_eq!(std::fs::read_to_string(&app_file).unwrap(), before);
    drop(s);
    // A new server never answers to an id the last one issued.
    let mut s = Serve::start(&dir, &[]);
    s.ask(json!({"method": "hello", "client": "one"}));
    let (fresh, _) = s.ask(json!({"method": "transaction", "files": {"app.tamber": digest(&app_file)}, "ops": [{"op": "set-token", "name": "base", "value": "#304050"}]}));
    assert_ne!(fresh["ok"]["inverse"], second["ok"]["inverse"]);
    let (old, _) = s.ask(json!({"method": "undo", "inverse": second["ok"]["inverse"]}));
    assert!(old["error"].is_object(), "{old}");
    assert!(std::fs::read_to_string(&brand).unwrap().contains("#202020"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A journal by hand, and whether a server starts over it.
fn start_over(dir: &std::path::Path, journal: Value) -> std::process::Output {
    std::fs::write(dir.join(".exact/tamber.journal"), journal.to_string()).unwrap();
    Command::new(env!("CARGO_BIN_EXE_contract"))
        .args(["tamber", "serve", "--stdio", dir.to_str().unwrap()])
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

#[test]
fn a_journal_that_leaves_the_app_or_lost_its_text_is_kept_and_refused() {
    let dir = app("journal", SMALL);
    std::fs::create_dir_all(dir.join(".exact")).unwrap();
    let outside = dir.with_extension("outside");
    std::fs::write(&outside, "keep").unwrap();
    std::fs::write(dir.join(".exact/tamber-1-0.tmp"), "moved").unwrap();
    let escape = start_over(
        &dir,
        json!({"renames": [{"tmp": "tamber-1-0.tmp", "file": outside.to_str().unwrap(), "digest": "0", "before": "0"}]}),
    );
    assert!(!escape.status.success());
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "keep");
    assert!(dir.join(".exact/tamber.journal").exists());
    let dotted = start_over(
        &dir,
        json!({"renames": [{"tmp": "../app.tamber", "file": "app.tamber", "digest": "0", "before": "0"}]}),
    );
    assert!(!dotted.status.success());
    // A temporary file gone, and the design without the recorded text.
    let lost = start_over(
        &dir,
        json!({"renames": [{"tmp": "tamber-9-9.tmp", "file": "app.tamber", "digest": "0", "before": "0"}]}),
    );
    assert!(!lost.status.success());
    assert!(String::from_utf8_lossy(&lost.stderr).contains("kept"));
    assert!(dir.join(".exact/tamber.journal").exists());
    // A set that cannot complete moves nothing: the first rename waits on
    // the second.
    let before = std::fs::read_to_string(dir.join("app.tamber")).unwrap();
    std::fs::write(
        dir.join("brand.tamber"),
        "tamber brand\n\ntheme\n  brand.ink = #101010\n",
    )
    .unwrap();
    std::fs::write(dir.join(".exact/tamber-2-0.tmp"), "tamber moved\n").unwrap();
    let tmp_digest = {
        use sha2::Digest;
        sha2::Sha256::digest(b"tamber moved\n")
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    let half = start_over(
        &dir,
        json!({"renames": [
            {"tmp": "tamber-2-0.tmp", "file": "app.tamber", "digest": tmp_digest, "before": digest(&dir.join("app.tamber"))},
            {"tmp": "tamber-2-1.tmp", "file": "brand.tamber", "digest": "0", "before": "0"}
        ]}),
    );
    assert!(!half.status.success());
    assert_eq!(
        std::fs::read_to_string(dir.join("app.tamber")).unwrap(),
        before
    );
    assert!(dir.join(".exact/tamber-2-0.tmp").exists());
    let garbled = {
        std::fs::write(dir.join(".exact/tamber.journal"), "{\"renames\": [").unwrap();
        Command::new(env!("CARGO_BIN_EXE_contract"))
            .args(["tamber", "serve", "--stdio", dir.to_str().unwrap()])
            .stdin(Stdio::null())
            .output()
            .unwrap()
    };
    assert!(!garbled.status.success());
    // A set none of which landed, over a file an editor changed since, is
    // withdrawn: the editor's text stays.
    let withdrawn = start_over(
        &dir,
        json!({"renames": [{"tmp": "tamber-2-0.tmp", "file": "app.tamber", "digest": tmp_digest, "before": "0"}]}),
    );
    assert!(
        withdrawn.status.success(),
        "{}",
        String::from_utf8_lossy(&withdrawn.stderr)
    );
    assert!(String::from_utf8_lossy(&withdrawn.stderr).contains("withdrawn"));
    assert_eq!(
        std::fs::read_to_string(dir.join("app.tamber")).unwrap(),
        before
    );
    // Gone, an orphan temporary file is swept at start.
    std::fs::write(dir.join(".exact/tamber-2-0.tmp"), "tamber moved\n").unwrap();
    let mut s = Serve::start(&dir, &[]);
    s.ask(json!({"method": "hello"}));
    assert!(!dir.join(".exact/tamber-1-0.tmp").exists());
    assert!(!dir.join(".exact/tamber-2-0.tmp").exists());
    let _ = std::fs::remove_file(&outside);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The wire's own requests (LLP 1122 T1): `compare` reads a commit's versions
/// as token edits, `revert-plan` inverts one on today's file (and refuses
/// where the file moved on its edit), `import-plan` plans an import from a
/// source's text. None writes.
#[test]
fn compare_revert_plan_and_import_plan() {
    const V1: &str = "tamber t\n\ntheme\n  ink = #000000\n  edge = #101010\n";
    let v2 = V1.replace("#101010", "#202020");
    let dir = app("history", &v2);
    let mut s = Serve::start(&dir, &[]);
    let pair = json!([{"file": "app.tamber", "before": V1, "after": v2}]);
    let (compared, _) = s.ask(json!({"method": "compare", "pairs": pair}));
    assert_eq!(
        compared["ok"]["files"][0]["edits"],
        json!([{"what": "token", "name": "edge", "when": [], "before": "#101010", "after": "#202020"}])
    );
    let (bad, _) = s.ask(json!({"method": "compare", "pairs": [{"file": "app.tamber", "before": "tamber\n  ??", "after": V1}]}));
    assert!(bad["ok"]["files"][0]["edits"].is_null());
    // On the file as the commit left it.
    let (plan, _) = s.ask(json!({"method": "revert-plan", "pairs": pair}));
    assert_eq!(
        plan["ok"]["ops"],
        json!([{"op": "set-token", "file": "app.tamber", "name": "edge", "when": [], "value": "#101010", "comment": null}])
    );
    assert_eq!(plan["ok"]["moved"], json!([]));
    // Moved since, on the commit's own edit: refused, naming it.
    std::fs::write(dir.join("app.tamber"), v2.replace("#202020", "#2a2a2a")).unwrap();
    let (clash, _) = s.ask(json!({"method": "revert-plan", "pairs": pair}));
    assert_eq!(clash["ok"]["ops"], json!([]));
    assert_eq!(clash["ok"]["moved"], json!(["app.tamber"]));
    assert_eq!(clash["ok"]["conflicts"][0]["edit"]["name"], "edge");
    assert_eq!(clash["ok"]["conflicts"][0]["now"], "#2a2a2a");
    // A file the commit created has no inverse.
    let (created, _) = s.ask(json!({"method": "revert-plan", "pairs": [{"file": "app.tamber", "before": null, "after": V1}]}));
    assert!(created["ok"]["conflicts"][0]["why"]
        .as_str()
        .unwrap()
        .contains("created"));
    // `requires`: a file the write does not edit, as it was read.
    std::fs::write(
        dir.join("brand.tamber"),
        "tamber b\n\ntheme\n  b.ink = #010101\n",
    )
    .unwrap();
    let app_digest = digest(&dir.join("app.tamber"));
    let (guarded, _) = s.ask(json!({"method": "transaction", "client": "t", "files": {"app.tamber": app_digest}, "requires": {"brand.tamber": "0".repeat(64)}, "ops": [{"op": "set-token", "name": "ink", "value": "#000001"}]}));
    assert_eq!(guarded["error"]["conflict"], true, "{guarded}");
    assert_eq!(guarded["error"]["stale"], json!(["brand.tamber"]));
    let css = include_str!("../../../../tamber/tests/import/shadcn-neutral.css");
    let (imported, _) = s.ask(json!({"method": "import-plan", "kind": "shadcn", "source": css}));
    assert_eq!(
        imported["ok"]["counted"],
        json!({"source": 47, "mapped": 46, "reported": 1})
    );
    assert_eq!(imported["ok"]["offers"].as_array().unwrap().len(), 5);
    // A DTCG export of several files, each named as the file picker gave it.
    let figma =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tamber/tests/import/figma");
    let sources: Vec<Value> = std::fs::read_dir(&figma)
        .unwrap()
        .map(|e| {
            let p = e.unwrap().path();
            json!({"name": p.file_name().unwrap().to_str().unwrap(), "text": std::fs::read_to_string(&p).unwrap()})
        })
        .collect();
    let (figma, _) = s.ask(json!({"method": "import-plan", "kind": "dtcg", "sources": sources}));
    assert_eq!(
        figma["ok"]["counted"],
        json!({"source": 104, "mapped": 40, "reported": 64}),
        "{figma}"
    );
    // A file name of any characters is no way to stop the server.
    let (named, _) =
        s.ask(json!({"method": "import-plan", "kind": "dtcg", "name": "日本語", "source": "{}"}));
    assert!(
        named.get("ok").is_some() || named.get("error").is_some(),
        "{named}"
    );
    let (astryx, _) = s.ask(json!({"method": "import-plan", "kind": "astryx", "source": "{}"}));
    assert!(astryx["error"]["message"]
        .as_str()
        .unwrap()
        .contains("bun exact.mjs tamber import astryx"));
    // Nothing was written.
    assert_eq!(
        std::fs::read_to_string(dir.join("app.tamber")).unwrap(),
        v2.replace("#202020", "#2a2a2a")
    );
    let _ = std::fs::remove_dir_all(&dir);
}
