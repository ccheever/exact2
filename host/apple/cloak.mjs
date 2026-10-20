// SpringBoard's fold cloak on the iPhone Duo simulator. When one display
// takes over from the other, SpringBoard covers the arriving display with a
// window of its own ("ContinuousSnapshot: cloaked arriving panel") that takes
// every touch until it fades out and SpringBoard drops its overlays, 1.4 to
// 3 s later. Closed with only the home screen up, the Duo does this when the
// next app launches, under a drive's first touch. Nothing the app or XCTest
// reads shows the cloak (the scene stays foreground-active with its window
// key, and `isHittable` holds), so the touch driver reads SpringBoard's log
// and holds a real touch until `CLOAK_MS` after the last cloak went up. It
// reads no other line: SpringBoard logs a cloak's lift, reveal and drop
// without naming its transition, and an earlier transition's can arrive
// during the next. If the lines change or the stream ends, touches stop
// waiting for new cloaks.
import { spawn } from 'node:child_process';

/** How SpringBoard's line for a cloak going up begins */
const UP = 'ContinuousSnapshot: cloaked arriving panel';

/** What `log stream` reads: SpringBoard's lines for a cloak going up */
export const PREDICATE = `process == "SpringBoard" AND category == "DisplayContentMode" AND eventMessage BEGINSWITH "${UP}"`;

/** How long after a cloak goes up a touch is held: the longest measured, 3.0 s to its overlays' drop, with a margin */
export const CLOAK_MS = 4000;

/** One line of `log stream --style ndjson` as `{ t, message }` (`t` in ms since the epoch), or null for its header or anything else */
export function logEntry(line) {
  let entry;
  try { entry = JSON.parse(line); } catch { return null; }
  // "2026-10-19 16:22:07.182859-0700": the local time with its offset
  const m = /^(\d{4}-\d\d-\d\d) (\d\d:\d\d:\d\d\.\d{3})\d*([+-]\d\d)(\d\d)$/.exec(entry?.timestamp ?? '');
  if (!m || typeof entry.eventMessage !== 'string') return null;
  return { t: Date.parse(`${m[1]}T${m[2]}${m[3]}:${m[4]}`), message: entry.eventMessage };
}

/** The time (ms since the epoch) until which logged cloaks cover a display: `before`, extended when `line`, one of `log stream --style ndjson`, is a cloak going up */
export function until(before, line) {
  const entry = logEntry(line);
  return entry?.message.startsWith(UP) ? Math.max(before, entry.t + CLOAK_MS) : before;
}

/**
 * Follows simulator `udid`'s log for `predicate` from now on, handing each
 * line of `log stream --style ndjson` to `onLine`. `ready()` resolves once
 * the stream is reading, or after 5 s; `close()` ends it.
 */
export function logStream(udid, predicate, onLine) {
  let buf = '';
  // End the stream when the shell's stdin closes, on `close()` or when this process dies by any signal, SIGKILL included
  const child = spawn('/bin/sh', ['-c', 'xcrun simctl spawn "$0" log stream --style ndjson --predicate "$1" & read -r _; kill $!', udid, predicate], { stdio: ['pipe', 'pipe', 'ignore'] });
  child.on('error', () => {});
  child.stdin.on('error', () => {});
  // `log stream` prints its filter first, once it is reading
  const ready = new Promise((r) => { child.stdout.once('data', r); child.once('exit', r); });
  child.stdout.setEncoding('utf8');
  child.stdout.on('data', (chunk) => {
    buf += chunk;
    for (let i; (i = buf.indexOf('\n')) >= 0; buf = buf.slice(i + 1)) onLine(buf.slice(0, i));
  });
  return {
    async ready() {
      let timer;
      await Promise.race([ready, new Promise((r) => { timer = setTimeout(r, 5000); })]);
      clearTimeout(timer);
    },
    close() { child.stdin.end(); },
  };
}

/**
 * Follows simulator `udid`'s cloak from now on. `ready()` resolves once the
 * stream is reading (so a cloak a later launch brings is seen), or after 5 s;
 * `clear()` waits until no cloak covers a display and resolves with how long
 * it waited.
 */
export function watchCloak(udid) {
  let covered = 0;
  const stream = logStream(udid, PREDICATE, (line) => { covered = until(covered, line); });
  return {
    ready: stream.ready,
    async clear() {
      const start = Date.now();
      while (Date.now() < covered) await new Promise((r) => setTimeout(r, 20));
      return Date.now() - start;
    },
    close: stream.close,
  };
}
