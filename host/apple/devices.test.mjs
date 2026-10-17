// LLP 1116.008 D8: a window capture of the app with the software keyboard put in from the simulator's display.
// A fake `xcrun` stands for `simctl io <udid> screenshot`: it copies a prepared display picture to the path it is given.
import { test, expect } from 'bun:test';
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, rmSync, utimesSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { decodePng, encodePng } from '../../scripts/png.mjs';
import { crashReports, crashSummary, crashWatch, withDisplay, phoneBridge } from './devices.mjs';
import { spawn } from 'node:child_process';
import { createConnection } from 'node:net';
import { once } from 'node:events';

// Native startup retries do not change the per-launch authentication boundary.
test('a refused phone token cannot admit a command carrier; the right token still can', async () => {
  const before = process.env.EXACT_AGENT_HOST;
  process.env.EXACT_AGENT_HOST = '127.0.0.1';
  let bridge;
  try { bridge = await phoneBridge(); }
  finally { if (before === undefined) delete process.env.EXACT_AGENT_HOST; else process.env.EXACT_AGENT_HOST = before; }
  const [host, port] = bridge.env.EXACT_AGENT_CONNECT.split(':');
  const sockets = [];
  try {
    let admitted = false;
    bridge.ready.then(() => { admitted = true; });
    const wrong = createConnection({host, port: Number(port)}); sockets.push(wrong);
    await once(wrong, 'connect');
    const refused = once(wrong, 'close');
    wrong.write(JSON.stringify({ready: true, token: 'wrong-token'}) + '\n');
    await refused;
    expect(admitted).toBe(false);
    const right = createConnection({host, port: Number(port)}); sockets.push(right);
    await once(right, 'connect');
    right.write(JSON.stringify({ready: true, token: bridge.env.EXACT_AGENT_TOKEN}) + '\n');
    const accepted = await bridge.ready;
    expect(accepted.announcement).toEqual({ready: true});
    expect(admitted).toBe(true);
  } finally { for (const socket of sockets) socket.destroy(); bridge.close(); }
});

const solid = (width, height, rgba) => encodePng({ width, height, data: Uint8Array.from({ length: width * height * 4 }, (_, i) => rgba[i % 4]) });

function withFakeSimctl(display, body) {
  const dir = mkdtempSync(resolve(tmpdir(), 'devices-test-'));
  const [path, env] = [resolve(dir, 'display.png'), { PATH: process.env.PATH, DEVELOPER_DIR: process.env.DEVELOPER_DIR }];
  try {
    writeFileSync(path, display);
    writeFileSync(resolve(dir, 'xcrun'), `#!/bin/sh\nfor last; do :; done\ncp '${path}' "$last"\n`);
    chmodSync(resolve(dir, 'xcrun'), 0o755);
    process.env.PATH = `${dir}:${env.PATH}`;
    process.env.DEVELOPER_DIR ??= '/Applications/Xcode.app/Contents/Developer';
    return body(dir);
  } finally {
    process.env.PATH = env.PATH;
    if (env.DEVELOPER_DIR === undefined) delete process.env.DEVELOPER_DIR;
    rmSync(dir, { recursive: true, force: true });
  }
}

test('the rows from the keyboard\'s top down are the display\'s, the app above them its own', () => {
  withFakeSimctl(solid(4, 8, [0, 0, 255, 255]), (dir) => {
    const shot = resolve(dir, 'app.png');
    writeFileSync(shot, solid(4, 8, [255, 0, 0, 255]));
    const r = withDisplay({ scale: 2, keyboard: { visible: true, top: 2, windowTop: 2 }, note: 'old' }, shot, 'UDID');
    const png = decodePng(readFileSync(shot)), at = (y) => [...png.data.subarray(y * 16, y * 16 + 4)];
    expect([at(0), at(3)]).toEqual([[255, 0, 0, 255], [255, 0, 0, 255]]);
    expect([at(4), at(7)]).toEqual([[0, 0, 255, 255], [0, 0, 255, 255]]);
    expect(r.keyboard.captured).toBe(true);
    expect(r.note).toContain('in the capture');
  });
});

