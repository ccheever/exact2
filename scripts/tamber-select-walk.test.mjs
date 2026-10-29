// tamber/1.17 `inspect` `select` by the page's own tree (the third hands-on pass's #4; theme2's rulings of the 25th),
// on both web pages, over apps/select-walk-fixture: each new copy owning an ancestor is a step, a slot's host among
// them; at step `up`, that copy's outermost node, past the last the root; the page says `moved: {up, stepped}`. Cases:
// (a) a fill authored in the caller lands on the slot's host; the clone's like row (a leaf in a row that only renders
// it, in the host's slot) lands on the host; (b) a host one component deeper; (c) a host wrapped by another, then the
// wrapper; (d) past the last copy, the root, said in `moved`; and a rebuild, whose new plan's copies the page is sent.
import { afterEach, expect, test } from 'bun:test';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnOwned, stopAll } from './test-children.mjs';
import { suiteBrowser } from './agent-launch.mjs';

const ROOT = resolve(new URL('..', import.meta.url).pathname), APP = resolve(ROOT, 'apps/select-walk-fixture');
const SOURCE = readFileSync(join(APP, 'app.contract'), 'utf8');
afterEach(() => { stopAll(); writeFileSync(join(APP, 'app.contract'), SOURCE); });
/** The line of the node a testId names in the fixture. */
const line = id => SOURCE.split('\n').findIndex(l => l.includes(`testId="${id}"`)) + 1;

async function devLoop(flags) {
  const listener = createServer();
  await new Promise(ok => listener.listen(0, '127.0.0.1', ok));
  const port = listener.address().port;
  await new Promise(ok => listener.close(ok));
  const dist = mkdtempSync(join(tmpdir(), 'exact-select-walk-'));
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

test('tamber/1.17 on both web pages: select walks the page\'s own tree by copies, slot hosts included, to the root', async () => {
  const browser = await suiteBrowser();
  const polls = [], stops = [];
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
      const page = await browser.newPage({ viewport: { width: 420, height: 700 } });
      await page.goto(`${base}/`);
      await until(() => said().includes('plan ready'), 900_000, 'plan ready');
      const build = await until(async () => (await post({ method: 'hello' })).ok?.builds?.find(b => b.host.startsWith('web')), 120_000, 'the build');
      expect((await post({ method: 'inspect', client: 'tool', on: true })).ok.builds).toEqual([build.id]);
      await page.locator('#exact-inspect').waitFor();
      // (`edge`: at its top left corner, in its own padding, not on a child.)
      const pick = async (id, edge = false) => {
        const box = await page.locator(`[data-testid="${id}"]`).first().boundingBox();
        const seen = events.length;
        await page.mouse.click(box.x + (edge ? 1 : Math.min(6, box.width / 2)), edge ? box.y + 1 : box.y + box.height / 2);
        return until(() => events.slice(seen).find(e => e.event === 'selection'), 30_000, `${id} selected`);
      };
      const up = async n => {
        const seen = events.length;
        const r = await post({ method: 'inspect', client: 'tool', build: build.id, select: { up: n } });
        expect(r.ok, JSON.stringify(r)).toEqual({ builds: [build.id] });
        return until(() => events.slice(seen).find(e => e.event === 'selection'), 30_000, `the move up ${n}`);
      };
      const lands = async (id, n, at, stepped = n) => {
        expect((await pick(id)).source.line).toBe(line(id));
        const s = await up(n);
        expect([page_, id, n, s.source.line, s.moved]).toEqual([page_, id, n, line(at), { up: n, stepped }]);
      };
      await lands('body', 1, 'card');           // (a) a caller's fill: its slot's host
      await lands('glyph', 1, 'card');          // the like row: Controls holds no view; the host
      await lands('panel-fill', 1, 'frame');    // (b) Panel only renders Frame: Frame's box
      await lands('wrapped', 1, 'card');        // (c) the nearer host, then its wrapper
      await lands('wrapped', 2, 'outer');
      await lands('glyph', 9, 'root', 3);       // (d) Card, Post, App: the root, three of nine
      // One rule (stage 15 D3, theme2's condition 1): a selection's chain is the page's own walk, so each entry is where
      // a move up of its index plus one lands, here on the page: its component, and a slot's host by its own call.
      const walks = async (id, edge = false) => {
        const chain = (await pick(id, edge)).source.chain;
        expect([page_, id, chain.length > 0]).toEqual([page_, id, true]);
        for (const [k, entry] of chain.entries()) {
          if (k) await pick(id, edge);
          const s = await up(k + 1);
          expect([page_, id, k + 1, s.source.component, s.moved.stepped]).toEqual([page_, id, k + 1, entry.component, k + 1]);
          // (A slot's host is named by its own call: that line of the source calls it.)
          if (entry.slot) expect([page_, id, k + 1, SOURCE.split('\n')[entry.line - 1].trim()]).toEqual([page_, id, k + 1, `${entry.component}()`]);
        }
        await pick(id, edge);
        expect((await up(chain.length + 1)).moved).toEqual({ up: chain.length + 1, stepped: chain.length });
        return chain.map(e => `${e.component}${e.slot ? ' (slot)' : ''}`);
      };
      expect(await walks('body')).toEqual(['Card (slot)', 'App']);
      expect(await walks('glyph')).toEqual(['Card (slot)', 'Post', 'App']);
      expect(await walks('panel-fill')).toEqual(['Frame (slot)']);
      expect(await walks('wrapped')).toEqual(['Card (slot)', 'Outer (slot)']);
      // No fill on the way: the chain as written (Card's own box, in Post).
      expect(await walks('card', true)).toEqual(['Post', 'App']);
      // A rebuild: the new plan's copies come with inspect mode as the page registers again; a move steps as before.
      const before = (await post({ method: 'hello' })).ok.builds.find(b => b.id === build.id)?.planDigest;
      writeFileSync(join(APP, 'app.contract'), SOURCE.replace('text "body" testId="body"', 'text "body!" testId="body"'));
      await until(async () => { const b = (await post({ method: 'hello' })).ok?.builds?.find(x => x.host.startsWith('web')); return b && b.planDigest && b.planDigest !== before && b; }, 600_000, 'the rebuilt plan');
      await page.locator('[data-testid="body"]', { hasText: 'body!' }).waitFor({ timeout: 120_000 });
      await lands('glyph', 1, 'card');
      writeFileSync(join(APP, 'app.contract'), SOURCE);
      state.polling = false;
      await poll.catch(() => {});
      await page.close();
      await loop.stop();
    }
  } finally {
    for (const p of polls) p.polling = false;
    await browser.close();
    for (const stop of stops) await stop();
  }
}, 1_800_000);
