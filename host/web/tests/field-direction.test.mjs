// LLP 1001 §1 in a real browser, the definition Apple's plain fields follow (FieldDirectionMacTests,
// FieldDirectionIOSTests): an `input` or a `textarea`, of either appearance, takes CSS `direction` as its
// text's base direction (`unicode-bidi: normal`, no `dir="auto"`, so no first strong character picks it),
// and an unsaid `text-align` is `start`, the right under `rtl`; one said stands. The app is compiled from
// Contract and built for the JS target, as `exact` builds one.
import { expect } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { cdpBrowser, chromium } from '../../../scripts/agent-launch.mjs';
import { decodePng } from '../../../scripts/png.mjs';
import { serveBuildTree } from '../serve.mjs';
import { need } from '../../../scripts/coverage.mjs';

const ROOT = resolve(new URL('../../..', import.meta.url).pathname);
const compiler = resolve(process.env.CARGO_TARGET_DIR ?? resolve(ROOT, 'target'), 'debug', process.platform === 'win32' ? 'contract.exe' : 'contract');
const { unavailable } = chromium();
const check = need('chrome', unavailable);

// Each field: its direction, its said alignment if any, and whether it is `appearance: none`.
const FIELDS = [
  ['in-rtl', 'input', 'rtl', '', false], ['in-ltr', 'input', 'ltr', '', false], ['in-rtl-left', 'input', 'rtl', 'left', false],
  ['ta-rtl', 'textarea', 'rtl', '', false], ['ta-ltr', 'textarea', 'ltr', '', false], ['ta-rtl-left', 'textarea', 'rtl', 'left', false],
  ['bare-rtl', 'textarea', 'rtl', '', true], ['bare-rtl-left', 'textarea', 'rtl', 'left', true],
];
const APP = `component App
  view
    column padding=20 gap=8 width="100%"
${FIELDS.map(([id, kind, dir, align, bare]) => `      ${kind} value="Hi" direction="${dir}"${align ? ` text-align="${align}"` : ''}${bare ? ' appearance="none"' : ''} aria-label="${id}" testId="${id}" width=300 height=${kind === 'input' ? 28 : 48}`).join('\n')}
`;

check('a plain field takes CSS direction as its base, and an unsaid text-align is start (the right under rtl)', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-field-direction-')), contract = join(dir, 'app.contract'), plan = join(dir, 'app.plan'), dist = join(dir, 'dist');
  writeFileSync(contract, APP);
  const compile = spawnSync(compiler, ['build', contract, '-o', plan], { cwd: ROOT, encoding: 'utf8' });
  if (compile.status !== 0) throw new Error(compile.stderr);
  const build = spawnSync(process.execPath, ['host/web-js/build.mjs', 'caltrain', '--plan', plan, '--out', dist, '--render', 'none'], { cwd: ROOT, encoding: 'utf8' });
  if (build.status !== 0) throw new Error(build.stderr);
  const server = createServer((request, response) => serveBuildTree(dist, request, response));
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const suite = await cdpBrowser({ args: ['--window-size=800,800'] }), { cdp } = suite;
  try {
    const { sessionId } = await suite.page();
    const call = (method, params) => cdp.send(method, params, sessionId);
    const evaluate = async (expression) => {
      const reply = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
      if (reply.exceptionDetails) throw new Error(reply.exceptionDetails.exception?.description ?? reply.exceptionDetails.text);
      return reply.result.value;
    };
    await call('Emulation.setDeviceMetricsOverride', { width: 800, height: 800, deviceScaleFactor: 1, mobile: false });
    await call('Page.navigate', { url: `http://127.0.0.1:${server.address().port}/` });
    for (let i = 0; !(await evaluate(`!!document.querySelector('[data-testid=bare-rtl-left]')`)); i++) { if (i > 4000) throw new Error('the page never showed'); await Bun.sleep(5); }
    await evaluate('new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))');
    const facts = await evaluate(`(() => Object.fromEntries(${JSON.stringify(FIELDS.map(f => f[0]))}.map(id => {
      const e = document.querySelector('[data-testid=' + id + ']'), s = getComputedStyle(e), r = e.getBoundingClientRect();
      return [id, { tag: e.tagName, direction: s.direction, bidi: s.unicodeBidi, dirAttr: e.getAttribute('dir'), align: s.textAlign, box: [r.x, r.y, r.width, r.height] }];
    })))()`);
    const { data } = await call('Page.captureScreenshot', { format: 'png' });
    const shot = decodePng(Buffer.from(data, 'base64'));
    for (const [id, kind, direction, align] of FIELDS) {
      const f = facts[id];
      expect(f.tag, id).toBe(kind.toUpperCase());
      // The base: the CSS direction, plainly; `plaintext` or `dir="auto"` would let the first strong character pick it.
      expect(f.direction, id).toBe(direction);
      expect(f.bidi, id).toBe('normal');
      expect(f.dirAttr === null || f.dirAttr === direction, `${id}: dir=${f.dirAttr}`).toBe(true);
      // Where "Hi" draws: the ink's middle within the field's inner box (its line and corners aside).
      const [x, y, w, h] = f.box.map(Math.round), xs = [];
      const ground = (() => { const i = ((y + h - 4) * shot.width + (x + Math.round(w / 2))) * 4; return [...shot.data.slice(i, i + 3)]; })();
      for (let py = y + 3; py < y + h - 3; py++) {
        const ink = [];
        for (let px = x + 3; px < x + w - 3; px++) { const i = (py * shot.width + px) * 4; if ([0, 1, 2].some(c => Math.abs(shot.data[i + c] - ground[c]) > 60)) ink.push(px); }
        if (ink.length < (w - 6) / 2) xs.push(...ink);
      }
      expect(xs.length, `${id}: "Hi" drew`).toBeGreaterThan(0);
      const middle = xs.reduce((a, b) => a + b, 0) / xs.length - x, right = (align || (direction === 'rtl' ? 'right' : 'left')) === 'right';
      if (right) expect(middle, `${id} (${direction}${align ? `, said ${align}` : ', unsaid'}): at the right`).toBeGreaterThan(w / 2);
      else expect(middle, `${id} (${direction}${align ? `, said ${align}` : ', unsaid'}): at the left`).toBeLessThan(w / 2);
    }
  } finally {
    await suite.close();
    server.close();
    rmSync(dir, { recursive: true, force: true });
  }
}, 120000);
