// Hold real touches until SpringBoard's main transitions and, on the iPhone
// Duo, fold cloaks clear. On an iPhone 18 Pro simulator with iOS 27.0, the
// scene became foreground-active 0.25 s before the launch's Complete line;
// a touch in between was lost or landed on another node.
//
// Hold a cloak from its logged arrival: its lift, reveal and drop lines do
// not identify the display change, so they can belong to an earlier cloak.
// Track main transitions by ID until Complete, with a bound for a missing line.
import { spawn } from 'node:child_process';

const UP = 'ContinuousSnapshot: cloaked arriving panel';
const CLOAK = `process == "SpringBoard" AND category == "DisplayContentMode" AND eventMessage BEGINSWITH "${UP}"`;
const TRANSITIONS = 'process == "SpringBoard" AND category == "AppSwitcher" AND eventMessage CONTAINS "type: MainTransition;"';

/** Hold cloaks beyond the longest measured arrival-to-overlay-drop time, 3.0 s */
export const CLOAK_MS = 4000;

/** Bound a missing Complete beyond the measured Prepare-to-Complete time, 0.5 s */
export const TRANSITION_MS = 3000;

/** Read an ndjson log entry as epoch milliseconds and a message, or null */
export function logEntry(line) {
  let entry;
  try { entry = JSON.parse(line); } catch { return null; }
  const m = /^(\d{4}-\d\d-\d\d) (\d\d:\d\d:\d\d\.\d{3})\d*([+-]\d\d)(\d\d)$/.exec(entry?.timestamp ?? '');
  if (!m || typeof entry.eventMessage !== 'string') { return null; }
  const t = Date.parse(`${m[1]}T${m[2]}${m[3]}:${m[4]}`);
  return Number.isFinite(t) ? { t, message: entry.eventMessage } : null;
}

/** Extend a cloak's deadline from its logged arrival, preserving later deadlines */
export function until(before, line) {
  const entry = logEntry(line);
  return entry?.message.startsWith(UP) ? Math.max(before, entry.t + CLOAK_MS) : before;
}

/** Update open transitions from a phase line, retaining each ID until Complete or expiry */
export function opened(open, line) {
  const entry = logEntry(line);
  const phase = entry && /type: MainTransition; transitionID: ([0-9A-F-]+); phase: (\w+);/i.exec(entry.message);
  if (!phase) { return open; }
  const next = new Map([...open].filter(([, t]) => t + TRANSITION_MS > entry.t));
  const id = phase[1].toUpperCase();
  if (phase[2].toLowerCase() === 'complete') { next.delete(id); }
  else { next.set(id, Math.max(next.get(id) ?? 0, entry.t)); }
  return next;
}

/** Return the latest deadline of open transitions, or 0 */
export function openUntil(open) {
  return Math.max(0, ...[...open.values()].map((t) => t + TRANSITION_MS));
}

/**
 * Read the simulator's log for `predicate`, passing complete ndjson lines to
 * `onLine`. Resolve ready on the first output or exit, bounded at 5 s.
 */
export function logStream(udid, predicate, onLine) {
  let buf = '';
  // End and reap the log process when stdin closes, including when this driver dies
  const child = spawn('/bin/sh', ['-c', 'xcrun simctl spawn "$0" log stream --style ndjson --predicate "$1" & read -r _; kill $!; wait $!', udid, predicate], { stdio: ['pipe', 'pipe', 'ignore'] });
  child.on('error', () => {});
  child.stdin.on('error', () => {});
  const exited = new Promise((r) => { child.once('close', r); });
  const ready = new Promise((r) => { child.stdout.once('data', r); child.once('exit', r); });
  child.stdout.setEncoding('utf8');
  child.stdout.on('data', (chunk) => {
    buf += chunk;
    for (let i; (i = buf.indexOf('\n')) >= 0; buf = buf.slice(i + 1)) { onLine(buf.slice(0, i)); }
  });
  return {
    async ready() {
      let timer;
      await Promise.race([ready, new Promise((r) => { timer = setTimeout(r, 5000); })]);
      clearTimeout(timer);
    },
    close() { child.stdin.end(); return exited; },
  };
}

/** Follow main transitions and optionally Duo cloaks, waiting until both clear */
export function watchCloak(udid, foldable = false, readLog = logStream) {
  let covered = 0, open = new Map();
  const changed = new Set();
  const stream = readLog(udid, foldable ? `(${CLOAK}) OR (${TRANSITIONS})` : TRANSITIONS, (line) => {
    if (foldable) { covered = until(covered, line); }
    open = opened(open, line);
    for (const wake of changed) { wake(); }
  });
  return {
    ready: stream.ready,
    async clear() {
      let cloaked = 0, transitioned = 0;
      for (;;) {
        const now = Date.now(), cloak = covered, transition = openUntil(open), end = Math.max(cloak, transition);
        if (now >= end) { return { cloaked, transitioned }; }
        let timer, wake;
        await new Promise((r) => {
          wake = r;
          changed.add(wake);
          timer = setTimeout(r, Math.min(...[cloak, transition].filter((t) => t > now)) - now);
        });
        changed.delete(wake);
        clearTimeout(timer);
        const elapsed = Date.now() - now;
        if (now < cloak) { cloaked += elapsed; }
        if (now < transition) { transitioned += elapsed; }
      }
    },
    close: stream.close,
  };
}
