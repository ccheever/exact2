import { test, expect } from 'bun:test';
import { resolve } from 'node:path';
import { suiteBrowser } from '../../../scripts/agent-launch.mjs';

async function browserTest(run) {
  const server = Bun.serve({ port: 0, hostname: '127.0.0.1', async fetch(request) {
    const path = new URL(request.url).pathname;
    if (path === '/') return new Response('<main id="exact-root"></main>', { headers: { 'content-type': 'text/html' } });
    if (path === '/names.js') return new Response('export const sourceTypes = {};', { headers: { 'content-type': 'text/javascript' } });
    if (path === '/assets/given.html' || path === '/deck/given.htm' || path === '/other/given.html') return new Response(GIVEN_PAGE, { headers: { 'content-type': 'text/html' } });
    const name = path.slice(1);
    for (const dir of ['host/web-js', 'host/web']) {
      const file = Bun.file(resolve(dir, name));
      if (await file.exists()) return new Response(file);
    }
    return new Response('missing', { status: 404 });
  } });
  const browser = await suiteBrowser('chromium', { needs: ['pdf'] });
  try {
    const page = await browser.newPage();
    await run(page, server);
  } finally {
    await browser.close();
    server.stop(true);
  }
}

test('app file frames load typed documents, report missing files, and release their URLs', () => browserTest(async (page, server) => {
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.goto(server.url.href);
    await page.evaluate(async () => {
      globalThis.rt = await import('/rt.js');
      globalThis.exact = {};
      rt.data.appId = 'app-frame-test';
      const { createFileStore } = await import('/storage-fs.js');
      const files = createFileStore(rt.data.appId);
      await files.writeFile('app:/data/document.html', new TextEncoder().encode('<h1>App document</h1>'));
      await files.writeFile('app:/data/document.pdf', new TextEncoder().encode('%PDF-1.4\n%%EOF'));
      globalThis.__exactRender = true;
      const rendered = rt.h(document.createElement('div'), 'iframe', 0, {}, 0);
      rt.P(rendered, 'src', () => 'app:/data/document.pdf');
      globalThis.prerender = [rendered.getAttribute('data-app-src'), rendered.getAttribute('src'), rt.inflight.n];
      globalThis.__exactRender = false;
      globalThis.events = [];
      globalThis.scope = rt.scope(() => {
        globalThis.frame = rt.h(document.querySelector('main'), 'iframe', 0, { 'data-app-src': 'app:/data/document.html', sandbox: 'allow-same-origin' }, 0);
        rt.on(frame, 'load', () => events.push('load'));
        rt.on(frame, 'error', message => events.push(message));
      });
    });
    expect(await page.evaluate(() => prerender)).toEqual(['app:/data/document.pdf', null, 0]);
    await page.waitForFunction(() => frame.contentDocument?.querySelector('h1')?.textContent === 'App document', { }, { timeout: 5000 });
    expect(await page.evaluate(() => events)).toEqual(['load']);
    const url = await page.evaluate(() => frame.src);
    expect(url).toStartWith('blob:');
    expect(await page.evaluate(() => frame.getAttribute('sandbox'))).toBe('allow-same-origin');
    await page.evaluate(() => rt.P(frame, 'src', () => 'app:/data/missing.pdf'));
    await page.waitForFunction(() => events.length === 2, {}, { timeout: 5000 });
    expect(await page.evaluate(() => events[1])).toContain('app:/data/missing.pdf');
    expect(await page.evaluate(async u => fetch(u).then(() => false, () => true), url)).toBe(true);
    for (const path of ['app:/outside/document.html', 'app:/data/../document.html']) {
      const count = await page.evaluate(() => events.length);
      await page.evaluate(path => rt.P(frame, 'src', () => path), path);
      await page.waitForFunction(n => events.length === n + 1, count, { timeout: 5000 });
      expect(await page.evaluate(() => events.at(-1))).toContain(path);
    }
    await page.evaluate(() => { frame.removeAttribute('sandbox'); rt.P(frame, 'src', () => 'app:/data/document.pdf'); });
    await page.waitForFunction(() => frame.src.startsWith('blob:'), {}, { timeout: 5000 });
    expect(await page.evaluate(async () => (await fetch(frame.src)).headers.get('content-type'))).toBe('application/pdf');
    const pdf = await page.evaluate(() => frame.src);
    await page.evaluate(() => { rt.end(scope); frame.remove(); });
    expect(await page.evaluate(async u => fetch(u).then(() => false, () => true), pdf)).toBe(true);
    expect(await page.evaluate(async () => {
      let late;
      const scope = rt.scope(() => { late = rt.h(document.querySelector('main'), 'iframe', 0, { 'data-app-src': 'app:/data/document.html' }, 0); });
      rt.end(scope); late.remove();
      while (rt.inflight.n) await new Promise(resolve => setTimeout(resolve, 50));
      return late.getAttribute('src');
    })).toBeNull();
    expect(errors).toEqual([]);
}), 90000);


