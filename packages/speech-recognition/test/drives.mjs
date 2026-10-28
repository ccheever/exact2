// LLP 1133.012 §10's recorder drives under the agent: the implementation's
// agent path answers behind the runner's permission and its tickets, which
// stand for the person, and `prefer extension speech-recognition.heard`
// stands for speech. Drive 4 (a tool's invocation) has no door on these
// hosts; contract/cli/tests/it/module_seam.rs drives it through the runner.
// Drive 7 is a person's (test/device.md).
//
// usage: bun packages/speech-recognition/test/drives.mjs [web|macos|ios|android] [--only 1,2,…]
//   Builds nothing: build the recorder for the host first (`bun host/web/build.mjs
//   recorder`; `bun host/apple/build.mjs recorder-apple`, `--ios --sim <udid>` for a
//   simulator; `bun scripts/agent-android.mjs build recorder --apk`, driven with
//   EXACT_ANDROID_READER=1 and ANDROID_SERIAL naming the emulator), which the driver
//   checks is current.
import { resolve } from 'node:path';

const root = resolve(import.meta.dir, '../../..');
const argv = process.argv.slice(2), host = argv.find((a) => ['web', 'macos', 'ios', 'android'].includes(a)) ?? 'web';
const only = argv.includes('--only') ? argv[argv.indexOf('--only') + 1].split(',').map(Number) : null;
const { open } = await import(resolve(root, 'scripts/agent.mjs'));
let failures = 0;
const check = (n, name, ok, detail) => {
  if (!ok) failures++;
  console.log(`${ok ? 'PASS' : 'FAIL'} ${host} drive ${n} ${name}${ok ? '' : `: ${detail}`}`);
};
const run = Date.now().toString(36);

async function launch(name, permissions, status) {
  const s = await open({ host, app: 'recorder', storage: `speech-${run}-${name}` });
  await s.prefer(Object.fromEntries(Object.entries(permissions).map(([k, v]) => [`permission.${k}`, v])));
  if (status) await s.prefer({ extension: `speech-recognition.speechRecognitionStatus=${JSON.stringify(status)}` });
  await s.clock('settle');
  return s;
}
const heard = (s, phrase) => s.prefer({ extension: `speech-recognition.heard=${JSON.stringify(phrase)}` });
const texts = async (s) => {
  const { nodes } = await s.tree(), byId = new Map(nodes.map((n) => [n.id, n]));
  const text = (n) => n.props?.text ?? (n.children ?? []).map((c) => text(byId.get(c))).join('');
  return Object.fromEntries(nodes.filter((n) => n.props?.testId).map((n) => [n.props.testId, text(n)]));
};
const tickets = async (s) => ((await s.state()).pending ?? []).filter((p) => p.device?.capability === 'permission');
/** Answers the one permission ticket held now; false when none is. */
async function allow(s) {
  await s.clock('settle');
  const [held] = await tickets(s);
  if (!held) return false;
  await s.answer('tap', `@${held.ticket}`, 'allow');
  await s.clock('settle');
  return true;
}
const takes = async (s) => (await s.state()).resources.library.takes;
/** The state once `want` holds of it, waiting on the wall clock up to 10 s: on Android's reader an extension answers
 * on the main thread, which `clock settle`, holding exact2's, cannot wait for. */
async function until(s, want) {
  const end = Date.now() + 10000;
  let st = await s.state();
  while (!want(st) && Date.now() < end) {
    await s.clock('+250 real');
    st = await s.state();
  }
  return st;
}
const drive = (n) => !only || only.includes(n);

async function each(n, name, body) {
  if (!drive(n)) return;
  let s;
  try {
    s = await body((label, perms, status) => launch(`${n}-${label}`, perms, status).then((x) => (s = x)));
  } catch (e) {
    check(n, name, false, e.stack ?? e.message);
  } finally {
    await s?.close?.();
  }
}

await each(1, 'a live transcript', async (go) => {
  const s = await go('live', { microphone: 'prompt', 'speech-recognition': 'prompt' });
  await s.tap('toggle');
  check(1, 'the microphone asks first', await allow(s), 'no ticket');
  check(1, 'then speech recognition', await allow(s), 'no second ticket');
  await heard(s, 'pick up milk and eggs');
  await s.clock('+3000');
  check(1, 'the line shows the phrase', (await texts(s))['live-transcript'] === 'pick up milk and eggs', JSON.stringify(await texts(s)));
  await s.tap('toggle');
  await s.clock('settle');
  const st = await until(s, (x) => x.slots.stopped?.readyState === 'ended' && x.resources.library.takes[0]?.complete), [take] = st.resources.library.takes;
  check(1, 'the row shows its transcript', take?.transcript === 'pick up milk and eggs' && take.complete, JSON.stringify(take));
  check(1, 'one file of one result', st.slots.transcribed?.files === 1 && st.slots.transcribed?.finalCount === 1, JSON.stringify(st.slots.transcribed));
  check(1, 'the track ended', st.slots.stopped?.readyState === 'ended', JSON.stringify(st.slots.stopped));
  return s;
});

