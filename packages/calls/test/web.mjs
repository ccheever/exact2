#!/usr/bin/env bun
/**
 * `@exact/calls` on the web (LLP 1133.033 §12 stage 1): Harmony's test call in a
 * browser with the web implementation bound and no agent, against a
 * `snapback4 dev --memory` server of this run's own.
 *
 * The browser calls a Media Session action handler only for a press on its own
 * controls, which no script can make, so the page's `setActionHandler` is
 * wrapped to keep each handler, and the drive calls the kept one as the
 * browser would. Checked: while the call is connected `hangup` and
 * `togglemicrophone` are set (where the browser has them); `togglemicrophone`
 * mutes the call (`callmute`); `hangup` ends it with nothing played
 * (`callhangup`) and clears both. A browser without the actions still makes
 * the call.
 *
 * usage: bun packages/calls/test/web.mjs [chromium|firefox|webkit]
 */
import { spawn, spawnSync } from 'node:child_process';
import { createServer } from 'node:http';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(fileURLToPath(new URL('../../..', import.meta.url)));
const APP = resolve(ROOT, 'apps/harmony/app'), BACKEND = resolve(APP, '..');
const engine = process.argv[2] ?? 'chromium';
let failures = 0;
const check = (name, ok, detail) => {
  if (!ok) failures++;
  console.log(`${ok ? 'PASS' : 'FAIL'} ${engine}: ${name}${ok ? '' : `: ${JSON.stringify(detail)}`.slice(0, 1000)}`);
};

// The backend, on a port of its own.
const server = spawn('bunx', ['snapback4', 'dev', '--memory', '--port', '0'], { cwd: BACKEND, stdio: ['ignore', 'pipe', 'pipe'] });
const origin = await new Promise((ok, fail) => {
  let text = '';
  const read = (b) => { text += b; const m = /SNAPBACK4_READY .*url=(\S+)/.exec(text); if (m) ok(m[1]); };
  server.stdout.on('data', read);
  server.stderr.on('data', read);
  server.on('exit', (code) => fail(new Error(`snapback4 dev exited ${code}`)));
});
const dist = resolve(ROOT, 'target/calls-web');
try {
  const { resolveApp } = await import(resolve(ROOT, 'scripts/app.mjs'));
  const built = spawnSync(process.execPath, ['host/web/build.mjs', resolveApp(APP).crate('web')], { cwd: ROOT, stdio: 'inherit',
    env: { ...process.env, EXACT_BACKEND_ORIGIN: origin, EXACT_WEB_DIST: dist, EXACT_APP_DIR: APP } });
  if (built.status !== 0) throw new Error(`the web build exited ${built.status}`);
  const { serveBuildTree } = await import(resolve(ROOT, 'host/web/serve.mjs'));
  const pages = createServer((req, res) => serveBuildTree(dist, req, res));
  await new Promise((ok) => pages.listen(0, '127.0.0.1', ok));
  const engines = await import(resolve(ROOT, 'node_modules/playwright-core/index.mjs'));
  // A fake microphone, granted.
  const browser = await engines[engine].launch(engine === 'chromium' ? { args: ['--use-fake-ui-for-media-stream', '--use-fake-device-for-media-stream'] }
    : engine === 'firefox' ? { firefoxUserPrefs: { 'media.navigator.streams.fake': true, 'media.navigator.permission.disabled': true, 'media.autoplay.default': 0 } } : {});
  try {
    const context = await browser.newContext(engine === 'firefox' ? {} : { permissions: ['microphone'] });
    await context.addInitScript(() => {
      const kept = (globalThis.__mediaActions = {});
      const ms = navigator.mediaSession;
      if (!ms?.setActionHandler) return;
      const set = (globalThis.__setAction = ms.setActionHandler.bind(ms));
      ms.setActionHandler = (action, handler) => { set(action, handler); if (handler) kept[action] = handler; else delete kept[action]; };
    });
    const page = await context.newPage();
    page.on('pageerror', (e) => console.log(`page error: ${e.message}`));
    await page.goto(`http://127.0.0.1:${pages.address().port}/`);
    await page.getByTestId('tab-you').click({ timeout: 60000 });
    await page.getByTestId('test-call').click({ timeout: 30000 });
    const status = page.getByTestId('test-call-status');
    await status.waitFor({ timeout: 10000 });
    check('the test call is placed', true);
    await page.getByText('Say something').waitFor({ timeout: 10000 }).catch(() => {});
    const actions = await page.evaluate(() => Object.keys(globalThis.__mediaActions ?? {}).sort().join(' '));
    // The actions this browser has: the unwrapped setter refuses one it lacks (it clears the browser's own; the kept
    // handler stays).
    const has = (await page.evaluate(() => ['hangup', 'togglemicrophone'].filter((a) => { try { globalThis.__setAction(a, null); return true; } catch { return false; } }))).join(' ');
    check(`connected: the actions this browser has are set (${has || 'none'})`, actions === has, actions);
    if (has.includes('togglemicrophone')) {
      await page.evaluate(() => globalThis.__mediaActions.togglemicrophone({ action: 'togglemicrophone' }));
      await page.getByTestId('test-call-muted').waitFor({ timeout: 5000 }).then(() => check('togglemicrophone mutes the call', true), (e) => check('togglemicrophone mutes the call', false, e.message));
    }
    if (has.includes('hangup')) {
      await page.evaluate(() => globalThis.__mediaActions.hangup({ action: 'hangup' }));
      await status.waitFor({ state: 'detached', timeout: 10000 }).then(() => check('hangup ends the call', true), (e) => check('hangup ends the call', false, e.message));
      check('nothing played', (await page.getByTestId('test-call-playback').count()) === 0, 'a take plays');
    } else {
      // Without hangup the call ends after its take plays back. A headless Firefox or WebKit plays no sound, so its
      // <audio> never ends: once the take is playing, its `ended` is dispatched.
      const playing = await page.getByText('Playing back').waitFor({ timeout: 30000 }).then(() => true, () => false);
      check('the take is recorded and plays back', playing, await status.textContent().catch(() => null));
      if (playing) await page.getByTestId('test-call-playback').evaluate((a) => { if (!a.ended) a.dispatchEvent(new Event('ended')); }).catch(() => {});
      await status.waitFor({ state: 'detached', timeout: 10000 }).then(() => check('the call ends after its playback', true), (e) => check('the call ends after its playback', false, e.message));
    }
    const after = await page.evaluate(() => Object.keys(globalThis.__mediaActions ?? {}).join(' '));
    check('ended: every action cleared', after === '', after);
    await context.close();
  } finally {
    await browser.close();
    pages.close();
  }
} finally {
  server.kill();
}
process.exit(failures ? 1 : 0);