test('an unchanged app source retains the guest document across record updates', () => browserTest(async (page, server) => {
  await page.goto(server.url.href);
  await page.evaluate(async () => {
    globalThis.rt = await import('/rt.js'); globalThis.exact = {};
    rt.data.appId = 'unchanged-frame';
    const { createFileStore } = await import('/storage-fs.js');
    await createFileStore(rt.data.appId).writeFile('app:/data/form.html', new TextEncoder().encode('<input>'));
    globalThis.selected = rt.sig({ path: 'app:/data/form.html', other: 0 });
    globalThis.loads = 0;
    globalThis.scope = rt.scope(() => {
      globalThis.frame = rt.h(document.querySelector('main'), 'iframe', 0, {}, 0);
      rt.P(frame, 'src', () => selected().path);
      rt.on(frame, 'load', () => loads++);
    });
  });
  await page.waitForFunction(() => loads === 1);
  const before = await page.evaluate(() => {
    frame.contentDocument.querySelector('input').value = 'keep this';
    const url = frame.src;
    rt.commit(() => rt.W(selected, { ...selected(), other: 1 }));
    return url;
  });
  await page.waitForFunction(() => rt.inflight.n === 0);
  expect(await page.evaluate(() => frame.src)).toBe(before);
  expect(await page.evaluate(() => frame.contentDocument.querySelector('input').value)).toBe('keep this');
  expect(await page.evaluate(() => loads)).toBe(1);
  await page.evaluate(() => { rt.end(scope); frame.remove(); });
  expect(await page.evaluate(async url => fetch(url).then(() => false, () => true), before)).toBe(true);
}), 90000);

test('bound sandbox changes preserve app frames after switching from data documents', () => browserTest(async (page, server) => {
  await page.goto(server.url.href);
  await page.evaluate(async () => {
    globalThis.rt = await import('/rt.js'); globalThis.exact = {};
    rt.data.appId = 'sandbox-source-switch';
    const { createFileStore } = await import('/storage-fs.js');
    await createFileStore(rt.data.appId).writeFile('app:/data/document.html', new TextEncoder().encode('<script>parent.postMessage("app loaded", "*")</script>'));
    globalThis.messages = [];
    addEventListener('message', e => { if (e.source === frame.contentWindow) messages.push([e.data, e.origin]); });
    globalThis.source = rt.sig('data:text/html,<script>parent.postMessage("data loaded", "*")</script>');
    globalThis.sandbox = rt.sig('allow-scripts');
    globalThis.scope = rt.scope(() => {
      globalThis.frame = rt.h(document.querySelector('main'), 'iframe', 0, {}, 0);
      rt.P(frame, 'sandbox', sandbox);
      rt.P(frame, 'src', source);
    });
  });
  await page.waitForFunction(() => messages.some(([text]) => text === 'data loaded'));
  expect(await page.evaluate(() => messages)).toContainEqual(['data loaded', 'null']);
  await page.evaluate(() => rt.commit(() => rt.W(source, 'app:/data/document.html')));
  await page.waitForFunction(() => messages.some(([text]) => text === 'app loaded'));
  const appURL = await page.evaluate(() => frame.src);
  expect(appURL).toStartWith('blob:');
  for (const value of ['allow-scripts allow-same-origin', null, 'allow-scripts']) {
    await page.evaluate(value => rt.commit(() => rt.W(sandbox, value)), value);
    expect(await page.evaluate(() => frame.src)).toBe(appURL);
  }
  await page.evaluate(() => rt.commit(() => { rt.W(sandbox, 'allow-scripts allow-same-origin'); rt.W(source, 'data:text/html,<script>parent.postMessage("data again", "*")</script>'); }));
  expect(await page.evaluate(() => frame.getAttribute('src'))).toBe('about:blank');
  expect(await page.evaluate(async url => fetch(url).then(() => false, () => true), appURL)).toBe(true);
  await page.evaluate(() => rt.commit(() => rt.W(sandbox, 'allow-scripts')));
  await page.waitForFunction(() => messages.some(([text]) => text === 'data again'));
  expect(await page.evaluate(() => messages)).toContainEqual(['data again', 'null']);
  await page.evaluate(() => { rt.end(scope); frame.remove(); });
}), 90000);

