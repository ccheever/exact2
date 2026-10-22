// LLP 1116.004 D6: the web runtime under a page's fake clock, installed before
// the page loads (Playwright's `clock.install`, `setFixedTime`; sinon's alike).
// Its Date and Intl.DateTimeFormat are fakes over the engine's prototypes, and
// a data module's guards (the JS target's ts-fetch.js, the wasm target's
// prelude) read `format`'s getter off the fake's prototype, found none, and
// the page drew nothing (bake-off round 6, water-tracker). Now the app starts,
// the guards still refuse an omitted date, and moving the fake clock fires the
// app's timers as the real one would.
import { afterAll, expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve, sep } from 'node:path';
import { chromium as installed, suiteBrowser } from '../../../scripts/agent-launch.mjs';
import { need } from '../../../scripts/coverage.mjs';

const ROOT = resolve(new URL('../../..', import.meta.url).pathname);
const chrome = installed();
const check = need('chrome', chrome.unavailable);
const dir = mkdtempSync(resolve(tmpdir(), 'exact-fake-clock-')), dist = resolve(dir, 'dist');
let served = null, browser = null;
// Chrome takes seconds to quit once it has opened a page.
afterAll(async () => { await browser?.close(); served?.stop(true); rmSync(dir, { recursive: true, force: true }); }, 30_000);

// An app with a data module (the guards are its) and a minute timer that reads the clock.
function serve() {
  writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Clock probe', app: { id: 'test.clock-probe', name: 'Clock probe' }, host: { web: {} } }));
  writeFileSync(resolve(dir, 'app.contract'), `shape Time
  epochAtZero: number
  utcOffset: number
shape Result
  value: string
component Probe
  state now = 0
  state ticks = 0
  resource time = exactTime() as shape Time
  resource result = probe() as shape Result
  derive at = time.epochAtZero + now
  task tick mount
    every(60000, step)
  action step
    now = performanceNow()
    ticks = ticks + 1
  view
    view
      text result.value testId="result"
      text formatDate(at, time.utcOffset, "iso") testId="day"
      text \`\${ticks}\` testId="ticks"
`);
  writeFileSync(resolve(dir, 'app.ts'), `import type { Answer, Sources } from './app.contract.d.ts';
export const appId = 'test.clock-probe', grants = '';
const attempt = (f: () => unknown) => { try { return String(f()); } catch { return 'refused'; } };
const sources: Sources = { probe: () => ({ value: [
  attempt(() => new Intl.DateTimeFormat('en-US', { timeZone: 'UTC', year: 'numeric' }).format(Date.UTC(2020, 5, 1))),
  attempt(() => new Date(86_400_000).toISOString().slice(0, 10)),
  attempt(() => new Intl.DateTimeFormat('en-US').format()),
  attempt(() => new Intl.DateTimeFormat('en-US').formatToParts().length),
].join('|') }) };
export const answer: Answer = (source, args, store, storage, native) => sources[source](args as never, store, storage, native) as never;
`);
  const built = spawnSync(process.execPath, ['host/web-js/build.mjs', 'clock-probe', '--out', dist, '--render', 'none'], { cwd: ROOT, encoding: 'utf8', env: { ...process.env, EXACT_APP_DIR: dir } });
  expect(built.status, built.stderr || built.stdout).toBe(0);
  return Bun.serve({ port: 0, fetch(request) {
    const path = new URL(request.url).pathname, file = resolve(dist, path === '/' ? 'index.html' : decodeURIComponent(path.slice(1)));
    return file.startsWith(dist + sep) && existsSync(file) ? new Response(Bun.file(file)) : new Response('not found', { status: 404 });
  } });
}

async function open(fake) {
  served ??= serve();
  browser ??= await suiteBrowser();
  const context = await browser.newContext({ timezoneId: 'UTC' }), page = await context.newPage(), errors = [];
  page.on('pageerror', error => errors.push(String(error)));
  await fake(page.clock);
  await page.goto(served.url.href);
  const text = id => page.evaluate(id => document.querySelector(`[data-testid="${id}"]`)?.textContent ?? null, id);
  // Real time between reads: a paused fake clock never moves on its own.
  const until = async (id, want) => {
    let seen;
    for (const end = Date.now() + 10_000; Date.now() < end; await Bun.sleep(20)) if ((seen = await text(id)) === want) return;
    throw new Error(`${id}: ${JSON.stringify(seen)}, not ${JSON.stringify(want)}; page errors: ${errors.join('; ') || 'none'}`);
  };
  return { page, until, errors, close: () => context.close() };
}

const before = Date.parse('2026-10-08T23:59:30Z');

check('the app starts under clock.install, and runFor and fastForward fire its timers', async () => {
  const { page, until, errors, close } = await open(async clock => { await clock.install({ time: before }); await clock.pauseAt(before); });
  try {
    await until('result', '2020|1970-01-02|refused|refused'); // an explicit date formats; an omitted one is still refused
    await until('day', '2026-10-08');
    await until('ticks', '0');
    await page.clock.runFor(61_000);
    await until('ticks', '1');
    await until('day', '2026-10-09');
    await page.clock.fastForward(120_000); // the minute repeat's two missed beats fire once
    await until('ticks', '2');
    expect(errors).toEqual([]);
  } finally { await close(); }
}, 60_000);

check('the app starts under setFixedTime', async () => {
  const { until, errors, close } = await open(clock => clock.setFixedTime(before));
  try {
    await until('result', '2020|1970-01-02|refused|refused');
    await until('day', '2026-10-08');
    expect(errors).toEqual([]);
  } finally { await close(); }
}, 60_000);

// The wasm target's module realm under the same fake: the executor's prelude
// (js/src/prelude.js, evaluated in that realm's iframe, which the fake covers
// too) guards the engine's DateTimeFormat, and puts it in place of the fake's,
// whose `format()` would answer an omitted date from the fake clock.
check("the module realm's prelude installs its guards under clock.install", async () => {
  browser ??= await suiteBrowser();
  const context = await browser.newContext(), page = await context.newPage();
  try {
    await page.clock.install({ time: before });
    await page.goto('data:text/html,<p>realm</p>');
    const answers = await page.evaluate(source => {
      const attempt = f => { try { return String(f()); } catch (error) { return error.message.replace(/ is unavailable in data sources.*/, ' refused'); } };
      return [attempt(() => { (0, eval)(source); return 'loaded'; }),
        attempt(() => new Intl.DateTimeFormat('en-US', { timeZone: 'UTC', year: 'numeric' }).format(Date.UTC(2020, 5, 1))),
        attempt(() => new Date(86_400_000).toISOString().slice(0, 10)), attempt(() => new Date()), attempt(() => Date.now()),
        attempt(() => new Intl.DateTimeFormat('en-US').format()), attempt(() => new Intl.DateTimeFormat('en-US').formatToParts())];
    }, readFileSync(resolve(ROOT, 'js/src/binary.js'), 'utf8') + '\n' + readFileSync(resolve(ROOT, 'js/src/prelude.js'), 'utf8'));
    expect(answers).toEqual(['loaded', '2020', '1970-01-02', 'new Date() refused', 'Date.now() refused',
      'Intl.DateTimeFormat.format() refused', 'Intl.DateTimeFormat.formatToParts() refused']);
  } finally { await context.close(); }
}, 60_000);
