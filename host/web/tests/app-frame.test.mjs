import { test, expect } from 'bun:test';
import { chromium } from 'playwright-core';
import { resolve } from 'node:path';

async function browserTest(run) {
  const server = Bun.serve({ port: 0, hostname: '127.0.0.1', async fetch(request) {
    const path = new URL(request.url).pathname;
    if (path === '/') return new Response('<main id="exact-root"></main>', { headers: { 'content-type': 'text/html' } });
    if (path === '/names.js') return new Response('export const sourceTypes = {};', { headers: { 'content-type': 'text/javascript' } });
    const name = path.slice(1);
    for (const dir of ['host/web-js', 'host/web']) {
      const file = Bun.file(resolve(dir, name));
      if (await file.exists()) return new Response(file);
    }
    return new Response('missing', { status: 404 });
  } });
  const browser = await chromium.launch({ executablePath: process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true });
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
