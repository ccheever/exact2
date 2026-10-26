// The process-group kill both halves of the app.ts check's watchdog use (scripts/check-supervise.mjs,
// scripts/check-leader.mjs): SIGKILL to group `pid` on POSIX; on Windows, which has no process groups to signal,
// the process tree under `pid` (`taskkill /T /F`). True when the kill was taken.
import { spawnSync } from 'node:child_process';

export function killGroup(pid) {
  if (!Number.isSafeInteger(pid) || pid <= 0) return false;
  if (process.platform === 'win32') return spawnSync('taskkill', ['/pid', String(pid), '/T', '/F'], { stdio: 'ignore' }).status === 0;
  try { process.kill(-pid, 'SIGKILL'); return true; } catch { return false; }
}

/** Gone, the parent this process was given: on POSIX it is re-parented (its ppid changes); Windows keeps a dead
 *  parent's pid as its ppid, so that pid is also asked whether it still runs. */
export function orphaned(parent) {
  if (process.ppid !== parent) return true;
  try { process.kill(parent, 0); return false; } catch (e) { return e.code !== 'EPERM'; }
}
