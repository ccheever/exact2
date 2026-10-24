// The touch runner (LLP 1080.000 D1/D2): real touches on iOS through XCTest's
// event synthesis, from an XCTest UI test (`touches.swift`) that stays alive
// across agent sessions. Built from Xcode's own XCTRunner.app — no Xcode
// project — installed on the destination, and started by xcodebuild, so
// nothing is installed or relaunched under a running app. The runner connects
// out over the phone carrier's token bridge, answers JSON lines, and is kept
// for the next session on a loopback port (a connection instead of
// xcodebuild's 5 s start). One session per device at a time, held by a lock
// file.
//
// Simulator only so far; a phone is LLP 1080.000 stage 4.
import { spawn, spawnSync } from 'node:child_process';
import { createHash, randomBytes } from 'node:crypto';
import { closeSync, cpSync, existsSync, mkdirSync, openSync, readFileSync, readdirSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { connect } from 'node:net';
import { homedir, tmpdir } from 'node:os';
import { dirname, resolve } from 'node:path';
import { iosTriple } from './build.mjs';
import { claimLive } from '../../scripts/app.mjs';
import { phoneBridge } from './devices.mjs';
import { watchCloak } from './cloak.mjs';

const ROOT = resolve(new URL('../..', import.meta.url).pathname);
const RUNNER_ID = 'com.exact.touches.xctrunner';
const read = (cmd, args) => spawnSync(cmd, args, { encoding: 'utf8' });
const must = (cmd, args) => {
  const r = read(cmd, args);
  if (r.status !== 0) throw new Error(`${cmd} ${args.slice(0, 3).join(' ')}: ${r.stderr || r.stdout || r.error}`);
  return r.stdout;
};

/** The runner's identity (D1's key): its source as read now, the arch, the
 * selected Xcode and its simulator SDK, each by its own file where it has
 * one (a launch compares it, so it costs a file read, not a tool's start). */
export function runnerKey() {
  const developer = must('xcode-select', ['-p']).trim();
  const platform = resolve(developer, 'Platforms/iPhoneSimulator.platform/Developer');
  const text = (path) => { try { return readFileSync(path, 'utf8'); } catch { return null; } };
  const source = readFileSync(resolve(ROOT, 'host/apple/touches.swift'), 'utf8');
  const key = createHash('sha256').update(JSON.stringify({
    source, destination: 'iphonesimulator', arch: process.arch, developer,
    sdk: text(resolve(platform, 'SDKs/iPhoneSimulator.sdk/SDKSettings.json')) ?? must('xcrun', ['--sdk', 'iphonesimulator', '--show-sdk-version']).trim(),
    xcode: text(resolve(developer, '../version.plist')) ?? must('xcodebuild', ['-version']).trim(),
  })).digest('hex').slice(0, 16);
  return { key, platform, source };
}

/** The runner for a simulator, built once per `runnerKey()`, from the source the key was made of, installed on `udid`. */
export function touchRunner(udid, { key, platform, source } = runnerKey()) {
  const dir = resolve(ROOT, 'target/touch-runner', key);
  const app = resolve(dir, 'ExactTouches-Runner.app');
  // Built in a directory of its own, then published by one rename: a
  // published runner is complete and never rewritten, so sessions on other
  // simulators can build and install from the cache at the same time.
  if (!existsSync(dir)) {
    const stage = `${dir}.build-${process.pid}-${randomBytes(4).toString('hex')}`;
    try {
      buildRunner(platform, source, stage);
      try { renameSync(stage, dir); } catch (e) { if (!['EEXIST', 'ENOTEMPTY'].includes(e.code)) throw e; } // another build published first: use it
    } finally { rmSync(stage, { recursive: true, force: true }); }
  }
  must('xcrun', ['simctl', 'install', udid, app]);
  return { dir, app };
}

/** Assemble the runner in `dir`: Xcode's XCTRunner.app, the frameworks it loads, the test bundle compiled from `text`, signed ad hoc. */
function buildRunner(platform, text, dir) {
  const app = resolve(dir, 'ExactTouches-Runner.app');
  mkdirSync(dir, { recursive: true });
  const source = resolve(dir, 'touches.swift');
  writeFileSync(source, text);
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
  rmSync(source, { force: true });
}

/** A plist from a flat object of strings, booleans, arrays of strings and nested objects. */
function plist(object) {
  const value = (v) => typeof v === 'boolean' ? `<${v}/>` : typeof v === 'number' ? `<integer>${v}</integer>` : Array.isArray(v) ? `<array>${v.map(value).join('')}</array>`
    : typeof v === 'object' ? `<dict>${Object.entries(v).map(([k, x]) => `<key>${k}</key>${value(x)}`).join('')}</dict>` : `<string>${String(v).replace(/&/g, '&amp;').replace(/</g, '&lt;')}</string>`;
  return `<?xml version="1.0" encoding="UTF-8"?>\n<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">\n<plist version="1.0">${value(object)}</plist>\n`;
}

/** `promise`, or `onTimeout()`'s value after `ms`; the timer is cleared either way. */
async function within(promise, ms, onTimeout) {
  let timer;
  try { return await Promise.race([promise, new Promise((r) => { timer = setTimeout(() => r(onTimeout()), ms); })]); }
  finally { clearTimeout(timer); }
}

const alive = (pid) => { try { process.kill(pid, 0); return true; } catch { return false; } };

/**
 * The lock that makes one session the device's only toucher (D2). Taken
 * atomically (an exclusive create) with a token naming this session; any
 * live holder refuses, this process included, so two sessions in one
 * process cannot share a device either. A lock that vanished between the
 * create and the read is retried by another exclusive create. A dead
 * holder's lock is removed only under an exclusive takeover file and only
 * if it still holds the dead token that was seen — only takeovers remove a
 * dead token, and they are serialized, so a live owner's lock is never
 * removed. Released only while it still holds this session's token.
 */
function lock(udid) {
  const path = resolve(tmpdir(), `exact-touches-${udid}.lock`);
  // The holder by its pid and start time, as a build's claim is (`claimLive`): a reused pid is no holder.
  const token = JSON.stringify({ pid: process.pid, started: new Date().toISOString(), id: randomBytes(8).toString('hex') });
  const create = () => { try { writeFileSync(path, token, { flag: 'wx' }); return true; } catch (e) { if (e.code === 'EEXIST') return false; throw e; } };
  const holder = () => { try { return readFileSync(path, 'utf8'); } catch (e) { if (e.code === 'ENOENT') return null; throw e; } };
  const owner = (t) => { try { return JSON.parse(t).pid; } catch { return Number(t.split(':')[0]); } };
  // Live unless shown dead: a holder whose start `ps` cannot read, or a lock that does not parse, is live (claimLive);
  // a lock written before the start time was recorded is its pid's.
  const live = (t) => /^\d+:[0-9a-f]+$/.test(t) ? alive(owner(t)) : claimLive(t);
  for (let attempt = 0; !create(); attempt++) {
    if (attempt > 20) throw new Error(`the device's touch lock keeps changing hands (${path})`);
    const seen = holder();
    if (seen == null) continue; // released between the create and the read: create again
    if (live(seen)) throw new Error(`the device's touches are held by PID ${owner(seen)}${owner(seen) === process.pid ? ' (another session in this process)' : ''}`);
    const takeover = path + '.takeover';
    try { writeFileSync(takeover, token, { flag: 'wx' }); } catch (e) {
      if (e.code === 'EEXIST') throw new Error(`another process is taking over the device's touch lock; if none is, remove ${takeover}`);
      throw e;
    }
    try { if (holder() === seen) rmSync(path); } finally { rmSync(takeover, { force: true }); }
  }
  return () => { if (holder() === token) rmSync(path, { force: true }); };
}

/** Each runner request's bound: a public tap includes XCTest's idle wait. */
const RUNNER_MS = 15000;
/** How long a runner waits for the next session before its test ends (touches.swift `accept`). */
const KEPT_IDLE = '10 minutes';

/** Where a device's kept runner is recorded: its `runnerKey`, loopback port
 * (null until its hello), token, log, and xcodebuild's PID, the one PID this
 * module may kill. */
const keptPath = (udid) => resolve(tmpdir(), `exact-touches-${udid}.runner.json`);
const logTail = (log, n = 20) => { try { return readFileSync(log, 'utf8').trim().split('\n').slice(-n).join('\n'); } catch { return ''; } };
/** Whether `pid` is still the xcodebuild a record names (a PID can be reused once it exits). */
const isRunner = (pid, run) => alive(pid) && read('ps', ['-p', String(pid), '-o', 'command=']).stdout.includes(run);

/** One JSON-lines conversation with a runner over `socket`. */
function conversation(socket, tail) {
  socket.resume();
  socket.setEncoding('utf8');
  socket.on('error', () => {});
  const waiting = [];
  let buf = '', broken = null;
  const fail = (why) => { broken ??= why; for (const w of waiting.splice(0)) w({ error: why }); };
  socket.on('data', (chunk) => {
    buf += chunk;
    for (let i; (i = buf.indexOf('\n')) >= 0;) { const line = buf.slice(0, i); buf = buf.slice(i + 1); waiting.shift()?.(JSON.parse(line)); }
  });
  socket.on('close', () => fail('the touch runner hung up:\n' + tail()));
  return {
    fail, broken: () => broken ?? (socket.destroyed ? 'the touch runner is gone' : null),
    request: (req) => new Promise((r) => { waiting.push(r); socket.write(JSON.stringify(req) + '\n'); }),
  };
}

/** Ends a kept runner (another key's, one that never said hello, or one that
 * stopped answering): SIGTERM, then SIGKILL, each given 3 s, and forgets it
 * only once its xcodebuild is gone; one that will not exit keeps its record
 * and fails the drive, so no second runner starts beside it. */
async function retire(udid, kept) {
  if (!kept) return; // nothing loaded: a record on disk is another's to retire, never unlinked unread
  const running = () => kept?.pid && isRunner(kept.pid, kept.run);
  for (const signal of ['SIGTERM', 'SIGKILL']) {
    if (!running()) break;
    try { process.kill(kept.pid, signal); } catch {}
    for (const until = Date.now() + 3000; running() && Date.now() < until;) await new Promise((r) => setTimeout(r, 100));
  }
  if (running()) throw new Error(`the kept touch runner's xcodebuild (PID ${kept.pid}) did not exit after SIGKILL; its record ${keptPath(udid)} is kept`);
  rmSync(keptPath(udid), { force: true });
}

/** The device's kept runner, of runner `key`, connected and greeted for `appId`, or null. */
async function keptRunner(udid, appId, key) {
  let kept;
  try { kept = JSON.parse(readFileSync(keptPath(udid), 'utf8')); } catch { return null; }
  if (kept.key !== key || kept.port == null || !isRunner(kept.pid, kept.run)) { await retire(udid, kept); return null; }
  const socket = await new Promise((ok) => { const s = connect(kept.port, '127.0.0.1'); s.once('connect', () => ok(s)); s.once('error', () => { s.destroy(); ok(null); }); });
  if (!socket) { await retire(udid, kept); return null; }
  const talk = conversation(socket, () => logTail(kept.log));
  const hello = await within(talk.request({ token: kept.token, app: appId }), 3000, () => null);
  // A runner that answers for another bundle (an older runner, or one another drive retargeted under us) is retired, not reused.
  if (!hello?.ready || (hello.app !== undefined && hello.app !== appId)) { socket.destroy(); await retire(udid, kept); return null; }
  return { socket, talk, kept };
}

// What starting an XCTest UI session turns on in a simulator's com.apple.Accessibility, and leaves on across
// reboots. With ApplicationAccessibilityEnabled on, every app loads UIKit's accessibility runtime at launch: euxy's
// main→didFinishLaunching went from 10 ms to 1.4 s (AccessibilityEnabled alone: 0.1 s; iPhone 17, iOS 27). A
// session needs all three on (without them a kept runner's tap never finishes and a menu's rows are not found), so
// it turns them on and puts them back at its end; a write takes effect at the next launch, with no reboot.
const AX = 'com.apple.Accessibility', AX_KEYS = ['AutomationEnabled', 'AccessibilityEnabled', 'ApplicationAccessibilityEnabled'];
/** `defaults <args>` in simulator `udid`, asynchronously: each spawn costs about 0.35 s, so a session runs them
 * together. One that has not answered in 10 s is killed, and fails: the device's lock waits on it. */
const simDefaults = (udid, args) => new Promise((done) => {
  const p = spawn('xcrun', ['simctl', 'spawn', udid, 'defaults', ...args], { stdio: ['ignore', 'pipe', 'pipe'] });
  const timer = setTimeout(() => { try { p.kill('SIGKILL'); } catch {} }, 10000);
  let out = '', err = '';
  p.stdout.on('data', (d) => { out += d; });
  p.stderr.on('data', (d) => { err += d; });
  p.on('error', (e) => { clearTimeout(timer); done({ status: -1, out, err: String(e) }); });
  p.on('close', (status, signal) => { clearTimeout(timer); done({ status: signal ? -1 : status, out, err: signal ? 'no answer in 10 s' : err }); });
});
/** The three keys as simulator `udid` has them (`'1'`, `'0'`, or undefined when unset), or null when it could not say. */
async function accessibilityKeys(udid) {
  const r = await simDefaults(udid, ['read', AX]);
  if (r.status !== 0 && !/does not exist/.test(r.err)) return null;
  return Object.fromEntries(AX_KEYS.map((k) => [k, new RegExp(`^\\s*${k} = (\\d+);`, 'm').exec(r.out)?.[1]]));
}
/** The keys as a session finds them: the record a session killed before its end left of what it found, else read
 * now and recorded (`record`, a file; under the device's lock, so no other session's). Null when they could not be read. */
export async function foundAccessibility(udid, record, read = accessibilityKeys) {
  try { return JSON.parse(readFileSync(record, 'utf8')); } catch { /* none: this session reads them */ }
  const keys = await read(udid);
  if (keys) { mkdirSync(dirname(record), { recursive: true }); writeFileSync(record, JSON.stringify(keys)); }
  return keys;
}
/** The keys put back as a session `found` them. AutomationEnabled already on means an earlier session's XCTest left
 * all three on (a person sets ApplicationAccessibilityEnabled alone, for `tree --ax`), so they go back to unset.
 * Resolves with what failed, or null. */
export async function restoreAccessibility(udid, found, run = simDefaults) {
  const left = found.AutomationEnabled === '1';
  const failed = (await Promise.all(AX_KEYS.map(async (k) => {
    const v = left ? undefined : found[k];
    const r = await run(udid, v == null ? ['delete', AX, k] : ['write', AX, k, '-bool', v === '1' ? 'true' : 'false']);
    // Deleting a key or domain that is not there fails, and leaves what was wanted.
    return r.status === 0 || (v == null && /Could not find key|not found/.test(r.err)) ? null : `${k}: ${r.err.trim() || `exit ${r.status}`}`;
  }))).filter(Boolean);
  return failed.length ? failed.join('; ') : null;
}

/**
 * The runner for `appId` on simulator `udid`: the device's kept one when it
 * answers (a connection, milliseconds), else a new one, started and waited
 * for (its hello, 30 s; xcodebuild's start is most of 5 s). A runner outlives
 * the drive that started it: closing hangs up and the runner waits
 * KEPT_IDLE for the next session. Returns `{ ask(req), close(), pid, started,
 * kept, cloak }`: `ask` sends one line and resolves with its reply, or with an error
 * at `RUNNER_MS`, which also ends the runner (a stalled runner never hangs a
 * drive). On a `foldable` simulator (the iPhone Duo) `cloak` follows
 * SpringBoard's fold cloak (cloak.mjs), which a real touch waits out. The
 * simulator's accessibility keys are on for the session and put back when it
 * ends (`AX_KEYS`).
 */
export async function openTouches({ udid, appId, foldable }) {
  const lockRelease = lock(udid);
  const started = Date.now();
  const cloak = foldable ? watchCloak(udid) : null;
  const axRecord = resolve(homedir(), '.cache/exact/simulator-accessibility', `${udid}.json`), axFound = foundAccessibility(udid, axRecord);
  let restored = null;
  // Unread keys were never turned on: the session ends before a write or a runner's start (`readKeys`).
  const putBack = () => (restored ??= axFound.then(async (keys) => {
    if (!keys) return;
    const failed = await restoreAccessibility(udid, keys);
    if (failed) console.error(`note: the simulator's accessibility was not put back (${failed}), so its launches stay about a second slower until the next --touch platform drive ends`);
    else rmSync(axRecord, { force: true });
  }));
  const release = async () => { await putBack(); lockRelease(); };
  let socket = null, talk = null, kept = null, child = null, bridge = null, exited = null, reused = false;
  // Ends the runner: hang up, then end xcodebuild by its recorded PID — this
  // process's child is waited for (3 s, then SIGKILL) and the lock kept if it
  // will not exit.
  const teardown = async () => {
    cloak?.close();
    try { socket?.destroy(); } catch {}
    bridge?.close();
    try {
      if (child && child.exitCode == null && child.signalCode == null) {
        let gone = false;
        for (const signal of ['SIGTERM', 'SIGKILL']) {
          try { child.kill(signal); } catch {}
          if ((gone = (await within(exited, 3000, () => 'timeout')) !== 'timeout')) break;
        }
        if (!gone) throw new Error(`the touch runner's xcodebuild (PID ${child.pid}) did not exit after SIGKILL; the device's touch lock and its record ${keptPath(udid)} are kept`);
        rmSync(keptPath(udid), { force: true });
      } else await retire(udid, kept);
    } catch (e) { await putBack(); throw e; } // the lock is kept while a runner may live; the keys go back regardless
    await release();
  };
  // The keys are found (and recorded) before anything turns them on. Unread, the session ends here and leaves them
  // as they are: with no record, the next session could not tell a person's setting from what this one left.
  const readKeys = async () => { if (!(await axFound)) throw new Error("the simulator's accessibility settings (com.apple.Accessibility) could not be read, so a session could not put them back after it; is the simulator booted?"); };
  try {
    const identity = runnerKey();
    const found = await keptRunner(udid, appId, identity.key);
    if (found) {
      ({ socket, talk, kept } = found), reused = true;
      // A new runner's XCTest turns them on; a kept one needs them written.
      await readKeys();
      const failed = (await Promise.all(AX_KEYS.map((k) => simDefaults(udid, ['write', AX, k, '-bool', 'true']))))
        .map((r, i) => r.status === 0 ? null : `${AX_KEYS[i]}: ${r.err.trim() || `exit ${r.status}`}`).filter(Boolean);
      if (failed.length) throw new Error(`the simulator's accessibility, which the kept touch runner needs, could not be turned on: ${failed.join('; ')}`);
    }
    else {
      await readKeys();
      const { app } = touchRunner(udid, identity);
      bridge = await phoneBridge();
      const run = resolve(tmpdir(), `exact-touches-${udid}.xctestrun`), log = resolve(tmpdir(), `exact-touches-${udid}.log`);
      writeFileSync(run, plist({
        ExactTouches: {
          // A simulator refuses `UseDestinationArtifacts` (device only), so
          // xcodebuild installs the target here: the runner itself, so one
          // runner serves every app (each session names its own) and the
          // driver's app is never installed or launched by it (D1's order).
          IsUITestBundle: true, TestHostPath: app, TestBundlePath: '__TESTHOST__/PlugIns/ExactTouches.xctest', UITargetAppPath: app,
          TestingEnvironmentVariables: { EXACT_TOUCH_CONNECT: bridge.env.EXACT_AGENT_CONNECT, EXACT_TOUCH_TOKEN: bridge.env.EXACT_AGENT_TOKEN, EXACT_TOUCH_APP: appId },
        },
        __xctestrun_metadata__: { FormatVersion: 1 },
      }));
      // Its own session and its output in a file: it outlives this process.
      const out = openSync(log, 'w');
      child = spawn('xcodebuild', ['test-without-building', '-xctestrun', run, '-derivedDataPath', resolve(ROOT, 'target/touch-runner/derived', udid), '-destination', `id=${udid}`, '-only-testing:ExactTouches/ExactTouches/testDrive'], { stdio: ['ignore', out, out], detached: true });
      closeSync(out);
      // Recorded before its hello: a driver killed while it waits leaves a runner the next one can retire.
      kept = { key: identity.key, port: null, token: bridge.env.EXACT_AGENT_TOKEN, pid: child.pid, run, log };
      writeFileSync(keptPath(udid), JSON.stringify(kept), { mode: 0o600 });
      exited = new Promise((r) => child.on('exit', (code) => r(code)));
      const first = await within(Promise.race([bridge.ready.then((x) => ({ x })), exited.then((code) => ({ code }))]), 30000, () => 'timeout');
      if (!first?.x) throw new Error(`the touch runner did not start (${first === 'timeout' ? 'no hello in 30 s' : `xcodebuild exited ${first.code}`}):\n${logTail(log)}`);
      socket = first.x.socket;
      talk = conversation(socket, () => logTail(log, 10));
      kept = { ...kept, port: first.x.announcement.listen };
      writeFileSync(keptPath(udid), JSON.stringify(kept), { mode: 0o600 });
      child.unref();
    }
    // Teardown runs once; a failure to end the runner is kept for `close()` to report.
    let closing = null;
    const end = () => (closing ??= teardown().then(() => null, (e) => e));
    const ask = async (req, ms = RUNNER_MS) => {
      const broken = talk.broken();
      if (broken) return { error: broken };
      return within(talk.request(req), ms, () => {
        // A late reply could answer the next request: the transport is spent.
        const why = `the touch runner did not answer ${req.op} within ${ms / 1000} s; it was stopped`;
        talk.fail(why);
        end();
        return { error: why };
      });
    };
    // Wait until the stream is reading, before the app's launch can bring a cloak
    await cloak?.ready();
    return {
      ask, pid: kept.pid, started: Date.now() - started, kept: reused, cloak,
      end,
      // A session that ends well hangs up and leaves the runner for the next.
      async close() {
        if (closing) { const failed = await closing; if (failed) throw failed; return; }
        if (talk.broken()) { const failed = await end(); if (failed) throw failed; return; }
        closing = Promise.resolve(null);
        cloak?.close();
        socket.end();
        await release();
      },
    };
  } catch (error) {
    try { await teardown(); } catch (e) { error.message += `; ${e.message}`; }
    throw error;
  }
}

const sameAim = (a, b) => a.orientation === b.orientation && a.session === b.session && a.generation === b.generation
  && ['w', 'h', 'x', 'y'].every((k) => a.screen[k] === b.screen[k]) && a.point[0] === b.point[0] && a.point[1] === b.point[1]
  && ['viewport', 'fixed', 'fixedAxes', 'display'].every(k => JSON.stringify(a[k]) === JSON.stringify(b[k]));

/** SpringBoard reads and gestures over the session's existing runner (WP14). */
export async function springboard(touches, request, alive) {
  const name = `SpringBoard ${request.action}`;
  if (!touches) throw new Error(`${name}: open the iOS session with touch: 'drag' or 'platform'`);
  alive(name);
  const result = await touches.ask({ ...request, op: 'springboard' });
  if (result.error) throw new Error(`${name}: ${result.error}`);
  alive(name);
  return result;
}

/** How long the ops during a still gesture wait for the touch to reach the view it landed on: a scroll view hands it
 *  on about 150 ms after the window dispatched it (`delaysContentTouches`). */
export const REACH_MS = 250;

/** How long before a real drag's lift its clock step (`drag.tick`, agent-drag.mjs `heldTick`) stops moving on real
 *  time: the touch is seen to begin a log read late, and the step's last request must land while the finger is down. */
export const TICK_MARGIN = 150;

/** A drag's bounds (LLP 1080.000 §11): each duration in ms; the travel is bounded so its moves fit the dispatch log's ring (D5). */
export const DRAG_BOUNDS = { press: 10000, hold: 10000, over: 2000, total: 10000 };

/**
 * A real tap on view `id` (LLP 1080.000 D4/D5/D8), or with `drag` one whole
 * real gesture from it (§11): the host aims (`at`, a viewport point, or the
 * target's middle), the aim is taken twice and must agree (orientation,
 * origin, size, point, session and display mapping), the runner checks the app is in the
 * foreground and injects, and the window's dispatch log must show exactly
 * one new touch that began and ended (for a drag that moves, moved by the
 * asked delta) — else an error that says what the log saw. `drag` is
 * `{dx, dy, press, over, hold, during, tick}`, points and ms; `during` are thunks
 * run while the finger is down after the move, each bracketed by the log:
 * begun before the first, not lifted after the last; `tick(until, down)`
 * moves the app's clock after them, given the local time to stop moving on
 * real time and whether the finger is still down. `ask` is the app's carrier; `touches`
 * the runner's.
 */
// What a lost touch's error advises: the synthesizer, unlike XCTest's public calls, does not wait for the app to go idle
const SETTLE_FIRST = 'a real touch played while native work still moves (a push, a sheet, the keyboard) can be lost, so `clock settle` first';

export async function realTap({ ask, touches, id, at, rel, drag, abandon }) {
  // A failure of the runner or of the app's carrier: no diagnostic read follows it (`tapRefusal`).
  const transport = (message) => Object.assign(new Error(message), { transport: true });
  const legs = drag?.path ? [drag, ...drag.path].map(({ dx, dy, press = 0, over = 250, hold = 0, during = [], tick }) => ({ dx, dy, press, over, hold, during, tick })) : null;
  let net = drag ? [drag.dx, drag.dy] : [0, 0];
  const what = drag ? `drag #${id}` : `tap #${id}`;
  // A plain tap aims at the target's own press (LLP 1012 §1: never a control inside it its middle reaches);
  // `rel` is a point in the target (`tap <target> at <x> <y>`), whatever a finger there reaches; a drag starts at the middle.
  const aimReq = { op: 'tap', id, aim: at ? { x: at[0], y: at[1] } : rel ? { at: rel } : drag ? true : { press: true } };
  // Wait out a fold cloak before aiming: it would take the touch (cloak.mjs)
  const cloaked = await touches.cloak?.clear() ?? 0;
  const first = await ask(aimReq);
  if (first.error) throw new Error(first.error);
  const fg = await touches.ask({ op: 'foreground' });
  if (fg.error) throw transport(`${what}: the touch runner: ${fg.error}`);
  if (fg.state !== 'runningForeground') throw new Error(`${what}: the app is ${fg.state ?? fg.error}, not in the foreground`);
  const aim = (await ask(aimReq));
  if (aim.error) throw new Error(aim.error);
  if (!sameAim(first.aim, aim.aim)) throw new Error(`${what}: the screen rotated or moved since aim`);
  const a = aim.aim;
  // `screen`: where XCTest's synthesizer presses a tap, on the display whose size is `display` (touches.swift).
  let request = { op: 'tap', point: a.point, ...(a.fixed ? { screen: a.fixed, display: a.display } : {}) }, moves = false, ms = 0;
  if (drag) {
    // The end, from the start the host resolved just now, in the viewport it reported with it.
    const end = [a.at[0] + drag.dx, a.at[1] + drag.dy], [vw, vh] = a.viewport;
    if (!(end[0] >= 0 && end[1] >= 0 && end[0] <= vw && end[1] <= vh)) throw new Error(`${what}: the drag would end at (${end}), outside the viewport (${vw} × ${vh})`);
    moves = drag.dx !== 0 || drag.dy !== 0;
    ms = drag.press + drag.hold + (moves ? drag.over : 0);
    // Points per second: what XCTest's velocity measured as on a 3x simulator, whatever its header says (§11).
    request = { op: 'drag', point: a.point, to: [a.point[0] + drag.dx, a.point[1] + drag.dy], press: drag.press / 1000, hold: drag.hold / 1000, velocity: moves ? Math.hypot(drag.dx, drag.dy) / (drag.over / 1000) : 0 };
    // Both ends in the screen's fixed space too, where XCTest's synthesizer plays the drag (touches.swift)
    if (a.fixed && a.fixedAxes) {
      const [[rx, ry], [dx, dy]] = a.fixedAxes;
      Object.assign(request, { screen: a.fixed, screenTo: [a.fixed[0] + drag.dx * rx + drag.dy * dx, a.fixed[1] + drag.dx * ry + drag.dy * dy], display: a.display });
    }
  }
  const checkpoints = [];
  if (legs) {
    if (!a.fixed || !a.fixedAxes) throw new Error(`${what}: a scripted path needs the screen coordinate axes`);
    const [[rx, ry], [ux, uy]] = a.fixedAxes;
    let x = 0, y = 0, offset = 0;
    request.path = legs.map((leg) => {
      offset += leg.press + (leg.dx || leg.dy ? leg.over : 0);
      x += leg.dx; y += leg.dy;
      const point = [a.at[0] + x, a.at[1] + y];
      if (point.some((v, i) => v < 0 || v > a.viewport[i])) throw new Error(`${what}: path waypoint (${point}) is outside the viewport`);
      checkpoints.push({ offset, end: offset + leg.hold, ops: leg.during, tick: leg.tick });
      offset += leg.hold;
      return { to: [a.fixed[0] + x * rx + y * ux, a.fixed[1] + x * ry + y * uy], over: (leg.dx || leg.dy ? leg.over : 0) / 1000, hold: leg.hold / 1000 };
    });
    net = [x, y]; ms = offset;
    moves = legs.some(l => l.dx || l.dy);
  }
  const injecting = touches.ask(request, RUNNER_MS + ms);
  let finished = false;
  injecting.then(() => { finished = true; });
  const during = [];
  // How long after the touch was seen to begin it reached its node (ms), or false: a still gesture's ops wait for it.
  let reachedIn;
  // The runner's call holds the finger and the app's carrier is free
  // meanwhile: `during` waits for the new touch to begin, runs the ops, and
  // checks that touch has not lifted. Every read and op races the gesture:
  // past its end (and a second's grace) nothing more runs, the dangling
  // request is left to the carrier's own bound, and the failure names it.
  // Whatever fails, the gesture is let finish first, and the runner's error
  // wins. Requests and touch dispatch share the main run loop, so a read
  // that finds no `ended` came before the window dispatched the lift.
  // A hold's clock (`drag.tick`) is moved the same way, after the ops, only while the touch is known to be down: a
  // gesture with no ops whose touch the log does not show down is left to the barrier below, its clock unmoved.
  const ops = legs ? checkpoints.flatMap(c => c.ops) : drag?.during ?? [];
  if (ops.length || drag?.tick || legs?.some(l => l.tick)) {
    const over = Symbol('over');
    const gestureEnd = injecting.then(() => new Promise((r) => setTimeout(r, 1000))).then(() => over);
    const raced = async (promise) => {
      promise.catch(() => {}); // a late failure after the race is not this drive's
      const r = await Promise.race([promise, gestureEnd]);
      if (r === over) {
        // The runner is done; the app has not answered a second after: its carrier is spent.
        const why = `${what}: the ops during it outlasted the gesture by a second; the app's carrier was stopped (lengthen hold)`;
        abandon?.(why);
        throw transport(why);
      }
      return r;
    };
    const ticked = async (tick, until, down) => {
      const work = tick(until, down);
      try { return await raced(work); }
      finally { await work.catch(() => {}); }
    };
    let reached = [], hostNow;
    const entries = async () => {
      const out = [];
      for (let cursor = a.seq; ;) {
        const page = await raced(ask({ op: 'tap', log: cursor }));
        if (page.error || page.lost) throw new Error(`${what}: the dispatch log: ${page.error ?? `dropped entries past seq ${cursor}`}`);
        out.push(...page.log);
        reached = page.reached ?? [];
        hostNow = page.now;
        if (page.log.length) cursor = page.log[page.log.length - 1].seq;
        if (!page.truncated) return out;
      }
    };
    // The one touch that began after the aim, and whether it has lifted.
    const ours = (log) => {
      const ids = [...new Set(log.filter((e) => e.phase === 'began').map((e) => e.touch))];
      if (ids.length > 1) throw new Error(`${what}: ambiguous: ${ids.length} touches after seq ${a.seq}: ${JSON.stringify(log.slice(0, 8))}`);
      if (!ids.length) return null;
      const began = log.find((e) => e.touch === ids[0] && e.phase === 'began');
      return { began: began.t, lifted: log.some((e) => e.touch === ids[0] && e.phase === 'ended'), reports: began.reports === true, reached: reached.includes(ids[0]) };
    };
    const early = `${what}: the touch lifted before the ops during it`;
    try {
      // A `return` here leaves the touch to the barrier below, with nothing more run and the clock unmoved.
      await (async () => {
        for (let touch; !(touch = ours(await entries()));) {
          if (finished && !ops.length) return;
          if (finished) throw new Error(ours(await entries())?.lifted ? `${early} began; lengthen hold` : `${what}: the runner finished and no touch had begun; ${SETTLE_FIRST}`);
          await new Promise((r) => setTimeout(r, 50));
        }
        let touch = ours(await entries());
        const seen = Date.now();
        // A still finger on a view that reports it (a node, a native button): its ops wait for its `touchesBegan`
        // (`reached`), so what they read is what a person holding it sees. A touch some recognizer keeps from the view
        // runs them at the bound, or, with `drag.reach` (a held contact's reads), fails before them.
        const initialStill = legs ? !legs[0].dx && !legs[0].dy : !moves;
        const initialReads = legs ? checkpoints[0].ops.length : ops.length;
        const checkpointEnd = legs ? touch.began + checkpoints[0].end / 1000 : Infinity;
        for (const by = Date.now() + REACH_MS; initialReads && initialStill && touch.reports && !touch.reached && !touch.lifted && Date.now() < by && (!legs || hostNow < checkpointEnd);) {
          await new Promise((r) => setTimeout(r, 16));
          touch = ours(await entries());
        }
        if (touch.lifted && !ops.length) return;
        if (touch.lifted) throw new Error(`${early} began; lengthen hold`);
        if (initialReads && initialStill && touch.reports) reachedIn = touch.reached ? Date.now() - seen : false;
        if ((drag.reach || legs && initialReads && initialStill) && reachedIn === false) throw new Error(`${what}: the touch did not reach the view it landed on within ${legs ? Math.min(REACH_MS, checkpoints[0].end) : REACH_MS} ms (a recognizer kept it), so nothing inside it ran`);
        if (legs) {
          const began = (await entries()).find(e => e.phase === 'began');
          if (!Number.isFinite(hostNow)) throw new Error(`${what}: rebuild the iOS app for checkpoint timestamps`);
          for (let i = 0; i < checkpoints.length; i++) {
            const checkpoint = checkpoints[i];
            if (!checkpoint.ops.length && !checkpoint.tick) continue;
            const start = began.t + checkpoint.offset / 1000, end = began.t + checkpoint.end / 1000;
            const held = async () => {
              const touch = ours(await entries());
              if (!touch || touch.lifted || finished || hostNow >= end) throw new Error(`${what}: checkpoint ${i + 1} ended before its reads finished; lengthen hold`);
            };
            while (hostNow < start) {
              await held();
              if (hostNow < start) await new Promise(r => setTimeout(r, Math.min(50, Math.max(1, (start - hostNow) * 1000))));
            }
            for (const op of checkpoint.ops) {
              await held();
              during.push(await raced(op()));
              await held();
            }
            if (checkpoint.tick) await ticked(checkpoint.tick, Date.now() + (end - hostNow) * 1000 - TICK_MARGIN, async () => {
              if (finished) return false;
              const touch = ours(await entries());
              return !!touch && !touch.lifted && !finished && hostNow < end;
            });
          }
        } else {
          // Reads follow the move, inside its hold.
          await new Promise((r) => setTimeout(r, Math.max(0, drag.press + (moves ? drag.over : 0) - (Date.now() - seen))));
          for (const op of ops) {
            const lifted = ours(await entries()).lifted;
            if (lifted || finished) throw new Error(`${early} finished; lengthen hold`);
            during.push(await raced(op()));
          }
          if (ops.length && ours(await entries()).lifted) throw new Error(`${early} finished; lengthen hold`);
        }
        const down = async () => {
          if (finished) return false;
          const touch = ours(await entries());
          return !finished && touch != null && !touch.lifted;
        };
        // Until shortly before the lift, asking before each step that the finger is still down (`heldTick`).
        if (drag.tick && !legs) await ticked(drag.tick, seen + drag.press + (moves ? drag.over : 0) + drag.hold - TICK_MARGIN, down);
      })();
    } catch (error) {
      const done = await injecting;
      if (done.error) throw transport(`${what}: the touch runner: ${done.error}`);
      throw error;
    }
  }
  const injected = await injecting;
  if (injected.error) throw transport(`${what}: the touch runner: ${injected.error}`);
  // The barrier: every entry after the aim's seq, paged by the last one read
  // (a reply holds 32), each read bounded by what is left of the second.
  const deadline = Date.now() + 1000;
  const seen = [];
  let cursor = a.seq;
  for (;;) {
    const left = deadline - Date.now();
    const page = await within(ask({ op: 'tap', log: cursor }), Math.max(left, 1), () => null);
    if (!page) {
      // The app did not answer within the second: its carrier is spent (a
      // late reply must not answer a later request) and the runner ends now.
      const why = `${what}: the dispatch log did not answer within 1 s of the touch; the app's carrier and the touch runner were stopped`;
      abandon?.(why);
      touches.end?.();
      throw transport(why);
    }
    if (page.error) throw new Error(`${what}: the dispatch log: ${page.error}`);
    if (page.lost) throw new Error(`${what}: the dispatch log dropped entries past seq ${cursor}`);
    seen.push(...page.log);
    if (page.log.length) cursor = page.log[page.log.length - 1].seq;
    if (page.truncated) continue; // more already logged: read it before judging
    const ids = [...new Set(seen.map((e) => e.touch))];
    if (ids.length > 1) throw new Error(`${what}: ambiguous: ${ids.length} touches after seq ${a.seq}: ${JSON.stringify(seen.slice(0, 8))}`);
    const began = seen.find((e) => e.phase === 'began'), end = seen.find((e) => e.phase === 'ended');
    if (began && end) {
      // A row UIKit's menu shows (`native`) is in no session and no node: the touch must land outside every node.
      if (a.native) {
        if (began.node != null) throw new Error(`${what}: the touch landed on node #${began.node}, not on UIKit's ${a.native.kind} "${a.native.label}"`);
      } else {
        if (began.session !== a.session || began.generation !== a.generation) throw new Error(began.session == null ? `${what}: the touch landed on ${began.view}, outside every session, not in ${a.session}` : `${what}: the touch landed in session ${began.session} (generation ${began.generation}), not ${a.session}`);
        if (began.node !== a.hit) throw new Error(`${what}: the touch landed on ${began.node == null ? began.view : `node #${began.node}`}, not on node #${a.hit} the aim hit-tested`);
      }
      // A grouped list's cells are all the list's node: the row and part too (LLP 1084).
      if (a.projected && (began.projected?.row !== a.projected.row || began.projected?.part !== a.projected.part)) throw new Error(`${what}: the touch landed on ${JSON.stringify(began.projected ?? null)} in the grouped list, not on ${JSON.stringify(a.projected)} the aim hit-tested`);
      const moved = seen.filter((e) => e.phase === 'moved');
      const travel = [end.at[0] - began.at[0], end.at[1] - began.at[1]];
      if (drag) {
        // It began where aimed and lifted where asked, each within a point
        // or 5% of the distance, in the window's space the log records.
        const tolerance = Math.max(1, 0.05 * Math.hypot(...net)), w = a.window;
        const startOff = Math.hypot(began.at[0] - w[0], began.at[1] - w[1]), endOff = Math.hypot(end.at[0] - w[0] - net[0], end.at[1] - w[1] - net[1]);
        if (startOff > tolerance) throw new Error(`${what}: the touch began at (${began.at}), not at the aimed (${w}) within ${tolerance.toFixed(1)} pt`);
        if (endOff > tolerance || (moves && !moved.length)) throw new Error(`${what}: the touch moved (${travel}) in ${moved.length} moves, not the asked (${net[0]}, ${net[1]}) within ${tolerance.toFixed(1)} pt`);
      }
      return {
        tapped: id, at: a.at, delivery: 'platform',
        landed: { session: began.session, generation: began.generation, node: began.node, view: began.view },
        touch: { id: began.touch, began: began.t, ended: end.t, ...(began.dispatched != null ? { dispatched: began.dispatched } : {}), moved: moved.length, type: began.type, ...(drag ? { travel, lastMove: moved.at(-1)?.t ?? null } : {}) },
        ...(drag ? { drag: { dx: drag.dx, dy: drag.dy, press: drag.press, over: moves ? drag.over : 0, hold: drag.hold, ...(legs ? { path: legs.map(({during, tick, ...leg}) => leg) } : {}) } } : {}),
        ...(during.length ? { during } : {}), ...(reachedIn !== undefined ? { reached: reachedIn } : {}),
        // The view the touch began on left the window before the lift (D5): it had no lift, though another view may
        // have acted on the touch (a scroller under a drag, a press on its down), so the caller reads the state
        ...(end.viewGone ? { viewGone: true } : {}),
        // A tap's down the window had only once the finger had lifted (D5): the app's main thread busy, or the system
        // slow to deliver it. A list cell's `UISwitch` takes a down and a lift that arrive together as no tap, so the
        // caller reads the state. A drag or a held contact says nothing: only a tap's were measured to lose anything.
        ...(!drag && began.dispatched >= end.t ? { downAfterLift: true } : {}),
        injected: injected.injected, aim: a.point, orientation: a.orientation,
        ...(a.avoided ? { avoided: a.avoided } : {}), ...(a.native ? { native: a.native } : {}),
      };
    }
    if (Date.now() >= deadline) throw new Error(`${what}: no touch reached the app's window within 1 s; the runner finished at ${injected.injected?.end} (log: ${JSON.stringify(seen)}); ${SETTLE_FIRST}${cloaked ? ` (after waiting ${cloaked} ms for SpringBoard's fold cloak, cloak.mjs)` : ''}`);
    await new Promise((r) => setTimeout(r, 16));
  }
}
