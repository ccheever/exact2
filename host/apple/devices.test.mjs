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

// `displays`: one picture every read gets, or one per read in turn; `reads(dir)` counts the reads.
async function withFakeSimctl(displays, body) {
  const dir = mkdtempSync(resolve(tmpdir(), 'devices-test-'));
  const env = { PATH: process.env.PATH, DEVELOPER_DIR: process.env.DEVELOPER_DIR };
  try {
    [displays].flat().forEach((display, i) => writeFileSync(resolve(dir, `display-${i}.png`), display));
    writeFileSync(resolve(dir, 'xcrun'), `#!/bin/sh\nfor last; do :; done\nn=$(cat '${dir}/n' 2>/dev/null || echo 0)\nf='${dir}/display-'$n'.png'\n[ -f "$f" ] || f='${dir}/display-0.png'\ncp "$f" "$last"\necho $((n + 1)) > '${dir}/n'\n`);
    chmodSync(resolve(dir, 'xcrun'), 0o755);
    process.env.PATH = `${dir}:${env.PATH}`;
    process.env.DEVELOPER_DIR ??= '/Applications/Xcode.app/Contents/Developer';
    return await body(dir);
  } finally {
    process.env.PATH = env.PATH;
    if (env.DEVELOPER_DIR === undefined) delete process.env.DEVELOPER_DIR;
    rmSync(dir, { recursive: true, force: true });
  }
}
const reads = (dir) => { try { return Number(readFileSync(resolve(dir, 'n'), 'utf8')); } catch { return 0; } };

// 8x16 pixels at scale 2: a 4x8-point window. The app drew it red; the display shows the status bar (rows 0-1) green,
// glass over rows 12-15 blue, and the keyboard, when there is one, from row 8 down. `paint` changes rows of a picture.
const RED = [255, 0, 0, 255], BLUE = [0, 0, 255, 255], GREEN = [0, 255, 0, 255], GREY = [128, 128, 128, 255];
const paint = (bands) => encodePng({ width: 8, height: 16, data: Uint8Array.from({ length: 8 * 16 * 4 }, (_, i) => {
  const y = Math.floor(i / 32), band = bands.find(([from, to]) => y >= from && y < to);
  return (band ? band[2] : RED)[i % 4];
}) });
const APP = paint([]), DISPLAY = paint([[0, 2, GREEN], [12, 16, BLUE]]);
const rows = (path) => { const png = decodePng(readFileSync(path)); return (y) => [...png.data.subarray(y * png.width * 4, y * png.width * 4 + 4)]; };
// The app's answer: in front at epoch 3, a plain point at (2, 2) points (pixel 4, 4); the probe after the reads says
// the same unless told otherwise. `fast` skips the gap before the third read.
const front = { front: true, epoch: 3, incarnation: 1, clock: 1500, scale: 2, opaque: [[2, 2]] };
const still = (more = {}) => ({ gap: 0, probe: async () => ({ ...front, ...more }) });
const keyboard = { visible: true, top: 4, windowTop: 4 };
// A capture the agent marked (header nodes struck through), which only the app's own picture shows.
const marked = { unshown: [{ id: 1, testId: null }] };
const shoot = async (dir, r, appTop, options) => {
  const shot = resolve(dir, 'app.png');
  writeFileSync(shot, APP);
  return [await withDisplay(r, shot, 'UDID', appTop, options), rows(shot)];
};

// drawHierarchy draws Liquid Glass roughly: a tab bar's glass left the rows under it legible (the euxy port).
test('a window capture is the display\'s, the status bar too when asked', async () => {
  await withFakeSimctl(DISPLAY, async (dir) => {
    const [r, at] = await shoot(dir, { ...front, statusBar: 1 }, 0, still());
    expect([at(0), at(4), at(13)]).toEqual([GREEN, RED, BLUE]);
    expect([r.statusBar, r.pixels, r.note]).toEqual([true, 'display', undefined]);
    expect(reads(dir)).toBe(3);
  });
});

