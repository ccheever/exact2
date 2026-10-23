import { test, expect } from 'bun:test';
import { suiteBrowser } from '../../scripts/agent-launch.mjs';

// LLP 1020.002 §11's web subset in real browsers: a same-origin guest's
// navigations as FrameNavigation records, the commands a page has over its
// frame, and a cross-origin guest's `load` and `navigate` alone.
const SAME = 16582, CROSS = 16583;
const html = body => new Response(body, { headers: { 'content-type': 'text/html' } });
const guest = (title, extra = '') => html(`<!doctype html><title>${title}</title><link rel="icon" href="/i/${title}.png"><body>${title}<a id="next" href="/g/b">b</a>${extra}`);

function serve() {
  const same = Bun.serve({ port: SAME, hostname: '127.0.0.1', async fetch(request) {
    const path = new URL(request.url).pathname;
    if (path === '/frame-navigation.js' || path === '/navigation.js') return new Response(Bun.file(new URL('.' + path, import.meta.url)), { headers: { 'content-type': 'text/javascript' } });
    if (path === '/g/a') return guest('A');
    if (path === '/g/b') return guest('B');
    if (path === '/g/redirect') return Response.redirect(`http://127.0.0.1:${SAME}/g/b`, 302);
    if (path === '/g/slow') { await Bun.sleep(3000); return guest('Slow'); }
    if (path === '/g/probe') return html(`<!doctype html><script>requestAnimationFrame(() => parent.postMessage({dark: matchMedia('(prefers-color-scheme: dark)').matches, width: innerWidth, ratio: devicePixelRatio}, '*'))</script>`);
    if (path.startsWith('/i/') || path === '/favicon.ico') return new Response('', { status: 404 });
    return html(`<!doctype html><main id="exact-root"></main><script type="module">
      import * as nav from '/frame-navigation.js';
      window.nav = nav; window.events = []; window.logs = []; window.probes = [];
      addEventListener('message', e => probes.push(e.data));
      window.mount = (src, style = '') => { const f = document.createElement('iframe'); f.id = 'tab'; f.style.cssText = style; f.src = src; document.querySelector('main').append(f); return f; };
      window.hear = f => nav.watch(f, (name, r) => events.push([name, r]));
      window.run = (name, ...args) => nav.command(name, ['tab', ...args], t => logs.push(t));
    </script>`);
  } });
  const cross = Bun.serve({ port: CROSS, hostname: '127.0.0.1', fetch: () => guest('X') });
  return { stop() { same.stop(true); cross.stop(true); } };
}

// The events since `from`, as [name, url or destination, extra] for a terse comparison.
const since = (page, from) => page.evaluate(n => events.slice(n).map(([name, r]) => [name, r.loading, r.url, r.destination, r.navigationType, r.error]), from);
const settle = (page, from, name, url) => page.waitForFunction(([n, name, url]) => events.slice(n).some(([e, r]) => e === name && (url == null || r.url.endsWith(url) || r.error === url)), [from, name, url], { timeout: 10000 });
const count = page => page.evaluate(() => events.length);

// `color-scheme` on the frame is the guest's `prefers-color-scheme`, and
// `zoom` its page zoom, as Chrome applies them (LLP 1020.002 D6). Firefox
// zooms the box but leaves the guest's devicePixelRatio at 1. No colour
// scheme emulation, and first: Chrome's applies to every frame and hides
// the frame's own, and it outlives a context of the shared browser.
for (const engine of ['chromium', 'firefox', 'webkit']) {
  test(`color-scheme and zoom on the frame reach the guest (${engine})`, async () => {
    const servers = serve();
    let browser;
    try {
      browser = await suiteBrowser(engine);
      const page = await browser.newPage({ colorScheme: null });
      await page.goto(`http://127.0.0.1:${SAME}/`);
      await page.waitForFunction(() => window.nav);
      await page.evaluate(() => mount('/g/probe', 'color-scheme: dark; zoom: 1.5; width: 400px; height: 100px; border: 0'));
      await page.waitForFunction(() => probes.length === 1);
      expect(await page.evaluate(() => probes[0])).toEqual({ dark: true, width: 400, ratio: engine === 'firefox' ? 1 : 1.5 });
    } finally {
      servers.stop();
      await browser?.close();
    }
  }, 30000);
}

