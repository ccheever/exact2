// tamber/1.16 (stage 13 E, the design tool's second hands-on pass; theme2's 09:29 and 10:03 rulings) on both web
// pages: an inherited row names the node it comes from by its source, and `inspect` `select` moves the selection up
// the call-site chain, past the outermost to the root.
import { afterEach, expect, test } from 'bun:test';
import { mkdtempSync, rmSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnOwned, stopAll } from './test-children.mjs';
import { suiteBrowser } from './agent-launch.mjs';

const ROOT = resolve(new URL('..', import.meta.url).pathname);
afterEach(() => stopAll());

/** A dev loop on `app` (`flags`: `--wasm` for the wasm page), as scripts/tamber-server.test.mjs starts one. */
async function devLoop(flags = [], app = 'theme-fixture') {
  const listener = createServer();
  await new Promise(ok => listener.listen(0, '127.0.0.1', ok));
  const port = listener.address().port;
  await new Promise(ok => listener.close(ok));
  const dist = mkdtempSync(join(tmpdir(), 'exact-tamber-pages-'));
  const dev = spawnOwned(process.execPath, [resolve(ROOT, 'host/web/dev.mjs'), '--app', app, '--port', String(port), ...flags], { cwd: ROOT, env: { ...process.env, EXACT_WEB_DIST: dist }, stdio: ['ignore', 'pipe', 'pipe'] });
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

test('tamber/1.16 on both web pages: an inherited row\'s fromAt, and select up to the root', async () => {
  const browser = await suiteBrowser();
  const polls = [], stops = [];
  try {
    for (const flags of [['--wasm'], []]) {
      const loop = await devLoop(flags, 'theme-fixture'), { base, said } = loop;
      stops.push(loop.stop);
      const { token } = await (await fetch(`${base}/__tamber/token`)).json();
      const post = async (body, path = '/__tamber') => (await fetch(`${base}${path}`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: JSON.stringify(body) })).json();
      const events = [], state = { polling: true };
      polls.push(state);
      const poll = (async () => { let after = 0, epoch; while (state.polling) { const r = await post(epoch ? { after, epoch, client: 'tool' } : { after: 0, wait: false, client: 'tool' }, '/__tamber/events'); events.push(...(r.events ?? [])); after = r.seq ?? after; epoch = r.epoch ?? epoch; await new Promise(ok => setTimeout(ok, 100)); } })();
      const until = async (pass, ms = 30_000, what = '') => { const end = Date.now() + ms; for (;;) { const got = await pass(); if (got) return got; if (Date.now() > end) throw new Error(`not seen: ${what} on ${base}`); await new Promise(ok => setTimeout(ok, 100)); } };
      const page = await browser.newPage({ viewport: { width: 420, height: 900 } });
      await page.goto(`${base}/`);
      await until(() => said().includes('plan ready'), 300_000, 'plan ready');
      const build = await until(async () => (await post({ method: 'hello' })).ok?.builds?.find(b => b.host.startsWith('web')), 120_000, 'the build');
      expect((await post({ method: 'inspect', client: 'tool', on: true })).ok.builds).toEqual([build.id]);
      await page.locator('#exact-inspect').waitFor();
      // The card's text colour comes from the page's scroll (app.contract:15), named so.
      const box = await page.locator('[data-testid="card"]').first().boundingBox();
      let seen = events.length;
      await page.mouse.click(box.x + 12, box.y + box.height / 2);
      const selection = await until(() => events.slice(seen).find(e => e.event === 'selection'), 30_000, 'the card selected');
      const color = selection.rows.find(r => r.name === 'text_color');
      expect([flags.join(''), color.source, color.fromAt?.component, color.fromAt?.line, String(color.fromAt?.file).endsWith('app.contract')]).toEqual([flags.join(''), 'inherited', 'App', 15, true]);
      expect('fromSite' in color).toBe(false);
      // (#7) While the selection's copies are outlined, the banner says what the dashed boxes are.
      await until(async () => (await page.locator('#exact-inspect').innerText()).includes('outlined in dashes (the selection\'s every copy, or what the tool outlined)'), 30_000, 'the legend');
      // Up one call site: the fixture has one component, so the root, its outermost node, as a click there selects it.
      seen = events.length;
      expect((await post({ method: 'inspect', client: 'tool', build: build.id, select: { up: 1 } })).ok).toEqual({ builds: [build.id] });
      const up = await until(() => events.slice(seen).find(e => e.event === 'selection'), 30_000, 'the root selected');
      expect([flags.join(''), up.source.line, up.source.component]).toEqual([flags.join(''), 15, 'App']);
      // Another client may not move it.
      expect((await post({ method: 'inspect', client: 'other', build: build.id, select: { up: 1 } })).error.message).toContain('inspect is not on for');
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
}, 900_000);
