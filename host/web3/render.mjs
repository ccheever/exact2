// Pages rendered by the JavaScript runtime itself, under Bun (`--render js`,
// LLP 1071): the app's generated module runs against a small DOM
// (`dom.js`), in a fresh VM context per render, with the app's own data
// source (TypeScript, or the Rust module's wasm), until nothing is in flight
// or the deadline passes. Its document, head and checkpoint are composed
// over the built shell as the Rust render host composes them
// (`host/render/src/page.rs` `page_js`), so the page adopts the same way.
//
//   bun host/web3/render.mjs <dist> --build                      pages for render=build routes
//   bun host/web3/render.mjs <dist> <location>…                  print pages
//   bun host/web3/render.mjs <dist> --serve [--port 8830]        a page per request
import { createHash } from 'node:crypto';
import { existsSync, readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { resolve, dirname, extname } from 'node:path';
import vm from 'node:vm';
import { createDocument } from './dom.js';

const DEADLINE = 3000;
const esc = s => String(s).replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]);

/** The renderer for one built dist: its server bundle compiled once. */
export function renderer(dist) {
  const script = new vm.Script(readFileSync(resolve(dist, '.gen/server.js'), 'utf8'), { filename: 'server.js' });
  const shell = readFileSync(resolve(dist, existsSync(resolve(dist, 'shell.html')) ? 'shell.html' : 'index.html'), 'utf8');
  const capture = readFileSync(new URL('./capture.js', import.meta.url), 'utf8').trim();
  const files = p => readFileSync(resolve(dist, p.replace(/^\.?\//, '')));
  const name = /<title>([^<]*)<\/title>/.exec(shell)?.[1] ?? '';
  return async function render(location, deadline = DEADLINE) {
    const document = createDocument();
    const url = new URL(location, 'http://render.invalid');
    const ctx = vm.createContext({
      document, location: { pathname: url.pathname, search: url.search, href: url.href, origin: '' },
      history: { replaceState() {}, pushState() {}, go() {} }, localStorage: { length: 0, key() {}, getItem() { return null; } },
      addEventListener() {}, removeEventListener() {}, requestAnimationFrame: () => 0,
      setTimeout, clearTimeout, queueMicrotask, performance, console, fetch, URL, URLSearchParams, TextEncoder, TextDecoder,
      WebAssembly, atob, btoa, Event: class {}, CustomEvent: class {}, crypto, __exactRender: true, __files: files,
    });
    ctx.globalThis = ctx; ctx.self = ctx;
    script.runInContext(ctx);
    const out = await ctx.__render(deadline);
    const head = `<title>${esc(out.title || name)}</title><meta name="viewport" content="${esc(/<meta name="viewport" content="([^"]*)"/.exec(shell)?.[1] ?? 'width=device-width, initial-scale=1')}">${out.description ? `<meta name="description" content="${esc(out.description)}">` : ''}`;
    const interaction = out.activate === 'interaction';
    const checkpoint = JSON.stringify({ location: url.pathname + url.search, time: out.time, logic: null, answers: out.answers, pending: out.pending });
    const digest = createHash('sha256').update(out.root).digest('hex').slice(0, 16);
    let html = shell.replace(/<html[^>]*>/, `<html lang="en" dir="ltr">`)
      .replace(/<title>[\s\S]*?<meta name="viewport"[^>]*>\n/, () => `${head}\n<script>${capture}</script>\n`)
      .replace('<div id="exact-root"></div>', () => `<div id="exact-root">${out.root}</div>`)
      .replace('<script type="module" src="./app.js"></script>', () => `<script type="application/vnd.exact.checkpoint" data-digest="${digest}" data-activate="${out.activate}">${checkpoint.replace(/</g, '\\u003c')}</script>\n${interaction ? '' : '<script type="module" src="./app.js"></script>'}`);
    return { html, status: out.notfound ? 404 : 200, settled: !out.pending.length, render: out.render, location };
  };
}

const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.json': 'application/json', '.png': 'image/png', '.svg': 'image/svg+xml', '.css': 'text/css', '.mp4': 'video/mp4', '.plan': 'application/octet-stream' };

if (import.meta.main) {
  const args = process.argv.slice(2);
  const dist = resolve(args[0]);
  const render = renderer(dist);
  const opt = (n, d) => { const i = args.indexOf(n); return i < 0 ? d : args[i + 1]; };
  if (args.includes('--serve')) {
    const port = Number(opt('--port', 8830));
    Bun.serve({ port, hostname: '127.0.0.1', async fetch(req) {
      const url = new URL(req.url);
      const file = resolve(dist, '.' + decodeURIComponent(url.pathname));
      if (file.startsWith(dist + '/') && !file.includes('/.gen/') && existsSync(file) && extname(file)) return new Response(Bun.file(file), { headers: { 'content-type': TYPES[extname(file)] ?? 'application/octet-stream', 'cache-control': 'no-store' } });
      const t = performance.now();
      const page = await render(url.pathname + url.search);
      console.log(`${page.location} ${page.status} ${(performance.now() - t).toFixed(1)} ms ${page.html.length} B`);
      return new Response(page.html, { status: page.status, headers: { 'content-type': 'text/html; charset=utf-8', 'cache-control': 'no-store' } });
    } });
    console.log(`serving ${dist} with pages rendered by the JavaScript runtime on ${port}`);
  } else if (args.includes('--build')) {
    const pages = JSON.parse(readFileSync(resolve(dist, '.gen/pages.json'), 'utf8'));
    for (const p of pages) {
      const page = await render(p.location);
      const file = p.notfound ? '404.html' : `${p.location.replace(/^\/|\/$/g, '')}/index.html`.replace(/^\//, '');
      mkdirSync(dirname(resolve(dist, file)), { recursive: true });
      writeFileSync(resolve(dist, file), page.html);
      console.log(`${p.location} → ${file} (${page.html.length} B${page.settled ? '' : ', at the deadline'})`);
    }
  } else for (const location of args.slice(1)) process.stdout.write((await render(location)).html);
}
