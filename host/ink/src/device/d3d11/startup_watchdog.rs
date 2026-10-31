//! One-shot startup sampling. Symbols and stack walking run in a helper process,
//! so a target holding the loader lock cannot deadlock DbgHelp in the observer.
//! @ref LLP 1149.001 Stage 1w; LLP 1149 D15.
use std::{
    io::Write,
    os::windows::process::CommandExt,
    path::PathBuf,
    process::Command,
    sync::{mpsc, OnceLock},
    time::{Duration, Instant},
};
use windows::Win32::System::Threading::CREATE_NO_WINDOW;
#[cfg(target_arch = "x86_64")]
mod stacks;

static PHASES: OnceLock<mpsc::Sender<Option<Instant>>> = OnceLock::new();
const HELPER: &str = "--exact-startup-stacks";

/// The A/B changes only when preparation starts; the normal policy is unchanged.
pub fn after_layout() -> bool {
    std::env::var_os("EXACT_STARTUP_STACKS").is_some()
        && std::env::var("EXACT_STARTUP_DEVICE_AT").as_deref() == Ok("layout")
}

/// Arm before the device worker can load its driver. No observer on ordinary
/// uninstrumented launches; the series supplies a path beside each launch log.
pub fn start() {
    let Some(path) = std::env::var_os("EXACT_STARTUP_STACKS").map(PathBuf::from) else {
        return;
    };
    if !cfg!(target_arch = "x86_64") {
        eprintln!("exact-windows: startup stacks require x64");
        return;
    }
    let (send, receive) = mpsc::channel();
    if PHASES.set(send).is_err() {
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("startup-watchdog".into())
        .spawn(move || {
            let Ok(Some(ready)) = receive.recv() else {
                return;
            };
            if receive.recv_timeout(
                (ready + Duration::from_millis(50)).saturating_duration_since(Instant::now()),
            ) != Err(mpsc::RecvTimeoutError::Timeout)
            {
                return;
            }
            let result = (|| -> std::io::Result<()> {
                let trigger = super::super::process_ms().unwrap_or(f64::NAN);
                let child = Command::new(std::env::current_exe()?)
                    .arg(HELPER)
                    .arg(std::process::id().to_string())
                    .arg(&path)
                    .arg(trigger.to_string())
                    .creation_flags(CREATE_NO_WINDOW.0)
                    .spawn()?;
                eprintln!(
                    "exact-windows: startup tail at {trigger:.3} ms; stack helper {} -> {}",
                    child.id(),
                    path.display()
                );
                Ok(())
            })();
            if let Err(error) = result {
                eprintln!("exact-windows: startup watchdog: {error}");
            }
        });
    if let Err(error) = spawned {
        eprintln!("exact-windows: startup watchdog: {error}");
    }
}

/// The existing process-clock phase marks delimit the interval, including work
/// between runner construction and layout. The test hold is never enabled by default.
pub fn phase(name: &str, at: Instant) {
    let Some(send) = PHASES.get() else { return };
    match name {
        "runner-ready" => {
            let _ = send.send(Some(at));
            if std::env::var("EXACT_STARTUP_TEST_HOLD").as_deref() == Ok("1") {
                std::thread::sleep(Duration::from_millis(200));
            }
        }
        "layout-ready" => {
            let _ = send.send(None);
        }
        _ => {}
    }
}

/// Dispatch before the helper initializes a window, runner or device.
pub fn helper() -> Option<i32> {
    if !cfg!(target_arch = "x86_64") || std::env::var_os("EXACT_STARTUP_STACKS").is_none() {
        return None;
    }
    let mut args = std::env::args_os().skip(1);
    if args.next()?.to_str() != Some(HELPER) {
        return None;
    }
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let _pid: u32 = args
            .next()
            .ok_or("missing PID")?
            .to_string_lossy()
            .parse()?;
        let path = PathBuf::from(args.next().ok_or("missing output path")?);
        let trigger = args.next().ok_or("missing trigger age")?;
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        writeln!(
            output,
            "pid={_pid} trigger_process_ms={}",
            trigger.to_string_lossy()
        )?;
        #[cfg(target_arch = "x86_64")]
        if let Err(error) = stacks::dump(_pid, &mut output) {
            writeln!(output, "capture failed: {error}")?;
            return Err(error);
        }
        Ok(())
    })();
    Some(match result {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("startup stacks: {error}");
            1
        }
    })
}
