import { afterAll, beforeAll, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { suiteBrowser } from '../../../scripts/agent-launch.mjs';
import { withLook } from '../look.mjs';

const WEB = resolve(import.meta.dir, '..');
const style = withLook(readFileSync(resolve(WEB, 'index.html'), 'utf8')).match(/<style>([\s\S]*?)<\/style>/)[1];
let server;
beforeAll(() => {
  server = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch(request) {
    const path = new URL(request.url).pathname;
    if (path === '/') return new Response(`<style>${style}</style><div id="exact-root"></div>
      <script type="module">
        import { createInputHandlers } from '/input-glue.js';
        createInputHandlers({ root: document.getElementById('exact-root'), views: new Map(), retiredViews: new Set(), ready: () => true, inertAncestor: () => false, dispatch() {} });
        window.ready = true;
      </script>`, { headers: { 'content-type': 'text/html' } });
    return new Response(Bun.file(resolve(WEB, path.slice(1))), { headers: { 'content-type': 'text/javascript' } });
  } });
});
afterAll(() => server?.stop(true));

for (const engine of ['chromium', 'webkit']) test(`${engine}: stationary physical and held presses follow computed scale targets`, async () => {
  const browser = await suiteBrowser(engine);
  try {
    const page = await browser.newPage();
    await page.goto(server.url.href);
    await page.waitForFunction(() => window.ready);
    const seen = await page.evaluate(async () => {
      const root = document.getElementById('exact-root'), results = [];
      for (const held of [false, true]) for (const initial of [null, '0.97', 'var(--scale-token)']) {
        root.innerHTML = '<div data-exact-on="press" style="width:100px;height:100px;scale:calc(1.25 * var(--exact-press-factor, 1))">Scale</div>';
        const el = root.firstElementChild, token = document.createElement('style');
        token.textContent = ':root { --scale-token: 0.97 }'; document.head.append(token);
        if (initial != null) el.style.setProperty('--exact-press', initial);
        const box = el.getBoundingClientRect();
        const pointer = type => el.dispatchEvent(new PointerEvent(type, { bubbles: true, pointerType: 'mouse', pointerId: 1, isPrimary: true, button: 0, buttons: type === 'pointerdown' ? 1 : 0, clientX: box.x + box.width / 2, clientY: box.y + box.height / 2 }));
        const settle = async () => {
          await new Promise(requestAnimationFrame);
          el.getAnimations().forEach(a => a.finish());
          await new Promise(requestAnimationFrame);
        };
        const scale = () => Math.round(Number(getComputedStyle(el).scale) / 1.25 * 1000) / 1000;
        if (held) exact.hold(el, 'active'); else pointer('pointerdown');
        await settle();
        const before = scale();
        if (initial?.startsWith('var')) {
          token.sheet.cssRules[0].style.setProperty('--scale-token', '0.92');
          exact.repaintPress(); // CSSOM edits use the runtime's refresh hook
        } else el.style.setProperty('--exact-press', initial == null ? '0.8' : '0.92');
        await settle();
        const changed = scale();
        el.style.removeProperty('--exact-press');
        await settle();
        const cleared = scale();
        el.style.setProperty('--exact-press', 'var(--scale-token)');
        await settle();
        token.textContent = ':root { --scale-token: 0.88 }';
        await settle();
        const again = scale();
        if (held) exact.hold(el, null); else pointer('pointerup');
        await settle();
        results.push([held, initial, before, changed, cleared, again, scale()]);
        token.remove();
      }
      return results;
    });
    expect(seen).toEqual([false, true].flatMap(held => [null, '0.97', 'var(--scale-token)'].map(initial =>
      [held, initial, initial == null ? 1 : 0.97, initial == null ? 0.8 : 0.92, 1, 0.88, 1])));
  } finally {
    await browser.close();
  }
}, 60_000);
