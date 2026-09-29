// The auth fixture, driven (LLP 1069.006 D7): sign-in with PAR, DPoP and
// PKCE against the local fixture server (`apps/auth-fixture/fixture.mjs`),
// on the web, macOS and iOS under the agent. Nothing opens: the request is
// held; the smoke asks the fixture's authorize endpoint for the callback URL
// a person's approval would produce and answers the hold with it (`type
// @t`). A cancel, a callback for another sign-in, and the pending entry's
// disclosure rules are checked too. `smoke.mjs` runs this for `--app
// auth-fixture`; it starts the fixture server when none answers.
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { Cdp } from './agent.mjs';
import { jsTargetBuild, serveBuildTree, serveStatic } from '../host/web/serve.mjs';

const ROOT = resolve(import.meta.dirname, '..');
const ISSUER = 'http://127.0.0.1:4331';
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function fixtureServer() {
  const up = () => fetch(`${ISSUER}/fixture/log`).then((r) => r.ok, () => false);
  if (await up()) return null;
  const child = spawn(process.execPath, [resolve(ROOT, 'apps/auth-fixture/fixture.mjs')], { stdio: 'ignore' });
  for (let i = 0; i < 50 && !(await up()); i++) await sleep(100);
  return child;
}

const held = (st) => (st.pending ?? []).find((p) => p.device?.capability === 'auth');
const text = async (s, id) => (await s.find(id).catch(() => null))?.props?.text ?? '';

export async function authSmoke({ host, open, check: record }) {
  let checks = 0, failed = 0;
  const check = (ok, what) => { checks += 1; if (!ok) failed += 1; return record(ok, `${host} auth: ${what}`); };
  const t0 = Date.now();
  const server = await fixtureServer();
  await fetch(`${ISSUER}/fixture/reset`);
  const s = await open({ host, app: 'auth-fixture' });
  try {
    await s.clock('settle');
    check((await text(s, 'session')) === 'Signed out', 'starts signed out');

    // Cancel: prepared, the press holds the request; `tap @t cancel` is 499.
    await s.tap('prepare'); await s.clock('settle');
    check((await text(s, 'prepared')) === 'Ready to sign in', `prepared: ${await text(s, 'prepared')}`);
    await s.tap('sign-in'); const settle = await s.clock('settle');
    let st = await s.state(), hold = held(st);
    check(settle.reason === 'device' && hold, `the press holds an auth request: ${JSON.stringify(settle)} ${JSON.stringify(st.pending)}`);
    const args = hold?.device?.args ?? {};
    check(args.origin === ISSUER && args.path === '/oauth/authorize' && JSON.stringify(args.params) === '["client_id","request_uri"]'
      && /^urn:ietf:params:oauth:request_uri:/.test(args.request_uri ?? '') && args.state?.length === 43,
      `pending shows the URL's origin, path, names and the two values, the callback and state: ${JSON.stringify(args)}`);
    const nativeCallback = 'test.exact.auth-fixture:/oauth';
    check(host === 'web' ? /^http:\/\/127\.0\.0\.1:\d+\/\.exact\/auth\/callback$/.test(args.callback) : args.callback === nativeCallback,
      `authCallback() is this carrier's: ${args.callback}`);
    await s.tap(`@${hold?.ticket}`, { choice: 'cancel' }); await s.clock('settle');
    check((await text(s, 'signed')) === 'Sign-in cancelled', `cancel is 499: ${await text(s, 'signed')}`);

    // Sign in: a fresh transaction; the fixture approves and the agent
    // answers with the callback URL it redirects to.
    await s.tap('prepare'); await s.clock('settle');
    await s.tap('sign-in'); await s.clock('settle');
    hold = held(await s.state());
    const a = hold?.device?.args ?? {};
    const approve = await fetch(`${ISSUER}/oauth/authorize?client_id=${encodeURIComponent(a.client_id)}&request_uri=${encodeURIComponent(a.request_uri)}`,
      { headers: { accept: 'application/json' } }).then((r) => r.json());
    check(typeof approve.location === 'string' && approve.location.startsWith(a.callback + '?'), `the fixture approves: ${JSON.stringify(approve)}`);
    // Another sign-in's callback is refused before the hold is spent.
    const wrong = approve.location.replace(/state=[^&]+/, 'state=someone-else');
    const refused = await s.type(`@${hold?.ticket}`, wrong).catch((e) => ({ error: String(e.message ?? e) }));
    check(/state is not the request's/.test(refused?.error ?? '') && !String(refused?.error).includes('code-'),
      `a callback for another sign-in is refused without echoing it: ${JSON.stringify(refused)}`);
    const answered = await s.type(`@${hold?.ticket}`, approve.location);
    check(answered?.delivery === 'substituted' && !JSON.stringify(answered).includes('code-'), `answered, substituted, never echoed: ${JSON.stringify(answered)}`);
    await s.clock('settle');
    const signed = await text(s, 'signed');
    check(signed === 'Signed in as fixture.test', `signed in: ${signed}`);
    check((await text(s, 'session')) === 'Signed in as fixture.test', `the session reads the kept tokens: ${await text(s, 'session')}`);
    const log = await fetch(`${ISSUER}/fixture/log`).then((r) => r.json());
    check(log.par === 2 && log.token === 1 && log.resource === 1 && log.pkce === 1 && log.dpopVerified >= 4 && log.nonceRetries >= 3,
      `the server verified PAR, PKCE and DPoP (with nonces): ${JSON.stringify(log)}`);
    const lines = (await s.logs()).lines;
    check(lines.some((l) => /device auth \d+ held \(agent\)/.test(l)) && lines.some((l) => /auth \d+: callback accepted/.test(l))
      && !lines.some((l) => l.includes('code-') || l.includes('at-')), 'the journal records the hold and the answer, never a code or token');
    st = await s.state();
    check(!JSON.stringify(st).includes('code-') && !JSON.stringify(st).includes('"at-'), 'state holds no code or token');
  } finally {
    await s.close?.();
    server?.kill();
  }
  console.log(`${host} auth: ${checks - failed} of ${checks} checks passed in ${((Date.now() - t0) / 1000).toFixed(1)} s (LLP 1069.006)`);
}

