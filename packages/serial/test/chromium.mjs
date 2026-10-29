// The three device packages' web conformance harness (LLP 1133.037 §16 stage 1): a page in Chromium that serves a
// package's directory, installs the suite's fake `navigator.serial`, `.hid` or `.usb` before anything loads, and
// imports the web implementation (`web/index.js`) as the page runtime would, with a `changed` that records each
// topic. The fakes implement the specifications' interfaces over the suite's own streams and records, so a suite
// proves the implementation's reading of the API, not a browser's own (that is a person's, with a device).
import { extname, join, resolve } from 'node:path';

export const root = resolve(import.meta.dir, '../../..');

const types = { '.js': 'text/javascript', '.html': 'text/html', '.json': 'application/json' };

/** A checker that prints each case and counts failures. */
export function checker(label) {
  let failed = 0;
  const check = (what, ok, detail = '') => { if (!ok) failed++; console.log(`${ok ? 'ok  ' : 'FAIL'} ${label} ${what}${ok ? '' : `: ${detail}`}`); };
  return { check, failed: () => failed };
}

/** Open package `name`'s implementation in Chromium with `fake` (a function run in the page before load). Answers
 * `{ page, call(name, args), eval(fn, arg), close }`: `call` runs one of the implementation's names in the page. */
export async function open(name, fake) {
  const { chromium } = await import(resolve(root, 'node_modules/playwright-core/index.mjs'));
  const dir = join(root, 'packages', name);
  const server = Bun.serve({ port: 0, async fetch(req) {
    let path = new URL(req.url).pathname;
    if (path === '/') return new Response('<!doctype html><title>suite</title>', { headers: { 'content-type': 'text/html' } });
    const file = Bun.file(join(dir, path));
    if (!(await file.exists())) return new Response('not found', { status: 404 });
    return new Response(file, { headers: { 'content-type': types[extname(path)] ?? 'application/octet-stream' } });
  } });
  const browser = await chromium.launch();
  const page = await browser.newPage();
  page.on('pageerror', (e) => console.log(`page error: ${e.message}`));
  await page.addInitScript(fake);
  await page.goto(`http://127.0.0.1:${server.port}/`);
  await page.evaluate(async () => {
    const m = await import('/web/index.js');
    globalThis.__changed = [];
    globalThis.__impl = m.implementation({ changed: (topic) => __changed.push(topic), emit: () => {}, foreground: () => true });
  });
  return {
    page,
    call: (n, args = {}) => page.evaluate(async ([n, args]) => { try { return await __impl[n](args); } catch (e) { return { thrown: String(e) }; } }, [n, args]),
    eval: (fn, arg) => page.evaluate(fn, arg),
    async close() { await browser.close(); server.stop(true); },
  };
}
