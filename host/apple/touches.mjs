// The touch runner (LLP 1080.000 D1/D2): real touches on iOS through public
// XCUICoordinate, from an XCTest UI test (`touches.swift`) that stays alive
// for an agent session. Built from Xcode's own XCTRunner.app — no Xcode
// project — installed on the destination, and started by xcodebuild with
// `UseDestinationArtifacts`, so nothing is installed or relaunched under a
// running app. The runner connects out over the phone carrier's token bridge
// and answers JSON lines. One runner per device, held by a lock file.
//
// Simulator only so far; a phone is LLP 1080.000 stage 4.
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { cpSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { iosTriple, phoneBridge } from './build.mjs';

const ROOT = resolve(new URL('../..', import.meta.url).pathname);
const RUNNER_ID = 'com.exact.touches.xctrunner';
const read = (cmd, args) => spawnSync(cmd, args, { encoding: 'utf8' });
const must = (cmd, args) => {
  const r = read(cmd, args);
  if (r.status !== 0) throw new Error(`${cmd} ${args.slice(0, 3).join(' ')}: ${r.stderr || r.stdout || r.error}`);
  return r.stdout;
};

/** The runner for a simulator, built once per source, SDK, arch and Xcode (D1's key), installed on `udid`. */
export function touchRunner(udid) {
  const developer = must('xcode-select', ['-p']).trim();
  const platform = resolve(developer, 'Platforms/iPhoneSimulator.platform/Developer');
  const source = resolve(ROOT, 'host/apple/touches.swift');
  const key = createHash('sha256').update(JSON.stringify({
    source: readFileSync(source, 'utf8'), destination: 'iphonesimulator', arch: process.arch,
    sdk: must('xcrun', ['--sdk', 'iphonesimulator', '--show-sdk-version']).trim(), xcode: must('xcodebuild', ['-version']).trim(),
  })).digest('hex').slice(0, 16);
  const dir = resolve(ROOT, 'target/touch-runner', key);
  const app = resolve(dir, 'ExactTouches-Runner.app');
  if (!existsSync(resolve(dir, 'done'))) {
    rmSync(dir, { recursive: true, force: true });
    mkdirSync(dir, { recursive: true });
    cpSync(resolve(platform, 'Library/Xcode/Agents/XCTRunner.app'), app, { recursive: true });
    must('plutil', ['-replace', 'CFBundleExecutable', '-string', 'XCTRunner', resolve(app, 'Info.plist')]);
    must('plutil', ['-replace', 'CFBundleIdentifier', '-string', RUNNER_ID, resolve(app, 'Info.plist')]);
    must('plutil', ['-replace', 'CFBundleName', '-string', 'ExactTouches-Runner', resolve(app, 'Info.plist')]);
    // XCTest and what it loads, as Xcode embeds them in a UI-test runner.
    const frameworks = resolve(app, 'Frameworks');
    mkdirSync(frameworks, { recursive: true });
    for (const [from, names] of [[resolve(platform, 'Library/Frameworks'), /^(XCTest|XCUIAutomation|Testing|_Testing_Foundation)\.framework$/], [resolve(platform, 'Library/PrivateFrameworks'), /^(XCT|XCUnit)/], [resolve(platform, 'usr/lib'), /^(libXCTestSwiftSupport|lib_TestingInterop)\.dylib$/]]) {
      for (const name of readdirSync(from).filter((n) => names.test(n))) cpSync(resolve(from, name), resolve(frameworks, name), { recursive: true });
    }
    const bundle = resolve(app, 'PlugIns/ExactTouches.xctest');
    mkdirSync(bundle, { recursive: true });
    must('xcrun', ['--sdk', 'iphonesimulator', 'swiftc', '-emit-library', '-O', '-module-name', 'ExactTouches', '-target', iosTriple,
      '-F', resolve(platform, 'Library/Frameworks'), '-F', resolve(platform, 'Library/PrivateFrameworks'), '-I', resolve(platform, 'usr/lib'), '-L', resolve(platform, 'usr/lib'),
      '-framework', 'XCTest', '-framework', 'XCUIAutomation', '-Xlinker', '-rpath', '-Xlinker', '@executable_path/Frameworks',
      '-module-cache-path', resolve(dir, 'cache'), '-o', resolve(bundle, 'ExactTouches'), source]);
    writeFileSync(resolve(bundle, 'Info.plist'), plist({ CFBundleExecutable: 'ExactTouches', CFBundleIdentifier: 'com.exact.touches', CFBundlePackageType: 'BNDL', CFBundleName: 'ExactTouches', CFBundleVersion: '1', CFBundleSupportedPlatforms: ['iPhoneSimulator'] }));
    must('codesign', ['--force', '--sign', '-', bundle]);
    for (const name of readdirSync(frameworks)) must('codesign', ['--force', '--sign', '-', resolve(frameworks, name)]);
    must('codesign', ['--force', '--sign', '-', '--entitlements', resolve(app, 'RunnerEntitlements.plist'), app]);
    rmSync(resolve(dir, 'cache'), { recursive: true, force: true });
    writeFileSync(resolve(dir, 'done'), '');
  }
  must('xcrun', ['simctl', 'install', udid, app]);
  return { dir, app };
}

/** A plist from a flat object of strings, booleans, arrays of strings and nested objects. */
function plist(object) {
  const value = (v) => typeof v === 'boolean' ? `<${v}/>` : typeof v === 'number' ? `<integer>${v}</integer>` : Array.isArray(v) ? `<array>${v.map(value).join('')}</array>`
    : typeof v === 'object' ? `<dict>${Object.entries(v).map(([k, x]) => `<key>${k}</key>${value(x)}`).join('')}</dict>` : `<string>${String(v).replace(/&/g, '&amp;').replace(/</g, '&lt;')}</string>`;
  return `<?xml version="1.0" encoding="UTF-8"?>\n<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">\n<plist version="1.0">${value(object)}</plist>\n`;
}

/** The lock that makes one session the device's only toucher (D2); a dead holder's lock is taken over. */
function lock(udid) {
  const path = resolve(tmpdir(), `exact-touches-${udid}.lock`);
  if (existsSync(path)) {
    const holder = Number(readFileSync(path, 'utf8'));
    let alive = false;
    try { process.kill(holder, 0); alive = holder !== process.pid; } catch {}
    if (alive) throw new Error(`the device's touches are held by PID ${holder}`);
  }
  writeFileSync(path, String(process.pid));
  return () => { try { if (Number(readFileSync(path, 'utf8')) === process.pid) rmSync(path); } catch {} };
}

/**
 * Start the runner for `appId` on simulator `udid` and wait for its hello
 * (30 s). Returns `{ ask(req), close(), pid, started }`: `ask` sends one line
 * and resolves with its reply. `onProcess` gets the xcodebuild child, whose
 * PID is the one this module may kill.
 */
export async function openTouches({ udid, appId, appPath, onProcess }) {
  const release = lock(udid);
  let child = null, bridge = null;
  try {
    const started = Date.now();
    const { dir, app } = touchRunner(udid);
    bridge = await phoneBridge();
    const run = resolve(dir, `run-${process.pid}.xctestrun`);
    writeFileSync(run, plist({
      ExactTouches: {
        // A simulator refuses `UseDestinationArtifacts` (device only), so
        // xcodebuild installs the target here: the runner starts before the
        // app is launched, never under it (D1's order).
        IsUITestBundle: true, TestHostPath: app, TestBundlePath: '__TESTHOST__/PlugIns/ExactTouches.xctest', UITargetAppPath: appPath ?? app,
        TestingEnvironmentVariables: { EXACT_TOUCH_CONNECT: bridge.env.EXACT_AGENT_CONNECT, EXACT_TOUCH_TOKEN: bridge.env.EXACT_AGENT_TOKEN, EXACT_TOUCH_APP: appId },
      },
      __xctestrun_metadata__: { FormatVersion: 1 },
    }));
    const log = [];
    child = spawn('xcodebuild', ['test-without-building', '-xctestrun', run, '-destination', `id=${udid}`, '-only-testing:ExactTouches/ExactTouches/testDrive'], { stdio: ['ignore', 'pipe', 'pipe'] });
    onProcess?.(child);
    for (const s of [child.stdout, child.stderr]) s.on('data', (d) => { for (const l of String(d).split('\n')) if (l.trim()) { log.push(l); if (log.length > 200) log.shift(); } });
    const exited = new Promise((r) => child.on('exit', (code) => r(code)));
    const timeout = new Promise((r) => setTimeout(() => r('timeout'), 30000));
    const first = await Promise.race([bridge.ready.then((x) => ({ x })), exited.then((code) => ({ code })), timeout]);
    if (!first?.x) throw new Error(`the touch runner did not start (${first === 'timeout' ? 'no hello in 30 s' : `xcodebuild exited ${first.code}`}):\n${log.slice(-20).join('\n')}`);
    const { socket } = first.x;
    socket.resume();
    socket.setEncoding('utf8');
    const waiting = [];
    let buf = '';
    socket.on('data', (chunk) => {
      buf += chunk;
      for (let i; (i = buf.indexOf('\n')) >= 0;) { const line = buf.slice(0, i); buf = buf.slice(i + 1); waiting.shift()?.(JSON.parse(line)); }
    });
    socket.on('close', () => { for (const w of waiting.splice(0)) w({ error: 'the touch runner hung up:\n' + log.slice(-10).join('\n') }); });
    const ask = (req) => new Promise((r) => { if (socket.destroyed) return r({ error: 'the touch runner is gone' }); waiting.push(r); socket.write(JSON.stringify(req) + '\n'); });
    return {
      ask, pid: child.pid, started: Date.now() - started, log,
      async close() {
        try { socket.end(); } catch {}
        const code = await Promise.race([exited, new Promise((r) => setTimeout(() => r('timeout'), 3000))]);
        if (code === 'timeout') { try { child.kill('SIGTERM'); } catch {} }
        bridge.close(); rmSync(run, { force: true }); release();
      },
    };
  } catch (error) {
    if (child && child.exitCode == null) { try { child.kill('SIGTERM'); } catch {} }
    bridge?.close(); release();
    throw error;
  }
}

const sameAim = (a, b) => a.orientation === b.orientation && a.session === b.session && a.generation === b.generation
  && ['w', 'h', 'x', 'y'].every((k) => a.screen[k] === b.screen[k]) && a.point[0] === b.point[0] && a.point[1] === b.point[1];

/**
 * A real tap on view `id` (LLP 1080.000 D4/D5/D8): the host aims (`at`, a
 * viewport point, or the target's middle), the aim is taken twice and must
 * agree (orientation, origin, size, point, session), the runner checks the
 * app is in the foreground and taps, and the window's dispatch log must show
 * exactly one new touch that began and ended — else an error that says what
 * the log saw. `ask` is the app's carrier; `touches` the runner's.
 */
export async function realTap({ ask, touches, id, at }) {
  const aimReq = { op: 'tap', id, aim: at ? { x: at[0], y: at[1] } : true };
  const first = await ask(aimReq);
  if (first.error) throw new Error(first.error);
  const fg = await touches.ask({ op: 'foreground' });
  if (fg.error) throw new Error(`tap #${id}: the touch runner: ${fg.error}`);
  if (fg.state !== 'runningForeground') throw new Error(`tap #${id}: the app is ${fg.state ?? fg.error}, not in the foreground`);
  const aim = (await ask(aimReq));
  if (aim.error) throw new Error(aim.error);
  if (!sameAim(first.aim, aim.aim)) throw new Error(`tap #${id}: the screen rotated or moved since aim`);
  const a = aim.aim;
  const injected = await touches.ask({ op: 'tap', point: a.point });
  if (injected.error) throw new Error(`tap #${id}: the touch runner: ${injected.error}`);
  const deadline = Date.now() + 1000;
  let seen = [];
  for (;;) {
    const log = await ask({ op: 'tap', log: a.seq });
    if (log.lost) throw new Error(`tap #${id}: the dispatch log dropped entries past seq ${a.seq}`);
    seen = log.log;
    const touches_ = [...new Set(seen.map((e) => e.touch))];
    const ended = touches_.filter((t) => seen.some((e) => e.touch === t && e.phase === 'began') && seen.some((e) => e.touch === t && e.phase === 'ended'));
    if (touches_.length > 1) throw new Error(`tap #${id}: ambiguous: ${touches_.length} touches after seq ${a.seq}: ${JSON.stringify(seen.slice(0, 8))}`);
    if (ended.length === 1) {
      const began = seen.find((e) => e.phase === 'began'), end = seen.find((e) => e.phase === 'ended');
      if (began.session !== a.session || began.generation !== a.generation) throw new Error(`tap #${id}: the touch landed in session ${began.session} (generation ${began.generation}), not ${a.session}`);
      if (began.node !== a.hit) throw new Error(`tap #${id}: the touch landed on ${began.node == null ? began.view : `node #${began.node}`}, not on node #${a.hit} the aim hit-tested`);
      return {
        tapped: id, at: at ?? null, delivery: 'platform',
        landed: { session: began.session, generation: began.generation, node: began.node, view: began.view },
        touch: { began: began.t, ended: end.t, moved: seen.filter((e) => e.phase === 'moved').length, type: began.type },
        injected: injected.injected, aim: a.point, orientation: a.orientation,
      };
    }
    if (Date.now() > deadline) throw new Error(`tap #${id}: no touch reached the app's window within 1 s; the runner finished at ${injected.injected?.end} (log: ${JSON.stringify(seen)})`);
    await new Promise((r) => setTimeout(r, 16));
  }
}
