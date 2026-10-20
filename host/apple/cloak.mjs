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

/** The time (ms since the epoch) until which logged cloaks cover a display: `before`, extended when `line`, one of `log stream --style ndjson`, is a cloak going up */
export function until(before, line) {
  let entry;
  try { entry = JSON.parse(line); } catch { return before; }
  // "2026-10-19 16:22:07.182859-0700": the local time with its offset
  const m = /^(\d{4}-\d\d-\d\d) (\d\d:\d\d:\d\d\.\d{3})\d*([+-]\d\d)(\d\d)$/.exec(entry?.timestamp ?? '');
  if (!m || !String(entry.eventMessage).startsWith(UP)) return before;
  return Math.max(before, Date.parse(`${m[1]}T${m[2]}${m[3]}:${m[4]}`) + CLOAK_MS);
}

/**
 * Follows simulator `udid`'s cloak from now on. `ready()` resolves once the
 * stream is reading (so a cloak a later launch brings is seen), or after 5 s;
 * `clear()` waits until no cloak covers a display and resolves with how long
 * it waited.
 */
export function watchCloak(udid) {
  let covered = 0, buf = '';
  // End the stream when the shell's stdin closes, on `close()` or when this process dies by any signal, SIGKILL included
  const child = spawn('/bin/sh', ['-c', 'xcrun simctl spawn "$0" log stream --style ndjson --predicate "$1" & read -r _; kill $!', udid, PREDICATE], { stdio: ['pipe', 'pipe', 'ignore'] });
  child.on('error', () => {});
  child.stdin.on('error', () => {});
  // `log stream` prints its filter first, once it is reading
  const ready = new Promise((r) => { child.stdout.once('data', r); child.once('exit', r); });
  child.stdout.setEncoding('utf8');
  child.stdout.on('data', (chunk) => {
    buf += chunk;
    for (let i; (i = buf.indexOf('\n')) >= 0; buf = buf.slice(i + 1)) covered = until(covered, buf.slice(0, i));
  });
  return {
    async ready() {
      let timer;
      await Promise.race([ready, new Promise((r) => { timer = setTimeout(r, 5000); })]);
      clearTimeout(timer);
    },
    async clear() {
      const start = Date.now();
      while (Date.now() < covered) await new Promise((r) => setTimeout(r, 20));
      return Date.now() - start;
    },
    close() { child.stdin.end(); },
  };
}