test('wasm iframe events before activation settle and dispatch once after activation', () => browserTest(async (page, server) => {
  const glue = await Bun.file('host/web/glue.js').text();
  const attach = glue.slice(glue.indexOf('function attach('), glue.indexOf('function viewFor('));
  await page.goto(server.url.href);
  const result = await page.evaluate(async attach => {
    // Production listener attachment, with the data executor held until both events arrive.
    const fixture = new Function(`
      const views = new Map(), retiredViews = new WeakSet(), iframeLoading = new WeakMap(), page = null;
      let inputReady = false, ready;
      const moduleReady = new Promise(r => ready = r), events = [];
      const wasm = {exact_dispatch: (...args) => args}, send = e => events.push(e), writeIn = s => s, now = () => 0;
      ${attach}
      return {views, retiredViews, iframeLoading, events, attach, activate() {inputReady = true; ready();}};
    `)();
    const frames = ['load', 'error', 'load', 'error'].map((event, id) => {
      const el = document.createElement('iframe');
      fixture.views.set(id, el); fixture.attach(el, id, ['load', 'error']);
      el.dispatchEvent(new CustomEvent(event, {detail: 'missing app file'}));
      return el;
    });
    const settled = frames.map(el => fixture.iframeLoading.get(el));
    const early = [...fixture.events];
    fixture.views.delete(2); fixture.retiredViews.add(frames[3]);
    fixture.activate(); await Promise.resolve();
    return {settled, early, events: fixture.events};
  }, attach);
  expect(result.settled).toEqual([false, false, false, false]);
  expect(result.early).toEqual([]);
  expect(result.events).toEqual([[0, 8, 0, 0], [1, 19, 'error\nmissing app file', 0]]);
}), 90000);

