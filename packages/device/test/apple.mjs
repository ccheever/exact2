// @exact/device's conformance suite on Apple (LLP 1133.031 §12 stage 2): the
// fixture app made as the web suite makes it, built for macOS or an iOS
// simulator, and driven by the agent with the Apple implementation bound
// (`--bind device`), so the platform answers and not the substitute. This
// Mac's and the simulator's values are their own, so the checks are of shape
// and range, never of a value; the suite prints what each read for the
// record. The hub's announcements are checked first, over recorded readings
// (`hub.swift`, compiled with the implementation and run here).
//
// usage: bun packages/device/test/apple.mjs [macos] [ios] [--sim <name|udid>]
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, dirname, join, resolve } from 'node:path';
import { checker, makeFixture, pkg, root, run, texts } from './fixture.mjs';

const argv = process.argv.slice(2), sim = argv.includes('--sim') ? argv[argv.indexOf('--sim') + 1] : process.env.EXACT_SIM;
const hosts = argv.filter((a, i) => ['macos', 'ios'].includes(a) && argv[i - 1] !== '--sim');
const { check, failed } = checker();

// The hub over recorded readings, on this Mac.
{
  const out = mkdtempSync(join(tmpdir(), 'exact-device-hub-'));
  run('swiftc', ['-parse-as-library', '-emit-library', '-static', '-module-name', 'ExactPackage', '-emit-module', '-emit-module-path', join(out, 'ExactPackage.swiftmodule'),
    resolve(root, 'host/apple/modules/ExactPackage.swift'), '-o', join(out, 'libExactPackage.a')], out);
  run('swiftc', ['-parse-as-library', '-I', out, '-L', out, '-lExactPackage', resolve(pkg, 'apple/Device.swift'), resolve(pkg, 'test/hub.swift'), '-o', join(out, 'hub')], out);
  for (const line of run(join(out, 'hub'), [], out).trim().split('\n')) {
    const [verdict, , ...name] = line.split(' ');
    if (verdict === 'PASS' || verdict === 'FAIL') check('apple-hub', name.join(' '), verdict === 'PASS', line);
  }
  rmSync(out, { recursive: true, force: true });
}

const { app } = makeFixture(), name = basename(app);
Object.assign(process.env, { EXACT_APP_DIR: app, EXACT2: root, HERMES_LEAN_SYS_OFFLINE: '1' });
const { open } = await import(resolve(root, 'scripts/agent.mjs'));

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
const TYPES = ['bluetooth', 'cellular', 'ethernet', 'mixed', 'none', 'other', 'unknown', 'wifi', 'wimax'];

for (const host of hosts.length ? hosts : ['macos', 'ios']) {
  const where = host === 'ios' ? 'ios-simulator' : 'macos';
  run('bun', [resolve(root, 'host/apple/build.mjs'), ...(host === 'ios' ? ['--ios', ...(sim ? ['--sim', sim] : [])] : []), `${name}-apple`], root);
  const s = await open({ host, app: name, bind: 'device' });
  try {
    const state = await s.state();
    check(where, 'the extension is bound, not substituted', state.extensions?.device?.bound === true, JSON.stringify(state.extensions?.device));
    const battery = await until(s, 'battery', (t) => /^(0(\.\d+)?|1) (true|false)$/.test(t ?? ''));
    const level = Number(battery?.split(' ')[0]);
    check(where, 'getBattery answers a level in hundredths and charging', level >= 0 && level <= 1 && Math.round(level * 100) === level * 100, battery);
    if (host === 'ios') check(where, 'the simulator answers the specification\'s no-battery values (D1)', battery === '1 true', battery);
    const network = await until(s, 'network', (t) => !!t && TYPES.includes(t.split(' ')[0]) && t.split(' ')[0] !== 'unknown');
    check(where, 'connection answers the path\'s type and Low Data Mode', TYPES.includes(network?.split(' ')[0]) && /(true|false)$/.test(network ?? ''), network);
    const power = await until(s, 'power', (t) => /^(true|false) (nominal|fair|serious|critical)$/.test(t ?? ''));
    check(where, 'powerState answers low power and a thermal word', /^(true|false) (nominal|fair|serious|critical)$/.test(power ?? ''), power);
    console.log(`RECORD ${where}: battery "${battery}", network "${network}", power "${power}"`);
    const words = JSON.parse((await texts(s)).words || '{}');
    check(where, 'the answers carry the apple words and no other platform\'s',
      typeof words.battery?.apple?.batteryState === 'string' && typeof words.network?.apple?.expensive === 'boolean' && !words.battery?.android && !words.power?.android,
      JSON.stringify(words));
    // The last reader gone: the runner releases each source to the implementation.
    await s.tap('toggle');
    await until(s, 'battery', (t) => t === 'off');
    await s.clock('+500 real');
    const logs = JSON.stringify(await s.logs());
    check(where, 'the last reader gone, getBattery and connection are released', ['getBattery', 'connection'].every((n) => logs.includes(`release device.${n}`)), logs.slice(-600));
  } finally {
    await s.close();
  }
}
rmSync(dirname(app), { recursive: true, force: true });
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
