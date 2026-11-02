//! x64 snapshot walking, loaded only in the explicitly armed helper.
use crate::device::windows_library::Library;
use std::{io::Write, os::windows::ffi::OsStrExt, time::Instant};
use windows::{
    core::{s, w, BOOL, PCWSTR},
    Win32::{
        Foundation::*,
        System::{
            Diagnostics::{Debug::*, ProcessSnapshotting::*},
            Threading::*,
        },
    },
};

struct Symbols {
    set_options: unsafe extern "system" fn(u32) -> u32,
    initialize: unsafe extern "system" fn(HANDLE, PCWSTR, BOOL) -> BOOL,
    cleanup: unsafe extern "system" fn(HANDLE) -> BOOL,
    functions: unsafe extern "system" fn(HANDLE, u64) -> *mut std::ffi::c_void,
    module: unsafe extern "system" fn(HANDLE, u64) -> u64,
    from_addr: unsafe extern "system" fn(HANDLE, u64, *mut u64, *mut SYMBOL_INFOW) -> BOOL,
    line: unsafe extern "system" fn(HANDLE, u64, *mut u32, *mut IMAGEHLP_LINEW64) -> BOOL,
    walk: unsafe extern "system" fn(
        u32,
        HANDLE,
        HANDLE,
        *mut STACKFRAME64,
        *mut std::ffi::c_void,
        PREAD_PROCESS_MEMORY_ROUTINE64,
        PFUNCTION_TABLE_ACCESS_ROUTINE64,
        PGET_MODULE_BASE_ROUTINE64,
        PTRANSLATE_ADDRESS_ROUTINE64,
    ) -> BOOL,
    _library: Library,
}
impl Symbols {
    fn new() -> windows::core::Result<Self> {
        let library = Library::open(w!("dbghelp.dll"))?;
        // Signatures match dbghelp.h. The library outlives every call and callback.
        unsafe {
            Ok(Self {
                set_options: library.get(s!("SymSetOptions"))?,
                initialize: library.get(s!("SymInitializeW"))?,
                cleanup: library.get(s!("SymCleanup"))?,
                functions: library.get(s!("SymFunctionTableAccess64"))?,
                module: library.get(s!("SymGetModuleBase64"))?,
                from_addr: library.get(s!("SymFromAddrW"))?,
                line: library.get(s!("SymGetLineFromAddrW64"))?,
                walk: library.get(s!("StackWalk64"))?,
                _library: library,
            })
        }
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

pub(super) fn dump(pid: u32, output: &mut impl Write) -> Result<(), Box<dyn std::error::Error>> {
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
        let symbols = Symbols::new()?;
        (symbols.set_options)(
            SYMOPT_LOAD_LINES | SYMOPT_UNDNAME | SYMOPT_FAIL_CRITICAL_ERRORS | SYMOPT_NO_PROMPTS,
        );
        (symbols.initialize)(process, PCWSTR(search.as_ptr()), BOOL(1)).ok()?;
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
            } else if let Err(error) = thread(&symbols, process, &*entry.ContextRecord, output) {
                writeln!(output, "unavailable: {error}")?;
            }
        }
        writeln!(output, "\nthreads_enumerated={count}")?;
        (symbols.cleanup)(process).ok()?;
    }
    Ok(())
}

#[cfg(target_arch = "x86_64")]
unsafe fn thread(
    symbols: &Symbols,
    process: HANDLE,
    captured: &CONTEXT,
    output: &mut impl Write,
) -> Result<(), Box<dyn std::error::Error>> {
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
            if !(symbols.walk)(
                0x8664,
                process,
                HANDLE::default(),
                &mut frame,
                (context as *mut CONTEXT).cast(),
                None,
                Some(symbols.functions),
                Some(symbols.module),
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
            if (symbols.from_addr)(process, *address, &mut displacement, symbol).as_bool() {
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
            if (symbols.line)(process, *address, &mut offset, &mut line).as_bool() {
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
