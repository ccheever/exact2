import { test, expect } from 'bun:test';
import { existsSync, readFileSync } from 'node:fs';
import { suiteBrowser } from '../../scripts/agent-launch.mjs';

test('resource observations keep request provenance through switching, failure, rollback and recovery', async () => {
  for (const development of [true, false]) {
    const server = Bun.serve({ port: 0, fetch(request) {
      const path = new URL(request.url).pathname;
      if (path === '/favicon.ico') return new Response(null, { status: 204 });
      if (path.endsWith('/names.js')) return new Response('export const sourceTypes = {};', { headers: { 'content-type': 'text/javascript' } });
      if (path === '/') return new Response('<div id="exact-root"></div>', { headers: { 'content-type': 'text/html' } });
      const local = new URL('../..' + path, import.meta.url), file = existsSync(local) ? local : new URL('../web/' + path.split('/').pop(), import.meta.url);
      if (path.endsWith('/rt.js') && development) return new Response(readFileSync(file, 'utf8').replace('const DEVELOPMENT_STORE = null;', 'const DEVELOPMENT_STORE = "fixture";'), { headers: { 'content-type': 'text/javascript' } });
      return new Response(Bun.file(file));
    } });
    const browser = await suiteBrowser(), page = await browser.newPage();
    try {
      await page.goto(server.url.href);
      await page.evaluate(async () => {
        const rt = await import('/host/web-js/rt.js'), owner = rt.sig('A'), completions = [];
        rt.clock.agent = true;
        rt.data.answer = () => ({ promise: new Promise(resolve => completions.push(resolve)) });
        const feed = rt.res('feed', 'feed', () => [owner()], undefined, undefined, 's', 'empty');
        window.fixture = { rt, owner, feed, completions,
          inspect: () => feed.r.observation?.(0),
          pick: v => rt.commit(() => rt.W(owner, v)),
          refresh: () => rt.commit(() => rt.R(feed)),
          finish: (i, value) => completions[i](value) };
        rt.commit(() => {});
      });
      const inspect = () => page.evaluate(() => fixture.inspect());
      const finish = async (index, value) => {
        await page.evaluate(([i, v]) => fixture.finish(i, v), [index, value]);
        await page.waitForFunction(() => fixture.rt.inflight.n === 0);
      };
      if (development) expect(await inspect()).toMatchObject({ displayed: 'fallback', pending: true, argumentsEqual: null });
      else expect(await inspect()).toBeUndefined();
      await finish(0, 'A rows');
      const first = await inspect();
      if (development) expect(first).toMatchObject({ displayed: 'current', pending: false, argumentsEqual: true });
      await page.evaluate(() => fixture.pick('B'));
      if (development) expect(await inspect()).toMatchObject({ displayed: 'retained', pending: true, argumentsEqual: false, displayedGeneration: first.displayedGeneration });
      await finish(1, 42);
      if (development) {
        expect(await inspect()).toMatchObject({ displayed: 'retained', pending: false, argumentsEqual: false });
        expect(await page.evaluate(() => fixture.feed.r.failures.count)).toBe(1);
      }
      await page.evaluate(() => fixture.pick('A'));
      if (development) expect(await inspect()).toMatchObject({ displayed: 'retained', pending: false, argumentsEqual: true });
      await page.evaluate(() => fixture.refresh());
      await finish(2, 'A fresh rows');
      const recovered = await inspect();
      if (development) expect(recovered.displayed).toBe('current');
      await page.evaluate(() => fixture.refresh());
      await finish(3, 42);
      if (development) {
        expect(await inspect()).toMatchObject({ displayed: 'retained', argumentsEqual: true, lastSuccessfulGeneration: recovered.displayedGeneration });
        expect(await page.evaluate(() => fixture.feed.r.failures.count)).toBe(2);
      }
      const before = await inspect();
      await page.evaluate(() => fixture.rt.commit(() => { fixture.rt.W(fixture.owner, 'C'); throw new fixture.rt.Refusal('refused'); }));
      expect(await inspect()).toEqual(before);
      expect(await page.evaluate(() => fixture.feed())).toBe('A fresh rows');
      await page.evaluate(() => { fixture.pick('B'); fixture.pick('C'); fixture.finish(4, 42); fixture.finish(5, 'C rows'); });
      await page.waitForFunction(() => fixture.rt.inflight.n === 0);
      if (development) {
        expect(await inspect()).toMatchObject({ displayed: 'current', argumentsEqual: true });
        expect(await page.evaluate(() => fixture.feed.r.failures.count)).toBe(2);
      }
      expect(await page.evaluate(() => fixture.feed())).toBe('C rows');
    } finally { await browser.close(); server.stop(true); }
  }
}, 30000);
