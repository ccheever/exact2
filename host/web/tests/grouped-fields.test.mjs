// LLP 1116.005 D2 in a real browser: a field in a grouped list's row is each platform's field there, and the web's is a
// visible field (its 1 px line, 8 px corners, at least 36 px tall, the browser's focus ring) that fits its row, in light
// and dark, at a phone's width and a desktop's. `appearance="none"` stays the author's bare box: no line, no corners,
// its text's height. The app is compiled from Contract and built for the JS target, as `exact` builds one.
import { test, expect } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { cdpBrowser, chromium } from '../../../scripts/agent-launch.mjs';
import { serveBuildTree } from '../serve.mjs';
import { need } from '../../../scripts/coverage.mjs';

const ROOT = resolve(new URL('../../..', import.meta.url).pathname);
const compiler = resolve(process.env.CARGO_TARGET_DIR ?? resolve(ROOT, 'target'), 'debug', process.platform === 'win32' ? 'contract.exe' : 'contract');
const { unavailable } = chromium();
const check = need('chrome', unavailable);

const FORM = `component Form
  state name = ""
  state notes = ""
  action editName(v: string)
    name = v
  action editNotes(v: string)
    notes = v
  view
    list appearance="auto" listStyle="inset-grouped" height="100%" testId="form"
      section
        row
          column flex=1 min-width=0 gap=4
            input value=name input=editName placeholder="Name" aria-label="Name" testId="unsaid"
        row
          text "Email"
          input placeholder="name@example.com" aria-label="Email" testId="beside" flex=1 min-width=0
        row
          input appearance="none" placeholder="Bare" aria-label="Bare" testId="bare" flex=1 min-width=0
      section
        header
          text "Notes"
        row padding-top=8 padding-bottom=8
          textarea value=notes input=editNotes placeholder="Optional" aria-label="Notes" rows=3 testId="area" flex=1 min-width=0
`;

/** The form, compiled and built for the JS target, served; `test(evaluate, size, scheme)` in headless Chrome. */
async function inChrome(test) {
  const dir = mkdtempSync(join(tmpdir(), 'exact-grouped-fields-')), contract = join(dir, 'app.contract'), plan = join(dir, 'app.plan'), dist = join(dir, 'dist');
  writeFileSync(contract, FORM);
  const compile = spawnSync(compiler, ['build', contract, '-o', plan], { cwd: ROOT, encoding: 'utf8' });
  if (compile.status !== 0) throw new Error(compile.stderr);
  const build = spawnSync(process.execPath, ['host/web-js/build.mjs', 'caltrain', '--plan', plan, '--out', dist, '--render', 'none'], { cwd: ROOT, encoding: 'utf8' });
  if (build.status !== 0) throw new Error(build.stderr);
  const server = createServer((request, response) => serveBuildTree(dist, request, response));
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const suite = await cdpBrowser({ args: ['--window-size=1400,900'] }), { cdp } = suite;
  try {
    const { sessionId } = await suite.page();
    const call = (method, params) => cdp.send(method, params, sessionId);
    const evaluate = async (expression) => {
      const reply = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
      if (reply.exceptionDetails) throw new Error(reply.exceptionDetails.exception?.description ?? reply.exceptionDetails.text);
      return reply.result.value;
    };
    const frames = () => evaluate('new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))');
    const size = async (width) => { await call('Emulation.setDeviceMetricsOverride', { width, height: 800, deviceScaleFactor: 1, mobile: false }); await frames(); };
    const scheme = async (value) => { await call('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value }] }); await frames(); };
    await size(390);
    await call('Page.navigate', { url: `http://127.0.0.1:${server.address().port}/` });
    for (let i = 0; !(await evaluate(`!!document.querySelector('[data-testid=area]')`)); i++) { if (i > 4000) throw new Error('the form never showed'); await Bun.sleep(5); }
    await frames();
    await test(evaluate, size, scheme);
  } finally {
    await suite.close();
    server.close();
    rmSync(dir, { recursive: true, force: true });
  }
}

// Each field's line, corners, box and background, its row's box, and the card's background behind it.
const FACTS = `(() => Object.fromEntries(['unsaid', 'beside', 'bare', 'area'].map(id => {
  const e = document.querySelector('[data-testid=' + id + ']'), s = getComputedStyle(e), r = e.getBoundingClientRect();
  const row = e.closest('[data-groupedrowseparator]');
  const rr = row.getBoundingClientRect();
  return [id, { line: s.borderTopWidth + ' ' + s.borderTopStyle, color: s.borderTopColor, radius: s.borderTopLeftRadius,
    height: r.height, width: r.width, inRow: r.left >= rr.left && r.right <= rr.right && r.top >= rr.top && r.bottom <= rr.bottom,
    background: s.backgroundColor, card: getComputedStyle(row.parentElement).backgroundColor }];
})))()`;

check('a field in a grouped row is a visible field on the web; appearance="none" is the bare box', () => inChrome(async (evaluate, size, scheme) => {
  for (const width of [390, 1280]) {
    await size(width);
    for (const mode of ['light', 'dark']) {
      await scheme(mode);
      const facts = await evaluate(FACTS);
      // Apple's continuous curve over the radius scaled to its reach, where the engine draws `corner-shape`.
      const radius = await evaluate(`CSS.supports('corner-shape', 'superellipse(2)') ? '12.16px' : '8px'`);
      for (const id of ['unsaid', 'beside', 'area']) {
        const f = facts[id], at = `${id} at ${width} in ${mode}`;
        expect(f.line, at).toBe('1px solid');
        expect(f.radius, at).toBe(radius);
        expect(f.height, at).toBeGreaterThanOrEqual(36);
        expect(f.inRow, `${at} fits its row`).toBe(true);
        expect(f.color, `${at}: its line shows on the card`).not.toBe(f.card);
      }
      // (A section is at most 720 px wide on the web, LLP 1115.000 D4.16.)
      expect(facts.unsaid.width, 'a field in a column fills the row').toBeGreaterThan(width === 390 ? 250 : 620);
      expect(facts.bare.line, `bare at ${width} in ${mode}`).toBe('0px none');
      expect(facts.bare.radius).toBe('0px');
      expect(facts.bare.height).toBeLessThan(30);
    }
  }
  // The browser's own focus ring on the platform's field; the light line under it is the field's.
  await evaluate(`document.querySelector('[data-testid=unsaid]').focus()`);
  const ring = await evaluate(`(() => { const s = getComputedStyle(document.querySelector('[data-testid=unsaid]')); return [s.outlineStyle, s.outlineWidth]; })()`);
  expect(ring[0]).not.toBe('none');
  expect(parseFloat(ring[1])).toBeGreaterThan(0);
}), 120000);
