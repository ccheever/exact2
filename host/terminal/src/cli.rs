//! The command line every terminal app shares:
//! `<app> [entry.contract | app.plan] [--size COLSxROWS] [--inline | --fullscreen] [op …]`
//!
//! With no operations and a terminal on stdin and stdout, the app runs in it
//! (LLP 1101 D10). Otherwise it runs headless at `--size` (80×24 by
//! default) and performs the operations in order, the agent's verbs:
//! `tap <testId>`, `type <testId> <text>`, `key <Name>`, `paste <text>`,
//! `wheel <testId> <rows>`, `resize <COLSxROWS>`, `wait <ms>`,
//! `until <text>` (wait up to 15 s for the screen to show it), `tree`,
//! `screenshot <file.txt|file.ans>`, `print` (inline: the whole document).

use crate::host::{Host, Key, Mode};
use exact_runner::DataSource;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn parse_size(s: &str) -> Option<(usize, usize)> {
    let (c, r) = s.split_once('x')?;
    Some((c.parse().ok()?, r.parse().ok()?))
}

/// A key named as the agent names it: `Enter`, `Control+c`, `Shift+Enter`, `a`.
pub fn key_of(name: &str) -> Key {
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
    if name.contains('+') {
        return Key::Chord(name.to_string());
    }
    if name == "Space" {
        return Key::Char(' ');
    }
    match NAMED.iter().find(|n| **n == name) {
        Some(n) => Key::Named(n),
        None => Key::Char(name.chars().next().unwrap_or(' ')),
    }
}

fn tree<D: DataSource>(host: &Host<D>) -> String {
    let mut out = String::new();
    let kernel = host.kernel();
    fn walk<D: DataSource>(
        host: &Host<D>,
        id: exact_kernel::ViewId,
        depth: usize,
        out: &mut String,
    ) {
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

fn screen<D: DataSource>(host: &mut Host<D>, ansi: bool) -> String {
    let grid = match host.mode {
        Mode::Fullscreen => host.frame().grid.clone(),
        Mode::Inline => host.render(0, host.document_rows().max(1)).grid,
    };
    if ansi {
        grid.ansi()
    } else {
        grid.text()
    }
}

/// Run an app: `entry` is the default `.contract` (or `.plan`) when the
/// arguments name none, `mode` the default occupancy.
pub fn run<D: DataSource>(
    mut args: Vec<String>,
    data: D,
    entry: Option<&str>,
    mode: Mode,
) -> ExitCode {
    let path = match args.first() {
        Some(a) if a.ends_with(".contract") || a.ends_with(".plan") => args.remove(0),
        _ => match entry {
            Some(e) => e.to_string(),
            None => {
                eprintln!("usage: exact-terminal <app.contract | app.plan> [--size COLSxROWS] [--inline] [op …]");
                return ExitCode::from(2);
            }
        },
    };
    let mut size = None;
    if let Some(i) = args.iter().position(|a| a == "--size") {
        size = args.get(i + 1).and_then(|s| parse_size(s));
        args.drain(i..(i + 2).min(args.len()));
    }
    let mut mode = mode;
    if let Some(i) = args
        .iter()
        .position(|a| a == "--inline" || a == "--fullscreen")
    {
        mode = if args[i] == "--inline" {
            Mode::Inline
        } else {
            Mode::Fullscreen
        };
        args.remove(i);
    }
    let plan = if path.ends_with(".contract") {
        match contract::compile_path_terminal(std::path::Path::new(&path)) {
            Ok(p) => p,
            Err(all) => {
                for e in all {
                    eprintln!("{e}");
                }
                return ExitCode::FAILURE;
            }
        }
    } else {
        let bytes = std::fs::read(&path).expect("plan bytes");
        exact_plan::Plan::decode(&bytes).expect("a plan")
    };
    let interactive = args.is_empty() && crate::term::is_terminal();
    let (cols, rows) = size
        .or_else(|| interactive.then(crate::term::size).flatten())
        .unwrap_or((80, 24));
    let mut host = match Host::boot(plan, data, mode, cols, rows) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("boot: {e}");
            return ExitCode::FAILURE;
        }
    };
    if interactive {
        return match crate::term::run(&mut host) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{e}");
                ExitCode::FAILURE
            }
        };
    }
    // Headless: the agent's verbs wait explicitly, so keys arm at once.
    host.arm = false;
    host.publish(&crate::term::record(
        host.mode,
        crate::image::Protocol::detect(),
        "",
    ));
    // Announcements land at the next wait.
    let woken = Arc::new(AtomicBool::new(true));
    let flag = woken.clone();
    host.listen(Arc::new(move || flag.store(true, Ordering::SeqCst)));
    let start = Instant::now();
    let pump = |host: &mut Host<D>| {
        if woken.swap(false, Ordering::SeqCst) {
            host.announced();
        }
        host.tick(start.elapsed().as_secs_f64() * 1000.0);
    };
    pump(&mut host);
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
            "paste" => host.paste(&arg()),
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
            "wait" => {
                let until = Instant::now() + Duration::from_millis(arg().parse().unwrap_or(100));
                while Instant::now() < until {
                    std::thread::sleep(Duration::from_millis(5));
                    pump(&mut host);
                }
            }
            "until" => {
                let want = arg();
                let until = Instant::now() + Duration::from_secs(15);
                loop {
                    pump(&mut host);
                    if screen(&mut host, false).contains(&want) {
                        break;
                    }
                    if Instant::now() > until {
                        eprintln!("until: the screen never showed {want:?}");
                        return ExitCode::FAILURE;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
            "tree" => print!("{}", tree(&host)),
            "print" => print!("{}", screen(&mut host, false)),
            "screenshot" => {
                let file = arg();
                let body = screen(&mut host, file.ends_with(".ans"));
                if let Err(e) = std::fs::write(&file, body) {
                    eprintln!("screenshot: {e}");
                }
            }
            other => eprintln!("unknown operation {other:?}"),
        }
    }
    ExitCode::SUCCESS
}
