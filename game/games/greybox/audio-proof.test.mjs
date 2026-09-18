import { test, expect } from 'bun:test';
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { audioProof } from '../../bench/probes/audio.mjs';
import { proofInputExcluded } from '../../proof.mjs';

test('probe setup failure reaps its process group and removes its profile', async () => {
  const out = mkdtempSync(resolve(tmpdir(), 'audio-teardown-'));
  let child, profile;
  try {
    await expect(audioProof({out, check() {}, say() {},
      spawnBrowser(_chrome, args, options) {
        profile = args.find(a => a.startsWith('--user-data-dir=')).split('=')[1];
        child = spawn(process.execPath, ['-e', 'setInterval(() => {}, 1000)'], options);
        return child;
      },
      connect() { throw new Error('injected CDP setup failure'); },
    })).rejects.toThrow('injected CDP setup failure');
    expect(child.exitCode !== null || child.signalCode !== null).toBe(true);
    expect(() => process.kill(-child.pid, 0)).toThrow();
    expect(existsSync(profile)).toBe(false);
  } finally {
    if (child?.pid) { try { process.kill(-child.pid, 'SIGKILL'); } catch {} }
    if (profile) rmSync(profile, {recursive:true, force:true});
    rmSync(out, {recursive:true, force:true});
  }
});

test('probe source is outside the deterministic proof input digest', () => {
  // The proof's own exclusion predicate, not a copied regex.
  expect(proofInputExcluded('game/bench/probes/audio.mjs', 'greybox')).toBe(true);
  expect(proofInputExcluded('game/games/greybox/logic/src/lib.rs', 'greybox')).toBe(false);
  expect(proofInputExcluded('game/games/beacons/logic/src/lib.rs', 'greybox')).toBe(true);
  expect(proofInputExcluded('game/games/greybox/proof.mjs', 'greybox')).toBe(true);
});

test('web visibility and page events reach every canvas under either clock', () => {
  const source = readFileSync(resolve(import.meta.dir, '../../../host/web/gpu-glue.js'), 'utf8');
  const start = source.indexOf('let hidden = document.hidden;');
  expect(start).toBeGreaterThanOrEqual(0);
  const code = source.slice(start, source.indexOf('function render(entry, now)', start));
  for (const seekable of [false, true]) {
    const listeners = {}, calls = [], canceled = [];
    let scheduled = 0;
    const document = {hidden:false, addEventListener(name, fn) { listeners[name] = fn; }};
    const window = {addEventListener(name, fn) { listeners[name] = fn; }};
    new Function('document','window','surfaces','gpu','exact','raf','cancelAnimationFrame','schedule', code)(
      document, window, new Map([[1, {id:11}], [2, {id:22}], [3, {id:0}]]),
      {gpu_lifecycle(id, code) { calls.push([id, code]); }}, seekable ? {now:() => 0} : {},
      9, id => canceled.push(id), () => scheduled++);
    document.hidden = true; listeners.visibilitychange();
    listeners.pagehide();
    document.hidden = false; listeners.pageshow({persisted:false});
    expect(calls).toEqual([[11,0],[22,0],[11,0],[22,0],[11,1],[22,1]]);
    expect(canceled).toEqual(seekable ? [] : [9]);
    expect(scheduled).toBe(1);
    document.hidden = true;
    listeners.pageshow({persisted:true});
    expect(calls.slice(-2)).toEqual([[11,1],[22,1]]);
    listeners.visibilitychange();
    expect(calls.slice(-2)).toEqual([[11,0],[22,0]]);
    listeners.pageshow({persisted:false});
    expect(calls.slice(-2)).toEqual([[11,0],[22,0]]);
  }
});