test('wasm holds boot-frame posts until activation and discards retired or unauthorized guests', () => browserTest(async (page, server) => {
  const glue = await Bun.file('host/web/glue.js').text();
  const listener = glue.slice(glue.indexOf('function ensureMessageListener('), glue.indexOf('visualViewport?.addEventListener', glue.indexOf('function ensureMessageListener(')));
  const attach = glue.slice(glue.indexOf('function attach('), glue.indexOf('function viewFor('));
  await page.goto(server.url.href);
  const result = await page.evaluate(async ({listener, attach}) => {
    const {commitGuestOrigin, guestMessageAuthorized} = await import('/navigation.js');
    const fixture = new Function('guestMessageAuthorized', `
      const views = new Map(), retiredViews = new WeakSet(), iframeLoading = new WeakMap(), page = null;
      const messageFrames = new Set(), messageViews = new Set();
      let inputReady = false, messageListening = false, ready;
      const moduleReady = new Promise(r => ready = r), events = [];
      const wasm = {exact_dispatch: (...args) => args}, send = e => events.push(e), writeIn = s => s, now = () => 0;
      ${listener}
      ${attach}
      return {views, retiredViews, events, attach, activate() {inputReady = true; ready();}};
    `)(guestMessageAuthorized);
    const frames = [];
    const arrived = [];
    for (let id = 0; id < 4; id++) {
      const el = document.createElement('iframe');
      el.sandbox = 'allow-scripts';
      el.src = `data:text/html,<script>parent.postMessage('first','*');parent.postMessage({second:2},'*')</script>`;
      commitGuestOrigin(el);
      fixture.views.set(id, el); fixture.attach(el, id, ['load', 'message']);
      arrived.push(new Promise(resolve => el.addEventListener('load', resolve, {once: true})));
      arrived.push(new Promise(resolve => {
        const heard = event => {
          if (event.source === el.contentWindow && event.data?.second === 2) {
            window.removeEventListener('message', heard); resolve();
          }
        };
        window.addEventListener('message', heard);
      }));
      document.querySelector('main').append(el); frames.push(el);
    }
    await Promise.all(arrived);
    const early = [...fixture.events];
    // Readiness must not revive a removed view or authorize a guest under a new origin.
    fixture.views.delete(1); fixture.retiredViews.add(frames[2]);
    frames[3].sandbox = 'allow-scripts allow-same-origin'; frames[3].removeAttribute('src');
    commitGuestOrigin(frames[3]);
    window.dispatchEvent(new MessageEvent('message', {source: frames[0].contentWindow, origin: location.origin, data: 'refused'}));
    fixture.activate(); await Promise.resolve();
    const delivered = [...fixture.events];
    window.dispatchEvent(new MessageEvent('message', {source: frames[0].contentWindow, origin: 'null', data: 'after'}));
    return {early, delivered, after: fixture.events.at(-1)};
  }, {listener, attach});
  expect(result.early).toEqual([]);
  expect(result.delivered.filter(e => e[1] === 9)).toEqual([[0, 9, 'first', 0], [0, 9, '{"second":2}', 0]]);
  expect(result.delivered.filter(e => e[1] === 8).sort((a, b) => a[0] - b[0])).toEqual([[0, 8, 0, 0], [3, 8, 0, 0]]);
  expect(result.after).toEqual([0, 9, 'after', 0]);
}), 90000);

test('opaque sandbox loads app text like Chrome, while sandboxed PDFs report error', () => browserTest(async (page, server) => {
  await page.goto(server.url.href);
  const literal = '<b>literal</b><script>window.executed = true</script>';
  const pdfObjects = ['<< /Type /Catalog /Pages 2 0 R >>', '<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
    '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents 4 0 R >>', '<< /Length 0 >>\nstream\n\nendstream'];
  let pdf = '%PDF-1.4\n'; const offsets = [0];
  for (const [i, object] of pdfObjects.entries()) { offsets.push(pdf.length); pdf += `${i + 1} 0 obj\n${object}\nendobj\n`; }
  const xref = pdf.length;
  pdf += `xref\n0 5\n0000000000 65535 f \n${offsets.slice(1).map(n => `${String(n).padStart(10, '0')} 00000 n \n`).join('')}trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF`;
  await page.evaluate(async ({literal, pdf}) => {
    globalThis.exact = {}; globalThis.rt = await import('/rt.js'); rt.data.appId = 'sandbox-frames';
    const { createFileStore } = await import('/storage-fs.js');
    const files = createFileStore(rt.data.appId);
    for (const [name, text] of [['notes.txt', literal], ['notes.html', literal], ['doc.pdf', pdf]])
      await files.writeFile(`app:/data/${name}`, new TextEncoder().encode(text));
  }, {literal, pdf});
  // The browser owns opaque origins: inspect through Playwright, never by granting same-origin. A frame's text is read
  // from its tree, without its scripts and styles: `innerText` would depend on whether the frame had been laid out yet
  const read = frame => frame.evaluate(() => {
    const body = document.body.cloneNode(true);
    for (const hidden of body.querySelectorAll('script, style')) hidden.remove();
    return {type: document.contentType, text: body.textContent, executed: window.executed === true, url: location.href};
  });
  for (const [name, type, text] of [['notes.txt', 'text/plain', literal], ['notes.html', 'text/html', literal], ['doc.pdf', 'application/pdf', pdf]]) {
    for (const sandbox of ['allow-scripts', 'allow-same-origin']) {
      await page.evaluate(async ({type, text, sandbox}) => {
        const oracle = document.createElement('iframe'); oracle.id = 'oracle'; oracle.sandbox = sandbox;
        oracle.src = URL.createObjectURL(new Blob([text], {type}));
        await new Promise(resolve => { oracle.onload = resolve; document.querySelector('main').append(oracle); });
      }, {type, text, sandbox});
      const oracle = await (await page.$('#oracle')).contentFrame();
      const observed = await read(oracle);
      if (type === 'application/pdf') expect(observed.url).toBe('chrome-error://chromewebdata/');
      else {
        expect(observed.type).toBe(type);
        expect(observed.text).toBe(type === 'text/plain' ? literal : 'literal');
        expect(observed.executed).toBe(type === 'text/html' && sandbox === 'allow-scripts');
      }
      await page.evaluate(({name, sandbox}) => {
        globalThis.events = [];
        globalThis.scope = rt.scope(() => {
          globalThis.frame = rt.h(document.querySelector('main'), 'iframe', 0, {sandbox}, 0);
          frame.id = 'app-frame';
          rt.on(frame, 'load', () => events.push('load'));
          rt.on(frame, 'error', message => events.push(message));
          rt.P(frame, 'src', () => `app:/data/${name}`);
        });
      }, {name, sandbox});
      await page.waitForFunction(() => events.length > 0);
      if (type === 'application/pdf') {
        expect(await page.evaluate(() => events)).toEqual(['iframe: app:/data/doc.pdf: sandbox blocks PDF documents']);
      } else {
        expect(await page.evaluate(() => events)).toEqual(['load']);
        const guest = await (await page.$('#app-frame')).contentFrame();
        const seen = await read(guest);
        expect({type: seen.type, text: seen.text, executed: seen.executed}).toEqual({type: observed.type, text: observed.text, executed: observed.executed});
      }
      await page.evaluate(() => { rt.end(scope); frame.remove(); const oracle = document.querySelector('#oracle'); URL.revokeObjectURL(oracle.src); oracle.remove(); });
    }
  }
}), 90000);

