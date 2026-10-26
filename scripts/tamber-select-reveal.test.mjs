// An outline brings the selection's own copy into view (Apple pass #2, theme2's 13:44 approval; host/web/dev.js and
// host/web-js/inspect-dev.js, as the Apple hosts' Inspect.revealing), on both web pages over apps/select-walk-fixture,
// whose Card has two copies (in Post, above, and in Outer, at the foot). The selection is made on the copy the page
// does NOT list first (the old rule's pick), so each case is red on the old code (theme2's 16:25 (3)): (a) a move up
// to that card leaves the page where it is; (c) at the other copy, an outline brings the selection's in; (e) that copy
// hidden, then gone, the outline scrolls nothing; (d) with no selection (the tool's Outline), the first copy in the
// page's order, as before. Each case is checked on its own and every red one said at the end.
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
      const shows = (nth) => page.evaluate((n) => { const r = document.querySelectorAll('[data-testid="card"]')[n].getBoundingClientRect(); return r.top >= 0 && r.bottom <= innerHeight; }, nth); // (wholly on screen)
      expect(await page.locator('[data-testid="card"]').count()).toBe(2);
      // The two copies, Post's (0, above) and Outer's (1, at the foot); a window as tall as the taller and a little
      // more, so one card can show without the other: at P Post's whole and not Outer's, at O the reverse.
      const rect = n => page.evaluate(n => { const r = document.querySelectorAll('[data-testid="card"]')[n].getBoundingClientRect(); return { top: r.top + scrollY, bottom: r.bottom + scrollY }; }, n);
      const [c0, c1] = [await rect(0), await rect(1)];
      expect(c1.top).toBeGreaterThan(c0.bottom);
      const h = Math.ceil(Math.max(c0.bottom - c0.top, c1.bottom - c1.top)) + 4;
      await page.setViewportSize({ width: 1000, height: h });
      await settle();
      const at = { P: Math.floor(c0.top - 2), O: Math.ceil(c1.bottom + 2 - h), top: 0 };
      const to = async (where) => { await page.evaluate(y => scrollTo(0, y), at[where]); await settle(); expect([page_, where, await scrollY()]).toEqual([page_, where, at[where]]); };
      const click = async (id) => {
        const box = await page.locator(`[data-testid="${id}"]`).boundingBox(), seen = events.length;
        await page.mouse.click(box.x + Math.min(6, box.width / 2), box.y + box.height / 2);
        return until(() => events.slice(seen).find(e => e.event === 'selection'), 30_000, `${id} selected`);
      };
      const moveUp = async () => {
        const seen = events.length;
        await unmarked();
        expect((await post({ method: 'inspect', client: 'tool', build: build.id, select: { up: 1 } })).ok).toEqual({ builds: [build.id] });
        const s = await until(() => events.slice(seen).find(e => e.event === 'selection'), 30_000, 'the move up');
        await marked();
        return s;
      };
      const restart = async () => {
        expect((await post({ method: 'inspect', client: 'tool', on: false })).ok).toBeTruthy();
        expect((await post({ method: 'inspect', client: 'tool', on: true })).ok.builds).toEqual([build.id]);
        await settle();
      };
      // The card's source, from a move to it; then, with no selection, which copy this page lists first (its first
      // dashed mark): the selection is made on the OTHER, so the old rule (the first copy) is wrong on every case.
      await to('P');
      await click('body');
      const card = (await moveUp()).source;
      expect(card.line).toBe(line('card'));
      const outline = async () => { await unmarked(); expect((await post({ method: 'outline', client: 'tool', at: [{ file: card.file, line: card.line, col: card.col }] })).ok?.builds).toEqual([build.id]); await marked(); };
      await restart();
      await to('top');
      await outline();
      const firstTop = await page.evaluate(() => document.querySelector('[data-exact-outline]').getBoundingClientRect().top + scrollY);
      const listedFirst = Math.abs(firstTop - c0.top) < Math.abs(firstTop - c1.top) ? 0 : 1, sel = 1 - listedFirst;
      const mine = sel === 0 ? 'P' : 'O', theirs = sel === 0 ? 'O' : 'P', start = sel === 0 ? 'body' : 'wrapped';
      // (a) The copy shown, its start clicked and moved up to its card: that outline (no click token) leaves the page.
      await to(mine);
      await click(start);
      await moveUp();
      check('(a) the move leaves the page', (await scrollY()) === at[mine] && await shows(sel), { listedFirst, at: at[mine], now: await scrollY() });
      // (c) At the other copy, an outline of the card brings the selection's copy in, not the one listed first.
      await to(theirs);
      await outline();
      check('(c) the selection\'s copy is brought in', await shows(sel), { listedFirst, now: await scrollY() });
      // (e) The selection's copy hidden (`visibility: hidden`, its box kept), then gone, neither copy on screen: the
      // outline scrolls nothing, never to the other copy.
      await page.evaluate(n => { document.querySelectorAll('[data-testid="card"]')[n].style.visibility = 'hidden'; }, sel);
      await to('top');
      await outline();
      check('(e) a hidden copy scrolls nothing', (await scrollY()) === 0, { listedFirst, now: await scrollY() });
      await page.evaluate(n => { document.querySelectorAll('[data-testid="card"]')[n].style.visibility = ''; }, sel);
      // (d) No selection (inspect mode off and on again), at the selection's copy: the first in the page's order is
      // brought in, as before.
      await restart();
      await to(mine);
      await outline();
      check('(d) the first copy without a selection', await shows(listedFirst), { listedFirst, now: await scrollY() });
      // (e) again with the copy gone: selected again, then removed from the page.
      await to(mine);
      await click(start);
      await moveUp();
      await page.evaluate(n => document.querySelectorAll('[data-testid="card"]')[n].remove(), sel);
      await to('top');
      await outline();
      check('(e) a gone copy scrolls nothing', (await scrollY()) === 0, { listedFirst, now: await scrollY() });
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