async function runSwiftFixture(binary) {
  const child = spawn('/bin/sh', ['-c', 'exec "$1" "$2"', 'audio-fixture', binary, binary + '.passed'], {stdio:['ignore','pipe','pipe']});
  let stdout = '', stderr = '';
  child.stdout.on('data', data => { stdout += data; });
  child.stderr.on('data', data => { stderr += data; });
  const timer = setTimeout(() => child.kill('SIGKILL'), 10000);
  try {
    const status = await new Promise((ok, fail) => { child.on('error', fail); child.on('close', ok); });
    return {status, stdout: existsSync(binary + '.passed') ? readFileSync(binary + '.passed', 'utf8') : stdout, stderr};
  } finally { clearTimeout(timer); }
}

// Compile the production notification adapter with a tiny canvas/ABI fixture.
// No device, window, Rust module or iOS simulator is opened.
test('Apple background notifications deliver in order on main', async () => {
  if (process.platform !== 'darwin') return;
  const dir = mkdtempSync(resolve(tmpdir(), 'audio-main-'));
  try {
    const source = readFileSync(resolve(import.meta.dir, '../../../host/apple/Sources/ExactKit/CanvasSeams.swift'), 'utf8');
    const adapter = source.slice(source.indexOf('// ExactKit owns session policy'));
    const stubs = `import Foundation
import AppKit
enum ExactEnv { static let agentMode = false }
final class Frames { func requestCanvas() {} }
final class Session { let frames = Frames() }
final class Module { var lifecycle: ((UInt32, UInt32) -> Void)? }
final class Canvases {
    final class Entry { let id: UInt32 = 1 }
    var module: Module? = Module()
    var entries = [1: Entry()]
    var session: Session? = Session()
    var visible = true
}
`;
    const fixture = `
let owner = Canvases()
var calls: [(UInt32, Bool)] = []
owner.module!.lifecycle = { _, code in calls.append((code, Thread.isMainThread)) }
let lifecycle = CanvasLifecycle(owner)
func post(_ name: Notification.Name, visible: Bool) {
    owner.visible = visible
    let done = DispatchSemaphore(value: 0)
    DispatchQueue.global().async {
        NotificationCenter.default.post(name: name, object: nil)
        done.signal()
    }
    let deadline = Date().addingTimeInterval(2)
    while done.wait(timeout: .now()) != .success && Date() < deadline {
        RunLoop.main.run(until: Date().addingTimeInterval(0.001))
    }
    RunLoop.main.run(until: Date().addingTimeInterval(0.02))
}
post(NSApplication.didHideNotification, visible: false)
post(NSApplication.didUnhideNotification, visible: true)
precondition(calls.map { $0.0 } == [0,1], "notification order")
precondition(calls.allSatisfy { $0.1 }, "GPU ABI must run on main")
print("PASS main queue notification delivery")
// Same predicates at creation and notification, transition-only, including late surfaces.
calls.removeAll()
NotificationCenter.default.post(name: NSApplication.didResignActiveNotification, object:nil)
RunLoop.main.run(until:Date().addingTimeInterval(0.02))
precondition(calls.isEmpty, "losing focus alone is not Hidden")
owner.visible = false
lifecycle.refresh()
lifecycle.refresh()
precondition(calls.map { $0.0 } == [0], "occlusion sends only its transition")
calls.removeAll()
lifecycle.deliver(2)
precondition(calls.map { $0.0 } == [0], "late surface inherits occlusion")
calls.removeAll()
owner.visible = true
lifecycle.deliver(1)
precondition(calls.map { $0.0 } == [1], "a new surface receives the changed aggregate only once")
// Activation failure holds Interrupted, retries Visible, and never lies about Resumed.
owner.visible = true
var allowed = false
var attempts = 0
let audio = CanvasLifecycle(owner, activate: { attempts += 1; return allowed })
calls.removeAll()
audio.requestAudio()
precondition(calls.map { $0.0 } == [2])
// A failed activation opens a 300-frame cooldown: a refresh alone does not retry,
// a gesture retries at once, and the cooldown's end retries again.
audio.refresh()
precondition(attempts == 1 && calls.map { $0.0 } == [2], "no retry inside the cooldown")
audio.gesture()
RunLoop.main.run(until:Date().addingTimeInterval(0.02))
precondition(attempts == 2 && calls.map { $0.0 } == [2], "a gesture retries at once and holds Interrupted")
allowed = true
for _ in 0..<300 { audio.frame() }
precondition(attempts == 3 && calls.map { $0.0 } == [2,3], "the cooldown's end retries and emits Resumed")
calls.removeAll()
let done = DispatchSemaphore(value: 0)
DispatchQueue.global().async {
    audio.interruption(began: true, shouldResume: false)
    audio.interruption(began: false, shouldResume: false)
    done.signal()
}
while done.wait(timeout:.now()) != .success { RunLoop.main.run(until:Date().addingTimeInterval(0.001)) }
RunLoop.main.run(until:Date().addingTimeInterval(0.02))
precondition(calls.map { $0.0 } == [2] && calls.allSatisfy { $0.1 })
audio.refresh()
precondition(calls.map { $0.0 } == [2], "honor shouldResume=false")
allowed = false
audio.interruption(began: false, shouldResume: true)
precondition(calls.map { $0.0 } == [2], "failed reactivation cannot emit Resumed")
allowed = true
for _ in 0..<300 { audio.frame() }
precondition(calls.map { $0.0 } == [2,3])
try "PASS aggregate visibility, activation retry, ordered interruptions and shouldResume".write(toFile:CommandLine.arguments[1], atomically:true, encoding:.utf8)
`;
    const file = resolve(dir, 'main.swift'), binary = resolve(dir, 'fixture');
    writeFileSync(file, stubs + adapter + fixture);
    const build = spawnSync('/Library/Developer/CommandLineTools/usr/bin/swiftc', ['-sdk', '/Library/Developer/CommandLineTools/SDKs/MacOSX26.sdk', file, '-o', binary], {encoding:'utf8'});
    expect(build.status, build.stderr).toBe(0);
    const run = await runSwiftFixture(binary);
    expect(run.status, run.stderr).toBe(0);
    expect(run.stdout).toContain("PASS aggregate visibility");
  } finally { rmSync(dir, {recursive:true, force:true}); }
}, 30000);

