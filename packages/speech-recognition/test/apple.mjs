// @exact/speech-recognition's conformance suite on Apple (LLP 1133.001 D7,
// LLP 1133.012 §10, stage 2): the fixture app made as the web suite makes it,
// built for macOS or an iOS simulator, and driven by the agent with both
// implementations bound (`--bind media-capture,speech-recognition`), so the
// platform's recognizer hears the spoken file, placed in the app's data before
// it opens. A track recognition needs a microphone that answers, which this
// suite cannot count on: that is test/device.md's.
//
// usage: bun packages/speech-recognition/test/apple.mjs [macos] [ios] [--sim <name|udid>]
import { cpSync, existsSync, mkdirSync, readdirSync, rmSync } from 'node:fs';
import { homedir } from 'node:os';
import { basename, dirname, join, resolve } from 'node:path';
import { PHRASE, checker, makeFixture, root, run, spokenFile } from './fixture.mjs';

const argv = process.argv.slice(2), sim = argv.includes('--sim') ? argv[argv.indexOf('--sim') + 1] : process.env.EXACT_SIM;
const hosts = argv.filter((a, i) => ['macos', 'ios'].includes(a) && argv[i - 1] !== '--sim');
const { app, id } = makeFixture(), name = basename(app);
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
/** The text at `testId` once `want` holds, or the last seen; the recognizer answers on its own time. */
const until = async (s, testId, want, ms = 20000) => {
  const end = Date.now() + ms;
  let got;
  while (Date.now() < end) {
    await s.clock('+250 real');
    if (want(got = (await texts(s))[testId] ?? '')) return got;
  }
  return got;
};
/** PHRASE forty times, two minutes of speech. */
function longFile() {
  const file = join(homedir(), '.cache/exact/speech/pick-up-milk-long.wav');
  if (!existsSync(file)) run('say', ['-o', file, '--data-format=LEI16@16000', Array(40).fill(`${PHRASE}.`).join(' ')], dirname(file));
  return file;
}
const words = (t) => t.toLowerCase().replace(/[^a-z ]/g, '');