// A page that reports what it can read of each `file` its query names (LLP 1020.001 D7): a fetch, and an `img`.
const GIVEN_PAGE = `<!doctype html><script>(async () => {
  const out = { search: location.search, hash: location.hash, files: [] };
  for (const file of new URLSearchParams(location.search).getAll('file')) {
    const img = await new Promise(r => { const i = new Image(); i.onload = () => r('load'); i.onerror = () => r('error'); i.src = file; });
    try { const r = await fetch(file); out.files.push({ file, status: r.status, type: r.headers.get('content-type'), bytes: (await r.arrayBuffer()).byteLength, img }); }
    catch (e) { out.files.push({ file, error: String(e), img }); }
  }
  parent.postMessage(JSON.stringify(out), '*');
})()</script>`;
const DOT = Uint8Array.from(atob('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=='), c => c.charCodeAt(0));
const ODD = ['report #2.pdf', 'a&b.pdf', 'a+b.pdf', '100%.pdf', 'my file.pdf'];

async function givenSetup(page, server) {
  await page.goto(server.url.href);
  await page.evaluate(async ({ dot, odd }) => {
    globalThis.rt = await import('/rt.js'); globalThis.exact = {};
    rt.data.appId = 'given-files';
    const { createFileStore } = await import('/storage-fs.js');
    const files = createFileStore(rt.data.appId), text = s => new TextEncoder().encode(s);
    await files.writeFile('app:/cache/big.bin', new Uint8Array(20 * 1024 * 1024 + 1));
    await files.writeFile('app:/cache/dot.png', new Uint8Array(dot));
    await files.writeFile('app:/cache/empty.txt', new Uint8Array(0));
    await files.writeFile('app:/tmp/note.txt', text('a note'));
    await files.writeFile('app:/data/secret.txt', text('secret'));
    for (const name of odd) await files.writeFile(`app:/cache/${name}`, text(name));
    globalThis.reports = []; globalThis.events = [];
    addEventListener('message', e => { if (typeof e.data === 'string' && e.data.startsWith('{')) reports.push(JSON.parse(e.data)); });
  }, { dot: [...DOT], odd: ODD });
}

