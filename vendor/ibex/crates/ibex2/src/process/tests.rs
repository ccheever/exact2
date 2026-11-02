use super::*;
use std::io::{Read, Write};
#[cfg(target_vendor = "apple")]
use std::os::fd::AsRawFd;

fn sh(script: &str, stdin: Stdio) -> Child {
    let args = vec!["-c".to_string(), script.to_string()];
    let env = vec![
        ("PATH".to_string(), "/usr/bin:/bin".to_string()),
        ("GREETING".to_string(), "hi".to_string()),
    ];
    spawn(Command {
        arg0: None,
        program: Path::new("/bin/sh"),
        args: &args,
        env: &env,
        cwd: Path::new("/"),
        stdin,
        stdout: Stdio::Pipe,
        stderr: Stdio::Pipe,
        session: false,
    })
    .expect("sh starts")
}

fn read_all(fd: OwnedFd) -> String {
    let mut out = String::new();
    std::fs::File::from(fd).read_to_string(&mut out).unwrap();
    out
}

#[test]
fn a_child_has_its_own_group_its_environment_its_directory_and_only_three_descriptors() {
    // A descriptor this process holds without close-on-exec, which no child may inherit.
    // SAFETY: a NUL-terminated path; the descriptor is closed below.
    let open = unsafe { libc::open(c"/dev/null".as_ptr(), libc::O_RDONLY) };
    let mut child = sh(&format!("echo $GREETING; pwd; ps -o pgid= -p $$ | tr -d ' '; [ -e /dev/fd/{open} ] && echo leaked || echo closed; echo err >&2"), Stdio::Pipe);
    // SAFETY: opened above.
    unsafe { libc::close(open) };
    drop(child.stdin.take());
    let out = read_all(child.stdout.take().unwrap());
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "hi");
    assert_eq!(lines[1], "/");
    assert_eq!(lines[2], child.pid.to_string(), "its own process group");
    assert_eq!(lines[3], "closed", "only 0, 1 and 2 are inherited");
    assert_eq!(read_all(child.stderr.take().unwrap()), "err\n");
    assert_eq!(wait(child.pid).unwrap(), Exit::Code(0));
}

#[test]
fn stdin_is_the_childs_and_a_signal_to_the_group_reaches_its_children() {
    let mut child = sh(
        "read line; echo \"got $line\"; sleep 30 & echo $!; wait",
        Stdio::Pipe,
    );
    let mut stdin = std::fs::File::from(child.stdin.take().unwrap());
    stdin.write_all(b"ping\n").unwrap();
    drop(stdin);
    let mut stdout = std::fs::File::from(child.stdout.take().unwrap());
    let mut seen = String::new();
    let mut byte = [0u8; 1];
    while seen.matches('\n').count() < 2 {
        stdout.read_exact(&mut byte).unwrap();
        seen.push(byte[0] as char);
    }
    let mut lines = seen.lines();
    assert_eq!(lines.next(), Some("got ping"));
    let grandchild: i32 = lines.next().unwrap().parse().unwrap();
    assert!(
        group_members(child.pid).contains(&grandchild),
        "the grandchild is in the group"
    );
    assert!(start_time(grandchild).unwrap() >= start_time(child.pid).unwrap());
    signal_group(child.pid, libc::SIGTERM).unwrap();
    assert_eq!(wait(child.pid).unwrap(), Exit::Signal(libc::SIGTERM));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while group_alive(child.pid) {
        assert!(
            std::time::Instant::now() < deadline,
            "the group outlived its signal"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[test]
fn a_missing_program_is_not_found_and_lookups_answer() {
    let err = spawn(Command {
        arg0: None,
        program: Path::new("/nonexistent/program"),
        args: &[],
        env: &[],
        cwd: Path::new("/"),
        stdin: Stdio::Null,
        stdout: Stdio::Null,
        stderr: Stdio::Null,
        session: false,
    })
    .err()
    .unwrap();
    assert_eq!(err.kind(), io::ErrorKind::NotFound);
    assert_eq!(
        find_on_path("sh", "/nonexistent:/bin"),
        Some(PathBuf::from("/bin/sh"))
    );
    assert_eq!(find_on_path("sh/x", "/bin"), None);
    // A relative entry names the app's directory, not the child's: skipped,
    // even one that leads from here to `/bin`.
    let depth = std::env::current_dir().unwrap().components().count();
    let up = format!("{}bin", "../".repeat(depth));
    assert!(executable(&Path::new(&up).join("sh")));
    assert_eq!(find_on_path("sh", &up), None);
    assert!(executable(Path::new("/bin/sh")));
    assert!(!executable(Path::new("/bin")));
    assert!(home_dir().is_some_and(|h| h.is_absolute()));
    assert!(boot_id().is_some());
    assert_eq!(signal_number("SIGKILL"), Some(libc::SIGKILL));
    assert_eq!(signal_name(libc::SIGTERM), "SIGTERM");
}

#[test]
fn a_session_child_leads_its_own_session() {
    let args = vec![
        "-c".to_string(),
        "ps -o sess= -p $$ >/dev/null; echo $$".to_string(),
    ];
    let child = spawn(Command {
        arg0: None,
        program: Path::new("/bin/sh"),
        args: &args,
        env: &[],
        cwd: Path::new("/"),
        stdin: Stdio::Null,
        stdout: Stdio::Pipe,
        stderr: Stdio::Null,
        session: true,
    })
    .unwrap();
    // SAFETY: plain syscall.
    let sid = unsafe { libc::getsid(child.pid) };
    let out = read_all(child.stdout.unwrap());
    assert_eq!(out.trim(), child.pid.to_string());
    assert!(sid == child.pid || sid == -1, "{sid}");
    let _ = wait(child.pid);
}

#[test]
fn a_write_to_a_child_that_has_gone_fails_without_a_signal() {
    let mut child = sh("echo $EXACT_TEST_TAG; exit 0", Stdio::Pipe);
    let stdin = child.stdin.take().unwrap();
    #[cfg(target_vendor = "apple")]
    assert!(
        sigpipe_off(stdin.as_raw_fd()),
        "the parent's end raises no SIGPIPE"
    );
    let _ = read_all(child.stdout.take().unwrap());
    wait(child.pid).unwrap();
    let mut file = std::fs::File::from(stdin);
    let e = file.write_all(&[0u8; 65536]).unwrap_err();
    assert_eq!(e.raw_os_error(), Some(libc::EPIPE));
    // A child is in this process's session, and names the program as told.
    let args = vec!["5".to_string()];
    let sleeper = spawn(Command {
        arg0: Some("named"),
        program: Path::new("/bin/sleep"),
        args: &args,
        env: &[],
        cwd: Path::new("/"),
        stdin: Stdio::Null,
        stdout: Stdio::Null,
        stderr: Stdio::Null,
        session: false,
    })
    .unwrap();
    assert_eq!(
        session_of(sleeper.pid),
        session_of(std::process::id() as i32)
    );
    assert!(session_of(sleeper.pid).is_some());
    signal_group(sleeper.pid, SIGKILL).unwrap();
    wait(sleeper.pid).unwrap();
}
