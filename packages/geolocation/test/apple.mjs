// @exact/geolocation's conformance suite on Apple (LLP 1133.001 D7, LLP
// 1133.002 stage 2): the fixture app made as the web suite makes it, built
// for macOS or an iOS simulator, and driven by the agent with the Apple
// implementation bound (`--bind geolocation`), so CoreLocation answers and
// not the substitute. The platform's own means answer its side: on a
// simulator `simctl privacy grant` and `simctl location set`. macOS lets no
// tool grant location, so there the suite checks what holds before a
// person's grant and names what it skips. On iOS a second fixture, granted
// the approximate line, checks that line's ceiling.
//
// usage: bun packages/geolocation/test/apple.mjs [macos] [ios] [--sim <name|udid>] [--ask]
//   (--ask: on macOS, the request opens the system's dialog for a person to answer)
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
/** The text at `testId` once `want` holds, or the last seen; CoreLocation answers on its own time. */
const until = async (s, testId, want, ms = 15000) => {
  const end = Date.now() + ms;
  let got;
  while (Date.now() < end) {
    await s.clock('+250 real');
    if (want(got = (await texts(s))[testId])) return got;
  }
  return got;
};
const WORDS = ['notDetermined', 'restricted', 'denied', 'authorizedWhenInUse', 'authorizedAlways'];

/** The approximate line as a ceiling on iOS (D7): the fixture made with it,
 * granted full accuracy, reads approximate positions and status. */
async function approximate(udid) {
  const where = 'ios-simulator approximate', fixture = makeFixture({ approximate: true });
  process.env.EXACT_APP_DIR = fixture.app;
  try {
    run('bun', [resolve(root, 'host/apple/build.mjs'), '--ios', '--sim', udid, `${name}-apple`], root);
    run('xcrun', ['simctl', 'privacy', udid, 'grant', 'location', fixture.id]);
    run('xcrun', ['simctl', 'location', udid, 'set', '48.8566,2.3522']);
    const s = await open({ host: 'ios', app: name, bind: 'geolocation' });
    try {
      const status = await until(s, 'status', (t) => /^true granted/.test(t ?? ''));
      check(where, 'full accuracy granted, the status is approximate', status === 'true granted approximate', status);
      await s.tap('watch');
      const spot = await until(s, 'spot', (t) => /^-?\d/.test(t ?? ''), 30000);
      const [lat, lon] = (spot ?? '').split(',').map(Number);
      const off = Math.hypot(lat - 48.8566, (lon - 2.3522) * Math.cos(lat * Math.PI / 180)) * 111;
      check(where, 'the position is reduced: near, not the point', spot !== '48.8566,2.3522' && off < 25, `${spot} (${off.toFixed(2)} km)`);
      await s.tap('raise');
      const precise = await until(s, 'precise-error', (t) => t !== undefined, 5000);
      const after = (await texts(s)).status;
      check(where, 'requestPreciseLocation asks nothing and stays approximate', precise === '' && after === 'true granted approximate', `${precise} ${after}`);
    } finally {
      await s.close();
    }
  } finally {
    process.env.EXACT_APP_DIR = app;
    rmSync(dirname(fixture.app), { recursive: true, force: true });
  }
}

