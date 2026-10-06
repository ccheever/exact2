//! The agent's verbs, run as the agent runs them: the binary, headless, on
//! an entry compiled with the terminal profile (LLP 1101.002 §0 P1, P7).

use std::path::PathBuf;
use std::process::Command;

const APP: &str = r#"component App
  state picked = "none"
  state draft = ""
  action pick(v: string)
    picked = v
  action write(value: string)
    draft = value
  view
    column
      text `picked ${picked}`
      input value=draft input=write placeholder="new task" testId="field"
      input value="locked" disabled=true testId="locked"
      button commandfor="d" command="show-modal" testId="open"
        text "open"
      dialog id="d" closedby="any"
        scroll max-height="3lh" testId="list"
          each v in ["alpha", "beta", "gamma", "delta", "epsilon"] key=v
            button press=pick(v) commandfor="d" command="close" text-align="start" testId=v
              text v
"#;

fn entry(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("exact-terminal-cli-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("app.contract");
    std::fs::write(&path, APP).unwrap();
    path
}

fn run(name: &str, ops: &[&str]) -> (bool, String, String) {
    run_in("--inline", name, ops)
}

fn run_in(mode: &str, name: &str, ops: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_exact-terminal"))
        .arg(entry(name))
        .args(["--size", "40x12", mode])
        .args(ops)
        .output()
        .expect("runs");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn wheel_and_tap_refuse_what_is_not_on_the_screen() {
    let (ok, out, err) = run("wheel", &["tap", "open", "wheel", "list", "2", "print"]);
    assert!(ok, "{err}");
    assert!(out.contains("delta") && !out.contains("alpha"), "{out}");
    // `epsilon` is below the list's three rows: neither verb reaches it.
    let (ok, _, err) = run("wheel-off", &["tap", "open", "wheel", "epsilon", "1"]);
    assert!(!ok && err.contains("not on the screen"), "{err}");
    let (ok, _, err) = run("tap-off", &["tap", "open", "tap", "epsilon"]);
    assert!(!ok && err.contains("not on the screen"), "{err}");
}

#[test]
fn a_verb_that_cannot_do_its_work_fails() {
    for ops in [
        &["tap", "open", "wheel", "list", "lots"][..],
        &["resize", "soon"][..],
        &["screenshot", "/no/such/dir/x.txt"][..],
    ] {
        let (ok, _, err) = run(&ops.join("-").replace('/', ""), ops);
        assert!(!ok, "{ops:?} succeeded: {err}");
    }
}

/// A bare field in a terminal entry is one row with a fill, no border
/// (LLP 1101.002 §0 P7), and a disabled one is faint.
#[test]
fn a_terminal_field_is_a_filled_row() {
    let (ok, out, err) = run("field", &["print"]);
    assert!(ok, "{err}");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[1].trim_end(), "new task", "{out}");
    assert_eq!(lines[2].trim_end(), "locked", "{out}");
    assert!(
        !out.contains('╭') && !out.contains('│'),
        "a field drew a border:\n{out}"
    );
    let shot = entry("field").with_file_name("field.ans");
    // Full screen, the screenshot is the grid, faint included.
    let (ok, _, err) = run_in(
        "--fullscreen",
        "field",
        &["screenshot", shot.to_str().unwrap()],
    );
    assert!(ok, "{err}");
    let ans = std::fs::read_to_string(&shot).unwrap();
    // The fill: #303030 behind the placeholder.
    assert!(ans.contains("48;2;48;48;48"), "{ans:?}");
    let before = &ans[..ans.find("locked").unwrap()];
    let sgr = &before[before.rfind("\x1b[").unwrap()..];
    let params: Vec<&str> = sgr[2..sgr.find('m').unwrap()].split(';').collect();
    assert!(params[..2].contains(&"2"), "not faint: {sgr:?}");
}
