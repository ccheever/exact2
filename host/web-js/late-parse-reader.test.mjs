// A reply's parse that read the store makes a store reader, though its answer is refused (LLP 1146 §2.3): a Rust
// source's parse says it read (`store`), its value falls outside the resource's shape, and the refused commit takes
// back the mark the parse made; a store write outside a commit must still ask the resource again.
import { test, expect } from 'bun:test';
import { existsSync } from 'node:fs';
import { suiteBrowser } from '../../scripts/agent-launch.mjs';

test("a parse's store read stands when its answer is refused", async () => {
  const server = Bun.serve({ port: 0, fetch(request) {
    const path = new URL(request.url).pathname;
    if (path === '/favicon.ico') return new Response(null, { status: 204 });
    if (path.endsWith('/names.js')) return new Response('export const sourceTypes = {};', { headers: { 'content-type': 'text/javascript' } });
    if (path === '/') return new Response('<div id="exact-root"></div>', { headers: { 'content-type': 'text/html' } });
    const local = new URL('../..' + path, import.meta.url), file = existsSync(local) ? local : new URL('../web/' + path.split('/').pop(), import.meta.url);
    return new Response(Bun.file(file));
  } });
  const browser = await suiteBrowser(), page = await browser.newPage();
  try {
    await page.goto(server.url.href);
    const asks = await page.evaluate(async () => {
      const rt = await import('/host/web-js/rt.js');
      rt.clock.agent = true;
      let asks = 0;
      rt.data.answer = () => (++asks === 1 ? { req: { method: 'GET', url: 'https://api.example/me', headers: [], body: '' } } : { v: 'asked again' });
      rt.data.fetch = async () => ({ status: 200, headers: [], body: '' });
      rt.data.parse = () => ({ v: 42, store: true });
      const reader = rt.res('reader', 'reader', () => [], undefined, undefined, 's', 'none');
      rt.commit(() => { reader(); });
      const settled = () => new Promise(resolve => { const check = () => (rt.inflight.n === 0 ? resolve() : setTimeout(check, 0)); check(); });
      await settled();
      const refused = asks;
      rt.Store.set('k', 'written');
      await new Promise(resolve => setTimeout(resolve, 0));
      await settled();
      return [refused, asks, reader.r.store];
    });
    expect(asks).toEqual([1, 2, true]);
  } finally {
    await page.close();
    server.stop(true);
  }
}, 120_000);
