// Pages rendered by the JavaScript runtime itself, under Bun (`--render js`,
// LLP 1071): the app's generated module runs against a small DOM
// (`dom.js`), in a fresh VM context per render, with the app's own data
// source (TypeScript, or the Rust module's wasm), until nothing is in flight
// or the deadline passes. Its document, head and checkpoint are composed
// over the built shell as the Rust render host composes them
// (`host/render/src/page.rs` `page_js`), so the page adopts the same way.
//
//   bun host/web-js/render.mjs <dist> --build                      pages for render=build routes
//   bun host/web-js/render.mjs <dist> <location>…                  print pages
//   bun host/web-js/render.mjs <dist> --serve [--port 8830]        a page per request
import { createHash } from 'node:crypto';
import { existsSync, readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { resolve, dirname, extname } from 'node:path';
import vm from 'node:vm';
import { brotliCompressSync, createBrotliCompress, constants } from 'node:zlib';
import { createDocument } from './dom.js';

const DEADLINE = 3000;
const esc = s => String(s).replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]);

/** The renderer for one built dist: its server bundle compiled once. */
export function renderer(dist) {
  const script = new vm.Script(readFileSync(resolve(dist, '.gen/server.js'), 'utf8'), { filename: 'server.js' });
  const shell = readFileSync(resolve(dist, existsSync(resolve(dist, 'shell.html')) ? 'shell.html' : 'index.html'), 'utf8');
  const capture = readFileSync(new URL('./capture.js', import.meta.url), 'utf8').trim();
  // A dist file once; a wasm compiled once, instantiated per render.
  const cache = new Map();
  const files = p => { let v = cache.get(p); if (!v) { v = readFileSync(resolve(dist, p.replace(/^\.?\//, ''))); if (p.endsWith('.wasm')) v = new WebAssembly.Module(v); cache.set(p, v); } return v; };
  const name = /<title>([^<]*)<\/title>/.exec(shell)?.[1] ?? '';
  // The shell's places, as page.rs `head_js`/`body_js` compose them: what
  // comes before anything the render decides (the head a server flushes as
  // a request arrives), then the head's fields, the document and its
  // checkpoint.
  const ROOT = '<div id="exact-root"></div>', ENTRY = '<script type="module" src="./app.js"></script>';
  const title = shell.indexOf('<title>'), stop = shell.indexOf('>\n', shell.indexOf('<meta name="viewport"', title)) + 2;
  const root = shell.indexOf(ROOT), entry = shell.indexOf(ENTRY);
  const viewport = esc(/<meta name="viewport" content="([^"]*)"/.exec(shell)?.[1] ?? 'width=device-width, initial-scale=1');
  const headOf = preload => shell.slice(0, title).replace(/<html[^>]*>/, '<html lang="en" dir="ltr">') + `<script>${capture}</script>\n`
    + (preload ? shell.slice(stop, root) : shell.slice(stop, root).replace(/<link rel="modulepreload" href="[^"]*">\n/g, ''));
  /** A render begun: its route's policy and the page's head, known before
   * any data is asked; `finish` settles it and composes the rest. */
  async function begin(location) {
    const document = createDocument(shell);
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
    const info = await ctx.__start();
    // The capture script imports the entry by the policy; an interaction
    // page drops the head's modulepreloads (page.rs `head_js`).
    const head = headOf(info.activate !== 'interaction');
    return { ...info, location, head, async finish(deadline = DEADLINE) {
      const out = await ctx.__render(deadline);
      const fields = `<title>${esc(out.title || name)}</title><meta name="viewport" content="${viewport}">${out.description ? `<meta name="description" content="${esc(out.description)}">` : ''}`;
      const checkpoint = `{"location":${JSON.stringify(url.pathname + url.search)},"time":${JSON.stringify(out.time)},"logic":null,"answers":${out.answers},"pending":${JSON.stringify(out.pending)}}`;
      const digest = createHash('sha256').update(out.root).digest('hex').slice(0, 16);
      const rest = `${fields}\n<div id="exact-root">${out.root}</div>${shell.slice(root + ROOT.length, entry)}`
        + `<script type="application/vnd.exact.checkpoint" data-digest="${digest}" data-activate="${info.activate}">${checkpoint.replace(/</g, '\\u003c')}</script>${shell.slice(entry + ENTRY.length)}`;
      return { html: head + rest, rest, status: info.notfound ? 404 : 200, settled: !out.pending.length, render: out.render, location, policy: info.policy };
    } };
  }
  const render = async (location, deadline = DEADLINE) => (await begin(location)).finish(deadline);
  render.begin = begin;
  return render;
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
      const begun = await render.begin(key);
      const keep = page => {
        if (page.policy === 'cached' && page.status === 200) { if (cache.size >= 64) cache.delete(cache.keys().next().value); cache.set(key, { html: page.html, status: page.status, until: Date.now() + 60000 }); }
        console.log(`render ${page.location} ${page.status} ${(performance.now() - t).toFixed(1)}ms bytes=${page.html.length}${page.flushed ? ' flushed' : ''}`);
      };
      // A browser's navigation gets the head now and the rest after the
      // render, as the Rust render server sends it (host/render/src/stream.rs):
      // a 200, `private, no-cache`; the not-found page waits for its 404.
      if (req.headers.get('sec-fetch-dest') === 'document' && !begun.notfound) {
        const br = /\bbr\b/.test(req.headers.get('accept-encoding') ?? '');
        let z;
        const body = new ReadableStream({ async start(out) {
          z = br && createBrotliCompress({ params: { [constants.BROTLI_PARAM_QUALITY]: 5 } });
          // A reader that went away closes the stream under us: its bytes go nowhere.
          if (z) z.on('data', d => { try { out.enqueue(d); } catch {} });
          const put = text => new Promise(ok => { if (!z) { out.enqueue(new TextEncoder().encode(text)); ok(); } else { z.write(text); z.flush(constants.BROTLI_OPERATION_FLUSH, ok); } });
          await put(begun.head);
          const page = await begun.finish().catch(e => ({ error: e }));
          await put(page.error ? "<title>Unavailable</title>\n<p>This page couldn't be rendered.</p>\n" : page.rest);
          if (z) await new Promise(ok => { z.on('end', ok); z.end(); });
          try { out.close(); } catch {}
          if (page.error) console.log(`render ${key} 500 ${page.error.message}`); else keep({ ...page, flushed: true });
        }, cancel() { z?.destroy?.(); } });
        const h = { 'content-type': 'text/html; charset=utf-8', 'cache-control': begun.policy === 'request' ? 'no-store' : 'private, no-cache', vary: 'Accept-Encoding' };
        if (br) h['content-encoding'] = 'br';
        return new Response(body, { status: 200, headers: h });
      }
      const page = await begun.finish();
      keep(page);
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