test('a bundled page is given the app files its query names, and only while its source lasts', () => browserTest(async (page, server) => {
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await givenSetup(page, server);
  await page.evaluate(() => {
    globalThis.source = rt.sig('assets/given.html?file=app:/cache/big.bin&file=app%3A%2Ftmp%2Fnote.txt&keep=a+b#top');
    globalThis.scope = rt.scope(() => {
      globalThis.frame = rt.h(document.querySelector('main'), 'iframe', 0, {}, 0);
      rt.P(frame, 'src', () => source());
      rt.on(frame, 'load', () => events.push('load'));
      rt.on(frame, 'error', message => events.push(message));
    });
  });
  const next = async (src, sandbox) => {
    const n = await page.evaluate(() => reports.length);
    await page.evaluate(({ src, sandbox }) => { if (sandbox) frame.setAttribute('sandbox', sandbox); else frame.removeAttribute('sandbox'); rt.commit(() => rt.W(source, src)); }, { src, sandbox });
    await page.waitForFunction(n => reports.length === n + 1, n, { timeout: 15000 });
    return page.evaluate(() => reports.at(-1));
  };
  await page.waitForFunction(() => reports.length === 1, {}, { timeout: 15000 });
  const first = await page.evaluate(() => reports[0]);
  expect(first.files).toEqual([
    { file: expect.stringMatching(/^blob:/), status: 200, type: 'application/octet-stream', bytes: 20 * 1024 * 1024 + 1, img: 'error' },
    { file: expect.stringMatching(/^blob:/), status: 200, type: 'text/plain', bytes: 6, img: 'error' },
  ]);
  // The other values, and the fragment, stay as written.
  expect([first.search.endsWith('&keep=a+b'), first.hash]).toEqual([true, '#top']);
  // Names a URL would split, encoded as encodeURIComponent encodes them, and an empty file.
  const odd = await next(`assets/given.html?${ODD.map(n => 'file=' + encodeURIComponent(`app:/cache/${n}`)).join('&')}&file=app:/cache/empty.txt`);
  expect(odd.files.map(f => [f.status, f.bytes])).toEqual([...ODD.map(n => [200, new TextEncoder().encode(n).length]), [200, 0]]);
  // A new source revokes the old one's URLs.
  expect(await page.evaluate(async urls => Promise.all(urls.map(u => fetch(u).then(() => 'read', () => 'refused'))), first.files.map(f => f.file))).toEqual(['refused', 'refused']);
  // What cannot be given stays as written; the page loads, and there is no error.
  const loads = await page.evaluate(() => events.length);
  for (const value of ['app:/data/secret.txt', 'app:/cache/../data/secret.txt', 'app:/cache/missing.bin', 'app:/cache']) {
    for (const written of [value, encodeURIComponent(value)]) {
      const report = await next(`assets/given.html?file=${written}`);
      expect([report.files[0].file, report.files[0].error !== undefined]).toEqual([value, true]);
    }
  }
  await page.waitForFunction(n => events.length >= n + 8, loads, { timeout: 15000 });
  expect(await page.evaluate(n => events.slice(n), loads)).toEqual(Array(8).fill('load'));
  // A page outside assets/ and deck/ is loaded as written.
  const other = await next('other/given.html?file=app:/cache/big.bin');
  expect([await page.evaluate(() => new URL(frame.src).pathname), other.files[0].file]).toEqual(['/other/given.html', 'app:/cache/big.bin']);
  // In an opaque sandbox the page cannot read the app's object URL, nor show it.
  const opaque = await next('assets/given.html?file=app:/cache/dot.png', 'allow-scripts');
  expect([opaque.files[0].error !== undefined, opaque.files[0].img]).toEqual([true, 'error']);
  // Unmounted, the frame's URLs are revoked.
  const shown = await next('assets/given.html?file=app:/cache/dot.png&shown=1');
  expect([shown.files[0].status, shown.files[0].img]).toEqual([200, 'load']);
  await page.evaluate(() => { rt.end(scope); frame.remove(); });
  expect(await page.evaluate(async u => fetch(u).then(() => false, () => true), shown.files[0].file)).toBe(true);
  // A literal source arrives as `data-app-src` (rows.rs), and is given its file the same way.
  await page.evaluate(() => { rt.h(document.querySelector('main'), 'iframe', 0, { 'data-app-src': 'deck/given.htm?file=app:/tmp/note.txt' }, 0); });
  await page.waitForFunction(n => reports.length === n, (await page.evaluate(() => reports.length)) + 1, { timeout: 15000 });
  expect(await page.evaluate(() => reports.at(-1).files.map(f => [f.status, f.bytes]))).toEqual([[200, 6]]);
  expect(errors).toEqual([]);
}), 120000);

