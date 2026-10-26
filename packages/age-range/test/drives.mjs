// LLP 1133.028 §8.4's age range drives 6 to 8 under the agent, and 11's
// Excord part, with Excord's age-restricted channel (#after-hours) against
// its fixture server. Drive 9 (a tool's request fails `person-required`) is
// the runner's test, contract/cli/tests/it/attestation_age_tracking.rs,
// until a tool door produces one.
//
// usage: bun packages/age-range/test/drives.mjs [ios|macos|android|web|linux|windows] [--only 6,7,…]
//   Builds nothing: build Excord first (host/apple/build.mjs excord-apple,
//   `--ios --sim <udid>` for a simulator). Starts Excord's fixture server on
//   4790 and stops it. On web, Linux and Windows the module is absent, and
//   the drive is 11: the confirmation screen alone, no ticket.
import { spawn } from 'node:child_process';
import { resolve } from 'node:path';

const root = resolve(import.meta.dir, '../../..');
const argv = process.argv.slice(2);
const host = argv.find((a) => ['ios', 'macos', 'android', 'web', 'linux', 'windows'].includes(a)) ?? 'ios';
const only = argv.includes('--only') ? argv[argv.indexOf('--only') + 1].split(',').map(Number) : null;
const run = (n) => !only || only.includes(n);
const absent = ['web', 'linux', 'windows'].includes(host);
const { open } = await import(resolve(root, 'scripts/agent.mjs'));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let failed = 0;
const check = (drive, what, ok, detail = '') => {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${host} drive ${drive}: ${what}${ok ? '' : `\n     ${detail}`}`);
  if (!ok) failed++;
};

const fixture = spawn('bun', [resolve(root, 'apps/excord/fixture/server.ts'), '--world', 'transcript', '--reset-on-identify'], { stdio: ['ignore', 'pipe', 'inherit'] });
await new Promise((ok, fail) => {
  fixture.stdout.once('data', ok);
  fixture.once('exit', (code) => fail(new Error(`the fixture server exited (${code}); is 4790 taken?`)));
});

const ids = async (s) => new Set((await s.tree()).nodes.map((n) => n.props?.testId).filter(Boolean));
const tickets = async (s) => ((await s.state()).pending ?? []).filter((p) => p.device?.capability === 'age-range');
async function until(s, read, ms = 20000) {
  for (const end = Date.now() + ms; ; await sleep(50)) {
    await s.clock('+100 real');
    const value = await read();
    if (value || Date.now() > end) return value;
  }
}
/** A fresh launch, signed in to the fixture, with #after-hours opened. */
async function opened(prefer) {
  const s = await open({ host, app: 'excord', storage: `age-range-${Date.now().toString(36)}` });
  if (prefer) await s.prefer({ extension: prefer });
  await until(s, async () => (await ids(s)).has('channel-after-hours'), 60000);
  await s.tap('channel-after-hours');
  await s.clock('settle');
  return s;
}
const gate = async (s) => {
  const seen = await ids(s);
  return ['age-gate', 'age-asking', 'age-confirmation', 'age-unavailable', 'age-continue'].filter((id) => seen.has(id));
};
const held = (s) => until(s, async () => (await tickets(s))[0], 10000);

const drives = {
  async 6() {
    const s = await opened();
    try {
      const t = await held(s);
      check(6, 'opening the channel holds an age-range ticket, gates [18]', JSON.stringify(t?.device.args.options?.ageGates) === '[18]', JSON.stringify(await tickets(s)));
      check(6, 'the channel waits behind the gate while it asks', (await gate(s)).includes('age-asking'), (await gate(s)).join(' '));
      await s.type(`@${t.ticket}`, '30');
      await until(s, async () => (await gate(s)).includes('age-confirmation'), 5000);
      check(6, '30: the confirmation screen', (await gate(s)).join(' ') === 'age-gate age-confirmation age-continue', (await gate(s)).join(' '));
      await s.tap('age-continue');
      await s.clock('settle');
      check(6, 'Continue opens the channel', (await gate(s)).length === 0 && (await ids(s)).has('composer'), (await gate(s)).join(' '));
      await s.tap('back');
      await s.tap('channel-after-hours');
      await s.clock('settle');
      check(6, 'asked once a launch: the second opening asks nothing and shows no gate', !(await tickets(s)).length && !(await gate(s)).length, (await gate(s)).join(' '));
    } finally { await s.close(); }
  },
  async 7() {
    const s = await opened();
    try {
      const t = await held(s);
      await s.type(`@${t.ticket}`, '15');
      await until(s, async () => (await gate(s)).includes('age-unavailable'), 5000);
      const g = await gate(s);
      check(7, '15: the channel is unavailable, with no Continue', g.includes('age-unavailable') && !g.includes('age-continue') && !g.includes('age-confirmation'), g.join(' '));
      await s.tap('age-leave');
      await s.tap('channel-after-hours');
      await s.clock('settle');
      check(7, 'opened again, still unavailable', (await gate(s)).includes('age-unavailable') && !(await tickets(s)).length, (await gate(s)).join(' '));
    } finally { await s.close(); }
  },
  async 8() {
    let s = await opened();
    try {
      const t = await held(s);
      check(8, 'the ticket offers decline', t?.device.choices?.includes('decline') ?? JSON.stringify(t), JSON.stringify(t));
      await s.tap(`@${t.ticket}`, { choice: 'decline' });
      await until(s, async () => (await gate(s)).includes('age-confirmation'), 5000);
      check(8, 'declined: the confirmation alone', (await gate(s)).includes('age-confirmation'), (await gate(s)).join(' '));
    } finally { await s.close(); }
    if (host === 'ios' || host === 'macos') {
      s = await opened('age-range.ageRangeStatus={"apple":{"requiredRegulatoryFeatures":["declaredAgeRangeRequired"]}}');
      try {
        const t = await held(s);
        const refused = await s.tap(`@${t.ticket}`, { choice: 'decline' }).then((r) => JSON.stringify(r), (e) => String(e?.message ?? e));
        check(8, 'where the region requires the signal, decline is refused', /not decline|refus|error/i.test(refused) && (await tickets(s)).length === 1, refused);
        await s.type(`@${t.ticket}`, '19');
        await until(s, async () => (await gate(s)).includes('age-confirmation'), 5000);
        check(8, 'and an age still answers it', (await gate(s)).includes('age-confirmation'), (await gate(s)).join(' '));
      } finally { await s.close(); }
    }
    if (host === 'android') {
      s = await opened();
      try {
        const t = await held(s);
        await s.tap(`@${t.ticket}`, { choice: 'unverified' });
        await until(s, async () => (await gate(s)).includes('age-unavailable'), 5000);
        const said = (await s.find('age-unavailable', false).catch(() => null))?.props?.text ?? '';
        check(8, 'unverified: unavailable, with Play\'s instruction', /Play Store/.test(said), said);
      } finally { await s.close(); }
    }
  },
  async 11() {
    const s = await opened();
    try {
      await until(s, async () => (await gate(s)).includes('age-confirmation'), 5000);
      const st = await s.state();
      check(11, 'no ticket is held', !(await tickets(s)).length, JSON.stringify(st.pending));
      check(11, 'the age-restricted channel shows its confirmation alone', (await gate(s)).join(' ') === 'age-gate age-confirmation age-continue', (await gate(s)).join(' '));
      const lines = (await s.logs({ all: true })).lines.join('\n');
      check(11, 'the request refused NotSupportedError', /requestAgeRange[^\n]*NotSupportedError/.test(lines), lines.split('\n').filter((l) => /age/i.test(l)).slice(-6).join('\n'));
    } finally { await s.close(); }
  },
};

try {
  for (const n of absent ? [11] : [6, 7, 8]) if (run(n)) await drives[n]();
} finally {
  fixture.kill();
}
console.log(failed ? `${failed} failed` : 'all passed');
process.exit(failed ? 1 : 0);
