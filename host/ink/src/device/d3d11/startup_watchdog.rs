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
            Diagnostics::{Debug::*, ProcessSnapshotting::*},
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
struct Snapshot(HPSS);
impl Drop for Snapshot {
    fn drop(&mut self) {
        unsafe {
            PssFreeSnapshot(GetCurrentProcess(), self.0);
        }
    }
}
struct Marker(HPSSWALK);
impl Drop for Marker {
    fn drop(&mut self) {
        unsafe {
            PssWalkMarkerFree(self.0);
        }
    }
}
fn checked(code: u32) -> std::io::Result<()> {
    if code == 0 {
        Ok(())
    } else {
        Err(std::io::Error::from_raw_os_error(code as i32))
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
            PROCESS_QUERY_INFORMATION
                | PROCESS_VM_READ
                | PROCESS_CREATE_PROCESS
                | PROCESS_DUP_HANDLE,
            false,
            pid,
        )?);
        // Freeze contexts and their stack memory before loading PDBs. The clone
        // is copy-on-write; the target runs while DbgHelp walks the snapshot.
        let started = Instant::now();
        let mut snapshot = HPSS::default();
        checked(PssCaptureSnapshot(
            process.0,
            PSS_CAPTURE_VA_CLONE | PSS_CAPTURE_THREADS | PSS_CAPTURE_THREAD_CONTEXT,
            Some(CONTEXT_FULL_AMD64.0),
            &mut snapshot,
        ))?;
        let snapshot = Snapshot(snapshot);
        let mut clone = PSS_VA_CLONE_INFORMATION::default();
        checked(PssQuerySnapshot(
            snapshot.0,
            PSS_QUERY_VA_CLONE_INFORMATION,
            (&mut clone as *mut PSS_VA_CLONE_INFORMATION).cast(),
            size_of_val(&clone) as u32,
        ))?;
        writeln!(
            output,
            "snapshot_ms={:.3}",
            started.elapsed().as_secs_f64() * 1000.
        )?;
        let process = clone.VaCloneHandle;
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
        SymInitializeW(process, PCWSTR(search.as_ptr()), true)?;
        let mut marker = HPSSWALK::default();
        checked(PssWalkMarkerCreate(None, &mut marker))?;
        let marker = Marker(marker);
        let mut count = 0;
        loop {
            let mut entry = PSS_THREAD_ENTRY::default();
            let bytes = std::slice::from_raw_parts_mut(
                (&mut entry as *mut PSS_THREAD_ENTRY).cast::<u8>(),
                size_of_val(&entry),
            );
            let result = PssWalkSnapshot(snapshot.0, PSS_WALK_THREADS, marker.0, Some(bytes));
            if result == ERROR_NO_MORE_ITEMS.0 {
                break;
            }
            checked(result)?;
            count += 1;
            writeln!(
                output,
                "\nthread {} capture_filetime={:08x}{:08x}",
                entry.ThreadId, entry.CaptureTime.dwHighDateTime, entry.CaptureTime.dwLowDateTime
            )?;
            if entry.ContextRecord.is_null() {
                writeln!(output, "unavailable: no captured context")?;
            } else if let Err(error) = thread(process, &*entry.ContextRecord, output) {
                writeln!(output, "unavailable: {error}")?;
            }
        }
        writeln!(output, "\nthreads_enumerated={count}")?;
        SymCleanup(process)?;
    }
    Ok(())
}

#[cfg(target_arch = "x86_64")]
unsafe fn thread(
    process: HANDLE,
    captured: &CONTEXT,
    output: &mut impl Write,
) -> Result<(), Box<dyn std::error::Error>> {
    unsafe extern "system" fn functions(process: HANDLE, address: u64) -> *mut std::ffi::c_void {
        unsafe { SymFunctionTableAccess64(process, address) }
    }
    unsafe extern "system" fn module(process: HANDLE, address: u64) -> u64 {
        unsafe { SymGetModuleBase64(process, address) }
    }
    unsafe {
        #[repr(C, align(16))]
        struct Aligned(CONTEXT);
        let mut aligned = Aligned(*captured);
        let context = &mut aligned.0;
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
                HANDLE::default(),
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
        // Raw addresses survive absent PDBs.
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
unsafe fn thread(
    _: HANDLE,
    _: &CONTEXT,
    _: &mut impl Write,
) -> Result<(), Box<dyn std::error::Error>> {
    Err("startup stack walking is currently qualified only on x64".into())
}
