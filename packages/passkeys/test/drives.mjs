// LLP 1133.011 §10's acceptance drives 1–6 under the agent, with the auth
// fixture against its server's relying party (apps/auth-fixture/fixture.mjs).
// Each ceremony holds a `credential` ticket; "answer" runs `authenticator.mjs` on
// the ticket's args and types its response, so the server verifies a real
// signature over its own challenge. Drive 7's origin rule is the runner's
// test (contract/cli/tests/it/passkeys.rs) until a tool door produces one.
//
// usage: bun packages/passkeys/test/drives.mjs [web|ios|macos] [--only 1,2,…]
//   Builds nothing on Apple: build the fixture first (host/apple/build.mjs
//   auth-fixture-apple, `--ios --sim <udid>` for a simulator), which the
//   driver checks is current. Starts the fixture server on 4331 when none
//   answers there, and stops what it started.
import { spawn } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { respond } from './authenticator.mjs';

const root = resolve(import.meta.dir, '../../..');
const argv = process.argv.slice(2), host = argv.find((a) => ['web', 'ios', 'macos'].includes(a)) ?? 'web';
const only = argv.includes('--only') ? argv[argv.indexOf('--only') + 1].split(',').map(Number) : null;
const { open } = await import(resolve(root, 'scripts/agent.mjs'));
const ISSUER = 'http://127.0.0.1:4331', RP = host === 'web' ? 'localhost' : 'auth-fixture.exact.test';
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let failed = 0;
const check = (drive, what, ok, detail = '') => {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${host} drive ${drive}: ${what}${ok ? '' : `\n     ${detail}`}`);
  if (!ok) failed++;
};

const up = () => fetch(`${ISSUER}/fixture/log`).then((r) => r.ok, () => false);
let server = null;
if (!(await up())) {
  server = spawn(process.execPath, [resolve(root, 'apps/auth-fixture/fixture.mjs')], { stdio: 'ignore' });
  for (let i = 0; i < 50 && !(await up()); i++) await sleep(100);
}
const log = () => fetch(`${ISSUER}/fixture/log`).then((r) => r.json());
const keys = join(mkdtempSync(join(tmpdir(), 'passkeys-drive-')), 'keys.json');

const text = async (s, id) => (await s.find(id, false).catch(() => null))?.props?.text ?? null;
const shown = async (s, id) => (await s.find(id, false).catch(() => null)) != null;
const tickets = async (s) => ((await s.state()).pending ?? []).filter((p) => p.device?.capability === 'credential' && p.device.args.options);
// The fixture server answers over real I/O, which `clock settle` does not wait for on every host: poll a while.
const until = async (s, read, ms = 5000) => {
  for (const end = Date.now() + ms; ; await sleep(50)) {
    await s.clock('settle');
    const value = await read();
    if (value || Date.now() > end) return value;
  }
};
const ticket = (s, want = () => true) => until(s, async () => (await tickets(s)).find((p) => want(p.device.args)));
const ceremonies = async (s) => (await s.state()).extensions?.passkeys?.ceremonies ?? [];
/** Answer ticket `t` as the person and their authenticator would. */
async function answer(s, t) {
  const out = await respond(t.device.args, keys);
  if (out.refuse) return s.tap(`@${t.ticket}`, { choice: out.refuse });
  return s.type(`@${t.ticket}`, JSON.stringify(out.response));
}

await fetch(`${ISSUER}/fixture/reset`);
const s = await open({ host, app: 'auth-fixture', storage: `passkeys-${Date.now().toString(36)}` });
const run = (n) => !only || only.includes(n);
try {
  await s.clock('settle');
  const conditional = host !== 'macos';
  if (host === 'macos') {
    // Drive 6's third part: a Mac build without a provisioning profile.
    const caps = (await s.state()).resources?.caps;
    check(6, 'a Mac build without a profile reads `available: false`', caps?.available === false, JSON.stringify(caps));
    check(6, 'and shows no passkey button', !(await shown(s, 'create-passkey')) && !(await shown(s, 'sign-in-passkey')));
    check(6, 'and holds no ticket', !(await tickets(s)).length, JSON.stringify(await tickets(s)));
  } else {
    if (run(1)) {
      const t = await ticket(s, (a) => a.ceremony === 'get');
      check(1, 'the sign-in screen holds a conditional get at launch', t?.device.args.mediation === 'conditional', JSON.stringify(await tickets(s)));
      await s.tap('create-passkey');
      const made = await ticket(s, (a) => a.ceremony === 'create');
      const args = made?.device.args ?? {};
      const origin = host === 'web' ? /^http:\/\/(127\.0\.0\.1|localhost):\d+$/ : /^https:\/\/auth-fixture\.exact\.test$/;
      check(1, 'create holds a passkey ticket with the RP ID and this target\'s origin', args.rpId === RP && origin.test(args.clientOrigin ?? ''), JSON.stringify(args));
      check(1, 'the modal ceremony retired the conditional one with AbortError', (await ceremonies(s)).some((c) => c.name === 'get' && c.outcome === 'AbortError'), JSON.stringify(await ceremonies(s)));
      await answer(s, made);
      await until(s, async () => (await text(s, 'session')) === 'Signed in as fixture.test');
      check(1, 'the server verified the registration and the app shows the account', (await text(s, 'session')) === 'Signed in as fixture.test' && (await log()).passkey.registered === 1,
        `${await text(s, 'session')} ${JSON.stringify((await log()).passkey)}`);
    }
    if (run(2)) {
      await s.tap('sign-out');
      const t = await ticket(s, (a) => a.ceremony === 'get');
      check(2, 'signed out, the task holds a conditional get', t?.device.args.mediation === 'conditional', JSON.stringify(await tickets(s)));
      const before = (await log()).passkey.assertions;
      await answer(s, t);
      await until(s, async () => (await text(s, 'session')) === 'Signed in as fixture.test');
      check(2, 'answered: signed in as the same account, one assertion', (await text(s, 'session')) === 'Signed in as fixture.test' && (await log()).passkey.assertions === before + 1,
        `${await text(s, 'session')} ${JSON.stringify((await log()).passkey)}`);
    }
    if (run(3)) {
      await s.tap('sign-out');
      const held = await ticket(s, (a) => a.mediation === 'conditional');
      await s.tap('sign-in-passkey');
      await ticket(s, (a) => a.mediation === 'optional');
      const all = await tickets(s);
      check(3, 'the press retires the conditional ticket and holds a modal one', !all.some((p) => p.ticket === held?.ticket) && all.length === 1 && all[0].device.args.mediation === 'optional', JSON.stringify(all));
      await s.tap(`@${all[0]?.ticket}`, { choice: 'cancel' });
      const next = await ticket(s, (a) => a.mediation === 'conditional' && a.options?.challenge !== held?.device.args.options?.challenge);
      check(3, 'a cancel shows no error line', (await text(s, 'passkey')) === '', String(await text(s, 'passkey')));
      check(3, 'and a new conditional ticket is held', next && next.ticket !== held?.ticket, `${JSON.stringify(await tickets(s))} ${JSON.stringify((await ceremonies(s)).slice(-4))}`);
    }
    if (run(4)) {
      const t = await ticket(s, (a) => a.mediation === 'conditional');
      if (t) await answer(s, t);
      await until(s, async () => (await text(s, 'session')) === 'Signed in as fixture.test');
      await s.tap('create-passkey');
      const made = await ticket(s, (a) => a.ceremony === 'create');
      check(4, 'signed in, create names the account\'s passkeys in excludeCredentials', (made?.device.args.options?.excludeCredentials ?? []).length === 1, JSON.stringify(made?.device.args.options));
      await answer(s, made); // the authenticator holds one: `tap @t excluded`
      await until(s, async () => (await text(s, 'passkey')) !== '');
      check(4, '"You already have a passkey for this account"', (await text(s, 'passkey')) === 'You already have a passkey for this account', String(await text(s, 'passkey')));
    }
    if (run(5)) {
      await s.tap('sign-out');
      await ticket(s, (a) => a.mediation === 'conditional');
      const registered = (await log()).passkey.registered;
      await s.type('username', 'second.test');
      await s.tap('create-passkey');
      const made = await ticket(s, (a) => a.ceremony === 'create');
      await s.tap('sign-out');
      await s.clock('settle');
      check(5, 'the conditional get the sign-in screen asks waits behind the held sheet', (await tickets(s)).length === 1, JSON.stringify(await tickets(s)));
      await answer(s, made);
      await until(s, async () => /its reply was dropped/.test((await s.logs({ all: true })).lines.join('\n')));
      const lines = (await s.logs({ all: true })).lines.join('\n');
      check(5, 'the response is dropped and the journal records its disposal', /dispose passkeys \S+: its reply was dropped/.test(lines), lines.split('\n').filter((l) => /passkey/.test(l)).slice(-8).join('\n'));
      check(5, 'the server registered nothing', (await log()).passkey.registered === registered, JSON.stringify((await log()).passkey));
      const after = await ticket(s, (a) => a.mediation === 'conditional');
      check(5, 'once the sheet is done, the conditional get holds its ticket', !!after, `${JSON.stringify(await tickets(s))} ${JSON.stringify((await ceremonies(s)).slice(-4))}`);
    }
  }
  if (run(6) && host !== 'macos') {
    // No answer, journal line or state field holds a response, a spent challenge or a signature (§10's test).
    const st = JSON.stringify(await s.state()), lines = (await s.logs({ all: true })).lines.join('\n');
    const secrets = /"(clientDataJSON|attestationObject|signature|authenticatorData)"/;
    check(6, 'no state field or journal line holds a response', !secrets.test(st) && !secrets.test(lines), (st.match(secrets) ?? lines.match(secrets))?.[0]);
  }
  void conditional;
} finally {
  await s.close();
  server?.kill();
  rmSync(resolve(keys, '..'), { recursive: true, force: true });
}
// Drive 6's first two parts are the compiler's and the runner's (contract/cli/tests/it/passkeys.rs).
console.log(failed ? `${failed} failed` : 'all passed');
process.exit(failed ? 1 : 0);
