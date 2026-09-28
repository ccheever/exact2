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
import { brotliCompressSync, constants } from 'node:zlib';
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
    return { html, status: out.notfound ? 404 : 200, settled: !out.pending.length, render: out.render, location, policy: out.policy };
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
    // A `cached` route's page is kept at the origin for its public lifetime
    // (60 s, 64 locations), as the Rust render server keeps it (serve.rs).
    const cache = new Map(), files = new Map();
    // Brotli when the browser takes it, as the Rust render server sends: a
    // page as it is sent (quality 5), a dist file once (quality 11).
    const send = (req, body, type, status, cacheControl, q = 5) => {
      const h = { 'content-type': type, 'cache-control': cacheControl, vary: 'Accept-Encoding' };
      if (/\bbr\b/.test(req.headers.get('accept-encoding') ?? '') && /html|javascript|css|json|wasm|svg/.test(type)) { body = brotliCompressSync(body, { params: { [constants.BROTLI_PARAM_QUALITY]: q } }); h['content-encoding'] = 'br'; }
      return new Response(body, { status, headers: h });
    };
    Bun.serve({ port, hostname: '127.0.0.1', async fetch(req) {
      const url = new URL(req.url);
      const file = resolve(dist, '.' + decodeURIComponent(url.pathname));
      if (file.startsWith(dist + '/') && !file.includes('/.gen/') && existsSync(file) && extname(file)) {
        const type = TYPES[extname(file)] ?? 'application/octet-stream';
        if (/\bbr\b/.test(req.headers.get('accept-encoding') ?? '') && /html|javascript|css|json|wasm|svg/.test(type)) {
          let br = files.get(file); if (!br) files.set(file, br = brotliCompressSync(readFileSync(file), { params: { [constants.BROTLI_PARAM_QUALITY]: 11 } }));
          return new Response(br, { headers: { 'content-type': type, 'content-encoding': 'br', 'cache-control': 'no-cache', vary: 'Accept-Encoding' } });
        }
        return new Response(Bun.file(file), { headers: { 'content-type': type, 'cache-control': 'no-cache' } });
      }
      const t = performance.now();
      const key = url.pathname + url.search, hit = cache.get(key);
      const revalidate = /no-cache|no-store|max-age=0/.test(req.headers.get('cache-control') ?? '');
      if (hit && hit.until > Date.now() && !revalidate) return send(req, hit.html, 'text/html; charset=utf-8', hit.status, 'public, max-age=0, s-maxage=60');
      const page = await render(key);
      if (page.policy === 'cached' && page.status === 200) { if (cache.size >= 64) cache.delete(cache.keys().next().value); cache.set(key, { html: page.html, status: page.status, until: Date.now() + 60000 }); }
      console.log(`render ${page.location} ${page.status} ${(performance.now() - t).toFixed(1)}ms bytes=${page.html.length}`);
      return send(req, page.html, 'text/html; charset=utf-8', page.status, page.policy === 'cached' ? 'public, max-age=0, s-maxage=60' : 'no-store');
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