test('a display of another size leaves the picture as it was, and says why', () => {
  withFakeSimctl(solid(6, 8, [0, 0, 255, 255]), (dir) => {
    const shot = resolve(dir, 'app.png'), app = solid(4, 8, [255, 0, 0, 255]);
    writeFileSync(shot, app);
    const r = withDisplay({ scale: 2, keyboard: { visible: true, top: 2, windowTop: 2 } }, shot, 'UDID');
    expect(readFileSync(shot).equals(Buffer.from(app))).toBe(true);
    expect(r.keyboard.captured).toBeUndefined();
    expect(r.note).toContain('the display is 6x8, the capture 4x8');
  });
});

test('the status bar\'s rows are the display\'s', () => {
  withFakeSimctl(solid(4, 8, [0, 0, 255, 255]), (dir) => {
    const shot = resolve(dir, 'app.png');
    writeFileSync(shot, solid(4, 8, [255, 0, 0, 255]));
    const r = withDisplay({ scale: 2, statusBar: 1 }, shot, 'UDID');
    const png = decodePng(readFileSync(shot)), at = (y) => [...png.data.subarray(y * 16, y * 16 + 4)];
    expect([at(0), at(1), at(2), at(7)]).toEqual([[0, 0, 255, 255], [0, 0, 255, 255], [255, 0, 0, 255], [255, 0, 0, 255]]);
    expect(r.statusBar).toBe(true);
    expect(r.note).toBeUndefined();
  });
});

test('a capture that says no keyboard top is left alone', () => {
  const r = withDisplay({ scale: 2, keyboard: { visible: true, top: 2 } }, '/nonexistent.png', 'UDID');
  expect(r.note).toContain('no keyboard top');
});

