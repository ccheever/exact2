// Pages rendered by the JavaScript runtime itself, under Bun (`--render js`,
// LLP 1071): the app's generated module runs against a small DOM
// (`dom.js`), in a fresh VM context per render, with the app's own data
// source (TypeScript, or the Rust module's wasm), until nothing is in flight
// or the deadline passes. Its fetches share an HTTP cache (`http-cache.mjs`). Its document, head and checkpoint are composed
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
import { cachingFetch, FRESH } from './http-cache.mjs';
import { canonical, matchPattern } from './route-pattern.js';
import { select, tokens } from './page-style.mjs';

const DEADLINE = 3000;
const esc = s => String(s).replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]);
// Use Place's default zone inside each realm, keeping Intl prototypes and exceptions local to that render.
// A render that formats in the default zone notes it: its page then waits for no interaction (\`infer\`).
const utcIntl = new vm.Script(`{
  const argsInUTC = ([locales, options]) => [locales, options === null || options?.timeZone !== undefined ? options : (globalThis.__exactZone = true, Object.create(Object(options), { timeZone: { value: 'UTC' } }))];
  Intl.DateTimeFormat = new Proxy(Intl.DateTimeFormat, {
    apply: (target, receiver, args) => Reflect.apply(target, receiver, argsInUTC(args)),
    construct: (target, args, derived) => Reflect.construct(target, argsInUTC(args), derived),
  });
}`);

/** An undeclared route's activation (LLP 1048.007 D1), as the render host
 * infers it (`exact_runner::activation`, whose rules the build wrote in
 * `out.activation`): `interaction` when the render left the runtime nothing
 * to do before the reader acts — nothing in the plan only the runtime
 * keeps, no answer pending, nothing due on the clock, no handler the
 * capture script can neither replay nor wake for — else `eager`. A render that formatted in
 * the default zone does not wait either. The page carries the plan's
 * viewport comparisons as it was laid out by them, which the capture
 * script checks (`wake`). */
function infer(out) {
  const { waits: kinds, kept } = out.activation;
  const on = [...out.root.matchAll(/ data-exact-on="([^"]*)"/g)].flatMap(m => m[1].split(' '));
  const waits = !kept && !out.pending.length && !out.due && !out.zoned && on.every(k => kinds.includes(k));
  return waits ? 'interaction' : 'eager';
}

