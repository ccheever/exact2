// The native carriers' wire, shared by the stdio, simulator and phone
// carriers of `agent.mjs`: JSON lines over a duplex, and why an app that
// stopped answering stopped.
import { crashSummary } from '../host/apple/devices.mjs';
import { MalformedOperation } from './agent-keys.mjs';

/** simctl can inherit a console FIFO left by an interrupted launch. Retry that exit once. */
export function simulatorConsole(spawn, hostLines, onProcess) {
  const launch = { process: null, done: false, exited: null, pid: null };
  let retried = false, output = '';
  const start = () => {
    launch.done = false; launch.pid = null; output = '';
    const child = launch.process = spawn();
    onProcess?.(child);
    for (const stream of [child.stdout, child.stderr]) stream.on('data', data => {
      output = (output + String(data)).slice(-8192);
      for (const line of String(data).split('\n')) {
        // simctl's first line names the app's process, `<bundle id>: <pid>`.
        const pid = /^[\w.-]+: (\d+)$/.exec(line)?.[1];
        if (pid && launch.pid == null) launch.pid = Number(pid);
        if (line && !/^com\.exact\.\w+: \d+$/.test(line)) hostLines.push('app: ' + line);
      }
    });
    launch.exited = new Promise(resolve => child.on('close', (code, signal) => { launch.done = true; resolve({ code, signal }); }));
  };
  launch.retry = () => {
    if (!launch.done || retried || !/Unable to establish FIFO[\s\S]*launch_console[\s\S]*File exists/i.test(output)) return false;
    retried = true; start(); return true;
  };
  start(); return launch;
}

/** What an op answers while the app is out of the foreground: it is suspended, and answers nothing until it is back. */
export const AWAY = 'the app left the foreground (a share extension or another app took the screen), so it is suspended and every op fails until it is back; `logs` has the host lines up to then';

/** JSON lines over a duplex: each request is answered by the next line the app writes; unmatched lines are host output (`hostLines`). `fail` rejects every pending request (the app is gone).
 * A line the app pushes unasked (`{"pushed":"background"|"foreground"}`, LLP 1143 D1) answers nothing: leaving fails the
 * pending requests at once and every later one until the app is back, and the replies they were owed are dropped when
 * they come. */
export function jsonLines(readable, writable, hostLines) {
  const waiting = [];
  let failure = null, away = false, owed = 0;
  // A large reply (a `state` carrying a 16 MiB surface record) arrives in
  // many chunks: hold them and look for the newline in each new chunk only.
  let parts = [];
  readable.setEncoding('utf8');
  readable.on('data', (d) => {
    let i;
    while ((i = d.indexOf('\n')) >= 0) {
      parts.push(d.slice(0, i));
      const line = parts.join('');
      parts = [];
      d = d.slice(i + 1);
      if (line.startsWith('{"pushed":')) {
        away = line.includes('"background"');
        if (away) { owed += waiting.length; for (const w of waiting.splice(0)) w.reject(new Error(AWAY)); }
        hostLines.push(`app: ${away ? 'left the foreground' : 'back in the foreground'}`);
        continue;
      }
      if (owed) { owed--; hostLines.push('app: a reply owed to an op that failed while the app was away: ' + line.slice(0, 200)); continue; }
      const w = waiting.shift();
      if (!w) { hostLines.push('app: ' + line); continue; }
      try { w.resolve(JSON.parse(line)); } catch { w.reject(new Error('unreadable reply: ' + line)); }
    }
    if (d) parts.push(d);
  });
  const next = () => failure ? Promise.reject(failure) : new Promise((resolve, reject) => waiting.push({ resolve, reject }));
  return {
    next,
    ask: (req) => {
      if (away && !failure) return Promise.reject(new Error(AWAY));
      const p = next(); if (!failure) writable.write(JSON.stringify(req) + '\n'); return p.then(MalformedOperation.reply);
    },
    fail: (why) => { failure ??= new Error(why); for (const w of waiting.splice(0)) w.reject(failure); },
  };
}
/** Why a native app stopped answering, for the error that ends the run: its pid alive or gone, how it exited, any crash report since launch, and the last 20 lines it wrote — not all of them since the last `logs`. */
export function hangup({ what, pid = null, exit = null, reports = [], hostLines = [] }) {
  const alive = pid == null ? null : (() => { try { process.kill(pid, 0); return true; } catch { return false; } })();
  const status = exit ? (exit.signal ? `killed by ${exit.signal}` : `exit code ${exit.code}`) : alive == null ? 'no pid known' : `pid ${pid} ${alive ? 'still running' : 'gone'}`;
  return [`${what} (${status})`, ...reports.slice(0, 1).flatMap((f) => crashSummary(f)), ...reports.slice(1).map((f) => `crash report: ${f}`), ...hostLines.slice(-20)].join('\n');
}

/** A native reply's deadline: past `clock settle`'s own 20 s bound, so only a wedged app reaches it (a phone has 45 s). */
export const REPLY_MS = 120000;
/** A refused boot; a format refusal usually means a host binary built before the plan's format changed. */
export const bootRefusal = (error) => 'the app booted with an error: ' + error +
  (/UnsupportedVersion|FormatDigestMismatch|KernelSchemaMismatch/.test(error) ? ' (the plan and the host binary were built from different formats: rebuild the host)' : '');

// Release completed operations' deadlines so a closed driver does not wait for a losing timeout.
export async function waitAtMost(operation, ms, onTimeout) {
  let timer;
  const deadline = new Promise(resolve => { timer = setTimeout(resolve, ms); }).then(onTimeout);
  try { return await Promise.race([operation, deadline]); }
  finally { clearTimeout(timer); }
}