test('a page under a release path is given its files, and nothing outside the app is', () => browserTest(async (page, server) => {
  await page.goto(server.url.href);
  expect(await page.evaluate(async () => {
    globalThis.exact = {};
    const { pageFiles } = await import('/picker-glue.js');
    const base = `https://app.example/.exact/root/web/releases/${'a'.repeat(64)}/index.html`;
    return ['assets/v.html?f=app:/cache/a', './deck/v.htm?f=app%3A%2Fcache%2Fa#x', '/assets/v.html?f=app:/cache/a', 'https://other.example/assets/v.html?f=app:/cache/a', 'assets/v.html?f=https://e.dev/a']
      .map(src => pageFiles(src, base)?.paths ?? null);
  })).toEqual([['app:/cache/a'], ['app:/cache/a'], null, null, null]);
}), 60000);

// A `javascript:` source the URL parser reads through a leading space, a control character or a tab inside its scheme,
// with `=app:` in it: refused, as any source that does not navigate is (rt.js `navigable`, glue.js `frameURL`).
const SCRIPTED = [' javascript:parent.ran=1//?x=app:', 'java\tscript:parent.ran=1//?x=app:', '\u0001javascript:parent.ran=1//?x=app:'];

test('a javascript: source with an app: value never runs on either web host', () => browserTest(async (page, server) => {
  const glue = await Bun.file('host/web/glue.js').text();
  const appFrameSource = glue.slice(glue.indexOf('function appFrameSource('), glue.indexOf('\n', glue.indexOf('function appFrameSource(')));
  await page.goto(server.url.href);
  const result = await page.evaluate(async ({ appFrameSource, scripted }) => {
    globalThis.rt = await import('/rt.js'); globalThis.exact = {};
    const nav = await import('/navigation.js'), picker = await import('/picker-glue.js');
    const settle = () => new Promise(r => setTimeout(r, 300));
    globalThis.ran = 0;
    const out = { rt: [], glue: [] };
    for (const src of scripted) {
      let el;
      rt.scope(() => { el = rt.h(document.querySelector('main'), 'iframe', 0, {}, 0); rt.P(el, 'src', () => src); });
      await settle();
      out.rt.push(el.getAttribute('src'));
    }
    // The wasm host's own function, with its admission as setProps writes it.
    const host = new Function('nav', 'picker', `
      const { commitGuestOrigin, pageGivenFiles, frameURL, refuseURL } = nav, iframeLoading = new WeakMap(), track = p => p, localAssetURL = u => u;
      const api = { appFrame: (el, state, committed, admit) => picker.appFrame(el, state, 'glue-given', committed, admit) }, pickerAPI = () => Promise.resolve(api);
      ${appFrameSource.replace('picker()', 'pickerAPI()')}
      return (el, value) => { if (!appFrameSource(el, value)) { if (frameURL(value, el.getAttribute('sandbox'))) el.setAttribute('src', localAssetURL(value)); else refuseURL(el, 'src', value); } };
    `)(nav, picker);
    for (const src of scripted) {
      const el = document.createElement('iframe');
      document.querySelector('main').append(el);
      host(el, src);
      await settle();
      out.glue.push(el.getAttribute('src'));
    }
    out.ran = globalThis.ran;
    return out;
  }, { appFrameSource, scripted: SCRIPTED });
  expect(result).toEqual({ rt: Array(3).fill('about:blank'), glue: Array(3).fill('about:blank'), ran: 0 });
}), 60000);
