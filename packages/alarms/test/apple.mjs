// @exact/alarms on iOS (LLP 1133.030 §14 stage 1): Metronome's practice timer with the AlarmKit implementation
// bound (`--bind alarms`), on a simulator of iOS 26 or later with a Dynamic Island. AlarmKit's own prompt and alert
// are answered through XCTest's SpringBoard (`touch: 'platform'`); the clock is the wall's, which AlarmKit runs on.
// A practice is five minutes, Metronome's shortest, so a run takes about twelve.
//
// 1. The status reads `prompt` before anything is asked; Start asks, Allow grants, and the alarm counts down.
// 2. With the app in the background (Settings in front), the countdown is AlarmKit's Live Activity (a screenshot of
//    the Dynamic Island). Back in the app, at the end the alert shows; Repeat starts a new countdown.
// 3. The app ended and launched again reads the new countdown's end from the Live Activity, as the system shows it.
// 4. Pause and resume from the app; at the second ring Cancel in the app ends the ring (the log says whether
//    `cancel(id:)` did or the module's `stop(id:)`), and the row is the chooser again.
// 5. The metronome playing at the ring stops and stays stopped.
//
// usage: bun packages/alarms/test/apple.mjs --sim <udid> [--out <dir>]
//   the simulator must be booted and have Metronome built for it:
//   EXACT_SIM=<udid> bun host/apple/build.mjs --ios metronome-apple
import { spawnSync } from 'node:child_process';
import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';

const root = resolve(import.meta.dir, '../../..');
const argv = process.argv.slice(2);
const udid = argv[argv.indexOf('--sim') + 1];
const out = argv.includes('--out') ? argv[argv.indexOf('--out') + 1] : resolve(root, 'apps/metronome/.exact/alarms');
if (!argv.includes('--sim') || !udid) throw new Error('usage: bun packages/alarms/test/apple.mjs --sim <udid> [--out <dir>]');
mkdirSync(out, { recursive: true });
process.env.EXACT_SIM = udid;
const { open, textOf } = await import(resolve(root, 'scripts/agent.mjs'));
const simctl = (...a) => spawnSync('xcrun', ['simctl', ...a], { encoding: 'utf8' });
const shot = (name) => simctl('io', udid, 'screenshot', resolve(out, name));
const failures = [];
const check = (ok, what, got) => { console.log(`${ok ? 'ok  ' : 'FAIL'} ${what}${ok ? '' : `: ${JSON.stringify(got)}`}`); if (!ok) failures.push(what); };

const launch = () => open({ host: 'ios', app: 'metronome', bind: 'alarms', touch: 'platform', timing: 'platform', storage: 'alarms-apple' });
const state = async (s, name) => (await s.op({ op: 'state' })).resources?.[name];
/** The wall clock alone: while the app is away every op fails, so nothing is asked of it. */
const wait = (ms) => new Promise((done) => setTimeout(done, ms));
const text = async (s, testId) => {
  const nodes = (await s.tree()).nodes, node = nodes.find((n) => n.props?.testId === testId);
  return node ? textOf(nodes, node) ?? '' : null;
};
const practice = async (s) => ((await state(s, 'alarmList')) ?? []).find((a) => a.tag === 'practice') ?? null;
/** Wait on the wall clock until `read` answers something `want` accepts. */
async function until(s, read, want, ms) {
  const end = Date.now() + ms;
  let got;
  while (Date.now() < end) {
    if (want(got = await read())) return got;
    await s.clock('+500 real');
  }
  return got;
}
/** A system alert's or the alarm's button, by its label, pressed through SpringBoard. */
async function press(s, label, ms = 10000) {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    const { elements = [] } = await s.springboard({ action: 'find', label });
    const [x, y, w, h] = elements[0]?.frame ?? [];
    if (w) { await s.springboard({ action: 'tap', point: [x + w / 2, y + h / 2] }); return true; }
    await s.clock('+250 real');
  }
  return null;
}
const foreground = () => simctl('launch', udid, 'com.exact.metronome');
/** A real touch once the app's window is key: the touch runner's start, a system alert or the ring can hold it a while,
 * and the app is brought to the front if it stays behind. */
async function tap(s, testId) {
  for (let i = 0; ; i++) {
    try {
      return await s.tap(testId);
    } catch (e) {
      if (i > 40 || !/not the foreground|not in the foreground|left the foreground/.test(e.message)) throw e;
      if (i % 8 === 7) foreground();
      await wait(500);
    }
  }
}

