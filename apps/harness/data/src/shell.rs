//! The shell tool's process (LLP 1101.001 P17): the command runs in its own
//! process group, its output is read as it comes into a bounded buffer
//! (the first 64 KiB and the last 16 KiB, the rest counted), and a cancel
//! or a timeout kills the whole group. Readers run on their own threads;
//! the caller never waits on a pipe a descendant may still hold.

use rustix::process::{kill_process_group, Pid, Signal};
use std::collections::VecDeque;
use std::io::Read;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Kept from the start of the output.
pub const HEAD: usize = 64 * 1024;
/// Kept from the end of the output.
pub const TAIL: usize = 16 * 1024;
/// How long readers get to drain after the command ends.
const GRACE: Duration = Duration::from_millis(250);

/// The output so far: its head, its tail, and how much fell between.
#[derive(Default)]
pub struct Bounded {
    head: Vec<u8>,
    tail: VecDeque<u8>,
    dropped: u64,
}

impl Bounded {
    /// Take `bytes`, keeping at most [`HEAD`] + [`TAIL`] of everything.
    pub fn push(&mut self, mut bytes: &[u8]) {
        if self.head.len() < HEAD {
            let n = (HEAD - self.head.len()).min(bytes.len());
            self.head.extend_from_slice(&bytes[..n]);
            bytes = &bytes[n..];
        }
        if bytes.len() >= TAIL {
            self.dropped += (self.tail.len() + bytes.len() - TAIL) as u64;
            self.tail.clear();
            self.tail.extend(&bytes[bytes.len() - TAIL..]);
            return;
        }
        self.tail.extend(bytes);
        let over = self.tail.len().saturating_sub(TAIL);
        if over > 0 {
            self.tail.drain(..over);
            self.dropped += over as u64;
        }
    }

    /// The bytes held now.
    #[cfg(test)]
    pub fn held(&self) -> usize {
        self.head.len() + self.tail.len()
    }

    /// The text, with a note where bytes were left out.
    pub fn text(&self) -> String {
        let mut out = String::from_utf8_lossy(&self.head).into_owned();
        if self.dropped > 0 {
            if !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&format!("[… {} bytes omitted …]\n", self.dropped));
        }
        let (a, b) = self.tail.as_slices();
        out.push_str(&String::from_utf8_lossy(&[a, b].concat()));
        out
    }
}

/// Reader threads alive now, across every command (tests watch it).
pub static READERS: AtomicUsize = AtomicUsize::new(0);

fn reader(mut pipe: impl Read + Send + 'static, out: Arc<Mutex<Bounded>>) -> Arc<AtomicBool> {
    let done = Arc::new(AtomicBool::new(false));
    let flag = done.clone();
    READERS.fetch_add(1, Ordering::SeqCst);
    std::thread::spawn(move || {
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            match pipe.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => out
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(&buf[..n]),
            }
        }
        flag.store(true, Ordering::SeqCst);
        READERS.fetch_sub(1, Ordering::SeqCst);
    });
    done
}

/// How a command ended.
pub struct Ran {
    pub text: String,
    /// The exit code; `None` when killed or ended by a signal.
    pub code: Option<i32>,
    /// Why it was killed, if it was.
    pub killed: Option<&'static str>,
}

/// Run `command` with `sh -c` in its own process group. `spawned` hears
/// the group's id (the shell's pid).
pub fn run(
    command: &str,
    cancel: &AtomicBool,
    timeout: Duration,
    spawned: &mut dyn FnMut(u32),
) -> Result<Ran, String> {
    let script = format!("{{\n{command}\n}} 2>&1");
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(script)
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot run sh: {e}"))?;
    let group = child.id();
    spawned(group);
    let out = Arc::new(Mutex::new(Bounded::default()));
    let mut done = Vec::new();
    if let Some(p) = child.stdout.take() {
        done.push(reader(p, out.clone()));
    }
    if let Some(p) = child.stderr.take() {
        done.push(reader(p, out.clone()));
    }
    let start = Instant::now();
    let mut killed = None;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {}
            Err(_) => break None,
        }
        if cancel.load(Ordering::SeqCst) {
            killed = Some("interrupted");
        } else if start.elapsed() > timeout {
            killed = Some("timed out");
        }
        if killed.is_some() {
            kill_group(group);
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    // Let the readers drain; a descendant still holding a pipe is left
    // to its reader, which is detached rather than waited on.
    let drained = Instant::now();
    while !done.iter().all(|d| d.load(Ordering::SeqCst)) && drained.elapsed() < GRACE {
        std::thread::sleep(Duration::from_millis(5));
    }
    let text = out.lock().unwrap_or_else(|e| e.into_inner()).text();
    Ok(Ran {
        text,
        code: status.and_then(|s| s.code()),
        killed,
    })
}

/// SIGKILL every process in group `group`.
pub fn kill_group(group: u32) {
    if let Some(pid) = Pid::from_raw(group as i32) {
        let _ = kill_process_group(pid, Signal::KILL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustix::process::test_kill_process_group;

    #[test]
    fn the_buffer_keeps_its_ends() {
        let mut b = Bounded::default();
        for _ in 0..1000 {
            b.push(&[b'x'; 1000]);
        }
        b.push(b"END");
        assert!(b.held() <= HEAD + TAIL);
        let text = b.text();
        assert!(text.ends_with("END"));
        assert!(text.contains(&format!("[… {} bytes omitted …]", 1_000_003 - HEAD - TAIL)));
    }

    #[test]
    fn a_cancel_kills_the_whole_group() {
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        let mut group = 0;
        let start = Instant::now();
        let watcher = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            stop.store(true, Ordering::SeqCst);
        });
        let ran = run(
            "sleep 30 & sleep 30",
            &cancel,
            Duration::from_secs(60),
            &mut |g| group = g,
        )
        .unwrap();
        watcher.join().unwrap();
        assert_eq!(ran.killed, Some("interrupted"));
        assert!(start.elapsed() < Duration::from_secs(2));
        // Both sleeps are gone: nothing is left in the group.
        let pid = Pid::from_raw(group as i32).unwrap();
        let gone = Instant::now();
        while test_kill_process_group(pid).is_ok() {
            assert!(
                gone.elapsed() < Duration::from_secs(1),
                "the group outlived its cancel"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn a_flood_is_read_bounded() {
        let cancel = AtomicBool::new(false);
        let ran = run(
            "yes | head -c 50000000",
            &cancel,
            Duration::from_secs(60),
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(ran.code, Some(0));
        assert!(ran.text.len() < HEAD + TAIL + 100, "{}", ran.text.len());
        assert!(ran
            .text
            .contains(&format!("[… {} bytes omitted …]", 50_000_000 - HEAD - TAIL)));
    }

    #[test]
    fn a_background_child_holding_the_pipe_is_not_waited_for() {
        let cancel = AtomicBool::new(false);
        let mut group = 0;
        let start = Instant::now();
        let ran = run(
            "echo hi; sleep 5 &",
            &cancel,
            Duration::from_secs(60),
            &mut |g| group = g,
        )
        .unwrap();
        assert!(start.elapsed() < Duration::from_secs(2));
        assert_eq!(ran.text, "hi\n");
        kill_group(group);
    }
}
