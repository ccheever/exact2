import { test, expect } from 'bun:test';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { runTests } from '../../scripts/agent-test.mjs';
import { abortServer, TESTS, within } from '../../scripts/abort-fixture.mjs';

// A fetch the app aborts, on the wasm target (scripts/abort-fixture.mjs): the realm reports it (door 23), the
// runner answers it aborted in the commit, and glue.js cancels the page's request, so the server sees its
// connection close; the aborted answer goes on, and the next request goes out at once. Against a built dist:
// `bun scripts/abort-fixture.mjs native <dir> [port]`, then its wasm build (the line that prints), then
// EXACT_ABORT_WASM_DIST=<dist> EXACT_ABORT_PORT=<port> (47713; one run at a time per port).
test.skipIf(!process.env.EXACT_ABORT_WASM_DIST)('an aborted fetch is cancelled on the wire, on the wasm target', async () => {
  const dist = resolve(process.env.EXACT_ABORT_WASM_DIST);
  let server, page, dir;
  try {
    server = abortServer({ port: Number(process.env.EXACT_ABORT_PORT ?? 47713) });
    page = Bun.serve({ hostname: '127.0.0.1', port: 0, async fetch(req) {
      const path = new URL(req.url).pathname;
      const file = Bun.file(resolve(dist, path === '/' ? 'index.html' : path.slice(1)));
      return await file.exists() ? new Response(file) : new Response('not found', { status: 404 });
    } });
    dir = mkdtempSync(resolve(import.meta.dir, '../../target/abort-wasm-'));
    const file = resolve(dir, 'app.test.contract');
    writeFileSync(file, TESTS);
    const ran = await runTests({ host: 'web', file, url: page.url.href, webDist: dist });
    expect(ran.results.map(r => [r.name, r.failures])).toEqual(ran.results.map(r => [r.name, []]));
    expect(ran.results.length).toBe(1);
    expect(server.seen).toEqual(['/held', '/next']);
    expect(await within(2000, () => server.closed.includes('/held'))).toBe(true);
  } finally {
    page?.stop(true);
    server?.stop();
    if (dir) rmSync(dir, { recursive: true, force: true });
  }
}, 300_000);
