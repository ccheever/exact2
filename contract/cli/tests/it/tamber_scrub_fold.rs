//! `scrub-fold` (LLP 1121 D13): a scrub's frame folded over the base the
//! scrub opened on, with no texts. The rows it answers, set on the committed
//! plan, must be the plan a full compile of the frame's text gives, row for
//! row: fuzzed over dials, tokens that read others, `when` lines, a synthetic
//! token (a Contract row's expression over tokens), a role override, a
//! length and a type token. The server runs on macOS and Linux only.
#![cfg(unix)]

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

const DESIGN: &str = "tamber fold

axes
  dark-style = dim | black  under scheme=dark

dials
  accent: color = #1083fe

theme
  accent.fill = light-dark(accent, #208bfe)
  accent.faded = color-mix(in srgb, accent.fill 50%, #ffffff)
  surface.page = light-dark(#ffffff, #151d28)
    when dark-style=black: #000000
  text.primary = light-dark(#0b0f14, #f1f3f5)
  border.hairline = light-dark(#d4dbe2, #2e4052)
  space.pad = 12px
  type.body = -exact-body/1.35
  AccentColor = #1d9bf0
";

const CONTRACT: &str = "component App
  view
    column background-color=\"surface.page\" padding=\"space.pad\" box-shadow=\"0 2px 8px color-mix(in srgb, border.hairline 60%, transparent)\"
      text \"a\" color=\"text.primary\" font=\"type.body\"
      text \"b\" color=\"accent.fill\"
      text \"c\" color=\"accent.faded\"
";

fn app() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("exact2-scrub-fold-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("app.contract"), CONTRACT).unwrap();
    std::fs::write(
        dir.join("app.tamber"),
        exact_tamber::format(DESIGN).unwrap(),
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
    fn start(dir: &std::path::Path) -> Serve {
        let mut child = Command::new(env!("CARGO_BIN_EXE_contract"))
            .args(["tamber", "serve", "--stdio", dir.to_str().unwrap()])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        Serve {
            child,
            input,
            output,
            next: 0,
        }
    }

    fn ask(&mut self, mut request: Value) -> Value {
        self.next += 1;
        request["id"] = json!(self.next);
        writeln!(self.input, "{request}").unwrap();
        self.input.flush().unwrap();
        loop {
            let mut line = String::new();
            assert!(
                self.output.read_line(&mut line).unwrap() > 0,
                "the server stopped"
            );
            let v: Value = serde_json::from_str(&line).unwrap();
            if v.get("event").is_none() {
                return v;
            }
        }
    }
}

impl Drop for Serve {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Each theme row of the app compiled with design `text`: its name, value
/// and slices.
fn rows(dir: &std::path::Path, text: &str) -> Vec<(String, String, Vec<String>)> {
    let root = dir.join("app.contract");
    let path = contract::design_path(&root).unwrap();
    let (plan, _) = contract::with_design_text(&path, text.to_string(), || {
        contract::compile_path_all(&root, false)
    })
    .unwrap_or_else(|e| {
        panic!(
            "{}",
            e.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    plan.theme
        .iter()
        .map(|r| {
            let slices = plan.theme_slices[r.slices.start as usize..][..r.slices.len as usize]
                .iter()
                .map(|s| plan.str(s.value).to_string())
                .collect();
            (
                plan.str(r.name).to_string(),
                plan.str(r.value).to_string(),
                slices,
            )
        })
        .collect()
}

/// A small deterministic generator (a 64-bit LCG).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn colour(&mut self) -> String {
        format!("#{:06x}", self.next() & 0xff_ffff)
    }
}

#[test]
fn a_frame_patched_from_its_fold_is_its_full_compile() {
    let dir = app();
    let mut serve = Serve::start(&dir);
    let hello = serve.ask(json!({"method": "hello"}));
    let files = hello["ok"]["files"].clone();
    let committed = std::fs::read_to_string(dir.join("app.tamber")).unwrap();
    let base = rows(&dir, &committed);
    let mut rng = Rng(0x5eed);
    let (mut live, mut synthetic, mut role) = (0, 0, 0);
    for i in 0..48 {
        let op = match rng.next() % 8 {
            0 => json!({"op": "set-dial", "name": "accent", "value": rng.colour()}),
            1 => {
                json!({"op": "set-token", "name": "accent.fill", "value": format!("light-dark(accent, {})", rng.colour())})
            }
            2 => {
                json!({"op": "set-token", "name": "surface.page", "value": format!("light-dark(#ffffff, {})", rng.colour())})
            }
            3 => {
                json!({"op": "set-token", "name": "surface.page", "when": [{"axis": "dark-style", "value": "black"}], "value": rng.colour()})
            }
            4 => {
                json!({"op": "set-token", "name": "border.hairline", "value": format!("light-dark({}, {})", rng.colour(), rng.colour())})
            }
            5 => json!({"op": "set-token", "name": "AccentColor", "value": rng.colour()}),
            6 => {
                json!({"op": "set-token", "name": "space.pad", "value": format!("{}px", 4 + rng.next() % 40)})
            }
            _ => {
                json!({"op": "set-token", "name": "type.body", "value": format!("-exact-body/{}", 1.0 + (rng.next() % 8) as f64 / 10.0)})
            }
        };
        let fold =
            serve.ask(json!({"method": "scrub-fold", "ops": [op], "files": files, "open": i == 0}));
        let proposed =
            serve.ask(json!({"method": "propose", "ops": [op], "files": files, "texts": true}));
        assert_eq!(
            fold["ok"]["class"], proposed["ok"]["class"],
            "{op}: the class\n{fold}\n{proposed}"
        );
        if fold["ok"]["class"] != "live" {
            continue;
        }
        live += 1;
        // An edit to the value the file has writes no text.
        let text = proposed["ok"]["texts"]["app.tamber"]
            .as_str()
            .unwrap_or(&committed);
        let mut patched = base.clone();
        for v in fold["ok"]["values"].as_array().unwrap() {
            let css: Vec<String> = serde_json::from_value(v["css"].clone()).unwrap();
            synthetic += usize::from(v["name"].as_str().unwrap().contains('('));
            role += usize::from(v["name"] == "AccentColor");
            let Some(row) = patched
                .iter_mut()
                .find(|r| r.0 == v["name"].as_str().unwrap())
            else {
                continue;
            };
            row.1.clone_from(&css[0]);
            if !row.2.is_empty() {
                row.2 = css;
            }
        }
        assert_eq!(patched, rows(&dir, text), "{op}: the patched rows");
    }
    assert!(
        live > 30 && synthetic > 0 && role > 0,
        "{live} live frames, {synthetic} synthetic rows, {role} role rows"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
