//! The command line every terminal app shares:
//! `<app> [entry.contract | app.plan] [--size COLSxROWS] [--inline | --fullscreen] [op …]`
//!
//! With no operations and a terminal on stdin and stdout, the app runs in it
//! (LLP 1101 D10). Otherwise it runs headless at `--size` (80×24 by
//! default) and performs the operations in order, the agent's verbs:
//! `tap <testId>`, `type <testId> <text>`, `key <Name>`, `paste <text>`,
//! `wheel <testId> <rows>`, `resize <COLSxROWS>`, `wait <ms>`,
//! `until <text>` (wait up to 15 s for the screen to show it), `tree`,
//! `screenshot <file.txt|file.ans>`, `print [--all]` (the screen; `--all`
//! puts the scrollback above it), `document` (the whole laid-out document),
//! `layout` (every node's box as JSON: px, cells, testId, corner radii),
//! `plan` (the running plan's SHA-256, as JSON), and the runner's own reads
//! the agent's `tree` and `layout <id>` make elsewhere: `runner-tree` (every
//! node, its id, parent, children and props, as JSON) and `node <id>` (one
//! node's every row with its value, token and source, as `{"node":…}`).
//!
//! Headless, frames go into a terminal emulator, and those verbs read its
//! screen: what a person would see (LLP 1101.002 §0 P1). `tap`, `type` and
//! `wheel` refuse a node that is not on it.

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
        } else {
            runs(host, id, depth + 1, out);
        }
    }
    /// A paragraph's inline runs that carry a `testId` (a link the agent
    /// taps): named, with their text, where the paragraph's own line is.
    fn runs<D: DataSource>(
        host: &Host<D>,
        id: exact_kernel::ViewId,
        depth: usize,
        out: &mut String,
    ) {
        let kernel = host.kernel();
        let Some(n) = kernel.node(id) else { return };
        for c in n.children() {
            let Some(run) = kernel.node(c) else { continue };
            if let Some(test_id) = run.props.str(exact_kernel::PropId::TestId) {
                let text: String = run.text_runs().iter().map(|r| r.text.to_string()).collect();
                let shown = if host.is_presented_run(c) {
                    ""
                } else {
                    " (off the screen)"
                };
                out.push_str(&format!(
                    "{}Run #{test_id} {text:?}{shown}\n",
                    "  ".repeat(depth)
                ));
            }
            runs(host, c, depth + 1, out);
        }
    }
    for root in kernel.roots() {
        walk(host, root, 0, &mut out);
    }
    out
}

/// Every displayed node's box, as one JSON line in the agent's shape (the
/// `layout` verb, the terminal carrier's own: LLP 1121 stage 11 C, for
/// scripts/scale-check.mjs): the viewport and each node's frame in the
/// kernel's px, the cell (`COLUMN`×`ROW` px), each node's cells as the
/// painter rounds them, its `testId`, and its corners' resolved radii in px
/// (the terminal draws no corner a picture could be measured by), as a host
/// sizes one against the box.
fn layout<D: DataSource>(host: &Host<D>) -> String {
    use exact_kernel::style::cells::{COLUMN, ROW};
    use std::fmt::Write;
    let kernel = host.kernel();
    let env = kernel.env();
    let (cols, rows) = host.size();
    let num = |n: f64| format!("{n}");
    let mut nodes = Vec::new();
    let mut stack: Vec<exact_kernel::ViewId> = kernel.roots();
    stack.reverse();
    while let Some(id) = stack.pop() {
        let Some(n) = kernel.node(id) else { continue };
        if n.is_inline_run() || n.style.display == exact_kernel::Display::None {
            continue;
        }
        let (f, c) = (n.frame, host.cells_of(id).expect("a node"));
        let radius = [
            n.style.border_radius_top_left,
            n.style.border_radius_top_right,
            n.style.border_radius_bottom_right,
            n.style.border_radius_bottom_left,
        ]
        // As a host sizes a corner: a percentage of the box's width across
        // and of its height down (`at_basis`), none below 0, one length where
        // the two agree, else the pair `[across, down]`.
        .map(|d| {
            let (across, down) = (
                d.at_basis(&env, f.width).max(0.0),
                d.at_basis(&env, f.height).max(0.0),
            );
            match across == down {
                true => num(f64::from(across)),
                false => format!("[{},{}]", num(f64::from(across)), num(f64::from(down))),
            }
        });
        let mut node = format!(
            "{{\"id\":{},\"x\":{},\"y\":{},\"w\":{},\"h\":{},\"cells\":{{\"x\":{},\"y\":{},\"w\":{},\"h\":{}}},\"radius\":[{}]",
            id,
            num(f64::from(f.x)),
            num(f64::from(f.y)),
            num(f64::from(f.width)),
            num(f64::from(f.height)),
            c.x,
            c.y,
            c.w,
            c.h,
            radius.join(",")
        );
        if let Some(t) = n
            .props
            .str(exact_kernel::PropId::TestId)
            .filter(|t| !t.is_empty())
        {
            let _ = write!(node, ",\"testId\":{}", json_string(t));
        }
        node.push('}');
        nodes.push(node);
        if n.node_type != exact_kernel::NodeType::Text {
            stack.extend(n.children().into_iter().rev());
        }
    }
    format!(
        "{{\"viewport\":{{\"w\":{},\"h\":{}}},\"cell\":{{\"w\":{},\"h\":{}}},\"nodes\":[{}]}}\n",
        num(cols as f64 * f64::from(COLUMN)),
        num(rows as f64 * f64::from(ROW)),
        num(f64::from(COLUMN)),
        num(f64::from(ROW)),
        nodes.join(",")
    )
}

