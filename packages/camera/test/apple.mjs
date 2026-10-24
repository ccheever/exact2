// @exact/camera's and media-capture's video half's conformance suite on
// Apple (LLP 1133.001 D7, LLP 1133.008 stage 2): the fixture app made as the
// web suite makes it, built for macOS or an iOS simulator, and driven by the
// agent with both implementations bound (`--bind`), so AVFoundation answers
// and not the agent's cameras.
//
// - macOS lets no tool grant the camera: without a person the suite checks
//   what holds before a grant and names what it skips. With --ask, the open
//   shows the system's dialog for a person to answer, and the suite then
//   takes photos with the Mac's built-in camera (never a phone's through
//   Continuity Camera): the preview, the photo written, a hidden viewfinder
//   letting the camera go and reopening it, and stopTrack.
// - The iOS simulator has no camera (Apple: "test your camera app on actual
//   devices"): the suite checks that the status says so and `getUserMedia`
//   fails `NotFoundError` before anything is asked. checklist.md is the
//   iPhone's.
//
// usage: bun packages/camera/test/apple.mjs [macos] [ios] [--sim <name|udid>] [--ask]
import { existsSync, readdirSync, rmSync } from 'node:fs';
import { basename, dirname, resolve } from 'node:path';
import { checker, makeFixture, root, run } from './fixture.mjs';

const argv = process.argv.slice(2), sim = argv.includes('--sim') ? argv[argv.indexOf('--sim') + 1] : process.env.EXACT_SIM;
const hosts = argv.filter((a, i) => ['macos', 'ios'].includes(a) && argv[i - 1] !== '--sim');
const { app } = makeFixture(), name = basename(app);
Object.assign(process.env, { EXACT_APP_DIR: app, EXACT2: root, HERMES_LEAN_SYS_OFFLINE: '1', EXACT_IDENTITY: process.env.EXACT_IDENTITY ?? '-' });
const { open } = await import(resolve(root, 'scripts/agent.mjs'));
const { check, failed } = checker();

/** Each testId's text in the tree. */
const texts = async (s) => {
  const { nodes } = await s.tree(), byId = new Map(nodes.map((n) => [n.id, n]));
  const text = (n) => n.props?.text ?? n.children.map((c) => text(byId.get(c))).join('');
  return Object.fromEntries(nodes.filter((n) => n.props?.testId).map((n) => [n.props.testId, text(n)]));
};
/** The text at `testId` once `want` holds, or the last seen; AVFoundation answers on its own time. */
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
  // On macOS the drive runs the binary inside the `.app`: the camera's dialog names a bundled app.
  if (host === 'macos') {
    const bundle = readdirSync(resolve(app, 'target'), { recursive: true }).find((f) => /macos/.test(f) && f.endsWith('.app/Contents/Info.plist'));
    if (!bundle) throw new Error('no macOS .app under target/ (build.mjs --bundle)');
    const contents = resolve(app, 'target', bundle, '..'), executable = JSON.parse(run('plutil', ['-convert', 'json', '-o', '-', resolve(contents, 'Info.plist')])).CFBundleExecutable;
    process.env.EXACT_MAC_BIN = resolve(contents, 'MacOS', executable);
  }
  // The build's Info.plist (D6): the usage key, its text the purpose.
  const plistAt = readdirSync(resolve(app, 'target'), { recursive: true }).find((f) => host === 'ios' ? /iphonesimulator|ios-sim/.test(f) && f.endsWith('.app/Info.plist') : /macos/.test(f) && f.endsWith('-Info.plist'));
  const plist = plistAt ? JSON.parse(run('plutil', ['-convert', 'json', '-o', '-', resolve(app, 'target', plistAt)])) : {};
  check(where, 'Info.plist carries NSCameraUsageDescription with the purpose', plist.NSCameraUsageDescription === 'The conformance suite takes a photo.', plistAt ?? 'no Info.plist under target/');
  const s = await open({ host, app: name, bind: 'media-capture,camera', ...(host === 'ios' && sim ? { sim } : {}) });
  try {
    const state = await s.state();
    check(where, 'both extensions are bound, not substituted', state.extensions?.camera?.bound === true && state.extensions?.['media-capture']?.bound === true, JSON.stringify(state.extensions));
    const status = await until(s, 'status', (t) => /^(true|false) (prompt|granted|denied)$/.test(t ?? ''));
    if (host === 'ios') {
      check(where, 'the simulator has no camera: cameraStatus says so', status === 'false prompt' || status === 'false granted' || status === 'false denied', status);
      await s.tap('open');
      const none = await until(s, 'open-error', (t) => !!t);
      check(where, 'getUserMedia(video) fails NotFoundError and asks nothing', none === 'NotFoundError', none);
      continue;
    }
    check(where, 'cameraStatus answers from AVFoundation', /^true (prompt|granted|denied)$/.test(status ?? ''), status);
    const before = (await texts(s)).cameras;
    if (status === 'true prompt') check(where, 'before a grant enumerateDevices names no camera', before === '1', before);
    if (status === 'true prompt' && argv.includes('--ask')) {
      console.log(`${where}: answer the camera dialog (Allow), within two minutes`);
      await s.tap('open-builtin');
      const asked = await until(s, 'status', (t) => t !== 'true prompt', 120000);
      check(where, 'the request asks, and the answer reaches the state', /^true (granted|denied)$/.test(asked ?? ''), asked);
    }
    if ((await texts(s)).status !== 'true granted') {
      console.log(`SKIP ${where} capture: the camera is not granted, and only a person grants it here (--ask)`);
      continue;
    }
    if ((await texts(s)).live !== 'video live false none true') await s.tap('open-builtin');
    const live = await until(s, 'live', (t) => t === 'video live false none true');
    check(where, 'the built-in camera opens, leased by the displayed viewfinder', live === 'video live false none true', live);
    const size = await until(s, 'size', (t) => /^[1-9]\d*x[1-9]\d*$/.test(t ?? ''));
    check(where, "the track's settings are the device's", /^[1-9]\d*x[1-9]\d*$/.test(size ?? ''), size);
    await s.tap('shutter');
    const shot = await until(s, 'shot', (t) => t && t !== 'none', 20000);
    check(where, 'takePhoto writes a JPEG at app:/tmp/camera/1.jpg, sized', /^app:\/tmp\/camera\/1\.jpg image\/jpeg true [1-9]\d*x[1-9]\d*$/.test(shot ?? ''), `${shot} ${(await texts(s))['shot-error']}`);
    await s.tap('toggle');
    const idle = await until(s, 'live', (t) => t === 'video live true idle true');
    check(where, 'a hidden viewfinder leaves the track idle', idle === 'video live true idle true', idle);
    const logs = JSON.stringify(await s.logs());
    check(where, 'and the capture stops', logs.includes('capture stopped'), 'no "capture stopped" in the journal');
    await s.tap('toggle');
    const again = await until(s, 'live', (t) => t === 'video live false none true');
    check(where, 'shown again, the viewfinder takes the camera back', again === 'video live false none true', again);
    await s.tap('stop');
    const ended = await until(s, 'live', (t) => t?.startsWith('video ended'));
    check(where, 'stopTrack ends the track', ended?.startsWith('video ended'), ended);
  } finally {
    await s.close();
  }
}
if (existsSync(app)) rmSync(dirname(app), { recursive: true, force: true });
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
