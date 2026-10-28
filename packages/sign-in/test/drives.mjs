// LLP 1133.014 §10's acceptance drives 1 to 7 under the agent, with the auth
// fixture against its server's sign-in routes (apps/auth-fixture/sign-in.mjs).
// Each request holds a `credential` ticket; "answer" runs `idp.mjs` on the
// ticket's args and types its credential, so the server verifies a real
// signature over its own nonce: natively the token, on the web through the
// server's return route. Drive 8's origin rule is the runner's test
// (contract/cli/tests/it/sign_in.rs) until a tool door produces one.
//
// usage: bun packages/sign-in/test/drives.mjs [web|ios|macos] [--only 1,2,…]
//   Builds nothing on Apple: build the fixture first (host/apple/build.mjs
//   auth-fixture-apple, `--ios --sim <udid>` for a simulator). Starts the
//   fixture server on 4331 with the fixture provider's key, and stops it;
//   a server already on 4331 is refused, since it trusts another key.
import { spawn } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { respond } from './idp.mjs';

const root = resolve(import.meta.dir, '../../..');
const argv = process.argv.slice(2), host = argv.find((a) => ['web', 'ios', 'macos'].includes(a)) ?? 'web';
const only = argv.includes('--only') ? argv[argv.indexOf('--only') + 1].split(',').map(Number) : null;
const { open } = await import(resolve(root, 'scripts/agent.mjs'));
const ISSUER = 'http://127.0.0.1:4331';
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let failed = 0;
const check = (drive, what, ok, detail = '') => {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${host} drive ${drive}: ${what}${ok ? '' : `\n     ${detail}`}`);
  if (!ok) failed++;
};

const up = () => fetch(`${ISSUER}/fixture/log`).then((r) => r.ok, () => false);
if (await up()) { console.error(`a server already answers on ${ISSUER}: stop it, so the drive's own trusts this drive's key`); process.exit(2); }
const dir = mkdtempSync(join(tmpdir(), 'sign-in-drive-')), keys = join(dir, 'keys.json');
const server = spawn(process.execPath, [resolve(root, 'apps/auth-fixture/fixture.mjs'), '--substitute', '--idp-keys', keys], { stdio: 'ignore' });
for (let i = 0; i < 50 && !(await up()); i++) await sleep(100);
const log = () => fetch(`${ISSUER}/fixture/log`).then((r) => r.json());

const text = async (s, id) => (await s.find(id, false).catch(() => null))?.props?.text ?? null;
const shown = async (s, id) => (await s.find(id, false).catch(() => null)) != null;
const tickets = async (s) => ((await s.state()).pending ?? []).filter((p) => p.device?.capability === 'credential');
const until = async (s, read, ms = 5000) => {
  for (const end = Date.now() + ms; ; await sleep(50)) {
    await s.clock('settle');
    const value = await read();
    if (value || Date.now() > end) return value;
  }
};
const identityTicket = (s, want = () => true) => until(s, async () => (await tickets(s)).find((p) => p.device.args.identity && want(p.device.args)));
const answer = async (s, t, first = false) => s.type(`@${t.ticket}`, JSON.stringify((await respond(t.device.args, keys, { first })).credential));
const session = (s) => text(s, 'session');
const signedIn = (s, name) => until(s, async () => (await session(s)) === `Signed in as ${name}`);
const audience = { web: { apple: 'com.exact.authfixture.web', google: 'fixture-web.apps.googleusercontent.com' },
  ios: { apple: 'com.exact.authfixture', google: 'fixture-ios.apps.googleusercontent.com' } };
audience.macos = audience.ios;

await fetch(`${ISSUER}/fixture/reset`);
let s;
try { s = await open({ host, app: 'auth-fixture', storage: `sign-in-${Date.now().toString(36)}` }); }
catch (e) { server.kill(); rmSync(dir, { recursive: true, force: true }); throw e; }
const run = (n) => !only || only.includes(n);
const appleHere = host !== 'macos';
try {
  await s.clock('settle');
  if (run(1) && appleHere) {
    // A passive offer may be held first where the target has one (drive 4); the press supersedes it.
    await until(s, () => shown(s, 'apple-sign-in'));
    await s.tap('apple-sign-in');
    const t = await identityTicket(s, (a) => a.identity.mode === 'active');
    const p = t?.device.args.identity.providers[0] ?? {};
    check(1, "the press holds a credential ticket with this target's audience", p.provider === 'apple' && p.audience === audience[host].apple && !!p.nonce, JSON.stringify(t?.device.args ?? await tickets(s)));
    if (t) await answer(s, t, true);
    await signedIn(s, 'Ada Fixture');
    const l = await log();
    check(1, 'the server verified the token and the account shows the name', (await session(s)) === 'Signed in as Ada Fixture' && l.signIn.verified === 1,
      `${await session(s)} [${await text(s, 'identity')}] ${JSON.stringify(l.signIn)}\n${(await s.logs({ all: true })).lines.filter((x) => /credential|sign-in|passkeys|identity/.test(x)).slice(-12).join('\n')}`);
    if (host === 'web') check(1, 'the web credential went through the return route', l.signIn.returned === 1 && l.signIn.redeemed === 1, JSON.stringify(l.signIn));
  }
  if (run(2) && appleHere) {
    await s.tap('sign-out');
    await until(s, () => shown(s, 'apple-sign-in'));
    await s.tap('apple-sign-in');
    const t = await identityTicket(s, (a) => a.identity.mode === 'active');
    if (t) await answer(s, t, false);
    await signedIn(s, 'Ada Fixture');
    check(2, 'without `user`, the same account, its name kept by the server', (await session(s)) === 'Signed in as Ada Fixture',
      `${await session(s)} ${JSON.stringify(await tickets(s))}\n${(await s.logs({ all: true })).lines.filter((x) => /credential|sign-in|passkeys|identity|offer/.test(x)).slice(-16).join('\n')}`);
  }
  if (host === 'macos') {
    // Drive 1's Mac part, and drive 7's: a Mac build without a profile offers no Apple.
    const caps = (await s.state()).resources?.identities;
    check(7, 'a Mac build without a profile reads providers.apple.active false', caps?.providers?.apple?.active === false, JSON.stringify(caps));
    check(7, 'and shows no Apple button', !(await shown(s, 'apple-sign-in')));
  }
  if (run(3)) {
    if ((await session(s))?.startsWith('Signed in')) await s.tap('sign-out');
    await until(s, () => shown(s, 'google-sign-in'));
    await s.tap('google-sign-in');
    const t = await identityTicket(s, (a) => a.identity.mode === 'active');
    const p = t?.device.args.identity.providers[0] ?? {};
    check(3, "Google's ticket names this target's audience", p.provider === 'google' && p.audience === audience[host].google, JSON.stringify(t?.device.args));
    if (t) await answer(s, t);
    await signedIn(s, 'fixture@google.example');
    check(3, 'signed in with Google', (await session(s)) === 'Signed in as fixture@google.example', String(await session(s)));
  }
  if (run(4)) {
    await s.tap('sign-out');
    if (host === 'ios') {
      const t = await identityTicket(s, (a) => a.identity.mode === 'passive');
      check(4, 'signed out, one ticket holds a passkey and passive Apple', !!t?.device.args.options?.challenge && t.device.args.identity.providers.map((p) => p.provider).join() === 'apple', JSON.stringify(await tickets(s)));
      if (t) await answer(s, t);
      await signedIn(s, 'Ada Fixture');
      check(4, 'answered with an identity credential: signed in', (await session(s)) === 'Signed in as Ada Fixture', String(await session(s)));
      await s.tap('sign-out');
    } else {
      await until(s, async () => (await text(s, 'offered')) !== '');
      check(4, `on ${host} the passive request beside a passkey fails NotSupportedError, with no identity ticket`, (await text(s, 'offered')) === 'No sign-in sheet here' && !(await tickets(s)).some((t) => t.device.args.identity), `${await text(s, 'offered')} ${JSON.stringify(await tickets(s))}`);
      check(4, 'and the app shows the buttons', (await shown(s, 'google-sign-in')) && (host === 'macos' || (await shown(s, 'apple-sign-in'))));
    }
  }
  if (run(5)) {
    await until(s, () => shown(s, 'google-sign-in'));
    const before = (await log()).signIn.nonces;
    await s.tap('google-sign-in');
    const t = await identityTicket(s, (a) => a.identity.mode === 'active');
    await s.tap(`@${t?.ticket}`, { choice: 'cancel' });
    await until(s, async () => (await log()).signIn.nonces > before);
    check(5, 'a cancel shows no error line', (await text(s, 'identity')) === '', String(await text(s, 'identity')));
    check(5, 'the buttons show again, with a fresh nonce', (await shown(s, 'google-sign-in')) && (await log()).signIn.nonces > before, JSON.stringify((await log()).signIn));
  }
  if (run(6)) {
    const before = (await log()).signIn.verified;
    await until(s, () => shown(s, 'google-sign-in'));
    await s.tap('google-sign-in');
    const t = await identityTicket(s, (a) => a.identity.mode === 'active');
    // Leaving: signing out supersedes the sign-in's mutation, as the screen leaving would.
    await s.tap('sign-out');
    await s.clock('settle');
    if (t) await answer(s, t);
    await until(s, async () => /dispose sign-in \S+: its reply was dropped/.test((await s.logs({ all: true })).lines.join('\n')));
    const lines = (await s.logs({ all: true })).lines.join('\n');
    check(6, 'the answer is dropped and the journal records it', /dispose sign-in \S+: its reply was dropped/.test(lines), lines.split('\n').filter((l) => /sign-in|credential/.test(l)).slice(-8).join('\n'));
    check(6, 'the server signed no one in', (await log()).signIn.verified === before, JSON.stringify((await log()).signIn));
  }
  if (run(7)) {
    // No answer, journal line or state field holds a token, a code or a return route's handle (§10's test).
    const st = JSON.stringify(await s.state()), lines = (await s.logs({ all: true })).lines.join('\n');
    const secret = /eyJ[A-Za-z0-9_-]{8,}\.|fixture-code-|fixture-apple-code|"relay":/;
    check(7, 'no state field or journal line holds a token, a code or a handle', !secret.test(st) && !secret.test(lines), (st.match(secret) ?? lines.match(secret))?.[0]);
  }
} finally {
  await s.close();
  server.kill();
  rmSync(dir, { recursive: true, force: true });
}
// Drive 7's compiler and admission parts are the runner's (contract/cli/tests/it/sign_in.rs).
console.log(failed ? `${failed} failed` : 'all passed');
process.exit(failed ? 1 : 0);
