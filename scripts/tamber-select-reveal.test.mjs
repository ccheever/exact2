// An outline brings the selection's own copy into view (Apple pass #2, theme2's 13:44 approval; host/web/dev.js, as
// the Apple hosts' Inspect.revealing), on both web pages over apps/select-walk-fixture, whose Card has two copies (in
// Post, near the top, and in Outer, at the foot). The page scrolled to Outer's: (a) a move up from `wrapped` lands on
// Outer's card and the page stays (it scrolled to Post's, the first copy, before); (c) scrolled back to the top, an
// outline of the card brings Outer's in, not Post's; (e) that copy hidden, then gone, the outline scrolls nothing;
// (d) with no selection (the tool's Outline), the first copy in the page's order, as before.
import { afterEach, expect, test } from 'bun:test';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnOwned, stopAll } from './test-children.mjs';
import { suiteBrowser } from './agent-launch.mjs';

const ROOT = resolve(new URL('..', import.meta.url).pathname), APP = resolve(ROOT, 'apps/select-walk-fixture');
const SOURCE = readFileSync(join(APP, 'app.contract'), 'utf8');
afterEach(() => stopAll());
/** The line of the node a testId names in the fixture. */
const line = id => SOURCE.split('\n').findIndex(l => l.includes(`testId="${id}"`)) + 1;

async function devLoop(flags) {
  const listener = createServer();
  await new Promise(ok => listener.listen(0, '127.0.0.1', ok));
  const port = listener.address().port;
  await new Promise(ok => listener.close(ok));
  const dist = mkdtempSync(join(tmpdir(), 'exact-select-reveal-'));
  const dev = spawnOwned(process.execPath, [resolve(ROOT, 'host/web/dev.mjs'), '--app', 'select-walk-fixture', '--port', String(port), ...flags], { cwd: ROOT, env: { ...process.env, EXACT_WEB_DIST: dist }, stdio: ['ignore', 'pipe', 'pipe'] });
  const stop = async () => { await dev.stop(30_000); rmSync(dist, { recursive: true, force: true }); };
  let out = '', err = '';
  dev.stderr.on('data', d => { err = (err + d).slice(-4000); });
  try {
    await new Promise((ok, fail) => {
      dev.stdout.on('data', d => { out += d; if (out.includes(`127.0.0.1:${port}/`)) ok(); });
      dev.on('error', fail);
      dev.on('exit', code => fail(new Error(`the dev loop exited ${code}: ${out}${err}`)));
    });
  } catch (e) { await stop(); throw e; }
  return { base: `http://127.0.0.1:${port}`, stop, said: () => out };
}

