// @exact/passkeys on the web, real (LLP 1133.011 §11 stage 1): the auth
// fixture's web build, with no agent, at `http://localhost:<port>` (the RP
// ID `localhost`; an IP address is not one, D14), against the fixture
// server's relying party, which verifies every response.
//
// Chromium runs each ceremony on its virtual authenticator (CDP's
// `WebAuthn.addVirtualAuthenticator`: CTAP 2.1, internal, resident keys,
// user verification). Firefox and WebKit have no virtual authenticator
// Playwright can reach, so there the test reads what the module reports
// and that a ceremony with no authenticator ends the app's call with an
// error line, not a hang.
//
// usage: bun packages/passkeys/test/web.mjs [chromium] [firefox] [webkit]
//   Builds the fixture (`host/web/build.mjs auth-fixture-web`). Starts the
//   fixture server on 4331 when none answers there, and stops what it started.
import { spawn, spawnSync } from 'node:child_process';
import { extname, join, resolve } from 'node:path';

const root = resolve(import.meta.dir, '../../..');
const engines = process.argv.slice(2).length ? process.argv.slice(2) : ['chromium', 'firefox', 'webkit'];
const { chromium, firefox, webkit } = await import(resolve(root, 'node_modules/playwright-core/index.mjs'));
const ISSUER = 'http://127.0.0.1:4331';
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let failed = 0;
const check = (engine, what, ok, detail = '') => {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${engine}: ${what}${ok ? '' : `\n     ${detail}`}`);
  if (!ok) failed++;
};

const built = spawnSync(process.execPath, [resolve(root, 'host/web/build.mjs'), 'auth-fixture-web'], { cwd: root, stdio: ['ignore', 'ignore', 'inherit'] });
if (built.status !== 0) process.exit(built.status ?? 1);
const dist = resolve(root, 'host/web/dist');
const types = { '.js': 'text/javascript', '.html': 'text/html', '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css' };
const page = Bun.serve({ hostname: '127.0.0.1', port: 0, async fetch(req) {
  let path = new URL(req.url).pathname;
  if (path === '/' || !extname(path)) path = '/index.html';
  const file = Bun.file(join(dist, path));
  if (!(await file.exists())) return new Response('not found', { status: 404 });
  return new Response(file, { headers: { 'content-type': types[extname(path)] ?? 'application/octet-stream' } });
} });
const origin = `http://localhost:${page.port}`;

const up = () => fetch(`${ISSUER}/fixture/log`).then((r) => r.ok, () => false);
let server = null;
if (!(await up())) {
  server = spawn(process.execPath, [resolve(root, 'apps/auth-fixture/fixture.mjs')], { stdio: 'ignore' });
  for (let i = 0; i < 50 && !(await up()); i++) await sleep(100);
}
const log = async () => (await (await fetch(`${ISSUER}/fixture/log`)).json()).passkey;

try {
  for (const engine of engines) {
    await fetch(`${ISSUER}/fixture/reset`);
    const browser = await { chromium, firefox, webkit }[engine].launch();
    try {
      const context = await browser.newContext();
      const tab = await context.newPage();
      const text = (id) => tab.getByTestId(id).innerText({ timeout: 10000 }).catch(() => null);
      const until = async (id, want, ms = 10000) => {
        let got;
        for (const end = Date.now() + ms; Date.now() < end; await tab.waitForTimeout(100)) if (want(got = await text(id))) return got;
        return got;
      };
      let cdp = null, authenticatorId = null;
      if (engine === 'chromium') {
        cdp = await context.newCDPSession(tab);
        await cdp.send('WebAuthn.enable');
        ({ authenticatorId } = await cdp.send('WebAuthn.addVirtualAuthenticator', { options: {
          protocol: 'ctap2', ctap2Version: 'ctap2_1', transport: 'internal', hasResidentKey: true,
          hasUserVerification: true, isUserVerified: true, automaticPresenceSimulation: true } }));
      }
      const credentials = async () => (await cdp.send('WebAuthn.getCredentials', { authenticatorId })).credentials;
      await tab.goto(origin);
      const shown = await tab.getByTestId('create-passkey').waitFor({ timeout: 10000 }).then(() => true, () => false);
      const caps = await tab.evaluate(async () => typeof PublicKeyCredential.getClientCapabilities === 'function' ? PublicKeyCredential.getClientCapabilities() : null);
      check(engine, `the module reads the browser's capabilities and shows the passkey buttons (${caps ? Object.entries(caps).filter(([, v]) => v).length + ' true' : 'no getClientCapabilities'})`, shown, JSON.stringify(caps));
      if (engine !== 'chromium') {
        // No authenticator answers: the ceremony must end the call, which the app shows as an error line or nothing.
        await tab.getByTestId('create-passkey').click();
        const line = await until('passkey', (t) => t !== null, 70000);
        check(engine, `with no authenticator, create ends the call (no hang): "${line}"`, line !== null, String(line));
        await context.close();
        continue;
      }
      await tab.getByTestId('create-passkey').click();
      const session = await until('session', (t) => t === 'Signed in as fixture.test');
      const made = await credentials();
      check(engine, 'create: the virtual authenticator made a resident passkey for localhost and the server verified it',
        session === 'Signed in as fixture.test' && made.length === 1 && made[0].rpId === 'localhost' && made[0].isResidentCredential && (await log()).registered === 1,
        `${session} ${JSON.stringify(made.map((c) => c.rpId))} ${JSON.stringify(await log())}`);
      await tab.getByTestId('create-passkey').click();
      const excluded = await until('passkey', (t) => t === 'You already have a passkey for this account');
      check(engine, 'create again: excludeCredentials refuses with InvalidStateError', excluded === 'You already have a passkey for this account', String(excluded));
      const presence = (enabled) => cdp.send('WebAuthn.setAutomaticPresenceSimulation', { authenticatorId, enabled });
      const signedOut = () => until('session', (t) => t === 'Signed out');
      // Signed out, the sign-in screen asks a conditional get, which the
      // virtual authenticator answers as a person picking the suggestion.
      await tab.getByTestId('sign-out').click();
      const suggested = await until('suggested', (t) => t === 'Signed in as fixture.test');
      check(engine, 'conditional get: the sign-in screen\'s request signs in, verified by the server',
        suggested === 'Signed in as fixture.test' && (await log()).assertions === 1, `${suggested} ${JSON.stringify(await log())}`);
      // With presence off, the conditional get stays pending; the press retires it (AbortError, no error line) and runs a modal one.
      await presence(false);
      await tab.getByTestId('sign-out').click();
      await signedOut();
      await tab.waitForTimeout(500);
      await presence(true);
      await tab.getByTestId('sign-in-passkey').click();
      const back = await until('passkey', (t) => t === 'Signed in as fixture.test');
      const quiet = await text('suggested');
      check(engine, 'modal get: it supersedes the pending conditional one, and the server verifies the assertion',
        back === 'Signed in as fixture.test' && quiet === '' && (await log()).assertions === 2, `${back} / ${quiet} ${JSON.stringify(await log())}`);
      // The server forgets the passkey (the app drops its record): the next sign-in hears `unknown-credential`, and the
      // app signals it (D11).
      await tab.getByTestId('forget-passkey').click();
      const forgot = await until('passkey', (t) => t === 'The server forgot this passkey');
      check(engine, 'forget: the app drops the passkey\'s record', forgot === 'The server forgot this passkey', String(forgot));
      await presence(false);
      await tab.getByTestId('sign-out').click();
      await signedOut();
      await tab.waitForTimeout(500);
      await presence(true);
      await tab.getByTestId('sign-in-passkey').click();
      const unknown = await until('passkey', (t) => t === 'That passkey is not for an account here');
      let left = await credentials();
      for (let i = 0; i < 20 && left.length; i++) { await sleep(100); left = await credentials(); }
      check(engine, 'an unknown credential is signalled, and the browser removes it', unknown === 'That passkey is not for an account here' && left.length === 0,
        `${unknown}; ${left.length} left`);
      await cdp.send('WebAuthn.removeVirtualAuthenticator', { authenticatorId });
      await context.close();
    } finally {
      await browser.close();
    }
  }
} finally {
  page.stop();
  server?.kill();
}
console.log(failed ? `${failed} failed` : 'all passed');
process.exit(failed ? 1 : 0);
