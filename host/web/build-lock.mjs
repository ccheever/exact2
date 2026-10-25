// The lock builds take for work of milliseconds on a tree others read: replaceBuild's swap of dist (serve.mjs) and the
// JS build's GPU cache rename and copy (host/web-js/gpu-cache.mjs). Under Bun on macOS and Linux it is the kernel's
// flock on the lock file, as scripts/sweep.mjs takes Cargo's: released when its holder exits, killed or not, so a lock a
// killed build left is never stolen (a stale lock file cannot be removed atomically: two waiters that both judge it
// left over each remove it, the second the one the first had just taken). The file stays, naming its last holder.
// Elsewhere (no bun:ffi, Windows) it is a file made exclusively, naming its pid, taken over when that pid is gone.
import { closeSync, fstatSync, ftruncateSync, openSync, readFileSync, rmSync, statSync, writeFileSync, writeSync } from 'node:fs';

const LOCK_SH = 1, LOCK_EX = 2, LOCK_NB = 4;
let flock;
const kernel = () => {
  if (flock !== undefined) return flock;
  flock = null;
  if (typeof Bun === 'undefined' || process.platform === 'win32') return flock;
  try {
    const { dlopen, FFIType } = import.meta.require('bun:ffi');
    flock = dlopen(process.platform === 'darwin' ? '/usr/lib/libSystem.B.dylib' : 'libc.so.6', { flock: { args: [FFIType.i32, FFIType.i32], returns: FFIType.i32 } }).symbols.flock;
  } catch { /* the file lock below */ }
  return flock;
};
const alive = (pid) => { try { process.kill(pid, 0); return true; } catch (e) { return e.code === 'EPERM'; } };
const holder = (lock) => { try { return readFileSync(lock, 'utf8').trim() || 'unknown'; } catch { return 'unknown'; } };
const waitFor = (lock, started) => {
  if (Date.now() - started > 30000) throw new Error(`${lock}: process ${holder(lock)} has held it for 30 s; remove it if that process is not a build`);
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 10);
};

/** `fn()` while this process holds `lock`, waiting up to 30 s for another holder. */
export function withLock(lock, fn) {
  const release = takeLock(lock);
  try { return fn(); } finally { release(); }
}

/** `lock` taken, waiting up to 30 s for another holder: what releases it. */
export function takeLock(lock) {
  const started = Date.now(), lockFile = kernel();
  if (!lockFile) return takeFileLock(lock, started);
  for (;;) {
    const fd = openSync(lock, 'a');
    let held = false;
    try {
      if (lockFile(fd, LOCK_EX | LOCK_NB) !== 0) { waitFor(lock, started); continue; }
      // Held, unless the file was removed or replaced since it was opened (a lock on a name no one else opens).
      try { held = statSync(lock).ino === fstatSync(fd).ino; } catch { /* removed */ }
      if (!held) continue;
      ftruncateSync(fd, 0); writeSync(fd, String(process.pid));
      return () => closeSync(fd);
    } finally { if (!held) closeSync(fd); }
  }
}

/** Whether a holder has `lock` now. */
export function lockHeld(lock) {
  const lockFile = kernel();
  if (!lockFile) {
    let owner;
    try { owner = Number(readFileSync(lock, 'utf8')); } catch { return false; }
    if (!Number.isInteger(owner) || owner <= 0) return true; // being written: a holder is starting
    return alive(owner);
  }
  let fd;
  try { fd = openSync(lock, 'r'); } catch { return false; }
  try { return lockFile(fd, LOCK_SH | LOCK_NB) !== 0; } finally { closeSync(fd); }
}

function takeFileLock(lock, started) {
  for (;;) {
    try { writeFileSync(lock, String(process.pid), { flag: 'wx' }); break; }
    catch (error) {
      if (error.code !== 'EEXIST') throw error;
      let owner = 0, age = 0;
      try { owner = Number(readFileSync(lock, 'utf8')); age = Date.now() - statSync(lock).mtimeMs; } catch { continue; }
      // A lock whose owner is gone, or that names none after a second, is left over (two waiters may both take it:
      // the race the flock above has not).
      if (Number.isInteger(owner) && owner > 0 ? !alive(owner) : age > 1000) { rmSync(lock, { force: true }); continue; }
      waitFor(lock, started);
    }
  }
  return () => rmSync(lock, { force: true });
}
