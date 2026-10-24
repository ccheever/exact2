// @exact/speech-synthesis's conformance suite on Apple (LLP 1133.001 D7,
// LLP 1133.012 §10, stage 2): the fixture app made as the web suite makes it,
// built for macOS or an iOS simulator, and driven by the agent with the
// implementation bound (`--bind speech-synthesis`), so AVSpeechSynthesizer
// speaks and not the substitute. It speaks aloud.
//
// usage: bun packages/speech-synthesis/test/apple.mjs [macos] [ios] [--sim <name|udid>]
import { rmSync } from 'node:fs';
import { basename, dirname, resolve } from 'node:path';
import { checker, makeFixture, root, run } from './fixture.mjs';

const argv = process.argv.slice(2), sim = argv.includes('--sim') ? argv[argv.indexOf('--sim') + 1] : process.env.EXACT_SIM;
const hosts = argv.filter((a, i) => ['macos', 'ios'].includes(a) && argv[i - 1] !== '--sim');
const { app } = makeFixture(), name = basename(app);
// The fixture's build is gigabytes: it goes however the suite ends.
process.on('exit', () => rmSync(dirname(app), { recursive: true, force: true }));
Object.assign(process.env, { EXACT_APP_DIR: app, EXACT2: root, HERMES_LEAN_SYS_OFFLINE: '1' });
const { open } = await import(resolve(root, 'scripts/agent.mjs'));
const { check, failed } = checker();

/** Each testId's text in the tree. */
const texts = async (s) => {
  const { nodes } = await s.tree(), byId = new Map(nodes.map((n) => [n.id, n]));
  const text = (n) => n.props?.text ?? n.children.map((c) => text(byId.get(c))).join('');
  return Object.fromEntries(nodes.filter((n) => n.props?.testId).map((n) => [n.props.testId, text(n)]));
};
/** The text at `testId` once `want` holds, or the last seen; the synthesizer speaks on the wall clock. */
const until = async (s, testId, want, ms = 15000) => {
  const end = Date.now() + ms;
  let got;
  while (Date.now() < end) {
    await s.clock('+100 real');
    if (want(got = (await texts(s))[testId] ?? '')) return got;
  }
  return got;
};

for (const host of hosts.length ? hosts : ['macos', 'ios']) {
  const where = host === 'ios' ? 'ios-simulator' : 'macos';
  run('bun', [resolve(root, 'host/apple/build.mjs'), ...(host === 'ios' ? ['--ios', ...(sim ? ['--sim', sim] : [])] : []), `${name}-apple`], root);
  const s = await open({ host, app: name, bind: 'speech-synthesis', storage: `synthesis-${Date.now()}` });
  try {
    const state = await s.state();
    check(where, 'the extension is bound, not substituted', state.extensions?.['speech-synthesis']?.bound === true, JSON.stringify(state.extensions));
    const voices = await until(s, 'voices', (t) => Number(t.split(' ')[0]) > 0, 5000);
    check(where, 'speechSynthesisVoices lists the system\'s voices, one the default', Number(voices.split(' ')[0]) > 0 && Number(voices.split(' ')[1]) >= 1, voices);
    const idle = (await texts(s)).state;
    check(where, 'idle: available, nothing speaking', /^true false false false 0 0 - 0 auto$/.test(idle), idle);

    // Two utterances, the second queued behind the first: the first's words reached in order, both ending `end`.
    await s.tap('say');
    const ids = await until(s, 'ids', (t) => /^utterance-\S+ utterance-\S+$/.test(t));
    check(where, 'speak answers an id at once', /^utterance-\S+ utterance-\S+$/.test(ids), ids);
    const queued = await until(s, 'state', (t) => /^true true true false/.test(t));
    // The unsaid type is the app's: `transient` in an app whose audio_session is not `playback` (D3).
    check(where, 'one speaks while the other is queued, claiming transient', /^true true true false \d+ \d+ \S+ 1 transient$/.test(queued), queued);
    const seen = new Set(), end = Date.now() + 20000;
    while (Date.now() < end) {
      const t = await texts(s), [, speaking, , , charIndex] = t.state.split(' ');
      if (speaking === 'true') seen.add(Number(charIndex));
      if (t.first.startsWith('ended')) break;
      await s.clock('+30 real');
    }
    const order = [...seen].sort((a, b) => a - b);
    check(where, 'word boundaries reach the state in order', order.length >= 3 && order.includes(4), JSON.stringify(order));
    const first = await until(s, 'first', (t) => t.startsWith('ended'));
    check(where, 'the first ends `end`', first === 'ended end -', first);
    const last = await until(s, 'last', (t) => t.startsWith('ended'));
    check(where, 'then the queued one speaks and ends `end`', last === 'ended end -', last);
    const after = await until(s, 'state', (t) => /^true false false false 0 0 - 0 auto$/.test(t));
    check(where, 'the queue empty, the state is idle and claims nothing', /^true false false false 0 0 - 0 auto$/.test(after), after);

    // Pause holds the queue and its claim; resume lets it finish.
    await s.tap('say');
    await until(s, 'state', (t) => /^true true /.test(t) && Number(t.split(' ')[4]) > 0);
    await s.tap('pause');
    const paused = await until(s, 'state', (t) => t.split(' ')[3] === 'true');
    await s.clock('+1500 real');
    const t = await texts(s);
    check(where, 'paused, the queue holds and claims nothing', t.state.split(' ')[3] === 'true' && t.state.split(' ')[4] === paused.split(' ')[4] && t.state.endsWith(' auto') && t.first.startsWith('paused'), `${t.state}; ${t.first}`);
    await s.tap('resume');
    const resumed = await until(s, 'last', (t) => t.startsWith('ended'));
    check(where, 'resumed, both end', resumed === 'ended end -' && (await texts(s)).first === 'ended end -', `${(await texts(s)).first}; ${resumed}`);

    // Cancel: the speaking one `interrupted`, the queued one `canceled`.
    await s.tap('say');
    await until(s, 'state', (t) => /^true true true /.test(t));
    await s.tap('cancel');
    const interrupted = await until(s, 'first', (t) => t.startsWith('ended'));
    const canceled = await until(s, 'last', (t) => t.startsWith('ended'));
    check(where, 'cancel: the speaking one interrupted, the queued one canceled', interrupted === 'ended error interrupted' && canceled === 'ended error canceled', `${interrupted}; ${canceled}`);

    // A voice the system does not have.
    await s.tap('say-bad-voice');
    const bad = await until(s, 'first', (t) => t.includes('voice-unavailable'));
    check(where, 'an unknown voice ends voice-unavailable', bad === 'ended error voice-unavailable', bad);

    // A playback utterance claims the session's spoken-audio configuration (D4).
    await s.tap('say-playback');
    const claimed = await until(s, 'state', (t) => /^true true /.test(t));
    check(where, 'a playback utterance claims playback', claimed.endsWith(' playback'), claimed);
    const logs = JSON.stringify(await s.logs());
    check(where, 'the claim reaches the host\'s audio session owner', /audio session: claimed for speech \(playback\)/.test(logs), logs.slice(-2000));
    await s.tap('cancel');
    const released = await until(s, 'state', (t) => / auto$/.test(t));
    check(where, 'and gives it back when the queue empties', released.endsWith(' auto') && /audio session: a (speech )?claim released/.test(JSON.stringify(await s.logs())), released);
  } catch (e) {
    check(where, 'ran', false, e.stack ?? e.message);
  } finally {
    await s.close();
  }
}
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
