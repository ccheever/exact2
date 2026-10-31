// LLP 1005 §3 in a real browser, the definition every host follows (PreventFocusMacTests,
// PreventFocusIOSTests): a `pointerdown` action that calls `preventDefault()` keeps the focus where it
// is, places no caret and starts no selection, and the press still fires, by mouse and by touch, as
// Chrome does for a prevented pointerdown. scripts/fixtures/prevent-focus.contract, built for the JS target.
import { expect } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
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

check('a prevented pointerdown keeps the focus and the press, by mouse and by touch', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-prevent-focus-')), plan = join(dir, 'app.plan'), dist = join(dir, 'dist');
  const compile = spawnSync(compiler, ['build', resolve(ROOT, 'scripts/fixtures/prevent-focus.contract'), '-o', plan], { cwd: ROOT, encoding: 'utf8' });
  if (compile.status !== 0) throw new Error(compile.stderr);
  const build = spawnSync(process.execPath, ['host/web-js/build.mjs', 'caltrain', '--plan', plan, '--out', dist, '--render', 'none'], { cwd: ROOT, encoding: 'utf8' });
  if (build.status !== 0) throw new Error(build.stderr);
  const server = createServer((request, response) => serveBuildTree(dist, request, response));
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const suite = await cdpBrowser({ args: ['--window-size=800,600'] }), { cdp } = suite;
  try {
    const { sessionId } = await suite.page();
    const call = (method, params) => cdp.send(method, params, sessionId);
    const evaluate = async (expression) => {
      const reply = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
      if (reply.exceptionDetails) throw new Error(reply.exceptionDetails.exception?.description ?? reply.exceptionDetails.text);
      return reply.result.value;
    };
    const frames = () => evaluate('new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))');
    await call('Emulation.setDeviceMetricsOverride', { width: 800, height: 600, deviceScaleFactor: 1, mobile: false });
    await call('Page.navigate', { url: `http://127.0.0.1:${server.address().port}/` });
    for (let i = 0; !(await evaluate(`!!document.querySelector('[data-testid=heard]')`)); i++) { if (i > 4000) throw new Error('the page never showed'); await Bun.sleep(5); }
    await frames();
    const testid = `(document.activeElement?.closest('[data-testid]')?.dataset.testid ?? document.activeElement?.tagName)`;
    const heard = () => evaluate(`document.querySelector('[data-testid=heard]').textContent.trim()`);
    const at = (id) => evaluate(`(() => { const r = document.querySelector('[data-testid=${id}]').getBoundingClientRect(); return [r.x + 12, r.y + r.height / 2]; })()`);
    for (const touch of [false, true]) {
      if (touch) await call('Emulation.setTouchEmulationEnabled', { enabled: true, maxTouchPoints: 1 });
      const press = async (id) => {
        const [x, y] = await at(id);
        if (touch) {
          await call('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x, y }] });
          await call('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
        } else {
          for (const type of ['mousePressed', 'mouseReleased']) await call('Input.dispatchMouseEvent', { type, x, y, button: 'left', clickCount: 1 });
        }
        await Bun.sleep(touch ? 400 : 100);
        await frames();
      };
      const how = touch ? 'touch' : 'mouse';
      await evaluate(`document.querySelector('[data-testid=a]').focus(); document.querySelector('[data-testid=a]').setSelectionRange(2, 2)`);
      await frames();
      for (const id of ['b', 'box', 'btn']) {
        const before = await heard();
        await press(id);
        expect(await evaluate(testid), `${how} on ${id}: the focus stays on a`).toBe('a');
        expect(await evaluate(`[document.activeElement.selectionStart, document.activeElement.selectionEnd]`), `${how} on ${id}: a's caret stays`).toEqual([2, 2]);
        if (id !== 'b') expect((await heard()).slice(before.length), `${how} on ${id}: the press still fires`).toContain(`press-${id}`);
      }
      for (const id of ['b', 'box', 'btn']) expect(await heard(), `${how}: ${id} never took the focus`).not.toContain(`focus-${id}`);
      await press('c');
      expect(await evaluate(testid), `${how} on c, which does not prevent: the focus moves`).toBe('c');
    }
  } finally {
    await suite.close();
    server.close();
    rmSync(dir, { recursive: true, force: true });
  }
}, 120000);
