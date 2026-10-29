// LLP 1133.028 §8.4's attestation drives 1 to 5 under the agent, with the
// auth fixture's "Check this build" against its server (apps/auth-fixture/
// fixture.mjs, started with --substitute so it accepts the agent's tokens).
//
// On web, Linux and Windows the module is absent, and the drive is 11: the
// status reads its declared default and the row shows no button.
//
// usage: bun packages/attestation/test/drives.mjs [ios|macos|android|web|linux|windows] [--only 1,2,…] [--port <n>]
//   Builds nothing: build the fixture first (host/apple/build.mjs
//   auth-fixture-apple, `--ios --sim <udid>` for a simulator). Starts the
//   fixture server on 4331 when none answers there, and stops what it started.
import { spawn } from 'node:child_process';
import { resolve } from 'node:path';

const root = resolve(import.meta.dir, '../../..');
const argv = process.argv.slice(2), host = argv.find((a) => ['ios', 'macos', 'android', 'web', 'linux', 'windows'].includes(a)) ?? 'ios';
const only = argv.includes('--only') ? argv[argv.indexOf('--only') + 1].split(',').map(Number) : null;
const run = (n) => !only || only.includes(n);
const { open } = await import(resolve(root, 'scripts/agent.mjs'));
// `--port` when another checkout's fixture server holds 4331: the app's
// ISSUER must name the same port.
const port = argv.includes('--port') ? argv[argv.indexOf('--port') + 1] : '4331';
const ISSUER = `http://127.0.0.1:${port}`;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let failed = 0;
const check = (drive, what, ok, detail = '') => {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${host} drive ${drive}: ${what}${ok ? '' : `\n     ${detail}`}`);
  if (!ok) failed++;
};

const up = () => fetch(`${ISSUER}/fixture/log`).then((r) => r.ok, () => false);
let server = null;
if (!(await up())) {
  server = spawn(process.execPath, [resolve(root, 'apps/auth-fixture/fixture.mjs'), '--substitute', '--port', port], { stdio: 'ignore' });
  for (let i = 0; i < 50 && !(await up()); i++) await sleep(100);
}
const log = async () => (await (await fetch(`${ISSUER}/fixture/log`)).json()).attestation;
const prefer = (s, word) => s.prefer({ extension: `attestation.${word}` });

const text = async (s, id) => (await s.find(id, false).catch(() => null))?.props?.text ?? null;
// The fixture server answers over real I/O, which `clock settle` does not wait for on every host: poll a while.
const until = async (s, read, ms = 8000) => {
  for (const end = Date.now() + ms; ; await sleep(50)) {
    await s.clock('settle');
    const value = await read();
    if (value || Date.now() > end) return value;
  }
};
/** Press "Check this build" and wait for the row to say something new. */
async function checkBuild(s) {
  const before = await text(s, 'checked');
  await s.tap('check-build');
  await until(s, async () => { const t = await text(s, 'checked'); return t && t !== before; });
  return text(s, 'checked');
}
const journal = async (s) => (await s.logs({ all: true })).lines.join('\n');
const lines = (text) => text.split('\n').filter((l) => /attestation/.test(l)).slice(-8).join('\n');
// Wait for the check's attest call to reach the implementation (its fetch of the challenge is real I/O).
const calling = (s) => until(s, async () => /attestation\.attest/.test(await journal(s)) || (await s.state()).extensions?.attestation?.calls?.length, 3000);

const drives = {
  async 1(s) {
    const status = (await s.state()).resources?.attestation;
    check(1, 'the status reads available under the agent', status?.available === true, JSON.stringify(status));
    const first = await checkBuild(s);
    check(1, 'the first check attests a key', first === 'Attested: the agent, development build', first);
    const second = await checkBuild(s);
    check(1, 'the second asserts with it, counter 1', second === 'Asserted (counter 1): the agent', second);
    const l = await log();
    check(1, 'the server stored the key and counted the assertion', l.attested === 1 && l.asserted === 1, JSON.stringify(l));
  },
  async 2(s) {
    await fetch(`${ISSUER}/fixture/reset`);
    const said = await checkBuild(s);
    check(2, 'the server forgot the key: reset, and the retry attests a new one', said === 'Unknown key; reset. Attested: the agent, development build', said);
    check(2, 'the server holds the new key', (await log()).attested === 1, JSON.stringify(await log()));
    const next = await checkBuild(s);
    check(2, 'which the next check asserts with', next === 'Asserted (counter 1): the agent', next);
  },
  async 3(s) {
    await prefer(s, 'fail="NetworkError"');
    const said = await checkBuild(s);
    check(3, 'the failure shows, and the retry with the same challenge attests the kept key',
      said === 'Failed: NetworkError; retried. Attested: the agent, development build', said);
    const j = await journal(s);
    check(3, 'the journal names the failure and no disposal', /NetworkError/.test(j) && !/dispose attestation/.test(j), lines(j));
    await prefer(s, 'fail=""');
  },
  async 4(s) {
    // An attestation the agent holds 5 s on its clock, superseded by a sign-out.
    await prefer(s, 'delay=5000');
    await s.tap('check-build');
    await calling(s);
    await s.clock('+1000');
    await s.tap('sign-out');
    await s.clock('+5000');
    await until(s, async () => /dispose attestation/.test(await journal(s)));
    let j = await journal(s);
    check(4, 'the dropped attestation\'s key is disposed', /dispose attestation \S+: its reply was dropped/.test(j), lines(j));
    check(4, 'and the server attested nothing', (await log()).attested === 0, JSON.stringify(await log()));
    await prefer(s, 'delay=0');
    const next = await checkBuild(s);
    check(4, 'the next call attests a new key', next === 'Attested: the agent, development build', next);
    // The same with an assertion: it carries no handle, so the key stays.
    const disposals = (j.match(/dispose attestation/g) ?? []).length;
    await prefer(s, 'delay=5000');
    await s.tap('check-build');
    await calling(s);
    await s.clock('+1000');
    await s.tap('sign-out');
    await s.clock('+5000');
    await s.clock('settle');
    await prefer(s, 'delay=0');
    const after = await checkBuild(s);
    j = await journal(s);
    check(4, 'a dropped assertion keeps the key: the next asserts', /^Asserted \(counter \d+\): the agent$/.test(after ?? '')
      && (j.match(/dispose attestation/g) ?? []).length === disposals, `${after}\n${lines(j)}`);
  },
  async 11(s) {
    const status = (await s.state()).resources?.attestation;
    check(11, 'the status reads the declared default, unavailable', status?.available === false && status?.provider === 'none', JSON.stringify(status));
    check(11, 'and the row shows no button', (await text(s, 'attestation-status')) === 'Attestation: unavailable' && !(await s.find('check-build', false).catch(() => null)), await text(s, 'attestation-status'));
  },
  async 5(s) {
    // After drive 1: no state field or journal line holds a token (D2).
    const st = JSON.stringify(await s.state()), j = await journal(s);
    const secret = /"token"|clientDataHash|exact-substitute/;
    check(5, 'no state field or journal line holds a token', !secret.test(st) && !secret.test(j), (st.match(secret) ?? j.match(secret))?.[0]);
  },
};

// Each group starts a fresh session: the agent's keys live in the app's
// process, so a relaunch is a device with no key.
const session = async () => {
  const s = await open({ host, app: 'auth-fixture', storage: `attestation-${Date.now().toString(36)}` });
  await s.clock('settle');
  return s;
};
const absent = ['web', 'linux', 'windows'].includes(host);
const groups = (absent ? [[11]] : [[1, 5, 2], [3], [4]]).map((g) => g.filter(run)).filter((g) => g.length);
try {
  for (const group of groups) {
    await fetch(`${ISSUER}/fixture/reset`);
    const s = await session();
    try { for (const n of group) await drives[n](s); } finally { await s.close(); }
  }
} finally {
  server?.kill();
}
console.log(failed ? `${failed} failed` : 'all passed');
process.exit(failed ? 1 : 0);
