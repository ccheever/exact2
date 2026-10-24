// @exact/recording's and @exact/media-capture's conformance suite on Apple
// (LLP 1133.001 D7, LLP 1133.009 §11, stage 2): the fixture app made as the
// web suite makes it, built for an iOS simulator or macOS, and driven by the
// agent with both implementations bound (`--bind media-capture,recording`),
// so AVAudioEngine records and not the substitute. An iOS simulator records
// from the Mac's microphone once `simctl privacy grant microphone` allows it;
// macOS lets no tool grant the microphone, so there the suite checks what
// holds before a person's grant and names what it skips (`--ask` opens the
// system's dialog for a person to answer).
//
// usage: bun packages/recording/test/apple.mjs [macos] [ios] [--sim <name|udid>] [--ask]
import { readdirSync, rmSync } from 'node:fs';
import { basename, dirname, resolve } from 'node:path';
import { checker, makeFixture, root, run } from './fixture.mjs';

const argv = process.argv.slice(2), sim = argv.includes('--sim') ? argv[argv.indexOf('--sim') + 1] : process.env.EXACT_SIM;
const hosts = argv.filter((a, i) => ['macos', 'ios'].includes(a) && argv[i - 1] !== '--sim');
const { app, id } = makeFixture(), name = basename(app);
Object.assign(process.env, { EXACT_APP_DIR: app, EXACT2: root, HERMES_LEAN_SYS_OFFLINE: '1' });
const { open } = await import(resolve(root, 'scripts/agent.mjs'));
const { check, failed } = checker();

/** Each testId's text in the tree. */
const texts = async (s) => {
  const { nodes } = await s.tree(), byId = new Map(nodes.map((n) => [n.id, n]));
  const text = (n) => n.props?.text ?? n.children.map((c) => text(byId.get(c))).join('');
  return Object.fromEntries(nodes.filter((n) => n.props?.testId).map((n) => [n.props.testId, text(n)]));
};
/** The text at `testId` once `want` holds, or the last seen; the platform answers on its own time. */
const until = async (s, testId, want, ms = 15000) => {
  const end = Date.now() + ms;
  let got;
  while (Date.now() < end) {
    await s.clock('+250 real');
    if (want(got = (await texts(s))[testId])) return got;
  }
  return got;
};