let s = await launch();
try {
  const status = await state(s, 'alarmState');
  check(status?.available === true && ['prompt', 'granted'].includes(status?.state), 'iOS 26: available, and prompt before asking', status);
  // An alarm an earlier run left is cancelled first, so the run starts at the chooser.
  if (await practice(s)) {
    await tap(s, 'practice-cancel');
    await until(s, () => practice(s), (p) => p == null, 10000);
  }
  await tap(s, 'toggle');
  await tap(s, 'practice-5');
  const started = Date.now();
  await tap(s, 'practice-start');
  // The first launch after an install over the app reads `notDetermined` while AlarmKit still holds the grant, and the
  // request then answers with no prompt: Allow is pressed only when the prompt shows.
  const allowed = status?.state === 'prompt' && await press(s, 'Allow');
  console.log(`note AlarmKit's prompt: ${status?.state !== 'prompt' ? 'not asked (granted)' : allowed ? 'shown, Allow pressed' : 'not shown (the grant was held)'}`);
  const after = await until(s, () => state(s, 'alarmState'), (st) => st?.state === 'granted', 15000);
  check(after?.state === 'granted', 'after Start the status reads granted', after);
  const first = await until(s, () => practice(s), (p) => p?.state === 'countdown', 15000);
  check(first?.state === 'countdown', 'the practice counts down', first);
  check(Math.abs((first?.at ?? 0) - (started + 300000)) < 5000, 'its end is five minutes from Start', { at: first?.at, started });
  check(/left$/.test(await text(s, 'practice-left') ?? ''), 'the row shows the time left', await text(s, 'practice-left'));
  await s.screenshot(resolve(out, '1-row.png'), true);
  // Away, the countdown is the Live Activity's; the app is back in front before the next op.
  simctl('launch', udid, 'com.apple.Preferences');
  await wait(2500);
  shot('2-island.png');
  foreground();
  await wait(2000);
  // The ring: AlarmKit's alert, at the end.
  const ringing = await until(s, () => practice(s), (p) => p?.state === 'alerting', Math.max(0, first.at - Date.now()) + 20000);
  check(ringing?.state === 'alerting', 'the practice rings at its end', ringing);
  shot('4-alert.png');
  check(await press(s, 'Repeat') != null, "the alert's Repeat pressed", null);
  const again = await until(s, () => practice(s), (p) => p?.state === 'countdown', 15000);
  check(Math.abs((again?.at ?? 0) - (Date.now() + 300000)) < 10000, 'Repeat starts a new five-minute countdown', again);
  foreground();
  await s.clock('+1000 real');
  check(await text(s, 'toggle') === 'Start', 'the metronome stopped at the ring and stays stopped', await text(s, 'toggle'));
  // The app ended and launched again reads the countdown the system shows.
  await s.close();
  s = await launch();
  const relaunched = await until(s, () => practice(s), (p) => p?.state === 'countdown', 20000);
  check(relaunched?.state === 'countdown' && Math.abs(relaunched.at - again.at) < 2000, 'a new launch reads the same end', { relaunched, again });
  await tap(s, 'practice-pause');
  const paused = await until(s, () => practice(s), (p) => p?.state === 'paused', 10000);
  check(paused?.state === 'paused' && paused.at == null && paused.remaining > 0, 'paused, with the time left', paused);
  shot('5-paused.png');
  await tap(s, 'practice-resume');
  const resumed = await until(s, () => practice(s), (p) => p?.state === 'countdown', 10000);
  check(resumed?.state === 'countdown' && resumed.at > Date.now(), 'resumed', resumed);
  // The second ring, ended from the app.
  const second = await until(s, () => practice(s), (p) => p?.state === 'alerting', Math.max(0, (resumed?.at ?? Date.now() + 300000) - Date.now()) + 20000);
  check(second?.state === 'alerting', 'the resumed countdown rings', second);
  foreground();
  await s.clock('+1000 real');
  await tap(s, 'practice-cancel');
  const gone = await until(s, () => practice(s), (p) => p == null, 10000);
  check(gone == null, 'Cancel during the ring ends it', gone);
  const lines = ((await s.logs({ all: true })).lines ?? []).filter((l) => /alarms: cancel/.test(l));
  console.log(`cancel: ${lines.join(' | ') || 'no line'}`);
  shot('6-after-cancel.png');
  check(await text(s, 'practice-start') != null, 'the row is the chooser again', null);
} finally {
  await s.close();
}
console.log(`alarms apple drive: ${failures.length ? `${failures.length} failure(s)` : 'ok'}; screenshots in ${out}`);
process.exit(failures.length ? 1 : 0);
