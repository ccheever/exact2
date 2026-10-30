//! One-shot startup sampling. Symbols and stack walking run in a helper process,
//! so a target holding the loader lock cannot deadlock DbgHelp in the observer.
//! @ref LLP 1149.001 Stage 1w; LLP 1149 D15.
use std::{
    io::Write,
    os::windows::{ffi::OsStrExt, process::CommandExt},
    path::PathBuf,
    process::Command,
    sync::{mpsc, OnceLock},
    time::{Duration, Instant},
};
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::*,
        System::{
            Diagnostics::{Debug::*, ToolHelp::*},
            Threading::*,
        },
    },
};

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
                    .env_remove("EXACT_STARTUP_STACKS")
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
pub fn phase(name: &str) {
    let Some(send) = PHASES.get() else { return };
    match name {
        "runner-ready" => {
            let _ = send.send(Some(Instant::now()));
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

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
struct Suspended(HANDLE);
impl Drop for Suspended {
    fn drop(&mut self) {
        unsafe {
            ResumeThread(self.0);
        }
    }
}

/// Dispatch before the helper initializes a window, runner or device.
pub fn helper() -> Option<i32> {
    let mut args = std::env::args_os().skip(1);
    if args.next()?.to_str() != Some(HELPER) {
        return None;
    }
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let pid: u32 = args
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
            "pid={pid} trigger_process_ms={}",
            trigger.to_string_lossy()
        )?;
        if let Err(error) = dump(pid, &mut output) {
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

fn dump(pid: u32, output: &mut impl Write) -> Result<(), Box<dyn std::error::Error>> {
    // DbgHelp has one caller in this process. Its search path is local only;
    // symbol-server downloads must not hold the sampled thread suspended.
    unsafe {
        let process = Handle(OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
            false,
            pid,
        )?);
        let exe = std::env::current_exe()?;
        let search: Vec<u16> = exe
            .parent()
            .unwrap()
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        SymSetOptions(
            SYMOPT_LOAD_LINES | SYMOPT_UNDNAME | SYMOPT_FAIL_CRITICAL_ERRORS | SYMOPT_NO_PROMPTS,
        );
        SymInitializeW(process.0, PCWSTR(search.as_ptr()), true)?;
        let snapshot = Handle(CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0)?);
        let mut entry = THREADENTRY32 {
            dwSize: size_of::<THREADENTRY32>() as u32,
            ..Default::default()
        };
        let mut more = Thread32First(snapshot.0, &mut entry).is_ok();
        let mut count = 0;
        while more {
            if entry.th32OwnerProcessID == pid {
                count += 1;
                writeln!(output, "\nthread {}", entry.th32ThreadID)?;
                if let Err(error) = thread(process.0, entry.th32ThreadID, output) {
                    writeln!(output, "unavailable: {error}")?;
                }
            }
            more = Thread32Next(snapshot.0, &mut entry).is_ok();
        }
        writeln!(output, "\nthreads_enumerated={count}")?;
        SymCleanup(process.0)?;
    }
    Ok(())
}

#[cfg(target_arch = "x86_64")]
unsafe fn thread(
    process: HANDLE,
    id: u32,
    output: &mut impl Write,
) -> Result<(), Box<dyn std::error::Error>> {
    unsafe extern "system" fn functions(process: HANDLE, address: u64) -> *mut std::ffi::c_void {
        unsafe { SymFunctionTableAccess64(process, address) }
    }
    unsafe extern "system" fn module(process: HANDLE, address: u64) -> u64 {
        unsafe { SymGetModuleBase64(process, address) }
    }
    unsafe {
        let thread = Handle(OpenThread(
            THREAD_GET_CONTEXT | THREAD_SUSPEND_RESUME | THREAD_QUERY_INFORMATION,
            false,
            id,
        )?);
        if SuspendThread(thread.0) == u32::MAX {
            return Err(windows::core::Error::from_thread().into());
        }
        let held = Suspended(thread.0);
        // Windows requires 16-byte alignment; win32metadata's CONTEXT is
        // repr(C) with only 8-byte alignment on x64.
        #[repr(C, align(16))]
        struct Aligned(CONTEXT);
        let mut aligned = Aligned(CONTEXT {
            ContextFlags: CONTEXT_FULL_AMD64,
            ..Default::default()
        });
        let context = &mut aligned.0;
        GetThreadContext(thread.0, context)?;
        let mut frame = STACKFRAME64::default();
        frame.AddrPC.Offset = context.Rip;
        frame.AddrStack.Offset = context.Rsp;
        frame.AddrFrame.Offset = context.Rbp;
        frame.AddrPC.Mode = AddrModeFlat;
        frame.AddrStack.Mode = AddrModeFlat;
        frame.AddrFrame.Mode = AddrModeFlat;
        let mut addresses = [0u64; 128];
        let mut count = 0;
        while count < addresses.len() && frame.AddrPC.Offset != 0 {
            addresses[count] = frame.AddrPC.Offset;
            count += 1;
            if !StackWalk64(
                0x8664,
                process,
                thread.0,
                &mut frame,
                (context as *mut CONTEXT).cast(),
                None,
                Some(functions),
                Some(module),
                None,
            )
            .as_bool()
            {
                break;
            }
            if count > 1 && frame.AddrPC.Offset == addresses[count - 1] {
                break;
            }
        }
        drop(held);
        // Resume before formatting or file I/O. Raw addresses survive absent PDBs.
        for address in &addresses[..count] {
            let mut buffer = [0u64; 512];
            let symbol = buffer.as_mut_ptr().cast::<SYMBOL_INFOW>();
            (*symbol).SizeOfStruct = size_of::<SYMBOL_INFOW>() as u32;
            (*symbol).MaxNameLen = 1024;
            let mut displacement = 0;
            write!(output, "0x{address:016x}")?;
            if SymFromAddrW(process, *address, Some(&mut displacement), symbol).is_ok() {
                let name =
                    std::slice::from_raw_parts((*symbol).Name.as_ptr(), (*symbol).NameLen as usize);
                write!(
                    output,
                    " {}+0x{displacement:x}",
                    String::from_utf16_lossy(name)
                )?;
            }
            let mut line = IMAGEHLP_LINEW64 {
                SizeOfStruct: size_of::<IMAGEHLP_LINEW64>() as u32,
                ..Default::default()
            };
            let mut offset = 0;
            if SymGetLineFromAddrW64(process, *address, &mut offset, &mut line).is_ok() {
                write!(
                    output,
                    " {}:{}",
                    line.FileName.to_string()?,
                    line.LineNumber
                )?;
            }
            writeln!(output)?;
        }
    }
    Ok(())
}

#[cfg(not(target_arch = "x86_64"))]
unsafe fn thread(_: HANDLE, _: u32, _: &mut impl Write) -> Result<(), Box<dyn std::error::Error>> {
    Err("startup stack walking is currently qualified only on x64".into())
}
