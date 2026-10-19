// A port test runs take in turn. The turn is a second loopback port, held
// by listening on it: the kernel lets one process listen there and frees it
// when that process ends however it ends, so a holder gone leaves no stale
// lock. Whoever holds it answers a connection with who it is (its label,
// pid and since when), which the next run says while it waits. The design
// tool's test port is one such: its contract tests' stub and
// scripts/design-tool.test.mjs's real tool run on the checkout's own
// (`checkoutPort`, or EXACT_TOOL_PORT), never a person's 7740.
// The real tool takes no turn, so a run that has the turn and still finds
// the port answering says whose process it is. A run started by one that
// holds the turn (named in EXACT_PORT_TURNS, which its children inherit)
// has it already.
import { createHash } from 'node:crypto';
import { createConnection, createServer } from 'node:net';

/** The checkout `root`'s own port for the design tool's test runs
 * (EXACT_TOOL_PORT's default there): one of 200 above 17740, by the
 * checkout's path, so two checkouts share one only by chance and then take
 * turns on it; never a person's 7740. */
export const checkoutPort = (root) => 17741 + (parseInt(createHash('sha256').update(root).digest('hex').slice(0, 8), 16) % 200);

/** The port that holds `port`'s turn. */
export const turnPort = (port) => (port + 10000 <= 65535 ? port + 10000 : port - 10000);

const held = (port) => (process.env.EXACT_PORT_TURNS ?? '').split(',').includes(String(port));

/** Who holds `port`'s turn (`{label, pid, at}`), or null when no run does. */
export function holderOf(port) {
  return new Promise((done) => {
    let text = '';
    const socket = createConnection({ host: '127.0.0.1', port: turnPort(port) });
    socket.setTimeout(2000, () => { socket.destroy(); done({ label: 'another run', pid: 0, at: '' }); });
    socket.on('data', (d) => { text += d; });
    socket.on('end', () => { try { done(JSON.parse(text)); } catch { done({ label: 'another run', pid: 0, at: '' }); } });
    socket.on('error', () => done(null));
  });
}

const listen = (port, card) => new Promise((done) => {
  const server = createServer((socket) => socket.end(card));
  server.once('error', () => done(null));
  server.listen({ host: '127.0.0.1', port: turnPort(port), exclusive: true }, () => { server.unref(); done(server); });
});

/** Waits for `port`'s turn (up to `waitMs`, saying once whom it waits for)
 * and takes it for `label`. Resolves to a function that gives it back
 * (once; this process's end does too). Rejects naming the holder when the
 * wait runs out. */
export async function takePort(port, { label = 'a test run', waitMs = 20 * 60_000, say = (line) => console.error(line) } = {}) {
  if (held(port)) return () => {};
  const card = JSON.stringify({ label, pid: process.pid, at: new Date().toISOString() }), end = Date.now() + waitMs;
  let told = false, server;
  while (!(server = await listen(port, card))) {
    const who = await holderOf(port);
    const name = who ? `${who.label}${who.pid ? ` (pid ${who.pid}, since ${who.at})` : ''}` : 'another run';
    if (Date.now() > end) throw new Error(`port ${port} is still taken by ${name} after ${Math.round(waitMs / 1000)} s: wait for it, or end it`);
    if (who && !told) { say(`port ${port}: waiting for ${name} to finish`); told = true; }
    await new Promise((ok) => setTimeout(ok, 500));
  }
  const before = process.env.EXACT_PORT_TURNS;
  process.env.EXACT_PORT_TURNS = [before, port].filter(Boolean).join(',');
  let given = false;
  return () => {
    if (given) return;
    given = true;
    server.close();
    process.env.EXACT_PORT_TURNS = (process.env.EXACT_PORT_TURNS ?? '').split(',').filter((p) => p && p !== String(port)).join(',');
  };
}
