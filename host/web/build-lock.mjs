// Build swaps and GPU-cache copies share a persistent lock file. Removing it would let a waiter lock the old
// inode while another opens a new one under the same name. The PID is diagnostic; only the OS grants ownership.
import { closeSync, ftruncateSync, openSync, writeSync } from 'node:fs';
import { constants as os } from 'node:os';
import { toNamespacedPath } from 'node:path';

let locking;
function kernel() {
  if (locking) return locking;
  const { dlopen, read } = import.meta.require('bun:ffi');
  if (process.platform === 'win32') {
    const { symbols } = dlopen('kernel32.dll', {
      CreateFileW: { args: ['ptr', 'u32', 'u32', 'ptr', 'u32', 'u32', 'u64'], returns: 'u64' },
      LockFileEx: { args: ['u64', 'u32', 'u32', 'u32', 'u32', 'ptr'], returns: 'i32' },
      WriteFile: { args: ['u64', 'ptr', 'u32', 'ptr', 'ptr'], returns: 'i32' },
      SetEndOfFile: { args: ['u64'], returns: 'i32' },
      CloseHandle: { args: ['u64'], returns: 'i32' },
      GetLastError: { args: [], returns: 'u32' },
    });
    const fail = (operation, code = symbols.GetLastError()) => Object.assign(new Error(`${operation}: Win32 error ${code}`), { code: code === 2 || code === 3 ? 'ENOENT' : 'EIO' });
    locking = {
      open(path, create) {
        // Read/write sharing keeps all contenders on the same file; deletion is refused while any handle is open.
        const handle = symbols.CreateFileW(Buffer.from(toNamespacedPath(path) + '\0', 'utf16le'), create ? 0xc0000000 : 0x80000000, 3, null, create ? 4 : 3, 0x80, 0);
        if (BigInt(handle) === 0xffffffffffffffffn) throw fail(`open build lock ${path}`);
        return handle;
      },
      take(handle, exclusive) {
        // Lock byte 64, beyond the PID: Windows byte locks also deny reads, so diagnostic reads stay outside it.
        // OVERLAPPED.Offset is 16 bytes into the 64-bit structure; the event and high offset stay zero.
        const overlapped = new Uint8Array(32);
        new DataView(overlapped.buffer).setUint32(16, 64, true);
        if (symbols.LockFileEx(handle, exclusive ? 3 : 1, 0, 1, 0, overlapped)) return true; // fail immediately
        const code = symbols.GetLastError();
        if (code === 33) return false;
        throw fail('lock build output', code);
      },
      mark(handle) {
        const pid = Buffer.from(String(process.pid)), written = new Uint32Array(1);
        if (!symbols.WriteFile(handle, pid, pid.length, written, null)) throw fail('write build lock owner');
        if (written[0] !== pid.length) throw new Error(`write build lock owner: wrote ${written[0]} of ${pid.length} bytes`);
        if (!symbols.SetEndOfFile(handle)) throw fail('truncate build lock owner');
      },
      close: handle => { if (!symbols.CloseHandle(handle)) throw fail('close build lock'); },
    };
  } else {
    const errno = process.platform === 'darwin' ? '__error' : '__errno_location';
    const { symbols } = dlopen(process.platform === 'darwin' ? '/usr/lib/libSystem.B.dylib' : 'libc.so.6', {
      flock: { args: ['i32', 'i32'], returns: 'i32' }, [errno]: { args: [], returns: 'ptr' },
    });
    locking = {
      open: (path, create) => openSync(path, create ? 'a+' : 'r'),
      take(fd, exclusive) {
        // NFS needs write access for LOCK_EX; inspection uses LOCK_SH.
        if (symbols.flock(fd, exclusive ? 6 : 5) === 0) return true;
        const code = read.i32(symbols[errno]());
        if (code === os.errno.EWOULDBLOCK || code === os.errno.EINTR) return false;
        throw new Error(`cannot lock build output: errno ${code}`);
      },
      mark(fd) { ftruncateSync(fd, 0); writeSync(fd, String(process.pid)); },
      close: closeSync,
    };
  }
  return locking;
}

/** `fn()` while this process holds `lock`, waiting up to 30 s for another holder. */
export function withLock(lock, fn) {
  const release = takeLock(lock);
  try { return fn(); } finally { release(); }
}

/** `lock` taken, waiting up to 30 s for another holder: what releases it. */
export function takeLock(lock) {
  const osLock = kernel(), handle = osLock.open(lock, true), started = Date.now();
  try {
    while (!osLock.take(handle, true)) {
      if (Date.now() - started > 30000) throw Object.assign(new Error(`${lock}: timed out after 30 s waiting for another build; wait for it to finish`), { code: 'ELOCKED' });
      Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 10);
    }
    osLock.mark(handle);
  } catch (e) { osLock.close(handle); throw e; }
  let held = true;
  return () => { if (held) { held = false; osLock.close(handle); } };
}

/** Whether a build owns `lock` now. */
export function lockHeld(lock) {
  const osLock = kernel();
  let handle;
  try { handle = osLock.open(lock, false); }
  catch (e) { if (e.code === 'ENOENT') return false; throw e; }
  try { return !osLock.take(handle, false); }
  finally { osLock.close(handle); }
}
