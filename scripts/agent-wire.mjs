// The native carriers' wire, shared by the stdio, simulator and phone
// carriers of `agent.mjs`: JSON lines over a duplex, and why an app that
// stopped answering stopped.
import { crashSummary } from '../host/apple/devices.mjs';
import { MalformedOperation } from './agent-keys.mjs';

/** JSON lines over a duplex: each request is answered by the next line the app writes; unmatched lines are host output (`hostLines`). `fail` rejects every pending request (the app is gone). */
export function jsonLines(readable, writable, hostLines) {
  const waiting = [];
  let failure = null;
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
      const w = waiting.shift();
      if (!w) { hostLines.push('app: ' + line); continue; }
      try { w.resolve(JSON.parse(line)); } catch { w.reject(new Error('unreadable reply: ' + line)); }
    }
    if (d) parts.push(d);
  });
  const next = () => failure ? Promise.reject(failure) : new Promise((resolve, reject) => waiting.push({ resolve, reject }));
  return {
    next,
    ask: (req) => { const p = next(); if (!failure) writable.write(JSON.stringify(req) + '\n'); return p.then(MalformedOperation.reply); },
    fail: (why) => { failure ??= new Error(why); for (const w of waiting.splice(0)) w.reject(failure); },
  };
}
/** Why a native app stopped answering, for the error that ends the run: its pid alive or gone, how it exited, any crash report since launch, and the last 20 lines it wrote — not all of them since the last `logs`. */
export function hangup({ what, pid = null, exit = null, reports = [], hostLines = [] }) {
  const alive = pid == null ? null : (() => { try { process.kill(pid, 0); return true; } catch { return false; } })();
  const status = exit ? (exit.signal ? `killed by ${exit.signal}` : `exit code ${exit.code}`) : alive == null ? 'no pid known' : `pid ${pid} ${alive ? 'still running' : 'gone'}`;
  return [`${what} (${status})`, ...reports.slice(0, 1).flatMap((f) => crashSummary(f)), ...reports.slice(1).map((f) => `crash report: ${f}`), ...hostLines.slice(-20)].join('\n');
}