for (const engine of ['chromium', 'firefox', 'webkit']) {
  test(`a frame's navigation records and commands on the web (${engine})`, async () => {
    const servers = serve();
    let browser;
    try {
      browser = await suiteBrowser(engine);
      const page = await browser.newPage();
      await page.goto(`http://127.0.0.1:${SAME}/`);
      await page.waitForFunction(() => window.nav);
      await page.evaluate(() => hear(mount('/g/a')));
      await settle(page, 0, 'navigatesuccess', '/g/a');
      const hasApi = await page.evaluate(() => 'navigation' in document.querySelector('#tab').contentWindow);
      const first = await page.evaluate(() => events.find(([n]) => n === 'navigatesuccess')[1]);
      expect(first.title).toBe('A');
      expect(first.icons).toEqual([`http://127.0.0.1:${SAME}/i/A.png`, `http://127.0.0.1:${SAME}/favicon.ico`]);
      expect(first.partitioned).toBe(false);
      expect(first.loading).toBe(false);
      expect(first.controller).toBe('person');
      expect(Object.keys(first)).toEqual(await page.evaluate(() => nav.FIELDS));

      // A link: the old document's `navigate` (where the browser has the Navigation API), then the new one's commit and end.
      let n = await count(page);
      await page.evaluate(() => document.querySelector('#tab').contentDocument.getElementById('next').click());
      await settle(page, n, 'navigatesuccess', '/g/b');
      const link = await since(page, n);
      expect(link.slice(-2).map(e => e[0])).toEqual(['currententrychange', 'navigatesuccess']);
      if (hasApi) expect(link[0]).toEqual(['navigate', true, `http://127.0.0.1:${SAME}/g/a`, `http://127.0.0.1:${SAME}/g/b`, 'push', '']);
      const ids = await page.evaluate(n => events.slice(n).map(([, r]) => r.navigationId), n);
      expect(new Set(ids).size).toBe(1);

      // pushState: a same-document navigation, three events together, where the Navigation API reports it.
      n = await count(page);
      await page.evaluate(() => document.querySelector('#tab').contentWindow.history.pushState({}, '', '/g/b#pushed'));
      if (hasApi) {
        await settle(page, n, 'navigatesuccess', '/g/b#pushed');
        expect((await since(page, n)).map(e => [e[0], e[4]])).toEqual([['navigate', 'push'], ['currententrychange', 'push'], ['navigatesuccess', 'push']]);
      }

      // A title change with no navigation.
      n = await count(page);
      await page.evaluate(() => { document.querySelector('#tab').contentDocument.title = 'B2'; });
      await page.waitForFunction(n => events.slice(n).some(([e, r]) => e === 'currententrychange' && r.title === 'B2'), n);
      expect((await since(page, n)).map(e => [e[0], e[1]])).toEqual([['currententrychange', false]]);

      // `navigate` through a server redirect: the committed URL is where it landed.
      n = await count(page);
      await page.evaluate(() => run('navigate', '/g/redirect'));
      await settle(page, n, 'navigatesuccess', '/g/b');
      if (hasApi) expect((await since(page, n))[0].slice(0, 4)).toEqual(['navigate', true, expect.any(String), `http://127.0.0.1:${SAME}/g/redirect`]);

      // traverseHistory: back one; a missing entry and ignoreCache are refused.
      n = await count(page);
      await page.evaluate(() => run('traverseHistory', -1));
      await page.waitForFunction(n => events.slice(n).some(([e]) => e === 'navigatesuccess'), n);
      if (hasApi) expect((await since(page, n))[0][4]).toBe('traverse');
      n = await count(page);
      await page.evaluate(() => run('traverseHistory', -50));
      if (hasApi) { await settle(page, n, 'navigateerror', 'InvalidStateError'); }
      n = await count(page);
      await page.evaluate(() => run('reload', true));
      await settle(page, n, 'navigateerror', 'NotSupportedError');
      n = await count(page);
      await page.evaluate(() => run('navigate', 'javascript:alert(1)'));
      await settle(page, n, 'navigateerror', 'NotSupportedError');

      // stopLoading ends a navigation in flight; the committed URL stays.
      if (hasApi) {
        n = await count(page);
        await page.evaluate(() => run('navigate', '/g/slow'));
        await page.waitForFunction(n => events.slice(n).some(([e]) => e === 'navigate'), n);
        const before = await page.evaluate(() => events.at(-1)[1].url);
        await page.evaluate(() => run('stopLoading'));
        await settle(page, n, 'navigateerror', 'AbortError');
        const stopped = await page.evaluate(() => events.at(-1)[1]);
        expect([stopped.loading, stopped.url]).toEqual([false, before]);
      }

      // A cross-origin guest: `navigate` reaches it; it reports nothing of its own, and the other commands are refused.
      n = await count(page);
      await page.evaluate(port => run('navigate', `http://127.0.0.1:${port}/x`), CROSS);
      await page.waitForFunction(port => { try { return document.querySelector('#tab').contentDocument === null; } catch { return true; } }, CROSS);
      await page.waitForFunction(n => events.slice(n).some(([e]) => e === 'navigatesuccess'), n);
      const crossed = await since(page, n);
      expect(crossed.at(-1)[2]).toBe(`http://127.0.0.1:${CROSS}/x`);
      n = await count(page);
      await page.evaluate(() => { run('reload'); run('traverseHistory', -1); run('stopLoading'); });
      await page.waitForFunction(n => events.slice(n).filter(([e, r]) => e === 'navigateerror' && r.error === 'NotSupportedError').length === 3, n);
      n = await count(page);
      await page.evaluate(port => run('navigate', `http://127.0.0.1:${port}/g/a`), SAME);
      await settle(page, n, 'navigatesuccess', '/g/a');
      expect(await page.evaluate(() => logs.filter(l => !/NotSupported|not a URL|another origin|no history entry|ignoreCache/.test(l)))).toEqual([]);
    } finally {
      servers.stop();
      await browser?.close();
    }
  }, 60000);
}