test('a window capture without its status bar keeps the app\'s rows there', async () => {
  await withFakeSimctl(DISPLAY, async (dir) => {
    const [r, at] = await shoot(dir, { ...front }, 1, still());
    expect([at(0), at(1), at(13)]).toEqual([RED, RED, BLUE]);
    expect([r.statusBar, r.pixels]).toEqual([undefined, 'display']);
  });
});

test('the keyboard is in a window capture, its top known or not', async () => {
  await withFakeSimctl(paint([[0, 2, GREEN], [8, 16, GREY]]), async (dir) => {
    const [r, at] = await shoot(dir, { ...front, keyboard: { visible: true, top: 4 }, note: 'old' }, 0, still());
    expect(at(15)).toEqual(GREY);
    expect(r.keyboard.captured).toBe(true);
    expect(r.note).toContain('in the capture');
  });
});

// Another app, a system alert or the home screen in front: none of the display is read, none of it taken.
test('an app not in front takes nothing of the display, and says why', async () => {
  await withFakeSimctl(DISPLAY, async (dir) => {
    const [r, at] = await shoot(dir, { ...front, front: 'the app is not active', statusBar: 1, keyboard: { ...keyboard } }, 0, still());
    expect([at(0), at(15)]).toEqual([RED, RED]);
    expect([reads(dir), r.pixels, r.pixelsWhy, r.statusBar, r.keyboard.captured]).toEqual([0, 'app', 'the app is not active', false, undefined]);
  });
});

test('an app that left the front while the display was read takes none of it', async () => {
  await withFakeSimctl(DISPLAY, async (dir) => {
    const [r, at] = await shoot(dir, { ...front, statusBar: 1, keyboard: { ...keyboard } }, 0, still({ front: 'the app is not active' }));
    expect([at(0), at(13)]).toEqual([RED, RED]);
    expect([r.pixelsWhy, r.statusBar, r.keyboard.captured]).toEqual(['the app is not active', false, undefined]);
  });
});

test('an app that changed while the display was read takes none of it', async () => {
  await withFakeSimctl(DISPLAY, async (dir) => {
    const [r, at] = await shoot(dir, { ...front }, 1, still({ epoch: 4 }));
    expect(at(13)).toEqual(RED);
    expect(r.pixelsWhy).toBe('the app changed while the display was read');
  });
});

// A keyboard sliding, a caret blinking: the display is of no one moment. Three reads, so a blink cannot fall in all.
test('a display that moves by the third read leaves the app\'s picture, and a keyboard band all three agree on', async () => {
  const kb = paint([[0, 2, GREEN], [6, 7, GREY], [8, 16, BLUE]]);
  await withFakeSimctl([kb, kb, paint([[0, 2, GREEN], [6, 7, GREEN], [8, 16, BLUE]])], async (dir) => {
    const [r, at] = await shoot(dir, { ...front, statusBar: 1, keyboard: { ...keyboard } }, 0, still());
    expect([at(0), at(6), at(13)]).toEqual([GREEN, RED, BLUE]);
    expect([r.pixels, r.pixelsWhy, r.keyboard.captured]).toEqual(['app', 'the display changed while it was read', true]);
    expect(reads(dir)).toBe(3);
  });
});

test('a keyboard band that moves between the reads is not taken', async () => {
  await withFakeSimctl([DISPLAY, DISPLAY, paint([[0, 2, GREEN], [12, 16, GREY]])], async (dir) => {
    const [r, at] = await shoot(dir, { ...front, keyboard: { ...keyboard } }, 1, still());
    expect(at(13)).toEqual(RED);
    expect(r.keyboard.captured).toBeUndefined();
    expect(r.note).toContain('not in the capture');
  });
});