for (const host of hosts.length ? hosts : ['macos', 'ios']) {
  const where = host === 'ios' ? 'ios-simulator' : 'macos';
  run('bun', [resolve(root, 'host/apple/build.mjs'), ...(host === 'ios' ? ['--ios', ...(sim ? ['--sim', sim] : [])] : ['--bundle']), `${name}-apple`], root);
  if (host === 'macos') {
    // The binary inside the `.app`: TCC asks for a bundled app, not a bare executable.
    const bundle = readdirSync(resolve(app, 'target'), { recursive: true }).find((f) => /macos/.test(f) && f.endsWith('.app/Contents/Info.plist'));
    if (!bundle) throw new Error('no macOS .app under target/ (build.mjs --bundle)');
    const contents = resolve(app, 'target', bundle, '..'), executable = JSON.parse(run('plutil', ['-convert', 'json', '-o', '-', resolve(contents, 'Info.plist')])).CFBundleExecutable;
    process.env.EXACT_MAC_BIN = resolve(contents, 'MacOS', executable);
  }
  let udid = null;
  if (host === 'ios') {
    const booted = JSON.parse(run('xcrun', ['simctl', 'list', 'devices', 'booted', '--json'])).devices;
    udid = Object.values(booted).flat().find((d) => !sim || d.name === sim || d.udid === sim)?.udid;
    if (!udid) throw new Error('no booted simulator: the build installs onto one (--sim <name|udid>)');
    run('xcrun', ['simctl', 'privacy', udid, 'grant', 'microphone', id]);
  }
  const plistAt = readdirSync(resolve(app, 'target'), { recursive: true }).find((f) => host === 'ios' ? /iphonesimulator|ios-sim/.test(f) && f.endsWith('.app/Info.plist') : /macos/.test(f) && f.endsWith('-Info.plist'));
  const plist = plistAt ? JSON.parse(run('plutil', ['-convert', 'json', '-o', '-', resolve(app, 'target', plistAt)])) : {};
  check(where, 'Info.plist carries the microphone\'s purpose (D9)', plist.NSMicrophoneUsageDescription === 'The conformance suite records a take.', plistAt ?? 'no Info.plist under target/');
  const s = await open({ host, app: name, bind: 'media-capture,recording', storage: `recording-${Date.now()}` });
  try {
    const state = await s.state();
    check(where, 'both extensions are bound, not substituted', state.extensions?.['media-capture']?.bound === true && state.extensions?.recording?.bound === true, JSON.stringify(state.extensions));
    const status = await until(s, 'status', (t) => /^(true|false) (prompt|granted|denied)$/.test(t ?? ''));
    check(where, 'microphoneStatus answers from the platform', /^true (prompt|granted|denied)$/.test(status ?? ''), status);
    if (status === 'true prompt' && argv.includes('--ask')) {
      console.log(`${where}: answer the microphone dialog (Allow), within two minutes`);
      await s.tap('open');
      await until(s, 'status', (t) => t !== 'true prompt', 120000);
    }
    if ((await texts(s)).status !== 'true granted') {
      console.log(`SKIP ${where} capture: the microphone is not granted, and only a person grants it here (--ask)`);
      continue;
    }
    await s.tap('open-nothing');
    const nothing = await until(s, 'opened-error', (t) => !!t);
    check(where, 'getUserMedia with no kind is TypeError', nothing === 'TypeError', nothing);
    await s.tap('open');
    const opened = await until(s, 'opened', (t) => /^stream-\S+ 1$/.test(t ?? ''));
    check(where, 'getUserMedia answers a stream with one track', /^stream-\S+ 1$/.test(opened ?? ''), opened);
    const idle = await until(s, 'track', (t) => t === 'live true idle true');
    check(where, 'a track no consumer joined is idle (D6)', idle === 'live true idle true', idle);
    await s.tap('record');
    const started = await until(s, 'started', (t) => t?.startsWith('recording'));
    check(where, 'startMediaRecorder records AAC to the admitted path, with its extension', started === 'recording audio/mp4 app:/data/takes/take-1.m4a', started);
    const live = await until(s, 'track', (t) => t === 'live false - true');
    check(where, 'a recorder leases its track: the engine runs, unmuted', live === 'live false - true', live);
    await s.tap('meter');
    const level = await until(s, 'level', (t) => Number(t?.split(' ')[0]) >= 10);
    check(where, 'the level is polled from the input', Number(level?.split(' ')[0]) >= 10, level);
    await s.tap('meter');
    await s.tap('pause');
    const paused = await until(s, 'rec', (t) => t?.startsWith('paused'));
    check(where, 'pause', /^paused \d+(\.\d+)? requested$/.test(paused ?? ''), paused);
    await s.tap('resume');
    await until(s, 'rec', (t) => t?.startsWith('recording'));
    await s.clock('+1500 real');
    await s.tap('stop');
    const saved = await until(s, 'saved', (t) => t && t !== 'none');
    check(where, 'stop answers the finished file', saved === 'app:/data/takes/take-1.m4a audio/mp4 true', saved);
    await s.clock('+1000 real');
    const media = (await s.state()).media ?? [];
    check(where, 'the file plays (fragmented MP4, AAC)', media.some((m) => (m.state?.duration ?? 0) > 1 && !m.state?.error), JSON.stringify(media.map((m) => m.state)));
    const idleAgain = await until(s, 'track', (t) => t === 'live true idle true');
    check(where, 'its last consumer gone, the track is idle', idleAgain === 'live true idle true', idleAgain);
    await s.tap('record-mp4');
    await until(s, 'rec', (t) => t?.startsWith('recording'));
    await s.tap('end-track');
    const ended = await until(s, 'rec', (t) => t?.startsWith('inactive'));
    check(where, 'a track that ends stops its recorder', /^inactive \d+(\.\d+)? track-ended$/.test(ended ?? ''), ended);
    await s.tap('bogus');
    const unknown = await until(s, 'trk-error', (t) => !!t);
    check(where, 'an id the session does not hold is NotFoundError', unknown === 'NotFoundError', unknown);
    await s.tap('record-outside');
    const outside = await until(s, 'started-error', (t) => !!t);
    check(where, 'a path outside the fs.write grant is refused', outside === 'NotAllowedError', outside);
  } finally {
    await s.close();
  }
}
rmSync(dirname(app), { recursive: true, force: true });
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
