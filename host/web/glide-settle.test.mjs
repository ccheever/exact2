import { test, expect } from 'bun:test';
import { resolve } from 'node:path';
import { open } from '../../scripts/agent.mjs';
import { driveGlide, expectGlide, glideServer } from '../../scripts/glide-fixture.mjs';

// `clock settle` on the wasm target waits for a list a drag left gliding, so its last scroll is delivered before it
// answers; a held drag settles at once; a scroller the page keeps moving gives up at 4 s with reason `scroll`, as a
// native host does (scripts/glide-fixture.mjs). Against a built dist: `bun scripts/glide-fixture.mjs native <dir> [port]`,
// then its wasm build (the line that prints), then EXACT_GLIDE_WASM_DIST=<dist> EXACT_GLIDE_PORT=<port> (47713).
test.skipIf(!process.env.EXACT_GLIDE_WASM_DIST)('the wasm target settles after a drag\'s glide, at once after a held drag, and gives up on a scroller that never stops', async () => {
  const dist = resolve(process.env.EXACT_GLIDE_WASM_DIST);
  let server, page, s;
  try {
    server = glideServer({ port: Number(process.env.EXACT_GLIDE_PORT ?? 47713) });
    page = Bun.serve({ hostname: '127.0.0.1', port: 0, async fetch(req) {
      const path = new URL(req.url).pathname;
      const file = Bun.file(resolve(dist, path === '/' ? 'index.html' : path.slice(1)));
      return await file.exists() ? new Response(file) : new Response('not found', { status: 404 });
    } });
    s = await open({ host: 'web', url: page.url.href, webDist: dist });
    expectGlide(await driveGlide(s), expect);
  } finally {
    await s?.close?.();
    page?.stop(true);
    server?.stop();
  }
}, 180_000);
