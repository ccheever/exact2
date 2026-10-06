//! `exact-terminal <app.contract | app.plan> [--size COLSxROWS] [op …]`
//!
//! With no operations and a terminal on stdin and stdout, the app runs full
//! screen (LLP 1101 D10). Otherwise it runs headless at `--size` (80×24 by
//! default) and performs the operations in order, the agent's verbs:
//! `tap <testId>`, `type <testId> <text>`, `key <Name>`, `wheel <testId> <rows>`,
//! `resize <COLSxROWS>`, `tree`, `screenshot <file.txt|file.ans>`, `print`.

use exact_terminal::host::{Host, Key};
use std::process::ExitCode;

fn parse_size(s: &str) -> Option<(usize, usize)> {
    let (c, r) = s.split_once('x')?;
    Some((c.parse().ok()?, r.parse().ok()?))
}

fn key_of(name: &str) -> Key {
    const NAMED: [&str; 14] = [
        "Enter",
        "Tab",
        "Escape",
        "Backspace",
        "Delete",
        "ArrowUp",
        "ArrowDown",
        "ArrowLeft",
        "ArrowRight",
        "Home",
        "End",
        "PageUp",
        "PageDown",
        "Space",
    ];
    if let Some(c) = name.strip_prefix("Control+") {
        return Key::Ctrl(c.chars().next().unwrap_or('c').to_ascii_lowercase());
    }
    if name == "Shift+Tab" {
        return Key::BackTab;
    }
    if name == "Space" {
        return Key::Char(' ');
    }
    match NAMED.iter().find(|n| **n == name) {
        Some(n) => Key::Named(n),
        None => Key::Char(name.chars().next().unwrap_or(' ')),
    }
}

fn tree(host: &Host) -> String {
    let mut out = String::new();
    let kernel = host.kernel();
    fn walk(host: &Host, id: exact_kernel::ViewId, depth: usize, out: &mut String) {
        let kernel = host.kernel();
        let Some(n) = kernel.node(id) else { return };
        if n.is_inline_run() || n.style.display == exact_kernel::Display::None {
            return;
        }
        let r = host.cells_of(id).expect("a node");
        let test_id = n.props.str(exact_kernel::PropId::TestId).unwrap_or("");
        let text = n
            .text_runs()
            .iter()
            .map(|r| r.text.to_string())
            .collect::<String>();
        out.push_str(&format!(
            "{}{:?}{} @{},{} {}x{}{}\n",
            "  ".repeat(depth),
            n.node_type,
            if test_id.is_empty() {
                String::new()
            } else {
                format!(" #{test_id}")
            },
            r.x,
            r.y,
            r.w,
            r.h,
            if text.is_empty() {
                String::new()
            } else {
                format!(" {text:?}")
            }
        ));
        if n.node_type != exact_kernel::NodeType::Text {
            for c in n.children() {
                walk(host, c, depth + 1, out);
            }
        }
    }
    for root in kernel.roots() {
        walk(host, root, 0, &mut out);
    }
    out
}

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let Some(path) = (!args.is_empty()).then(|| args.remove(0)) else {
        eprintln!("usage: exact-terminal <app.contract | app.plan> [--size COLSxROWS] [op …]");
        return ExitCode::from(2);
    };
    let mut size = None;
    if let Some(i) = args.iter().position(|a| a == "--size") {
        size = args.get(i + 1).and_then(|s| parse_size(s));
        args.drain(i..(i + 2).min(args.len()));
    }
    let plan = if path.ends_with(".contract") {
        match contract::compile_path(std::path::Path::new(&path)) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        let bytes = std::fs::read(&path).expect("plan bytes");
        exact_plan::Plan::decode(&bytes).expect("a plan")
    };
    let interactive = args.is_empty() && exact_terminal::term::is_terminal();
    let (cols, rows) = size
        .or_else(|| interactive.then(exact_terminal::term::size).flatten())
        .unwrap_or((80, 24));
    let mut host = match Host::boot(plan, cols, rows) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("boot: {e}");
            return ExitCode::FAILURE;
        }
    };
    if interactive {
        return match exact_terminal::term::run(&mut host) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{e}");
                ExitCode::FAILURE
            }
        };
    }
    if args.is_empty() {
        args.push("print".into());
    }
    let mut ops = args.into_iter();
    while let Some(op) = ops.next() {
        let mut arg = || ops.next().unwrap_or_default();
        match op.as_str() {
            "tap" => {
                let id = arg();
                match host.by_test_id(&id) {
                    Some(v) => host.press(v),
                    None => eprintln!("tap: no node #{id}"),
                }
            }
            "type" => {
                let (id, text) = (arg(), arg());
                match host.by_test_id(&id) {
                    Some(v) => {
                        host.press(v);
                        for c in text.chars() {
                            host.key(Key::Char(c));
                        }
                    }
                    None => eprintln!("type: no node #{id}"),
                }
            }
            "key" => {
                host.key(key_of(&arg()));
            }
            "wheel" => {
                let (id, n) = (arg(), arg());
                if let Some(r) = host.by_test_id(&id).and_then(|v| host.cells_of(v)) {
                    host.wheel(r.x, r.y, n.parse().unwrap_or(1));
                }
            }
            "resize" => {
                if let Some((c, r)) = parse_size(&arg()) {
                    host.resize(c, r);
                }
            }
            "tree" => print!("{}", tree(&host)),
            "print" => print!("{}", host.frame().grid.text()),
            "screenshot" => {
                let file = arg();
                let grid = &host.frame().grid;
                let body = if file.ends_with(".ans") {
                    grid.ansi()
                } else {
                    grid.text()
                };
                if let Err(e) = std::fs::write(&file, body) {
                    eprintln!("screenshot: {e}");
                }
            }
            other => eprintln!("unknown operation {other:?}"),
        }
    }
    ExitCode::SUCCESS
}