// A notification banner leaves the app active and its window key; the app's own plain points show it.
test('a display that differs from the app where the app drew itself leaves the app\'s picture', async () => {
  await withFakeSimctl(paint([[0, 2, GREEN], [2, 7, GREY], [12, 16, BLUE]]), async (dir) => {
    const [r, at] = await shoot(dir, { ...front, opaque: [[2, 2], [2, 2.5], [2, 3]] }, 1, still());
    expect([at(4), at(13)]).toEqual([RED, RED]);
    expect([r.pixels, r.pixelsWhy]).toEqual(['app', 'the display differs from the app\'s drawing at 3 of 3 plain points (a notification banner or another overlay)']);
  });
});

// A banner's frosted material over a light page was 3 or 4 levels off it; where nothing lies over the app the display
// matches its drawing exactly.
test('a banner a few levels off the page is seen, and a 2-level difference is not', async () => {
  const opaque = [[2, 2], [2, 2.5], [2, 3]];
  await withFakeSimctl(paint([[0, 2, GREEN], [2, 7, [252, 0, 0, 255]], [12, 16, BLUE]]), async (dir) => {
    const [r] = await shoot(dir, { ...front, opaque }, 1, still());
    expect(r.pixels).toBe('app');
  });
  await withFakeSimctl(paint([[0, 2, GREEN], [2, 7, [253, 0, 0, 255]], [12, 16, BLUE]]), async (dir) => {
    const [r] = await shoot(dir, { ...front, opaque }, 1, still());
    expect(r.pixels).toBe('display');
  });
});

test('an app that names no points to check leaves its own picture', async () => {
  await withFakeSimctl(DISPLAY, async (dir) => {
    const [r, at] = await shoot(dir, { ...front, opaque: [] }, 1, still());
    expect(at(13)).toEqual(RED);
    expect(r.pixelsWhy).toBe('the app named no points to check the display against');
  });
});

test('UIKit motion still running leaves the app\'s picture', async () => {
  await withFakeSimctl(DISPLAY, async (dir) => {
    const [r, at] = await shoot(dir, { ...front, moving: true }, 1, still());
    expect([at(13), reads(dir)]).toEqual([RED, 0]);
    expect(r.pixelsWhy).toBe('UIKit\'s motion had not settled');
  });
});

test('a capture asked for as the app\'s own drawing stays it, silently, reading nothing', async () => {
  await withFakeSimctl(DISPLAY, async (dir) => {
    const [r, at] = await shoot(dir, { ...front }, 1, { ...still(), own: true });
    expect([at(13), reads(dir)]).toEqual([RED, 0]);
    expect([r.pixels, r.note]).toEqual(['app', undefined]);
  });
});

test('a marked capture takes the display\'s rows from the keyboard\'s top down, the app above them its own', async () => {
  const kb = paint([[0, 2, GREEN], [8, 16, GREY]]);
  await withFakeSimctl(kb, async (dir) => {
    const [r, at] = await shoot(dir, { ...front, ...marked, keyboard: { ...keyboard }, note: 'old' }, 0, still());
    expect([at(0), at(7), at(8), at(15)]).toEqual([RED, RED, GREY, GREY]);
    expect(r.keyboard.captured).toBe(true);
    expect(r.note).toContain('from keyboard.top down');
  });
});

test('a marked capture takes only the status bar\'s rows from the display', async () => {
  await withFakeSimctl(DISPLAY, async (dir) => {
    const [r, at] = await shoot(dir, { ...front, ...marked, statusBar: 1 }, 0, still());
    expect([at(0), at(1), at(2), at(13)]).toEqual([GREEN, GREEN, RED, RED]);
    expect([r.statusBar, r.pixelsWhy]).toEqual([true, 'header nodes are struck through in it']);
  });
});

test('a display of another size leaves the picture as it was, and says why', async () => {
  await withFakeSimctl(solid(6, 16, BLUE), async (dir) => {
    const [r] = await shoot(dir, { ...front, keyboard: { ...keyboard } }, 0, still());
    expect(readFileSync(resolve(dir, 'app.png')).equals(Buffer.from(APP))).toBe(true);
    expect(r.keyboard.captured).toBeUndefined();
    expect(r.note).toContain('the display is 6x16, the capture 8x16');
  });
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
