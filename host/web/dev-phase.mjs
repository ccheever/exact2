// The resident loop's long steps, said as they start and end (dtool2's pass
// 3 #8). The JS loop keeps the latest `phase: ` line and answers a native
// client's envelope request with it while the resident starts, as
// `application/vnd.exact.dev-status+json` (llp/1023.001-serving-stage-1.spec.md);
// a `phase: failed: ` line before the first plan is the error it answers.
import { spawnSync } from 'node:child_process';
import { closeSync, mkdtempSync, openSync, readSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

/** The prefix of a resident's phase line on its stdout. */
export const PHASE = 'phase: ';
/** A phase line that says the step failed, and why. */
export const FAILED = `${PHASE}failed: `;
/** The media type of a dev loop's status answer to a native client. */
export const DEV_STATUS_TYPE = 'application/vnd.exact.dev-status+json';

/** Text as a status line carries it: no terminal colours, on one line. */
export const plain = (text) => String(text).replace(/\x1b\[[0-9;]*[A-Za-z]/g, '').trim();

/** An error's gist on one line: its `error` lines (a compiler's diagnostic,
 * not a runtime trace after it), else its last lines; plain. */
export function gist(error) {
  const lines = plain(error).split('\n').map(l => l.trim()).filter(Boolean);
  const said = lines.filter(l => /^error\b/i.test(l)).slice(-3);
  return (said.length ? said : lines.slice(-3)).join(' · ');
}

/** Say that a step failed, and why, as one phase line. */
export function failedPhase(error, log = console.log) {
  log(`${FAILED}${gist(error)}`);
}

/**
 * Build with `command`, saying `what` it builds before and how it went
 * after. Its errors are shown as they were and kept: a build that fails
 * says them as a failed phase (the client's error, not the trace that
 * follows when the resident gives up).
 */
export function phased(what, command, args, options, log = console.log, echo = (bytes) => process.stderr.write(bytes)) {
  log(`${PHASE}building ${what}`);
  const started = Date.now();
  // Its errors go to a file, however many (no capture limit to kill it at),
  // then on to stderr as they were, the last of them kept.
  const dir = mkdtempSync(join(tmpdir(), 'exact-phase-')), path = join(dir, 'stderr');
  let tail = '', result;
  try {
    const out = openSync(path, 'w');
    try { result = spawnSync(command, args, { ...options, stdio: ['inherit', 'inherit', out] }); } finally { closeSync(out); }
    const file = openSync(path, 'r'), chunk = Buffer.alloc(64 * 1024);
    try {
      for (let n; (n = readSync(file, chunk, 0, chunk.length, null)) > 0;) {
        echo(Buffer.from(chunk.subarray(0, n)));
        tail = (tail + chunk.toString('utf8', 0, n)).slice(-16 * 1024);
      }
    } finally { closeSync(file); }
  } finally { rmSync(dir, { recursive: true, force: true }); }
  log(`${PHASE}${what}: ${result.status === 0 ? 'built' : 'failed'} in ${((Date.now() - started) / 1000).toFixed(1)} s`);
  if (result.status !== 0) failedPhase(`${what} did not build\n${tail}`, log);
  return result;
}