/** The renderer for one built dist: its server bundle compiled once. */
export function renderer(dist) {
  const script = new vm.Script(readFileSync(resolve(dist, '.gen/server.js'), 'utf8'), { filename: 'server.js' });
  const shell = readFileSync(resolve(dist, existsSync(resolve(dist, 'shell.html')) ? 'shell.html' : 'index.html'), 'utf8');
  const capture = readFileSync(new URL('./capture.js', import.meta.url), 'utf8').trim();
  // A dist file once; a wasm compiled once, instantiated per render.
  const cache = new Map();
  const files = p => { let v = cache.get(p); if (!v) { v = readFileSync(resolve(dist, p.replace(/^\.?\//, ''))); if (p.endsWith('.wasm')) v = new WebAssembly.Module(v); cache.set(p, v); } return v; };
  // Every render's fetches share one HTTP cache, for as long as each answer's headers allow (http-cache.mjs).
  const fetched = cachingFetch(fetch);
  const name = /<title>([^<]*)<\/title>/.exec(shell)?.[1] ?? '';
  // The shell's places, as page.rs `head_js`/`body_js` compose them: what
  // comes before anything the render decides (the head a server flushes as
  // a request arrives), then the head's fields, the document and its
  // checkpoint.
  // The root carries what the build wrote on it: `data-audio-session` (LLP 1096 D7).
  const ROOT = /<div id="exact-root"( data-audio-session="[a-z-]+")?><\/div>/.exec(shell), ENTRY = '<script type="module" src="./app.js"></script>';
  const title = shell.indexOf('<title>'), stop = shell.indexOf('>\n', shell.indexOf('<meta name="viewport"', title)) + 2;
  const root = ROOT?.index ?? -1, entry = shell.indexOf(ENTRY), rootAttrs = ROOT?.[1] ?? '';
  const viewport = esc(/<meta name="viewport" content="([^"]*)"/.exec(shell)?.[1] ?? 'width=device-width, initial-scale=1');
  // The document's language is the shell's, which the build wrote from the manifest's `lang` (LLP 1115.000 D4.6).
  const language = /<html[^>]* lang="([^"]*)"/.exec(shell)?.[1] || 'en-US';
  // The shell's sheet as rules, when the build analyzed it: a page then carries only those its document can match, after
  // its render, and the head sent before the render carries none (page-style.mjs, LLP 1048.007 D4).
  const rules = existsSync(resolve(dist, '.gen/page-style.json')) ? JSON.parse(readFileSync(resolve(dist, '.gen/page-style.json'), 'utf8')) : null;
  const sheet = rules ? /<style>[\s\S]*?<\/style>\n?/ : /(?!)/;
  // A rendered page paints without the runtime, which only adopts it, so its preloads are low priority (page.rs `push_low`).
  const headOf = preload => shell.slice(0, title).replace(/<html[^>]*>/, `<html lang="${language}" dir="ltr">`) + `<script>${capture}</script>\n`
    + shell.slice(stop, root).replace(sheet, '').replace(/(<link rel="modulepreload" href="[^"]*")>\n/g, preload ? '$1 fetchpriority="low">\n' : '');
  const preloads = (shell.slice(stop, root).match(/<link rel="modulepreload" href="[^"]*">\n/g) ?? []).map(l => l.replace('>', ' fetchpriority="low">')).join('');
  // A realm for one render: a document, a context with the server bundle run in it, and its location, which `begin`
  // fills in. Nothing has run in it but the bundle's own loading, so making it ahead takes that off a request's path
  // and every render still has a realm of its own (LLP 1048.000 D10, as the render host's `make_realm`).
  const realm = () => {
    const document = createDocument(shell);
    // The bake's page facts (runner/src/page.rs): visible, online, focused, no share sheet or pickers. The
    // base is the shell's `<base href="/">`, against which the runtime checks that a link navigates (rt.js `navigable`).
    Object.assign(document, { visibilityState: 'visible', hasFocus: () => true, baseURI: 'http://render.invalid/' });
    const location = { pathname: '/', search: '', href: 'http://render.invalid/', origin: '' };
    // What the render read that another render may not read alike: how long its answers stay fresh for any reader
    // (http-cache.mjs `FRESH`), and whether a source drew a random number (its only other reading the clock and
    // `Math.random` refuse, ts-fetch.js) — so how long its page may be given again (`keep`).
    const read = { fresh: Infinity, random: false };
    const noted = response => { read.fresh = Math.min(read.fresh, response?.[FRESH] ?? 0); return response; };
    const drew = f => (...args) => { read.random = true; return f(...args); };
    const ctx = vm.createContext({
      document, location,
      navigator: { onLine: true, language },
      getComputedStyle: () => ({ fontSize: '16px' }), devicePixelRatio: 1,
      history: { replaceState() {}, pushState() {}, go() {} }, localStorage: { length: 0, key() {}, getItem() { return null; } },
      addEventListener() {}, removeEventListener() {}, requestAnimationFrame: () => 0,
      // No reader's device facts: use runner/src/viewport.rs's default size and media bits.
      innerWidth: 390, innerHeight: 844,
      matchMedia: media => ({ media, matches: false, addEventListener() {}, removeEventListener() {}, addListener() {}, removeListener() {} }),
      setTimeout, clearTimeout, queueMicrotask, performance, console, Request, Response, Headers, URL, URLSearchParams, TextEncoder, TextDecoder,
      fetch: (input, init) => fetched(input, init).then(noted, error => { read.fresh = 0; throw error; }),
      crypto: { subtle: crypto.subtle, getRandomValues: drew(a => crypto.getRandomValues(a)), randomUUID: drew(() => crypto.randomUUID()) },
      WebAssembly, atob, btoa, Event: class {}, CustomEvent: class {}, __exactRender: true, __files: files,
    });
    ctx.globalThis = ctx; ctx.self = ctx;
    utcIntl.runInContext(ctx);
    script.runInContext(ctx);
    // What loading the bundle read is every render's alike.
    Object.assign(read, { fresh: Infinity, random: false });
    return { ctx, location, read };
  };
  // The next render's realm, made once the last render is done and nothing waits (`render.ahead` false turns it off).
  let ahead = null, making = false;
  const makeAhead = () => {
    if (ahead || making || render.ahead === false) return;
    making = true;
    setImmediate(() => { making = false; if (!ahead && render.ahead !== false) ahead = realm(); });
  };
  /** A render begun: its route's policy and the page's head, known before
   * any data is asked; `finish` settles it and composes the rest. */
  async function begin(location) {
    const url = new URL(location, 'http://render.invalid');
    const made = ahead ?? realm();
    ahead = null;
    const { ctx, read } = made;
    Object.assign(made.location, { pathname: url.pathname, search: url.search, href: url.href });
    const info = await ctx.__start();
    // The capture script imports the entry by the policy; an interaction
    // page drops the head's modulepreloads, and an undeclared one adds them
    // after its head once its render chose `eager` (page.rs `head_js`).
    const head = headOf(info.activate !== 'interaction' && info.activate !== 'inferred');
    return { ...info, location, head, async finish(deadline = DEADLINE) {
      const out = await ctx.__render(deadline);
      const activate = info.activate === 'inferred' ? infer(out) : info.activate;
      const late = info.activate === 'inferred' && activate !== 'interaction' ? preloads : '';
      const wake = activate === 'interaction' && out.wake.length ? ` data-wake="${esc(JSON.stringify(out.wake))}"` : '';
      const fields = `<title>${esc(out.title || name)}</title><meta name="viewport" content="${viewport}">${out.description ? `<meta name="description" content="${esc(out.description)}">` : ''}`;
      const checkpoint = `{"location":${JSON.stringify(url.pathname + url.search)},"time":${JSON.stringify(out.time)},"logic":null,"answers":${out.answers},"pending":${JSON.stringify(out.pending)}}`;
      const digest = createHash('sha256').update(out.root).digest('hex').slice(0, 16);
      const style = rules ? `<style data-exact-page>${select(rules, tokens(out.root + shell.slice(root)))}</style>\n` : '';
      const body = late => `${fields}\n${late}${style}<div id="exact-root"${rootAttrs}>${out.root}</div>${shell.slice(root + ROOT[0].length, entry)}`
        + `<script type="application/vnd.exact.checkpoint" data-digest="${digest}" data-activate="${activate}"${wake}>${checkpoint.replace(/</g, '\\u003c')}</script>${shell.slice(entry + ENTRY.length)}`;
      // A complete page marks html and names the preloads in its head; a streamed head has gone already, so the rest
      // marks it before the body and names them after the head's fields, as page.rs does.
      const whole = late ? headOf(true) : head;
      const markedHead = out.scrollDocument ? whole.replace('<html ', '<html data-scrolldocument ') : whole;
      const mark = out.scrollDocument ? '<script>document.documentElement.setAttribute("data-scrolldocument","")</script>\n' : '';
      makeAhead();
      // A page made only of answers any reader may still be given (one at least), by a render that drew nothing at
      // random and read no clock or reader's device (the plan's `kept`), from TypeScript sources alone, is the page
      // another render would make until the first of those answers goes stale: it may be given again until then, for
      // at most 60 s (LLP 1048.007 D8).
      const keep = !info.notfound && !out.pending.length && out.activation?.kept === false && !out.rust && !read.random && read.fresh > 0 && read.fresh < Infinity ? Math.min(read.fresh, 60000) : 0;
      return { html: markedHead + body(''), rest: mark + body(late), status: info.notfound ? 404 : 200, settled: !out.pending.length, render: out.render, location, policy: info.policy, activate, keep };
    } };
  }
  const render = async (location, deadline = DEADLINE) => (await begin(location)).finish(deadline);
  render.begin = begin;
  return render;
}

const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.json': 'application/json', '.png': 'image/png', '.svg': 'image/svg+xml', '.css': 'text/css', '.mp4': 'video/mp4', '.plan': 'application/octet-stream' };

/** A page per request for one built dist, as the Rust render server sends it (host/render/src/serve.rs, stream.rs):
 * `page(location, header)`, where `header(name)` reads a request header, answers `{ status, headers, body }`, the body
 * a string, bytes, or a ReadableStream of bytes. A `cached` route's page is kept at the origin for its public lifetime
 * (60 s, 64 locations); brotli (quality 5) when the reader takes it; a browser's navigation gets the head at once and
 * the rest after the render. `render.mjs --serve` and `host/web/serve.mjs` (`serve`, LLP 1153 D3.4) answer with it. */
export function pageServer(dist, render = renderer(dist), log = console.log) {
  const cache = new Map();
  const brotli = header => /\bbr\b/.test(header('accept-encoding') ?? '');
  const send = (header, html, status, cacheControl) => {
    const headers = { 'content-type': 'text/html; charset=utf-8', 'cache-control': cacheControl, vary: 'Accept-Encoding' };
    if (!brotli(header)) return { status, headers, body: html };
    headers['content-encoding'] = 'br';
    return { status, headers, body: brotliCompressSync(html, { params: { [constants.BROTLI_PARAM_QUALITY]: 5 } }) };
  };
  return async function page(key, header) {
    const t = performance.now(), hit = cache.get(key);
    const revalidate = /no-cache|no-store|max-age=0/.test(header('cache-control') ?? '');
    if (hit && hit.until > Date.now() && !revalidate) return send(header, hit.html, hit.status, hit.cacheControl);
    const begun = await render.begin(key);
    const keep = page => {
      // A `cached` route's page is kept 60 s; a `request` route's, as long as what it was made of (`keep`).
      const ms = page.status !== 200 ? 0 : page.policy === 'cached' ? 60000 : page.policy === 'request' ? page.keep : 0;
      if (ms > 0) { if (cache.size >= 64) cache.delete(cache.keys().next().value); cache.set(key, { html: page.html, status: page.status, until: Date.now() + ms, cacheControl: page.policy === 'cached' ? 'public, max-age=0, s-maxage=60' : 'no-store' }); }
      log(`render ${page.location} ${page.status} ${(performance.now() - t).toFixed(1)}ms bytes=${page.html.length}${page.flushed ? ' flushed' : ''}`);
    };
    // A browser's navigation gets the head now and the rest after the
    // render (host/render/src/stream.rs): a 200, `private, no-cache`; the
    // not-found page waits for its 404.
    if (header('sec-fetch-dest') === 'document' && !begun.notfound) {
      const br = brotli(header);
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
        if (page.error) log(`render ${key} 500 ${page.error.message}`); else keep({ ...page, flushed: true });
      }, cancel() { z?.destroy?.(); } });
      const headers = { 'content-type': 'text/html; charset=utf-8', 'cache-control': begun.policy === 'request' ? 'no-store' : 'private, no-cache', vary: 'Accept-Encoding' };
      if (br) headers['content-encoding'] = 'br';
      return { status: 200, headers, body };
    }
    const page = await begun.finish();
    keep(page);
    return send(header, page.html, page.status, page.policy === 'cached' ? 'public, max-age=0, s-maxage=60' : 'no-store');
  };
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  const dist = resolve(args[0]);
  const render = renderer(dist);
  const opt = (n, d) => { const i = args.indexOf(n); return i < 0 ? d : args[i + 1]; };
  if (args.includes('--serve')) {
    const port = Number(opt('--port', 8830));
    const page = pageServer(dist, render), files = new Map();
    const routes = existsSync(resolve(dist, 'routes.json')) ? JSON.parse(readFileSync(resolve(dist, 'routes.json'), 'utf8')).routes : [];
    Bun.serve({ port, hostname: '127.0.0.1', async fetch(req) {
      const url = new URL(req.url);
      const file = resolve(dist, '.' + decodeURIComponent(url.pathname));
      if (file.startsWith(dist + '/') && !file.includes('/.gen/') && existsSync(file) && extname(file)) {
        const type = TYPES[extname(file)] ?? 'application/octet-stream';
        // A dist file compressed once, at quality 11.
        if (/\bbr\b/.test(req.headers.get('accept-encoding') ?? '') && /html|javascript|css|json|wasm|svg/.test(type)) {
          let br = files.get(file); if (!br) files.set(file, br = brotliCompressSync(readFileSync(file), { params: { [constants.BROTLI_PARAM_QUALITY]: 11 } }));
          return new Response(br, { headers: { 'content-type': type, 'content-encoding': 'br', 'cache-control': 'no-cache', vary: 'Accept-Encoding' } });
        }
        return new Response(Bun.file(file), { headers: { 'content-type': type, 'cache-control': 'no-cache' } });
      }
      // A route drawn in the browser (`render=client`) gets the shell and one rendered at build its page, as
      // `serve` gives them (LLP 1153.001 D1); the rest are rendered here.
      const at = matchPattern(routes, canonical(url.pathname)), built = resolve(file, 'index.html');
      const html = { 'content-type': 'text/html', 'cache-control': 'no-cache' };
      if (at >= 0 && routes[at].render === 'build' && built.startsWith(dist + '/') && existsSync(built)) return new Response(Bun.file(built), { headers: html });
      if (at >= 0 && routes[at].render === 'client') return new Response(Bun.file(resolve(dist, 'shell.html')), { headers: html });
      const { status, headers, body } = await page(url.pathname + url.search, name => req.headers.get(name));
      return new Response(body, { status, headers });
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