for (const host of hosts.length ? hosts : ['macos', 'ios']) {
  const where = host === 'ios' ? 'ios-simulator' : 'macos';
  run('bun', [resolve(root, 'host/apple/build.mjs'), ...(host === 'ios' ? ['--ios', ...(sim ? ['--sim', sim] : [])] : ['--bundle']), `${name}-apple`], root);
  // On macOS the drive runs the binary inside the `.app`: CoreLocation shows its dialog for a bundled app, not a bare executable.
  if (host === 'macos') {
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
    run('xcrun', ['simctl', 'privacy', udid, 'grant', 'location', id]);
    run('xcrun', ['simctl', 'location', udid, 'set', '48.8566,2.3522']);
  }
  // The build's Info.plist (D7, D10): the usage key, iOS's reduced default, and a purpose for each key the app passes.
  const plistAt = readdirSync(resolve(app, 'target'), { recursive: true }).find((f) => host === 'ios' ? /iphonesimulator|ios-sim/.test(f) && f.endsWith('.app/Info.plist') : /macos/.test(f) && f.endsWith('-Info.plist'));
  const plist = plistAt ? JSON.parse(run('plutil', ['-convert', 'json', '-o', '-', resolve(app, 'target', plistAt)])) : {};
  check(where, 'Info.plist carries the usage key and the temporary purpose', !!plist.NSLocationWhenInUseUsageDescription && plist.NSLocationTemporaryUsageDescriptionDictionary?.['purpose.geolocation'] === 'The conformance suite reads the position.', plistAt ?? 'no Info.plist under target/');
  check(where, 'no NSLocationDefaultAccuracyReduced: the fixture asks for precise (D7)', !('NSLocationDefaultAccuracyReduced' in plist), JSON.stringify(plist.NSLocationDefaultAccuracyReduced));
  const s = await open({ host, app: name, bind: 'geolocation' });
  try {
    const state = await s.state();
    check(where, 'the extension is bound, not substituted', state.extensions?.geolocation?.bound === true, JSON.stringify(state.extensions));
    const status = await until(s, 'status', (t) => /^true (prompt|granted|denied)/.test(t ?? ''));
    check(where, 'geolocationStatus answers from CoreLocation', /^true (prompt|granted|denied) (unknown|precise|approximate)$/.test(status ?? ''), status);
    const now = await until(s, 'status-now', (t) => !!t);
    check(where, 'the synchronous form answers from the module', now === status?.split(' ')[1], `${now} beside ${status}`);
    const apple = (await s.state('status'))?.apple ?? JSON.parse(JSON.stringify(state.resources?.status ?? {})).apple;
    check(where, 'its apple record is Apple\'s words', WORDS.includes(apple?.authorizationStatus) && ['fullAccuracy', 'reducedAccuracy'].includes(apple?.accuracyAuthorization), JSON.stringify(apple));
    // macOS lets no tool grant: with --ask, a person answers the system's dialog the request opens.
    if (status?.startsWith('true prompt') && argv.includes('--ask')) {
      console.log(`${where}: answer the location dialog (Allow), within two minutes`);
      await s.tap('locate');
      const asked = await until(s, 'status', (t) => !t?.startsWith('true prompt'), 120000);
      check(where, 'the request asks, and the answer reaches the state', /^true (granted|denied)/.test(asked ?? ''), asked);
    }
    if (!(await texts(s)).status?.startsWith('true granted')) {
      console.log(`SKIP ${where} positions: location is not granted, and only a person grants it here (--ask)`);
      continue;
    }
    // The simulator is where `simctl location` put it; a Mac is wherever it is.
    const here = udid ? /^48\.8566,2\.3522/ : /^-?\d+(\.\d+)?,-?\d+(\.\d+)?/;
    await s.tap('locate');
    const once = await until(s, 'once', (t) => here.test(t ?? ''), 30000);
    check(where, 'getCurrentPosition answers the position', here.test(once ?? ''), once);

    await s.tap('watch');
    const spot = await until(s, 'spot', (t) => here.test(t ?? ''), 30000);
    check(where, 'watchPosition in data code', here.test(spot ?? ''), spot);
    if (udid) {
      run('xcrun', ['simctl', 'location', udid, 'set', '51.5074,-0.1278']);
      const moved = await until(s, 'spot', (t) => t === '51.5074,-0.1278');
      check(where, 'a move asks the reader again', moved === '51.5074,-0.1278', moved);
    }
    // Each fix asks the reader again; while it does, its module request is what reads the watch.
    const kept = JSON.stringify(await s.logs());
    check(where, 'the watch is kept while its reader is asked again', !kept.includes('release geolocation.watchPosition'));
    await s.tap('watch');
    const off = await until(s, 'spot', (t) => t === 'off');
    const logs = JSON.stringify(await s.logs());
    check(where, 'the last reader gone, the watch is released', off === 'off' && logs.includes('release geolocation.watchPosition'), off);

    await s.tap('raise');
    const precise = await until(s, 'precise-error', (t) => t !== undefined, 5000);
    check(where, 'requestPreciseLocation answers the status', precise === '', precise);
  } finally {
    await s.close();
  }
  if (udid) await approximate(udid);
}
rmSync(dirname(app), { recursive: true, force: true });
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