test('an outline on both web pages brings the selection\'s own copy into view, never another', async () => {
  const browser = await suiteBrowser();
  const polls = [], stops = [];
  // Each case checked on its own (none stops the next), all said at the end: on the old code each red case shows.
  const fails = [];
  try {
    for (const flags of [[], ['--wasm']]) {
      const loop = await devLoop(flags), { base, said } = loop, page_ = flags.join('') || 'js';
      stops.push(loop.stop);
      const { token } = await (await fetch(`${base}/__tamber/token`)).json();
      const post = async (body, path = '/__tamber') => (await fetch(`${base}${path}`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: JSON.stringify(body) })).json();
      const events = [], state = { polling: true };
      polls.push(state);
      const poll = (async () => { let after = 0, epoch; while (state.polling) { const r = await post(epoch ? { after, epoch, client: 'tool' } : { after: 0, wait: false, client: 'tool' }, '/__tamber/events'); events.push(...(r.events ?? [])); after = r.seq ?? after; epoch = r.epoch ?? epoch; await Bun.sleep(100); } })();
      const until = async (pass, ms = 60_000, what = '') => { const end = Date.now() + ms; for (;;) { const got = await pass(); if (got) return got; if (Date.now() > end) throw new Error(`not seen: ${what} on ${page_}`); await Bun.sleep(100); } };
      // Wide, so the banner (top centre) leaves the cards' left edge to click; short (below), so the page scrolls.
      const check = (name, ok, info) => { if (!ok) fails.push(`${page_} ${name}: ${JSON.stringify(info)}`); };
      const page = await browser.newPage({ viewport: { width: 1000, height: 700 } });
      await page.goto(`${base}/`);
      await until(() => said().includes('plan ready'), 900_000, 'plan ready');
      const build = await until(async () => (await post({ method: 'hello' })).ok?.builds?.find(b => b.host.startsWith('web')), 120_000, 'the build');
      expect((await post({ method: 'inspect', client: 'tool', on: true })).ok.builds).toEqual([build.id]);
      await page.locator('#exact-inspect').waitFor();
      const scrollY = () => page.evaluate(() => scrollY);
      const settle = () => page.waitForTimeout(400);
      // An outline applied: its dashed marks drawn again (the page scrolls before it draws them; Astra's round 1).
      const unmarked = () => page.evaluate(() => { for (const m of document.querySelectorAll('[data-exact-outline]')) m.remove(); });
      const marked = async () => { await until(() => page.evaluate(() => document.querySelectorAll('[data-exact-outline]').length > 0), 30_000, 'the outline drawn'); await page.waitForTimeout(100); };
      const shows = (nth) => page.evaluate((n) => { const r = document.querySelectorAll('[data-testid="card"]')[n].getBoundingClientRect(); return r.bottom > 0 && r.top < innerHeight; }, nth); // (on screen at all)
      const cards = await page.locator('[data-testid="card"]').count();
      expect(cards).toBe(2);
      // As short as Outer's card and what follows it (the fixture is short), so the page scrolls to it with Post's card
      // above the top.
      const rect = n => page.evaluate(n => { const r = document.querySelectorAll('[data-testid="card"]')[n].getBoundingClientRect(); return { top: r.top + scrollY, bottom: r.bottom + scrollY }; }, n);
      const [c0, c1] = [await rect(0), await rect(1)];
      const tall = await page.evaluate(() => document.querySelector('[data-testid="root"]').getBoundingClientRect().bottom + scrollY);
      expect(c1.top - 4).toBeGreaterThan(c0.bottom);
      await page.setViewportSize({ width: 1000, height: Math.floor(tall - c1.top + 4) });
      await settle();
      const toOuter = async () => { await page.evaluate(y => scrollTo(0, y), Math.floor(c1.top - 4)); await settle(); };
      // Outer's card in view, Post's not.
      await toOuter();
      expect([page_, await shows(1), await shows(0)], JSON.stringify({ c0, c1, tall, y: await scrollY() })).toEqual([page_, true, false]);
      const box = await page.locator('[data-testid="wrapped"]').boundingBox();
      let seen = events.length;
      await page.mouse.click(box.x + Math.min(6, box.width / 2), box.y + box.height / 2);
      await until(() => events.slice(seen).find(e => e.event === 'selection'), 30_000, 'wrapped selected');
      await settle();
      const at = await scrollY();
      // (a) Up one: Outer's card; its outline (no click token) leaves the page where it is.
      seen = events.length;
      await unmarked();
      expect((await post({ method: 'inspect', client: 'tool', build: build.id, select: { up: 1 } })).ok).toEqual({ builds: [build.id] });
      const moved = await until(() => events.slice(seen).find(e => e.event === 'selection'), 30_000, 'the move up');
      expect(moved.source.line).toBe(line('card'));
      await marked();
      check('(a) the move leaves the page', (await scrollY()) === at && await shows(1), { at, now: await scrollY() });
      // (c) Back at the top, an outline of the card brings Outer's copy in, the selection's, not Post's.
      const outline = async () => { await unmarked(); expect((await post({ method: 'outline', client: 'tool', at: [{ file: moved.source.file, line: moved.source.line, col: moved.source.col }] })).ok?.builds).toEqual([build.id]); await marked(); };
      await page.evaluate(() => scrollTo(0, 0));
      await settle();
      expect(await shows(1)).toBe(false);
      await outline();
      check('(c) the selection\'s copy is brought in', await shows(1), { now: await scrollY() });
      // (d) No selection (inspect mode off and on again), neither copy on screen: the first in the page's order is
      // brought in, as before.
      expect((await post({ method: 'inspect', client: 'tool', on: false })).ok).toBeTruthy();
      expect((await post({ method: 'inspect', client: 'tool', on: true })).ok.builds).toEqual([build.id]);
      await page.evaluate(() => scrollTo(0, 0));
      await settle();
      expect([await shows(0), await shows(1)]).toEqual([false, false]);
      await outline();
      // (the first mark is the first copy in the page's own order: it is on screen)
      check('(d) the first copy without a selection', await page.evaluate(() => { const r = document.querySelector('[data-exact-outline]').getBoundingClientRect(); return r.bottom > 0 && r.top < innerHeight; }), { now: await scrollY() });
      // (e) A held selection whose copy is gone from the page: the outline scrolls nothing, never to Post's copy.
      await toOuter();
      const box2 = await page.locator('[data-testid="wrapped"]').boundingBox();
      seen = events.length;
      await page.mouse.click(box2.x + Math.min(6, box2.width / 2), box2.y + box2.height / 2);
      await until(() => events.slice(seen).find(e => e.event === 'selection'), 30_000, 'wrapped selected again');
      seen = events.length;
      await post({ method: 'inspect', client: 'tool', build: build.id, select: { up: 1 } });
      await until(() => events.slice(seen).find(e => e.event === 'selection'), 30_000, 'the move up again');
      await settle();
      // Hidden (`visibility: hidden`, its box kept), then gone from the page.
      const kept = await scrollY();
      await page.evaluate(() => { document.querySelectorAll('[data-testid="card"]')[1].style.visibility = 'hidden'; scrollTo(0, 0); });
      await settle();
      const atTop = await scrollY();
      await outline();
      check('(e) a hidden copy scrolls nothing', (await scrollY()) === atTop, { atTop, now: await scrollY() });
      await page.evaluate(y => { document.querySelectorAll('[data-testid="card"]')[1].remove(); scrollTo(0, y); }, kept);
      await settle();
      const goneAt = await scrollY(); // (the page shorter by that card)
      await outline();
      check('(e) a gone copy scrolls nothing', (await scrollY()) === goneAt, { goneAt, now: await scrollY() });
      state.polling = false;
      await poll.catch(() => {});
      await page.close();
      await loop.stop();
    }
    expect(fails).toEqual([]);
  } finally {
    for (const p of polls) p.polling = false;
    await browser.close();
    for (const stop of stops) await stop();
  }
}, 1_800_000);