test('iOS notification mapping honors shouldResume and failed activation on main', async () => {
  if (process.platform !== 'darwin') return;
  const dir = mkdtempSync(resolve(tmpdir(), 'audio-ios-'));
  try {
    const source = readFileSync(resolve(import.meta.dir, '../../../host/apple/Sources/ExactKit/CanvasSeams.swift'), 'utf8');
    const adapter = source.slice(source.indexOf('// ExactKit owns session policy'))
      .replaceAll('#if os(macOS)', '#if false').replaceAll('#if os(iOS)', '#if true');
    const stubs = `import Foundation
let AVAudioSessionInterruptionTypeKey = "type"
let AVAudioSessionInterruptionOptionKey = "options"
enum ExactEnv { static let agentMode = false }
enum UIApplication {
    static let willResignActiveNotification = Notification.Name("resign")
    static let didEnterBackgroundNotification = Notification.Name("background")
    static let willEnterForegroundNotification = Notification.Name("foreground")
    static let didBecomeActiveNotification = Notification.Name("active")
}
final class AVAudioSession {
    enum InterruptionType: UInt { case began = 1, ended = 0 }
    struct InterruptionOptions: OptionSet { let rawValue: UInt; static let shouldResume = Self(rawValue:1) }
    enum Category { case ambient }
    static let interruptionNotification = Notification.Name("interruption")
    static let instance = AVAudioSession()
    static func sharedInstance() -> AVAudioSession { precondition(Thread.isMainThread); return instance }
    var fails = true
    var attempts = 0
    func setCategory(_ category: Category) throws { precondition(Thread.isMainThread) }
    func setActive(_ active: Bool) throws {
        precondition(Thread.isMainThread)
        attempts += 1
        if fails { throw NSError(domain:"injected", code:1) }
    }
}
final class Frames { func requestCanvas() {} }
final class Session { let frames = Frames() }
final class Module { var lifecycle: ((UInt32, UInt32) -> Void)? }
final class Canvases {
    final class Entry { let id: UInt32 = 1 }
    var module: Module? = Module()
    var entries = [1:Entry()]
    var session: Session? = Session()
    var visible = true
}
`;
    const fixture = `
let owner = Canvases()
var calls: [UInt32] = []
owner.module!.lifecycle = { _, code in precondition(Thread.isMainThread); calls.append(code) }
let lifecycle = CanvasLifecycle(owner)
func background(_ work: @escaping () -> Void) {
    let done = DispatchSemaphore(value:0)
    DispatchQueue.global().async { work(); done.signal() }
    while done.wait(timeout:.now()) != .success { RunLoop.main.run(until:Date().addingTimeInterval(0.001)) }
    RunLoop.main.run(until:Date().addingTimeInterval(0.02))
}
background { CanvasAudio.activate() }
precondition(!CanvasAudio.active && CanvasAudio.wanted)
lifecycle.requestAudio()
precondition(calls == [2])
// A failed activation opens a 300-frame cooldown: becoming active again inside it
// does not retry, a gesture retries at once, the cooldown's end retries again.
NotificationCenter.default.post(name:UIApplication.didBecomeActiveNotification, object:nil)
RunLoop.main.run(until:Date().addingTimeInterval(0.02))
precondition(calls == [2] && AVAudioSession.instance.attempts == 2, "no retry inside the cooldown")
lifecycle.gesture()
RunLoop.main.run(until:Date().addingTimeInterval(0.02))
precondition(calls == [2] && AVAudioSession.instance.attempts == 3, "a gesture retries at once")
AVAudioSession.instance.fails = false
for _ in 0..<300 { lifecycle.frame() }
precondition(calls == [2,3], "the cooldown's end retries and emits Resumed")
calls.removeAll()
background {
    NotificationCenter.default.post(name:AVAudioSession.interruptionNotification, object:nil, userInfo:[AVAudioSessionInterruptionTypeKey:UInt(1)])
    NotificationCenter.default.post(name:AVAudioSession.interruptionNotification, object:nil, userInfo:[AVAudioSessionInterruptionTypeKey:UInt(0), AVAudioSessionInterruptionOptionKey:UInt(0)])
}
precondition(calls == [2], "do not automatically resume without shouldResume")
AVAudioSession.instance.fails = true
background {
    NotificationCenter.default.post(name:AVAudioSession.interruptionNotification, object:nil, userInfo:[AVAudioSessionInterruptionTypeKey:UInt(0), AVAudioSessionInterruptionOptionKey:UInt(1)])
}
precondition(calls == [2], "failed setActive cannot emit Resumed")
AVAudioSession.instance.fails = false
for _ in 0..<300 { lifecycle.frame() }
precondition(calls == [2,3])
try "PASS iOS background notification order, main-thread activation, retries, shouldResume".write(toFile:CommandLine.arguments[1], atomically:true, encoding:.utf8)
`;
    const file = resolve(dir, 'main.swift'), binary = resolve(dir, 'fixture');
    writeFileSync(file, stubs + adapter + fixture);
    const build = spawnSync('/Library/Developer/CommandLineTools/usr/bin/swiftc', ['-sdk', '/Library/Developer/CommandLineTools/SDKs/MacOSX26.sdk', file, '-o', binary], {encoding:'utf8'});
    if (build.status !== 0) throw new Error(build.stderr);
    const run = await runSwiftFixture(binary);
    if (run.status !== 0) throw new Error(run.stderr);
    expect(run.stdout).toContain('PASS iOS background notification order');
  } finally { rmSync(dir, {recursive:true, force:true}); }
}, 30000);
