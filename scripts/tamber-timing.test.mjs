// The design server's clocks (tamber-timing.mjs): the real ones pinned (a long poll's 25 s wait, the 5 s sweep;
// presence's 35 s and 90 s are pinned in tamber-events-get.test.mjs), and the test-only knobs read only under
// EXACT_TAMBER_TEST_TIMING=1: a serve without it keeps the real clocks however the knobs are set.
import { expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { EventEmitter } from 'node:events';
import { PassThrough } from 'node:stream';
import { resolve } from 'node:path';
import { developmentGate } from './install-page.mjs';
import { REAL, TIMING, timingOf } from './tamber-timing.mjs';

const ROOT = resolve(new URL('..', import.meta.url).pathname);
const KNOBS = { EXACT_TAMBER_POLL_MS: '2000', EXACT_TAMBER_PRESENT_MS: '3000', EXACT_TAMBER_LONG_PRESENT_MS: '8000', EXACT_TAMBER_SWEEP_MS: '500', EXACT_TAMBER_VIA_WAIT_MS: '1500' };

test('the real clocks: 25 s, 35 s, 90 s and 5 s, and --via\'s 20 min wait, which this run (no test switch) uses', () => {
  expect(REAL).toEqual({ poll: 25_000, present: 35_000, longPresent: 90_000, sweep: 5_000, viaWait: 20 * 60_000 });
  expect(TIMING).toBe(REAL);
});

test('the knobs count only under the test switch, and only to shorten', () => {
  // Without the switch (a person's serve, whatever its environment says): the real clocks.
  expect(timingOf(KNOBS)).toBe(REAL);
  expect(timingOf({ ...KNOBS, EXACT_TAMBER_TEST_TIMING: 'true' })).toBe(REAL);
  expect(timingOf({ ...KNOBS, EXACT_TAMBER_TEST_TIMING: '1' })).toEqual({ poll: 2000, present: 3000, longPresent: 8000, sweep: 500, viaWait: 1500 });
  expect(timingOf({ EXACT_TAMBER_TEST_TIMING: '1', EXACT_TAMBER_POLL_MS: '2000' })).toEqual({ ...REAL, poll: 2000 });
  for (const bad of ['26000', '49', '2s', '-1', '1.5', ''])
    expect(() => timingOf({ EXACT_TAMBER_TEST_TIMING: '1', EXACT_TAMBER_POLL_MS: bad })).toThrow(/EXACT_TAMBER_POLL_MS .*test timing only/);
});

test('a server process reads the knobs only under the switch, and says so when it does', () => {
  const run = env => spawnSync(process.execPath, ['-e', "const s = await import('./scripts/tamber-server.mjs'); const t = await import('./scripts/tamber-timing.mjs'); console.log(JSON.stringify(t.TIMING))"],
    { cwd: ROOT, encoding: 'utf8', env: { ...process.env, EXACT_TAMBER_TEST_TIMING: '', ...env } });
  const plain = run(KNOBS);
  expect(plain.status, plain.stderr).toBe(0);
  expect(JSON.parse(plain.stdout)).toEqual(REAL);
  expect(plain.stderr).not.toContain('TEST timing');
  const tested = run({ ...KNOBS, EXACT_TAMBER_TEST_TIMING: '1' });
  expect(tested.status, tested.stderr).toBe(0);
  expect(JSON.parse(tested.stdout)).toEqual({ poll: 2000, present: 3000, longPresent: 8000, sweep: 500, viaWait: 1500 });
  expect(tested.stderr).toContain('tamber: TEST timing (EXACT_TAMBER_TEST_TIMING=1): poll 2000 ms, present 3000 ms, longPresent 8000 ms, sweep 500 ms');
});

/** A stand-in for `contract tamber serve` that answers every request. */
function fakeServe() {
  const child = Object.assign(new EventEmitter(), { stdin: new PassThrough(), stdout: new PassThrough(), stderr: new PassThrough(), exitCode: null, signalCode: null });
  child.kill = () => { child.exitCode = 0; child.emit('exit', 0); };
  let buf = '';
  child.stdin.on('data', chunk => {
    buf += chunk; const lines = buf.split('\n'); buf = lines.pop();
    for (const line of lines) child.stdout.write(`${JSON.stringify({ id: JSON.parse(line).id, ok: { files: {} } })}\n`);
  });
  return child;
}

test('the server waits 25 s on a long poll with nothing new, and sweeps every 5 s (fake timers)', async () => {
  const timeouts = [], intervals = [];
  const [realTimeout, realInterval] = [globalThis.setTimeout, globalThis.setInterval];
  // Each timer recorded; the 25 s one runs at once, so the poll answers now.
  globalThis.setTimeout = (fn, ms, ...rest) => { timeouts.push(ms); return realTimeout(fn, ms === 25_000 ? 1 : ms, ...rest); };
  globalThis.setInterval = (fn, ms, ...rest) => { intervals.push(ms); return realInterval(fn, ms, ...rest); };
  const { tamberServer } = await import('./tamber-server.mjs');
  const tamber = tamberServer({ appDir: '/nowhere', spawnServe: fakeServe, log: () => {} });
  try {
    expect(intervals).toContain(5_000);
    const gate = developmentGate([{ origin: 'http://127.0.0.1:8818' }], 8818);
    const headers = { host: '127.0.0.1:8818', 'x-exact-tamber-token': tamber.token };
    const ask = async path => {
      const req = Object.assign(new PassThrough(), { method: 'GET', headers, socket: { remoteAddress: '127.0.0.1' } });
      let status = 0, text = '';
      const res = Object.assign(new EventEmitter(), { destroyed: false, writableEnded: false, setHeader() {}, writeHead(s) { status = s; }, write(t) { text += t; return true; }, end(t) { if (t) text += t; this.writableEnded = true; this.emit('finish'); } });
      queueMicrotask(() => req.end(''));
      await tamber.handle(req, res, new URL(path, 'http://x'), gate.check(req));
      return { status, body: text ? JSON.parse(text) : null };
    };
    const first = await ask('/__tamber/events?client=tool');
    expect(first.status).toBe(200);
    const before = timeouts.length;
    const quiet = await ask(`/__tamber/events?after=${first.body.cursor}&epoch=${first.body.epoch}&client=tool`);
    expect(quiet.status).toBe(204);
    expect(timeouts.slice(before)).toContain(25_000);
  } finally {
    globalThis.setTimeout = realTimeout; globalThis.setInterval = realInterval;
    await tamber.close();
  }
});
