//! The agent's verbs, run as the agent runs them: the binary, headless, on
//! an entry compiled with the terminal profile (LLP 1101.002 §0 P1, P7).

use std::path::PathBuf;
use std::process::Command;

#[cfg(unix)]
#[test]
fn termination_signals_restore_the_terminal() {
    for mode in ["--inline", "--fullscreen"] {
        for signal in [libc::SIGTERM, libc::SIGHUP, libc::SIGINT, libc::SIGQUIT] {
            signal_restores(mode, signal, false);
        }
    }
    for mode in ["--inline", "--fullscreen"] {
        signal_restores(mode, libc::SIGTERM, true);
        signal_restores(mode, 0, false);
    }
}

#[cfg(unix)]
fn signal_restores(mode: &str, signal: i32, pending: bool) {
    use std::fs::File;
    use std::io::{Read, Write};
    use std::mem::MaybeUninit;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::process::ExitStatusExt;
    use std::time::{Duration, Instant};

    let (mut master_fd, mut slave_fd) = (-1, -1);
    assert_eq!(
        unsafe {
            libc::openpty(
                &mut master_fd,
                &mut slave_fd,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        },
        0
    );
    let mut master = unsafe { File::from_raw_fd(master_fd) };
    let slave = unsafe { File::from_raw_fd(slave_fd) };
    let mut before = MaybeUninit::<libc::termios>::uninit();
    assert_eq!(
        unsafe { libc::tcgetattr(slave.as_raw_fd(), before.as_mut_ptr()) },
        0
    );
    let before = unsafe { before.assume_init() };
    assert_ne!(before.c_lflag & libc::ICANON, 0);
    assert_eq!(
        unsafe { libc::fcntl(master.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK) },
        0
    );

    let mut command = if pending {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args(["--exact", "pending_data_child", "--nocapture"]);
        command.env("EXACT_TERMINAL_PENDING_TEST", mode);
        command
    } else {
        let path = entry("signal");
        std::fs::write(
            &path,
            "component App\n  view\n    column\n      text \"Ready\"\n",
        )
        .unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_exact-terminal"));
        command.arg(path).args(["--size", "20x6", mode]);
        command
    };
    let mut child = command
        .stdin(slave.try_clone().unwrap())
        .stdout(slave.try_clone().unwrap())
        .stderr(slave.try_clone().unwrap())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut output = Vec::new();
    loop {
        let mut chunk = [0; 4096];
        if let Ok(n) = master.read(&mut chunk) {
            output.extend_from_slice(&chunk[..n]);
        }
        let mut now = MaybeUninit::<libc::termios>::uninit();
        assert_eq!(
            unsafe { libc::tcgetattr(slave.as_raw_fd(), now.as_mut_ptr()) },
            0
        );
        let now = unsafe { now.assume_init() };
        let ready = output.windows(5).any(|part| part == b"Ready");
        let work = !pending || output.windows(12).any(|part| part == b"WORK_PENDING");
        if ready && work && now.c_lflag & libc::ICANON == 0 {
            break;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!(
                "terminal did not enter raw mode: {}",
                String::from_utf8_lossy(&output)
            );
        }
        assert!(
            child.try_wait().unwrap().is_none(),
            "child exited before signal: {}",
            String::from_utf8_lossy(&output)
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    if signal == 0 {
        master.write_all(&[3]).unwrap();
    } else {
        assert_eq!(unsafe { libc::kill(child.id() as i32, signal) }, 0);
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        let mut chunk = [0; 4096];
        if let Ok(n) = master.read(&mut chunk) {
            output.extend_from_slice(&chunk[..n]);
        }
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("child did not exit on signal {signal}");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    if signal == 0 {
        assert!(status.success(), "{mode}, pending={pending}: {status}");
    } else {
        assert_eq!(status.signal(), Some(signal), "{mode}, pending={pending}");
    }
    let mut after = MaybeUninit::<libc::termios>::uninit();
    assert_eq!(
        unsafe { libc::tcgetattr(slave.as_raw_fd(), after.as_mut_ptr()) },
        0
    );
    let after = unsafe { after.assume_init() };
    assert_eq!(after.c_lflag & libc::ICANON, before.c_lflag & libc::ICANON);
    assert_eq!(after.c_lflag & libc::ECHO, before.c_lflag & libc::ECHO);
    let mut chunk = [0; 4096];
    while let Ok(n) = master.read(&mut chunk) {
        if n == 0 {
            break;
        }
        output.extend_from_slice(&chunk[..n]);
    }
    assert!(
        output
            .windows(b"\x1b[?25h".len())
            .any(|part| part == b"\x1b[?25h"),
        "cursor was not restored: {}",
        String::from_utf8_lossy(&output)
    );
    if mode == "--fullscreen" {
        assert!(
            output
                .windows(b"\x1b[?1049l".len())
                .any(|part| part == b"\x1b[?1049l"),
            "alternate screen was not left: {}",
            String::from_utf8_lossy(&output)
        );
    }
}

#[cfg(unix)]
#[test]
fn signal_restores_raw_mode_when_output_is_blocked() {
    use std::fs::File;
    use std::mem::MaybeUninit;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::process::ExitStatusExt;
    use std::time::{Duration, Instant};

    let (mut master_fd, mut slave_fd) = (-1, -1);
    assert_eq!(
        unsafe {
            libc::openpty(
                &mut master_fd,
                &mut slave_fd,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        },
        0
    );
    let _master = unsafe { File::from_raw_fd(master_fd) };
    let slave = unsafe { File::from_raw_fd(slave_fd) };
    assert_eq!(unsafe { libc::tcflow(slave.as_raw_fd(), libc::TCOOFF) }, 0);
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "blocked_output_child", "--nocapture"])
        .env("EXACT_TERMINAL_BLOCKED_OUTPUT_TEST", "1")
        .stdin(slave.try_clone().unwrap())
        .stdout(slave.try_clone().unwrap())
        .stderr(slave.try_clone().unwrap())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut stable = 0;
    loop {
        let mut termios = MaybeUninit::<libc::termios>::uninit();
        assert_eq!(
            unsafe { libc::tcgetattr(slave.as_raw_fd(), termios.as_mut_ptr()) },
            0
        );
        let raw = unsafe { termios.assume_init() }.c_lflag & libc::ICANON == 0;
        stable = if raw { stable + 1 } else { 0 };
        if stable >= 5 {
            break;
        }
        if Instant::now() >= deadline || child.try_wait().unwrap().is_some() {
            let _ = child.kill();
            let _ = child.wait();
            panic!("child did not enter raw mode with output stopped");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGTERM) }, 0);
    let deadline = Instant::now() + Duration::from_secs(3);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("SIGTERM did not release the blocked PTY write");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(status.signal(), Some(libc::SIGTERM));
    let mut termios = MaybeUninit::<libc::termios>::uninit();
    assert_eq!(
        unsafe { libc::tcgetattr(slave.as_raw_fd(), termios.as_mut_ptr()) },
        0
    );
    assert_ne!(unsafe { termios.assume_init() }.c_lflag & libc::ICANON, 0);
}

#[cfg(unix)]
#[test]
fn normal_exit_waits_for_room_to_restore_screen() {
    use std::fs::File;
    use std::io::{Read, Write};
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::time::{Duration, Instant};

    let (mut master_fd, mut slave_fd) = (-1, -1);
    assert_eq!(
        unsafe {
            libc::openpty(
                &mut master_fd,
                &mut slave_fd,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        },
        0
    );
    let mut master = unsafe { File::from_raw_fd(master_fd) };
    let slave = unsafe { File::from_raw_fd(slave_fd) };
    let path = entry("restore-backpressure");
    std::fs::write(&path, "component App\n  view\n    text \"Ready\"\n").unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_exact-terminal"))
        .arg(path)
        .args(["--size", "20x6", "--fullscreen"])
        .stdin(slave.try_clone().unwrap())
        .stdout(slave.try_clone().unwrap())
        .stderr(slave.try_clone().unwrap())
        .spawn()
        .unwrap();
    assert_eq!(
        unsafe { libc::fcntl(master.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK) },
        0
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut output = Vec::new();
    while !output.windows(5).any(|part| part == b"Ready") {
        let mut chunk = [0; 4096];
        if let Ok(n) = master.read(&mut chunk) {
            output.extend_from_slice(&chunk[..n]);
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("terminal did not draw Ready");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let filler = [b'X'; 4096];
    loop {
        let n = unsafe { libc::write(slave.as_raw_fd(), filler.as_ptr().cast(), filler.len()) };
        if n < 0 {
            assert_eq!(
                std::io::Error::last_os_error().kind(),
                std::io::ErrorKind::WouldBlock
            );
            break;
        }
        assert!(n > 0);
    }
    master.write_all(&[3]).unwrap();
    // The old leave path timed out after two seconds even on a normal quit.
    let blocked_until = Instant::now() + Duration::from_millis(2300);
    while Instant::now() < blocked_until {
        assert!(
            child.try_wait().unwrap().is_none(),
            "quit abandoned a blocked leave"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let mut chunk = [0; 4096];
        if let Ok(n) = master.read(&mut chunk) {
            output.extend_from_slice(&chunk[..n]);
        }
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "{status}");
            break;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("terminal did not exit after output drained");
        }
    }
    let mut chunk = [0; 4096];
    while let Ok(n) = master.read(&mut chunk) {
        if n == 0 {
            break;
        }
        output.extend_from_slice(&chunk[..n]);
    }
    assert!(
        output
            .windows(b"\x1b[?25h\x1b[?1049l".len())
            .any(|part| part == b"\x1b[?25h\x1b[?1049l"),
        "leave sequence was truncated"
    );
}

#[cfg(unix)]
#[test]
fn blocked_output_child() {
    use exact_terminal::host::{Host, Mode};

    if std::env::var_os("EXACT_TERMINAL_BLOCKED_OUTPUT_TEST").is_none() {
        return;
    }
    let source = "component App\n  view\n    text \"Ready\"\n";
    let plan = contract::compile(source).unwrap();
    let mut host = Host::boot(plan, (), Mode::Fullscreen, 200, 160).unwrap();
    exact_terminal::term::run(&mut host).unwrap();
}

#[cfg(unix)]
#[test]
fn pending_data_child() {
    use exact_plan::Value;
    use exact_runner::{Answer, DataError, DataSource, Outcome, Request, Store};
    use exact_terminal::host::{Host, Mode};

    struct Pending;
    impl DataSource for Pending {
        fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
            Ok(Value::record(vec![Value::str("loading")]))
        }

        fn answer(&mut self, _: &mut Store, _: &str, _: &[Value]) -> Result<Answer, DataError> {
            Ok(Answer::Later(Request::continuation(7)))
        }

        fn continuation(&mut self, _: u64) -> Option<Box<dyn FnOnce() -> Outcome + Send>> {
            Some(Box::new(|| {
                eprintln!("WORK_PENDING");
                loop {
                    std::thread::park();
                }
            }))
        }
    }

    let Some(mode) = std::env::var_os("EXACT_TERMINAL_PENDING_TEST") else {
        return;
    };
    let plan = contract::compile(
        "shape Word\n  text: string\ncomponent App\n  resource word = word(\"x\") as shape Word\n  view\n    column\n      text \"Ready\"\n      text `word ${word.text}`\n",
    )
    .unwrap();
    let mode = if mode == "--fullscreen" {
        Mode::Fullscreen
    } else {
        Mode::Inline
    };
    let mut host = Host::boot(plan, Pending, mode, 20, 6).unwrap();
    exact_terminal::term::run(&mut host).unwrap();
}

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
fn nonbreaking_space_does_not_add_a_terminal_row() {
    let path = entry("nbsp");
    std::fs::write(
        &path,
        "component App\n  view\n    column\n      text \"XX\u{a0}YY\" testId=\"nbsp\"\n      text \"Ready\"\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_exact-terminal"))
        .arg(path)
        .args(["--size", "4x6", "tree", "document"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("Text #nbsp @0,0 4x1"), "{text}");
    assert!(text.contains("Text @0,1 4x1 \"Ready\""), "{text}");
    assert!(text.contains("XX\u{a0}Y\nRead"), "{text}");
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

/// A native field in a terminal entry is one row with a host-painted fill,
/// no border (LLP 1104 D7), and a disabled one is faint.
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
    // The host's fill: #303030 behind the placeholder, without compiled rows.
    assert!(ans.contains("48;2;48;48;48"), "{ans:?}");
    let before = &ans[..ans.find("locked").unwrap()];
    let sgr = &before[before.rfind("\x1b[").unwrap()..];
    let params: Vec<&str> = sgr[2..sgr.find('m').unwrap()].split(';').collect();
    assert!(params[..2].contains(&"2"), "not faint: {sgr:?}");
}

#[test]
fn native_cell_look_keeps_bare_fields_and_authored_ink() {
    use exact_terminal::{
        grid::Rgb,
        host::{Host, Mode},
    };
    let plan = contract::compile_path_terminal(&entry("look")).unwrap();
    let host = Host::boot(plan, (), Mode::Fullscreen, 40, 12).unwrap();
    assert!(host.kernel().env().control_text_styles.is_some());
    let field = host
        .kernel()
        .node_by_key(host.kernel().find_by_test_id("field")[0])
        .unwrap();
    assert!(!field.style.mask.has(exact_kernel::StyleId::BackgroundColor));
    assert_eq!(host.kernel().provisional_layouts(), 0);

    let plan = contract::compile(
        r##"component App
  view
    column color="#ff0000" font-size=32 font-style="italic" font-weight=700 letter-spacing=4
      input width="10ch" value="native" testId="native"
      input width="10ch" value="authored" color="#00ff00"
      input width="10ch" value="bare" appearance="none" disabled=true
      textarea width="10ch" rows=2 value="two\nlines"
"##,
    )
    .unwrap();
    let mut host = Host::boot(plan, (), Mode::Fullscreen, 40, 12).unwrap();
    let n = host
        .kernel()
        .node_by_key(host.kernel().find_by_test_id("native")[0])
        .unwrap();
    let s = n.computed_style(exact_kernel::StyleMask::INHERITED);
    assert_eq!(s.font_family, 5);
    assert_eq!(s.font_size, 16.0);
    assert_eq!(s.font_weight, 400);
    assert_eq!(s.font_style, exact_kernel::FontStyle::Normal);
    assert_eq!(s.letter_spacing, 0.0);
    assert_eq!(n.text_color().resolve(false).g(), 0);
    for dark in [false, true] {
        host.dark = dark;
        let painted = host.render(0, 12);
        let grid = &painted.grid;
        let fill = if dark {
            Rgb(48, 48, 48)
        } else {
            Rgb(228, 228, 228)
        };
        let ink = if dark {
            Rgb(255, 255, 255)
        } else {
            Rgb(0, 0, 0)
        };
        assert_eq!(grid.cell(0, 0).style.fg, Some(ink));
        assert_eq!(
            grid.cell(9, 0).style.bg,
            Some(fill),
            "empty cells have fill"
        );
        assert_eq!(grid.cell(0, 1).style.fg, Some(Rgb(0, 255, 0)));
        assert_eq!(grid.cell(0, 2).style.fg, Some(Rgb(255, 0, 0)));
        assert_eq!(grid.cell(0, 2).style.bg, None);
        assert!(
            !grid.cell(0, 2).style.faint,
            "bare disabled paint stays authored"
        );
        assert_eq!(
            grid.cell(9, 4).style.bg,
            Some(fill),
            "textarea's second row"
        );
    }
}

/// LLP 1116 D8: a determinate `progress` is a row of blocks, `█` to its
/// value and `░` after, across its box (HTML's 10em).
#[test]
fn a_progress_bar_is_a_row_of_blocks() {
    let dir = std::env::temp_dir().join(format!("exact-terminal-cli-{}-bar", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("app.contract");
    std::fs::write(
        &path,
        "component App\n  view\n    column\n      progress value=3 max=4\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_exact-terminal"))
        .arg(&path)
        .args(["--size", "40x6", "--inline", "print"])
        .output()
        .expect("runs");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(text.contains("███████████████░░░░░"), "{text}");
}