// The web's real path, outside the agent (LLP 1069.006 D4): Chrome with a
// fresh profile, the build served on loopback, real mouse presses (user
// activation) through CDP. The popup opens in the press's call stack; the
// fixture's page is approved; the callback page returns by `opener`, and
// with the provider's COOP severing the opener, by the BroadcastChannel. A
// press without activation (a script's `click()`) is 428, with no popup.
export async function authPopupWeb({ webDist, check: record }) {
  let checks = 0, failed = 0;
  const check = (ok, what) => { checks += 1; if (!ok) failed += 1; return record(ok, `web auth popup: ${what}`); };
  const t0 = Date.now();
  const fixture = await fixtureServer();
  await fetch(`${ISSUER}/fixture/reset`);
  // A JS-target build (LLP 1071) is served as its tree, as serve.mjs serves it.
  const js = jsTargetBuild(webDist);
  const server = createServer((req, res) => js ? serveBuildTree(webDist, req, res) : serveStatic(webDist, req, res));
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const origin = `http://127.0.0.1:${server.address().port}`;
  const profile = mkdtempSync(resolve(tmpdir(), 'exact-auth-popup-'));
  const chrome = process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
  const child = spawn(chrome, ['--headless=new', '--remote-debugging-pipe', '--window-size=500,700', `--user-data-dir=${profile}`,
    '--no-sandbox', '--no-first-run', '--disable-background-networking', 'about:blank'], { detached: true, stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
  const cdp = new Cdp(child.stdio[3], child.stdio[4]);
  const popups = [];
  cdp.listeners.push((msg) => {
    if (msg.method === 'Target.targetCreated' && msg.params.targetInfo.type === 'page' && msg.params.targetInfo.openerId) popups.push(msg.params.targetInfo);
  });
  try {
    await cdp.send('Target.setDiscoverTargets', { discover: true });
    const { targetInfos } = await cdp.send('Target.getTargets');
    const { sessionId } = await cdp.send('Target.attachToTarget', { targetId: targetInfos.find((t) => t.type === 'page').targetId, flatten: true });
    const call = (method, params, session = sessionId) => cdp.send(method, params, session);
    const evaluate = async (expression, session) => (await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true }, session)).result.value;
    const until = async (expression, what, ms = 15000) => {
      for (const end = Date.now() + ms; Date.now() < end; await sleep(50)) if (await evaluate(expression).catch(() => false)) return true;
      check(false, `timed out waiting for ${what}: ${JSON.stringify(await evaluate('document.body.innerText').catch(() => '?'))}`);
      return false;
    };
    const press = async (label) => { // a real press: user activation, as a person's
      const box = await evaluate(`(() => { const b = [...document.querySelectorAll('button')].find(b => b.innerText.trim() === ${JSON.stringify(label)}); if (!b) return null; const r = b.getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; })()`);
      if (!box) throw new Error(`no button ${label}`);
      for (const type of ['mousePressed', 'mouseReleased']) await call('Input.dispatchMouseEvent', { type, x: box[0], y: box[1], button: 'left', clickCount: 1 });
    };
    const approve = async () => {
      for (const end = Date.now() + 10000; !popups.length && Date.now() < end;) await sleep(50);
      const popup = popups.shift();
      if (!popup) return check(false, 'no popup opened');
      const { sessionId: p } = await cdp.send('Target.attachToTarget', { targetId: popup.targetId, flatten: true });
      const ready = () => cdp.send('Runtime.evaluate', { expression: '!!document.getElementById("approve")', returnByValue: true }, p).then((r) => r.result.value, () => false);
      for (const end = Date.now() + 10000; Date.now() < end && !(await ready());) await sleep(50);
      await cdp.send('Runtime.evaluate', { expression: 'document.getElementById("approve").click()' }, p).catch(() => {});
      return true;
    };
    await call('Page.enable');
    await call('Page.navigate', { url: `${origin}/` });
    const textOf = (id) => `document.querySelector('[data-testid=${id}]')?.innerText`;
    const is = (id, text) => `${textOf(id)} === ${JSON.stringify(text)}`;
    // Prepared: the fixture saw one more PAR, and the press is no longer busy.
    const prepare = async () => {
      const par = (await fetch(`${ISSUER}/fixture/log`).then((r) => r.json())).par;
      // A press before the source is ready is not dispatched (the page
      // gates input until then), so the first one may be pressed again.
      for (let attempt = 0, seen = false; attempt < 3 && !seen; attempt++) {
        await press('Prepare sign-in');
        for (const end = Date.now() + 3000; Date.now() < end && !seen; await sleep(50)) {
          seen = (await fetch(`${ISSUER}/fixture/log`).then((r) => r.json())).par > par;
        }
      }
      await until(`!document.querySelector('[data-testid=prepare]').disabled && ${is('prepared', 'Ready to sign in')}`, 'prepared');
    };
    await until(is('session', 'Signed out'), 'boot');
    for (const coop of [false, true]) {
      await fetch(`${ISSUER}/fixture/coop?on=${coop ? 1 : 0}`);
      await prepare();
      await press('Sign in');
      await approve();
      if (await until(is('session', 'Signed in as fixture.test'), `signed in${coop ? ' (COOP)' : ''}`)) {
        check(true, `signed in through the popup${coop ? ', the provider severing its opener (COOP): the BroadcastChannel' : ', by opener'}`);
      }
      await press('Sign out');
      await until(is('session', 'Signed out'), 'signed out');
    }
    // No activation: a script's click is not a press, so no popup opens.
    // Chrome's transient activation outlives a press by five seconds (the
    // web's rule, which the glue asks), so the last real press ages out first.
    await prepare();
    await sleep(5500);
    const before = popups.length;
    await evaluate(`document.querySelector('[data-testid=sign-in]').click()`);
    await until(is('signed', 'Sign-in failed (428)'), 'a blocked popup is 428');
    check(popups.length === before, 'no popup without a press');
    const log = await fetch(`${ISSUER}/fixture/log`).then((r) => r.json());
    check(log.token === 2 && log.resource === 2 && log.pkce === 2, `two code exchanges, PKCE and DPoP checked: ${JSON.stringify(log)}`);
  } catch (error) {
    check(false, error.message);
  } finally {
    try { process.kill(-child.pid, 'SIGKILL'); } catch {}
    server.close();
    fixture?.kill();
    rmSync(profile, { recursive: true, force: true });
  }
  console.log(`web auth popup: ${checks - failed} of ${checks} checks passed in ${((Date.now() - t0) / 1000).toFixed(1)} s (LLP 1069.006 D4, outside the agent)`);
}