await each(2, 'speech recognition denied', async (go) => {
  const s = await go('denied', { microphone: 'prompt', 'speech-recognition': 'denied' });
  await s.tap('toggle');
  check(2, 'the microphone asks', await allow(s), 'no ticket');
  check(2, 'no second ticket', !(await allow(s)), 'a second ticket');
  await s.clock('+2000');
  await s.tap('toggle');
  await s.clock('settle');
  const [take] = (await until(s, (x) => x.resources.library.takes.length > 0)).resources.library.takes, t = await texts(s);
  check(2, 'the take records with no transcript', take && take.transcript === '', JSON.stringify(take));
  check(2, 'the denied line', 'speech-denied' in t, JSON.stringify(t));
  return s;
});

await each(3, 'a transcript from the file', async (go) => {
  const s = await go('file', { microphone: 'granted', 'speech-recognition': 'prompt' }, { inputs: ['microphone', 'file'] });
  await s.tap('toggle');
  await s.clock('settle');
  check(3, 'no ticket while recording', (await tickets(s)).length === 0, JSON.stringify(await tickets(s)));
  await s.clock('+2000');
  check(3, 'no live line', !('live-transcript' in (await texts(s))), JSON.stringify(await texts(s)));
  await s.tap('toggle');
  await s.clock('settle');
  await heard(s, 'call the plumber');
  await s.tap('transcribe-take-1');
  check(3, 'Transcribe asks speech recognition', await allow(s), 'no ticket');
  const [take] = await takes(s);
  check(3, 'its transcript is the phrase', take?.transcript === 'call the plumber' && take.complete, JSON.stringify(take));
  return s;
});

await each(3, 'neither where only the microphone is taken', async (go) => {
  const s = await go('neither', { microphone: 'granted', 'speech-recognition': 'granted' }, { inputs: ['microphone'] });
  await s.tap('toggle');
  await s.clock('+2000');
  check(3, 'no live line', !('live-transcript' in (await texts(s))), JSON.stringify(await texts(s)));
  await s.tap('toggle');
  await s.clock('settle');
  check(3, 'no Transcribe', !('transcribe-take-1' in (await texts(s))), JSON.stringify(await texts(s)));
  return s;
});

await each(5, 'a stop before the start is answered', async (go) => {
  const s = await go('overtaken', { microphone: 'granted', 'speech-recognition': 'prompt' });
  await s.tap('toggle');
  await s.clock('settle');
  check(5, 'the start waits on its ticket', (await tickets(s)).length === 1, JSON.stringify(await tickets(s)));
  await s.tap('toggle');
  await s.clock('settle');
  const st = await s.state(), logs = (await s.logs()).join?.('\n') ?? JSON.stringify(await s.logs());
  check(5, 'its ticket is retired', (await tickets(s)).length === 0, JSON.stringify(st.pending));
  check(5, 'no recognition started', !/recognition-\d+: on track/.test(logs), logs);
  check(5, 'the take records', (await takes(s)).length === 1, JSON.stringify(await takes(s)));
  return s;
});

await each(6, 'thirty phrases, then a backgrounded take', async (go) => {
  const s = await go('long', { microphone: 'granted', 'speech-recognition': 'granted' });
  await s.tap('toggle');
  await s.clock('settle');
  for (let i = 1; i <= 30; i++) {
    await heard(s, `phrase number ${i}`);
    await s.clock('+2000');
  }
  const window = (await s.state()).resources.heard;
  check(6, 'the window holds the last 20', window.results.length === 20 && window.results[0].alternatives[0].transcript === 'phrase number 11', JSON.stringify(window.results.map((r) => r.index)));
  check(6, 'finalCount reads 30', window.finalCount === 30, String(window.finalCount));
  await s.tap('toggle');
  await s.clock('settle');
  const done = (await s.state()).slots.transcribed;
  check(6, 'one file of 30 lines', done?.files === 1 && done.finalCount === 30, JSON.stringify(done));

  await s.tap('toggle');
  await s.clock('settle');
  await heard(s, 'first half');
  await s.clock('+2000');
  await s.prefer({ 'visibility-state': 'hidden' });
  await s.clock('+1000');
  await s.prefer({ 'visibility-state': 'visible' });
  await s.clock('settle');
  await s.tap('toggle');
  await s.clock('settle');
  const cut = (await takes(s))[1];
  check(6, 'leaving the foreground aborts it', (await s.state()).slots.transcribed?.error === 'aborted' && cut && !cut.complete, JSON.stringify(cut));
  await heard(s, 'the whole take');
  await s.tap('transcribe-take-2');
  await s.clock('settle');
  const whole = (await takes(s))[1];
  check(6, 'Transcribe replaces the partial transcript', whole?.transcript === 'the whole take' && whole.complete, JSON.stringify(whole));
  return s;
});

console.log(failures ? `${failures} failed` : 'all passed');
process.exit(failures ? 1 : 0);
