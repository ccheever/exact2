// The process-group kill both halves of the app.ts check's watchdog use (scripts/check-supervise.mjs,
// scripts/check-leader.mjs): SIGKILL to group `pid` on POSIX; on Windows, which has no process groups to signal,
// the process tree under `pid` (`taskkill /T /F`). True when the kill was taken.
import { spawnSync } from 'node:child_process';

export function killGroup(pid) {
  if (!Number.isSafeInteger(pid) || pid <= 0) return false;
  if (process.platform === 'win32') return spawnSync('taskkill', ['/pid', String(pid), '/T', '/F'], { stdio: 'ignore' }).status === 0;
  try { process.kill(-pid, 'SIGKILL'); return true; } catch { return false; }
}
