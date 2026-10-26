// LLP 1133.028 §8.4's tracking drives under the agent, with the package's
// fixture app (test/fixture): drive 10 on iOS and tvOS, and drive 11 on the
// targets without App Tracking Transparency (web, macOS, Linux), where the
// status reads the declared defaults and the request refuses.
//
// usage: bun packages/tracking-consent/test/drives.mjs <ios|tvos|macos|web|linux> [--bind] [--keep]
//   Makes the fixture with `exact new`, builds it for the host (an iOS or
//   Apple TV simulator named by EXACT_SIM, else the one build.mjs picks) and
//   drives it; `--bind` binds the real framework instead of the agent's
//   stand-in, which on tvOS shows the real alert (stage 1). `--keep` keeps
//   the fixture's directory.
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { checker } from '../../camera/test/fixture.mjs';
import { texts, until } from '../../photos/test/fixture.mjs';
import { build, launch, makeFixture } from './fixture.mjs';

const argv = process.argv.slice(2);
const host = argv.find((a) => ['ios', 'tvos', 'macos', 'web', 'linux'].includes(a)) ?? 'ios';
const bind = argv.includes('--bind');
const { check, failed } = checker();
const present = host === 'ios' || host === 'tvos';
const sim = process.env.EXACT_SIM;

const fixture = makeFixture();
const tickets = async (s) => ((await s.state()).pending ?? []).filter((p) => p.device?.capability === 'tracking');
const held = async (s) => {
  for (let i = 0; i < 40; i++) {
    const t = (await tickets(s))[0];
    if (t) return t;
    await s.clock('+250 real');
  }
  return null;
};
/** Continue on the pre-alert screen, then the ticket's answer; the row it answered. */
async function ask(s, choice) {
  await s.tap('ask');
  check(host, 'the pre-alert screen shows one Continue', (await texts(s))['pre-alert'] !== undefined && (await texts(s)).continue === 'Continue', JSON.stringify(await texts(s)));
  await s.tap('continue');
  const t = await held(s);
  if (!t) return `no ticket: ${JSON.stringify((await s.state()).pending)}`;
  await s.tap(`@${t.ticket}`, { choice });
  return until(s, 'asked', (v) => v?.startsWith('Answered: '));
}

try {
  build(fixture, host, sim && host !== 'macos' ? ['--sim', sim] : []);
  const options = bind ? { bind: 'tracking-consent' } : {};
  if (present && !bind) {
    let s = await launch(fixture, host, options);
    try {
      check(host, 'drive 10: undecided at first, the identifier zeros', /notDetermined \(prompt\)/.test((await texts(s)).status) && (await texts(s)).idfa === 'Identifier: zeros', JSON.stringify(await texts(s)));
      const first = await ask(s, 'allow');
      const t = await until(s, 'idfa', (v) => v === 'Identifier: set');
      check(host, 'drive 10: allow answers authorized, and the identifier is set', first === 'Answered: authorized' && t === 'Identifier: set' && /authorized \(granted\)/.test((await texts(s)).status), `${first} ${JSON.stringify(await texts(s))}`);
      const second = await ask(s, 'deny');
      check(host, 'drive 10: a second request answers authorized whatever its tap', second === 'Answered: authorized', second);
    } finally { await s.close(); }
    s = await launch(fixture, host, options);
    try {
      const denied = await ask(s, 'deny');
      check(host, 'drive 10: a fresh session, deny answers denied and the identifier stays zeros', denied === 'Answered: denied' && (await texts(s)).idfa === 'Identifier: zeros', `${denied} ${JSON.stringify(await texts(s))}`);
    } finally { await s.close(); }
    s = await launch(fixture, host, options);
    try {
      await s.prefer({ extension: 'tracking-consent.trackingAuthorizationStatus={"status":"restricted"}' });
      await until(s, 'status', (v) => /restricted/.test(v ?? ''));
      const restricted = await ask(s, 'allow');
      check(host, 'drive 10: restricted answers restricted whatever the tap', restricted === 'Answered: restricted' && /restricted \(denied\)/.test((await texts(s)).status), `${restricted} ${JSON.stringify(await texts(s))}`);
    } finally { await s.close(); }
  } else if (present) {
    // Bound: the framework's own status, which a fresh simulator has undecided.
    const s = await launch(fixture, host, options);
    try {
      await until(s, 'status', (v) => /Status: /.test(v ?? ''));
      const shown = await texts(s);
      check(host, 'bound: the framework\'s status reads, with the identifier', /Status: (notDetermined|denied|authorized|restricted)/.test(shown.status) && /Identifier: (zeros|set)/.test(shown.idfa), JSON.stringify(shown));
      console.log(`${host} bound reading: ${shown.status}; ${shown.idfa}`);
      if (/notDetermined/.test(shown.status)) {
        await s.tap('ask');
        await s.tap('continue');
        await s.clock('+1500 real');
        // The app's own screenshot leaves the system's alert out; the simulator's display has it.
        const shot = join(tmpdir(), `tracking-alert-${host}.png`);
        if (sim) spawnSync('xcrun', ['simctl', 'io', sim, 'screenshot', shot]);
        else await s.screenshot(shot).catch((e) => console.log(`screenshot: ${e.message}`));
        console.log(`${host} bound: asked, ${JSON.stringify((await texts(s)).asked ?? '')}; the screen is ${shot}`);
      }
    } finally { await s.close(); }
  } else {
    const s = await launch(fixture, host, options);
    try {
      await s.clock('settle');
      const shown = await texts(s);
      check(host, 'drive 11: the declared defaults, denied and unavailable, and no button', shown.status === 'Status: denied (denied)' && shown.ask === undefined, JSON.stringify(shown));
      check(host, 'drive 11: the identifier refuses', /Identifier: error NotSupportedError/.test(shown.idfa ?? ''), shown.idfa);
      check(host, 'drive 11: no ticket', !(await tickets(s)).length, JSON.stringify((await s.state()).pending));
    } finally { await s.close(); }
  }
} finally {
  if (!argv.includes('--keep')) fixture.remove();
  else console.log(`kept ${fixture.app}`);
}
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