/// `s` as a JSON string.
fn json_string(s: &str) -> String {
    let mut out = String::from('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The whole laid-out document, as the host sees it rather than as a
/// terminal shows it: the `document` verb.
fn document<D: DataSource>(host: &mut Host<D>) -> String {
    let grid = match host.mode {
        Mode::Fullscreen => host.frame().grid.clone(),
        Mode::Inline => host.render(0, host.document_rows().max(1)).grid,
    };
    grid.text()
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
    run_plan(args, data, plan, mode)
}

/// Run an app whose terminal entry was compiled at build time. The bytes
/// are its encoded plan, typically embedded with `include_bytes!`. No
/// source files or compiler work are needed when the app starts.
pub fn run_embedded<D: DataSource>(
    args: Vec<String>,
    data: D,
    bytes: &[u8],
    mode: Mode,
) -> ExitCode {
    let plan = match exact_plan::Plan::decode(bytes) {
        Ok(plan) => plan,
        Err(e) => {
            eprintln!("embedded terminal plan: {e:?}");
            return ExitCode::FAILURE;
        }
    };
    run_plan(args, data, plan, mode)
}

fn run_plan<D: DataSource>(
    mut args: Vec<String>,
    data: D,
    plan: exact_plan::Plan,
    mut mode: Mode,
) -> ExitCode {
    let mut size = None;
    if let Some(i) = args.iter().position(|a| a == "--size") {
        size = args.get(i + 1).and_then(|s| parse_size(s));
        args.drain(i..(i + 2).min(args.len()));
    }
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
    let mut colors = None;
    if let Some(i) = args
        .iter()
        .position(|a| a == "--color" || a.starts_with("--color="))
    {
        let value = if let Some(value) = args[i].strip_prefix("--color=") {
            let value = value.to_owned();
            args.remove(i);
            value
        } else {
            args.remove(i);
            if i < args.len() {
                args.remove(i)
            } else {
                String::new()
            }
        };
        use crate::color::ColorDepth;
        colors = match value.as_str() {
            "auto" => None,
            "never" => Some(ColorDepth::Monochrome),
            "16" => Some(ColorDepth::Ansi16),
            "256" => Some(ColorDepth::Ansi256),
            "always" | "truecolor" => Some(ColorDepth::TrueColor),
            _ => {
                eprintln!("--color expects auto, never, 16, 256, or truecolor (always)");
                return ExitCode::FAILURE;
            }
        };
    }
    let interactive = args.is_empty() && crate::term::is_terminal();
    let (cols, rows) = size
        .or_else(|| interactive.then(crate::term::size).flatten())
        .unwrap_or((80, 24));
    let mut host = match Host::boot_with_clock(plan, data, mode, cols, rows, interactive) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("boot: {e}");
            return ExitCode::FAILURE;
        }
    };
    host.set_colors(colors.unwrap_or_else(crate::color::ColorDepth::detect));
    if interactive {
        return match crate::term::run(&mut host) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{e}");
                ExitCode::FAILURE
            }
        };
    }
    // Headless: the frames go into a terminal emulator, which the verbs
    // read. Keys stay armed: each verb is one read, against the screen as
    // the last verb left it (LLP 1101.002 §0 P1).
    let mut vt = crate::vt::Vt::new(&mut host);
    // Announcements land at the next wait.
    let woken = Arc::new(AtomicBool::new(true));
    let flag = woken.clone();
    host.listen(Arc::new(move || flag.store(true, Ordering::SeqCst)));
    let start = Instant::now();
    host.live_from(start);
    let pump = |host: &mut Host<D>, vt: &mut crate::vt::Vt| {
        if woken.swap(false, Ordering::SeqCst) {
            host.announced();
        }
        host.tick(start.elapsed().as_secs_f64() * 1000.0);
        vt.render(host);
    };
    pump(&mut host, &mut vt);
    if args.is_empty() {
        args.push("print".into());
    }
    let mut ops = args.into_iter().peekable();
    while let Some(op) = ops.next() {
        let mut arg = || ops.next().unwrap_or_default();
        host.read_at = host.frames;
        if matches!(op.as_str(), "tap" | "type" | "wheel") {
            host.advance_for_input();
        }
        match op.as_str() {
            "tap" => {
                let id = arg();
                match host.by_test_id(&id) {
                    // An inline run (a link in a paragraph) as a click on it.
                    Some(v) if host.is_presented_run(v) => {
                        host.press_run(v);
                    }
                    Some(v) if host.on_screen(v) => host.press(v),
                    Some(_) => {
                        eprintln!("tap: #{id} is not on the screen");
                        return ExitCode::FAILURE;
                    }
                    None => {
                        eprintln!("tap: no node #{id}");
                        return ExitCode::FAILURE;
                    }
                }
            }
            "type" => {
                let (id, text) = (arg(), arg());
                match host.by_test_id(&id) {
                    Some(v) if !host.on_screen(v) => {
                        eprintln!("type: #{id} is not on the screen");
                        return ExitCode::FAILURE;
                    }
                    Some(v) if host.input_eligible(v) => {
                        host.press(v);
                        for c in text.chars() {
                            host.key(Key::Char(c));
                        }
                    }
                    Some(_) => {
                        eprintln!("type: #{id} is not available for input");
                        return ExitCode::FAILURE;
                    }
                    None => {
                        eprintln!("type: no node #{id}");
                        return ExitCode::FAILURE;
                    }
                }
            }
            "key" => {
                if host.key(key_of(&arg())) == crate::host::After::Quit {
                    break;
                }
            }
            "paste" => host.paste(&arg()),
            "wheel" => {
                let (id, n) = (arg(), arg());
                let Ok(rows) = n.parse::<i32>() else {
                    eprintln!("wheel: {n:?} is not a number of rows");
                    return ExitCode::FAILURE;
                };
                // The named node is on the screen (a scroller, or a control
                // a person could point at), and a scroller holds it.
                let moved = host
                    .by_test_id(&id)
                    .filter(|v| host.on_screen(*v))
                    .is_some_and(|v| host.wheel_on(v, rows));
                if !moved {
                    eprintln!("wheel: #{id} is not on the screen in a scroller");
                    return ExitCode::FAILURE;
                }
            }
            "resize" => {
                let size = arg();
                let Some((c, r)) = parse_size(&size) else {
                    eprintln!("resize: {size:?} is not COLSxROWS");
                    return ExitCode::FAILURE;
                };
                host.resize(c, r);
            }
            "wait" => {
                let until = Instant::now() + Duration::from_millis(arg().parse().unwrap_or(100));
                while Instant::now() < until {
                    std::thread::sleep(Duration::from_millis(5));
                    pump(&mut host, &mut vt);
                }
            }
            "until" => {
                // The screen, and what scrolls off it while this waits: a
                // fast reply can pass through between two looks.
                let want = arg();
                let from = vt.scrolled();
                let until = Instant::now() + Duration::from_secs(15);
                loop {
                    pump(&mut host, &mut vt);
                    if vt.text_since(from).contains(&want) {
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
            "print" => {
                let all = ops.next_if(|a| a == "--all").is_some();
                print!("{}", vt.text(all));
            }
            "document" => print!("{}", document(&mut host)),
            "layout" => print!("{}", layout(&host)),
            "runner-tree" => println!("{}", exact_runner::agent::tree(&host.runner)),
            "node" => {
                let id = arg();
                let Ok(id) = id.parse::<u32>() else {
                    eprintln!("node: {id:?} is not a view id");
                    return ExitCode::FAILURE;
                };
                println!(
                    "{{\"node\":{}}}",
                    exact_runner::agent::node(&host.runner, id)
                );
            }
            "plan" => println!("{{\"plan\":\"{}\"}}", host.plan_digest()),
            "screenshot" => {
                let file = arg();
                let body = if file.ends_with(".ans") {
                    vt.ansi()
                } else {
                    vt.text(false)
                };
                if let Err(e) = std::fs::write(&file, body) {
                    eprintln!("screenshot: {file}: {e}");
                    return ExitCode::FAILURE;
                }
            }
            other => {
                eprintln!("unknown operation {other:?}");
                return ExitCode::FAILURE;
            }
        }
        pump(&mut host, &mut vt);
        // Headless, nothing opens: say what would have.
        for url in host.outbound.drain(..) {
            eprintln!("open {url}");
        }
        if vt.unanswered() > 0 {
            eprintln!("{op}: the writer is waiting on a position answer it never got");
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}