for (const host of hosts.length ? hosts : ['macos', 'ios']) {
  const where = host === 'ios' ? 'ios-simulator' : 'macos';
  run('bun', [resolve(root, 'host/apple/build.mjs'), ...(host === 'ios' ? ['--ios', ...(sim ? ['--sim', sim] : [])] : ['--bundle']), `${name}-apple`], root);
  const storage = `recognition-${Date.now()}`;
  let data;
  if (host === 'macos') {
    // The binary inside the `.app`: TCC asks for a bundled app, not a bare executable.
    const bundle = readdirSync(resolve(app, 'target'), { recursive: true }).find((f) => /macos/.test(f) && f.endsWith('.app/Contents/Info.plist'));
    if (!bundle) throw new Error('no macOS .app under target/ (build.mjs --bundle)');
    const contents = resolve(app, 'target', bundle, '..'), executable = JSON.parse(run('plutil', ['-convert', 'json', '-o', '-', resolve(contents, 'Info.plist')])).CFBundleExecutable;
    process.env.EXACT_MAC_BIN = resolve(contents, 'MacOS', executable);
    data = join(homedir(), 'Library/Caches/exact', id, 'agent', storage, 'data');
  } else {
    const booted = JSON.parse(run('xcrun', ['simctl', 'list', 'devices', 'booted', '--json'])).devices;
    const udid = Object.values(booted).flat().find((d) => !sim || d.name === sim || d.udid === sim)?.udid;
    if (!udid) throw new Error('no booted simulator: the build installs onto one (--sim <name|udid>)');
    const container = run('xcrun', ['simctl', 'get_app_container', udid, id, 'data']).trim();
    data = join(container, 'Library/Caches/exact', id, 'agent', storage, 'data');
  }
  mkdirSync(data, { recursive: true });
  cpSync(spokenFile(), join(data, 'spoken.wav'));
  const plistAt = readdirSync(resolve(app, 'target'), { recursive: true }).find((f) => host === 'ios' ? /iphonesimulator|ios-sim/.test(f) && f.endsWith('.app/Info.plist') : /macos/.test(f) && f.endsWith('-Info.plist'));
  const plist = plistAt ? JSON.parse(run('plutil', ['-convert', 'json', '-o', '-', resolve(app, 'target', plistAt)])) : {};
  check(where, 'Info.plist carries the recognizer\'s purpose (D11)', plist.NSSpeechRecognitionUsageDescription === 'The conformance suite recognises what the track says.', plistAt ?? 'no Info.plist under target/');
  const s = await open({ host, app: name, bind: 'media-capture,speech-recognition', storage });
  try {
    const state = await s.state();
    check(where, 'the extension is bound, not substituted', state.extensions?.['speech-recognition']?.bound === true, JSON.stringify(state.extensions));
    // The SpeechAnalyzer engines need no authorization and run on the device; the inputs are a track and a file (D8).
    const status = await until(s, 'status', (t) => /^(true|false) /.test(t), 5000);
    check(where, 'the status: available, granted, a track or a file, local', status === 'true granted track,file true', status);
    await s.tap('on-nothing');
    const none = await until(s, 'started-error', (t) => !!t);
    check(where, 'a start with no input is TypeError', none === 'TypeError', none);
    await s.tap('on-microphone');
    const microphone = await until(s, 'held-error', (t) => !!t);
    check(where, 'microphone=true is NotSupportedError: a track carries the microphone here', microphone === 'NotSupportedError', microphone);

    const availability = await until(s, 'availability', (t) => !!t);
    check(where, `availability is a word (${availability})`, /^(unavailable|downloadable|downloading|available)$/.test(availability), availability);
    if (availability !== 'available') {
      await s.tap('install');
      const installed = await until(s, 'installed', (t) => t !== 'none', 300000);
      check(where, 'installSpeechRecognition installs en-US', installed === 'true', installed);
      const now = await until(s, 'availability', (t) => t === 'available', 10000);
      check(where, `and availability then reads available, or unavailable where no model runs (${now})`, /^(available|unavailable)$/.test(now), now);
    }

    if ((await texts(s)).availability !== 'available') {
      // The iOS simulator offers DictationTranscriber alone and runs none of its models.
      await s.tap('on-file');
      const refused = await until(s, 'heard', (t) => / ended /.test(t));
      check(where, 'with no model that runs, a file recognition ends language-not-supported', / ended .* language-not-supported$/.test(refused), refused);
      console.log(`SKIP ${where} file recognition and abort: no recognizer model runs here (test/device.md)`);
      continue;
    }
    // The spoken file, heard on the device.
    await s.tap('on-file');
    const started = await until(s, 'started', (t) => /^recognition-\S+ false -$/.test(t));
    check(where, 'a file recognition starts on the device', /^recognition-\S+ false -$/.test(started), `${started}; ${(await texts(s))['started-error']}`);
    const ended = await until(s, 'heard', (t) => / ended /.test(t), 60000);
    const finals = (await texts(s)).finals;
    const said = async () => JSON.stringify((await s.logs({ all: true })).lines.filter((l) => /speech-recognition/.test(JSON.stringify(l))));
    check(where, 'it ends at the file\'s end with its results and no error', / ended false false [1-9]\d* false -$/.test(ended), `${ended}; ${await said()}`);
    check(where, `it hears "${PHRASE}"`, words(finals).includes('pick up milk'), finals);

    // Abort, while a long file is being heard.
    cpSync(longFile(), join(data, 'spoken.wav'));
    await s.tap('on-file');
    // The analyzer hears a file many times faster than it plays: abort once the start has answered.
    await s.clock('data');
    await s.tap('abort');
    const aborted = await until(s, 'heard', (t) => / ended /.test(t));
    check(where, 'abort ends it aborted', / ended .* aborted$/.test(aborted), aborted);
  } catch (e) {
    check(where, 'ran', false, e.stack ?? e.message);
  } finally {
    await s.close();
  }
}
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