// The euxy port (friction item 13): every simulator's copy of an app writes `<name>-<time>.ips`, so the name alone
// read other simulators' crashes as this one's. A report is this app's on this simulator by its header's bundle id
// and its process path; `logs` answers the newest one since the build, once.
test("crash reports are this app's on this simulator, newest first, summarized by the crashing thread", () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-crash-'));
  try {
    const report = (file, bundleID, udid, seconds) => {
      const body = { procPath: `/Users/USER/Library/Developer/CoreSimulator/Devices/${udid}/data/Containers/Bundle/Application/X/euxy.app/euxy`,
        exception: { type: 'EXC_BREAKPOINT', signal: 'SIGTRAP' }, termination: { indicator: 'Trace/BPT trap: 5' },
        asi: { 'libswiftCore.dylib': ['Fatal error: no engine'] }, faultingThread: 0, usedImages: [{ name: 'libexact_modules.dylib' }],
        threads: [{ triggered: true, queue: 'com.apple.main-thread', frames: [{ symbol: 'Engine.start()', symbolLocation: 12, imageIndex: 0 }, { imageOffset: 255, imageIndex: 0 }] }] };
      writeFileSync(resolve(dir, file), JSON.stringify({ bundleID, timestamp: `t${seconds}` }) + '\n' + JSON.stringify(body));
      utimesSync(resolve(dir, file), new Date(seconds * 1000), new Date(seconds * 1000));
    };
    report('euxy-1.ips', 'dev.brent.euxy', 'MINE', 100);
    report('euxy-2.ips', 'dev.brent.euxy', 'MINE', 200);
    report('euxy-3.ips', 'dev.brent.euxy', 'OTHER', 300);
    report('euxy-4.ips', 'dev.other.euxy', 'MINE', 400);
    const scope = { bundleId: 'dev.brent.euxy', udid: 'MINE', dir };
    expect(crashReports('euxy', 0, scope).map((f) => f.slice(dir.length + 1))).toEqual(['euxy-2.ips', 'euxy-1.ips']);
    expect(crashReports('euxy', 150_000, scope).map((f) => f.slice(dir.length + 1))).toEqual(['euxy-2.ips']);
    expect(crashSummary(resolve(dir, 'euxy-2.ips'))).toEqual([
      `crash report: ${resolve(dir, 'euxy-2.ips')} (t200)`,
      '  EXC_BREAKPOINT (SIGTRAP), Trace/BPT trap: 5; thread 0 (com.apple.main-thread) crashed:',
      '  Fatal error: no engine',
      '   0 Engine.start() + 12 (libexact_modules.dylib)',
      '   1 0xff (libexact_modules.dylib)',
    ]);
    expect(crashSummary(resolve(dir, 'missing.ips'))).toEqual([`crash report: ${resolve(dir, 'missing.ips')}`]);
    // The older text form: placed by its Identifier and Path lines, summarized from its fields and crashed thread.
    const legacy = (file, udid, seconds) => {
      writeFileSync(resolve(dir, file), [
        'Process:               euxy [84952]', `Path:                  /Users/USER/Library/Developer/CoreSimulator/Devices/${udid}/data/euxy.app/euxy`,
        'Identifier:            dev.brent.euxy', 'Date/Time:             2026-10-16 06:10:55.000 -0700', '',
        'Exception Type:        EXC_CRASH (SIGABRT)', 'Termination Reason:    SIGNAL 6 Abort trap: 6', '',
        'Application Specific Information:', 'abort() called', '',
        'Thread 1 Crashed::  Dispatch queue: euxy.sound', '0   libsystem_kernel.dylib        \t0x1b2c3d4e5 __pthread_kill + 8',
        '1   libexact_modules.dylib        \t0x100200300 Engine.start() + 12', '', 'Thread 2:', '0   libsystem_kernel.dylib  0x1 x + 1',
      ].join('\n'));
      utimesSync(resolve(dir, file), new Date(seconds * 1000), new Date(seconds * 1000));
    };
    legacy('euxy-5.crash', 'MINE', 500);
    legacy('euxy-6.crash', 'OTHER', 600);
    expect(crashReports('euxy', 0, scope).map((f) => f.slice(dir.length + 1))).toEqual(['euxy-5.crash', 'euxy-2.ips', 'euxy-1.ips']);
    expect(crashSummary(resolve(dir, 'euxy-5.crash'))).toEqual([
      `crash report: ${resolve(dir, 'euxy-5.crash')} (2026-10-16 06:10:55.000 -0700)`,
      '  EXC_CRASH (SIGABRT), SIGNAL 6 Abort trap: 6; thread 1 (euxy.sound) crashed:',
      '  abort() called',
      '   0 __pthread_kill + 8 (libsystem_kernel.dylib)',
      '   1 Engine.start() + 12 (libexact_modules.dylib)',
    ]);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

// A launch that crashes before the agent's socket opens (Astra's review): the failure reads a report already written since the
// build, and waits for the one macOS writes a moment after the process dies.
test('a failed launch reads the crash report written before it, and waits for the one its own crash writes', () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-crash-watch-')), bundle = resolve(dir, 'euxy.app');
  try {
    const at = (file, ms) => utimesSync(file, new Date(ms), new Date(ms));
    const report = (file) => writeFileSync(resolve(dir, file), JSON.stringify({ bundleID: 'dev.brent.euxy', timestamp: file }) + '\n' + JSON.stringify({ procPath: '/CoreSimulator/Devices/MINE/data/euxy.app/euxy' }));
    mkdirSync(bundle);
    writeFileSync(resolve(bundle, 'receipt.json'), '{}');
    at(resolve(bundle, 'receipt.json'), Date.now() - 60_000);
    report('euxy-old.ips');
    at(resolve(dir, 'euxy-old.ips'), Date.now() - 30_000);
    const scope = { bundleId: 'dev.brent.euxy', udid: 'MINE', dir };
    expect(crashWatch('euxy', bundle, scope)()[0]).toContain('euxy-old.ips'); // a crash before this launch, by hand
    const watch = crashWatch('euxy', bundle, scope);
    const late = spawn('sh', ['-c', `sleep 0.4; cp '${resolve(dir, 'euxy-old.ips')}' '${resolve(dir, 'euxy-new.ips')}'`]);
    const started = Date.now(), lines = watch(5000);
    expect(lines[0]).toContain('euxy-new.ips');
    expect(Date.now() - started).toBeLessThan(5000);
    expect(watch()).toEqual([]); // said once
    late.kill();
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
