import { test, expect } from 'bun:test';
import { resolve } from 'node:path';
import { open } from '../../scripts/agent.mjs';
import { driveHeld, holdingServer } from '../../scripts/held-fixture.mjs';

// A request the server holds (`exactHeld`, LLP 1016.000 D9) on the wasm target, through the wasm module's
// `fetch`, the runner and glue.js: no clock waits on it, each clock reply names it, the answer's work after
// its reply is waited on again, and the same ask unmarked is waited on before a due timer (calendar F10).
// Against a built dist: `bun scripts/held-fixture.mjs native <dir> [port]`, then
// `(cd <dir> && EXACT_APP_DIR=<dir> EXACT_WEB_DIST=<dist> bun <checkout>/host/web/build.mjs <dir's name>-web --wasm)`,
// then EXACT_HELD_WASM_DIST=<dist> EXACT_HELD_PORT=<port> (47711).
test.skipIf(!process.env.EXACT_HELD_WASM_DIST)('the wasm target leaves a held fetch out of every clock, names it, and counts the work after its reply', async () => {
  const dist = resolve(process.env.EXACT_HELD_WASM_DIST);
  const server = holdingServer({ detailDelay: 1500, plainAfter: 3000, port: Number(process.env.EXACT_HELD_PORT ?? 47711) });
  const page = Bun.serve({ hostname: '127.0.0.1', port: 0, async fetch(req) {
    const path = new URL(req.url).pathname;
    const file = Bun.file(resolve(dist, path === '/' ? 'index.html' : path.slice(1)));
    // (a path the dist lacks is a 404, not a thrown ENOENT)
    return await file.exists() ? new Response(file) : new Response('not found', { status: 404 });
  } });
  let s;
  try {
    s = await open({ host: 'web', url: page.url.href, webDist: dist });
    const seen = await driveHeld(s, server);
    expect(seen.jump.ms).toBeLessThan(5000);
    expect(seen.jump.r).toMatchObject({ clock: 1000, held: 1, heldTargets: ['item'] });
    expect(seen.jump.r.inflight).toBeUndefined();
    expect(seen.data.r).toMatchObject({ settled: true, held: 1, heldTargets: ['item'] });
    expect(seen.settle.ms).toBeLessThan(5000);
    expect(seen.settle.r).toMatchObject({ settled: true, held: 1, heldTargets: ['item'] });
    expect(seen.state.pending).toEqual([{ name: 'item', ticket: expect.any(Number), held: true }]);
    expect(seen.after.ms).toBeGreaterThan(1000);
    expect(seen.after.r.held).toBeUndefined();
    expect(seen.item).toMatch(/^r\d+\|detail$/);
    // `clock held`: waited ~1 s for the reply, no timer fired meanwhile, the work it started landed.
    expect(seen.held.ms).toBeGreaterThan(800);
    expect(seen.held.r).toMatchObject({ settled: true });
    expect(seen.held.r.held).toBeUndefined();
    expect(seen.nAfter).toBe(seen.nBefore);
    expect(seen.heldItem).toMatch(/^r\d+\|detail$/);
    // Past its bound: not settled, naming the target.
    expect(seen.timeout.ms).toBeGreaterThan(900);
    expect(seen.timeout.r).toMatchObject({ settled: false, reason: 'held', heldTargets: ['item'] });
    expect(seen.plain.ms).toBeGreaterThan(2000);
    expect(seen.plain.r.held).toBeUndefined();
  } finally {
    await s?.close?.();
    page.stop(true);
    server.stop();
  }
}, 180_000);
