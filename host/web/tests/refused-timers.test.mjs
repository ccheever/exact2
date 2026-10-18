import { expect, afterAll } from 'bun:test';
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { need } from '../../../scripts/coverage.mjs';
import { grantOrigins } from '../navigation.js';
import { createTimerScheduler } from '../timer-glue.js';

const root = resolve(new URL('../../..', import.meta.url).pathname);
const target = resolve(process.env.CARGO_TARGET_DIR ?? resolve(root, 'target'));
const wasm = resolve(process.env.EXACT_WEB_DIST ?? resolve(root, 'host/web/dist'), 'app.wasm');
const compiler = resolve(target, 'debug/contract');
const check = need('fixture', !existsSync(wasm) || !existsSync(compiler)
  ? 'build Caltrain with --wasm and cargo build -p contract --bin contract' : null);
let scratch, plan;
afterAll(() => { if (scratch) rmSync(scratch, { recursive: true, force: true }); });

async function bridge() {
  if (!plan) {
    scratch = mkdtempSync(resolve(target, 'refused-timers-'));
    const file = resolve(scratch, 'app.plan');
    const built = spawnSync(compiler, ['build', resolve(root, 'host/web/tests/fixtures/refused-timers.contract'), '-o', file], { encoding: 'utf8' });
    expect(built.status, built.stderr).toBe(0);
    plan = readFileSync(file);
  }
  const unexpected = () => { throw new Error('the timer fixture called a data or geometry import'); };
  const { instance } = await WebAssembly.instantiate(readFileSync(wasm), {
    exact_grants: grantOrigins(() => instance.exports.memory), exact_js: { call: unexpected },
    exact_rust: { load: unexpected, call: unexpected, read: unexpected, drop: unexpected },
    exact_data: { random: unexpected, agent_seed: unexpected }, exact_clock: { now: unexpected },
    exact_geometry: { read: unexpected, point: unexpected, points: unexpected, shown: unexpected },
  });
  const w = instance.exports;
  const input = bytes => { const ptr = w.exact_in(bytes.length); new Uint8Array(w.memory.buffer, ptr, bytes.length).set(bytes); return bytes.length; };
  const output = len => JSON.parse(new TextDecoder().decode(new Uint8Array(w.memory.buffer, w.exact_out(), len)));
  expect(output(w.exact_boot_plan(input(plan), 400, 400, 0, 0)).error).toBeNull();
  return { w, input, output };
}

for (const kind of ['malformed scroll', 'failed reload']) {
  check(`${kind} preserves the page scheduler's live timeout and frame subscription`, async () => {
    const { w, input, output } = await bridge();
    const live = output(w.exact_frame(10));
    expect(live.timer_due_ms).toBe(1000);
    let now = 10, serial = 0;
    const timers = new Map(), frames = new Map(), fired = [];
    const scheduler = createTimerScheduler({
      now: () => now,
      advance(time) { const batch = output(w.exact_advance(time, 0)); fired.push(batch); scheduler.update(batch.timer_due_ms, batch.frames); },
      present(time) { const batch = output(w.exact_frame(time)); scheduler.update(batch.timer_due_ms, batch.frames); },
      raf(fn) { frames.set(++serial, fn); return serial; }, cancel(id) { frames.delete(id); },
      delay(fn, ms) { timers.set(++serial, { fn, ms }); return serial; }, clearDelay(id) { timers.delete(id); },
    });
    try {
      scheduler.update(live.timer_due_ms, live.frames);
      expect(timers.size).toBe(1);
      now = 500;
      const refused = kind === 'malformed scroll'
        ? output(w.exact_dispatch(0, 13, input(new TextEncoder().encode('invalid')), now))
        : output(w.exact_boot_plan(input(new Uint8Array()), 400, 400, 0, 0));
      expect(refused.error).toContain(kind === 'malformed scroll' ? 'invalid scroll' : 'boot:');
      scheduler.update(refused.timer_due_ms, refused.frames);
      expect(timers.size).toBe(1);
      expect(frames.size).toBe(1);
      const timeout = [...timers.values()][0];
      expect(timeout.ms).toBe(500);
      timers.clear(); now = 1000; timeout.fn();
      expect(fired).toHaveLength(1);
      expect(fired[0].timer_due_ms).toBe(2000);
      expect(fired[0].ops.some(op => op.set?.text === '1')).toBe(true);
    } finally { scheduler.dispose(); }
    expect(timers.size + frames.size).toBe(0);
  });
}
