#!/usr/bin/env bun
// Drives any iOS simulator app by bundle id, the same way whatever it was built
// with: real HID touches and the app's accessibility tree through the touch
// runner (host/apple/touches.mjs), and the simulator's own controls through
// simctl. Ops run in order; the first failure stops the run with exit 1.
// Usage: drive.md beside this file.
//   bun bench/bakeoff/drive.mjs --udid U --app BUNDLE [--timing] <op> [args…] …
import { spawnSync } from 'node:child_process';
import { mkdirSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { Refusal, line, outline, readTree, refuse, tapPoint } from '../../host/apple/elements.mjs';
import { openTouches } from '../../host/apple/touches.mjs';

const USAGE = `usage: bun bench/bakeoff/drive.mjs --udid <simulator udid> --app <bundle id> [--timing] <op> [args] …
ops: launch · relaunch · terminate · home · tree · tap x,y · tap "label" · type "text"
     drag x1,y1 x2,y2 [velocity] · swipe up|down|left|right · wait ms · screenshot file.png
     appearance light|dark · textsize default|largest|<content size category>`;

const SIZES = ['extra-small', 'small', 'medium', 'large', 'extra-large', 'extra-extra-large', 'extra-extra-extra-large',
  'accessibility-medium', 'accessibility-large', 'accessibility-extra-large', 'accessibility-extra-extra-large', 'accessibility-extra-extra-extra-large'];
const SIZE_ALIASES = { default: 'large', largest: 'accessibility-extra-extra-extra-large' };
const POINT = /^(-?\d+(?:\.\d+)?),(-?\d+(?:\.\d+)?)$/;

/** The ops as [name, args], each checked before anything runs. */
function parse(words) {
  const ops = [];
  const arity = { launch: 0, relaunch: 0, terminate: 0, home: 0, tree: 0, tap: 1, type: 1, swipe: 1, wait: 1, screenshot: 1, appearance: 1, textsize: 1 };
  for (let i = 0; i < words.length;) {
    const name = words[i++];
    if (name === 'drag') {
      const args = words.slice(i, i + 2);
      if (args.length < 2 || !args.every((a) => POINT.test(a))) refuse(`drag needs two points, as drag 200,600 200,200 [velocity]`);
      i += 2;
      if (/^\d+(\.\d+)?$/.test(words[i] ?? '')) args.push(words[i++]);
      ops.push([name, args]);
      continue;
    }
    if (!(name in arity)) refuse(`unknown op "${name}"\n${USAGE}`);
    const args = words.slice(i, i + arity[name]);
    if (args.length < arity[name]) refuse(`${name} needs an argument\n${USAGE}`);
    i += arity[name];
    if (name === 'swipe' && !['up', 'down', 'left', 'right'].includes(args[0])) refuse('swipe takes up, down, left or right (the way the finger moves)');
    if (name === 'wait' && !/^\d+$/.test(args[0])) refuse('wait takes milliseconds, as wait 500');
    if (name === 'appearance' && !['light', 'dark'].includes(args[0])) refuse('appearance takes light or dark');
    if (name === 'textsize' && !SIZES.includes(SIZE_ALIASES[args[0]] ?? args[0])) refuse(`textsize takes default, largest or one of: ${SIZES.join(', ')}`);
    ops.push([name, args]);
  }
  return ops;
}

function simctl(args, what) {
  const r = spawnSync('xcrun', ['simctl', ...args], { encoding: 'utf8' });
  if (r.status !== 0) refuse(`${what}: ${(r.stderr || r.stdout || String(r.error)).trim()}`);
  return r.stdout.trim();
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function main() {
  const argv = process.argv.slice(2);
  const flag = (name) => { const i = argv.indexOf(name); if (i < 0) return null; const v = argv[i + 1]; argv.splice(i, 2); return v; };
  const timing = argv.includes('--timing') && argv.splice(argv.indexOf('--timing'), 1);
  const udid = flag('--udid'), app = flag('--app');
  if (!udid || !app || !argv.length) refuse(USAGE);
  const ops = parse(argv);

  const devices = JSON.parse(simctl(['list', 'devices', '-j'], 'listing simulators')).devices;
  const device = Object.values(devices).flat().find((d) => d.udid === udid);
  if (!device) refuse(`no simulator with udid ${udid}; xcrun simctl list devices names them`);
  if (device.state !== 'Booted') refuse(`simulator ${device.name} is ${device.state}; boot it with: xcrun simctl boot ${udid}`);

  // The touch runner's session, opened at the first op that needs it. While one is open the simulator's
  // accessibility keys are on (touches.mjs AX_KEYS), so an app that `launch` starts loads UIKit's accessibility runtime
  // at launch, which slows its start by up to a second or so. An app launched with them off turns accessibility on
  // only when it is told to, which a kept runner does not reliably do: its reads fail (kAXErrorAPIDisabled).
  let touches = null;
  const session = async () => {
    if (!touches) {
      try { touches = await openTouches({ udid, appId: app }); }
      catch (e) { refuse(`could not start the touch runner: ${e.message}`); }
    }
    return touches;
  };
  const closeSession = async () => {
    const t = touches;
    touches = null;
    await t?.close();
  };
  const ask = async (req) => {
    const r = await (await session()).ask(req);
    if (r.error) refuse(r.error + (/not in the foreground/.test(r.error) ? '; launch it first' : ''));
    return r;
  };
  let screen;
  /** The display in portrait points, and whether a point in the app is a point on it (one display, not turned:
   * XCTest reports `unknown` until something sets the orientation). Then a gesture goes through XCTest's event
   * synthesizer (about 0.3 s a tap) rather than XCUICoordinate, which waits for the app to go idle (0.6 s or more). */
  const display = async () => {
    screen ??= await ask({ op: 'screen' });
    const [w, h] = screen.displays[0] ?? [0, 0];
    return { w, h, direct: screen.displays.length === 1 && ['portrait', 'unknown'].includes(screen.orientation) };
  };
  const tap = async (x, y) => {
    const d = await display();
    await ask({ op: 'tap', point: [x, y], ...(d.direct ? { screen: [x, y], display: [d.w, d.h] } : {}) });
  };
  const drag = async ([x1, y1], [x2, y2], velocity) => {
    const d = await display();
    await ask({ op: 'drag', point: [x1, y1], to: [x2, y2], press: 0.05, hold: 0, velocity,
      ...(d.direct ? { screen: [x1, y1], screenTo: [x2, y2], display: [d.w, d.h] } : {}) });
  };
  const tree = async () => readTree(async (req) => (await session()).ask(req));
  const terminate = () => spawnSync('xcrun', ['simctl', 'terminate', udid, app], { encoding: 'utf8' });
  const launch = async () => {
    // From the Home Screen: launched over another app, the status bar would name it ("◀ App").
    await ask({ op: 'springboard', action: 'home' });
    terminate();
    await sleep(500);
    simctl(['launch', udid, app], `launching ${app} (is it installed? xcrun simctl install ${udid} <path.app>)`);
    for (const until = Date.now() + 10000; ;) {
      const { state } = await ask({ op: 'foreground' });
      if (state === 'runningForeground') break;
      if (Date.now() > until) refuse(`${app} launched but is ${state} after 10 s`);
      await sleep(100);
    }
    // Done once two reads agree: the first can come before the app has drawn.
    let last = -1;
    for (const until = Date.now() + 8000; Date.now() < until;) {
      const n = (await tree()).elements.length;
      if (n > 1 && n === last) break;
      last = n;
      await sleep(200);
    }
    return `launched ${app}`;
  };

  const run = {
    launch,
    relaunch: launch,
    terminate: () => { const r = terminate(); return r.status === 0 ? `terminated ${app}` : `${app} was not running`; },
    home: async () => { await ask({ op: 'springboard', action: 'home' }); return 'pressed Home'; },
    tree: async () => outline(await tree()),
    tap: async ([target]) => {
      const m = POINT.exec(target);
      if (m) { await tap(Number(m[1]), Number(m[2])); return `tapped ${m[1]},${m[2]}`; }
      const { x, y, e } = tapPoint(await tree(), target);
      await tap(x, y);
      return `tapped ${line(e)} at ${x},${y}`;
    },
    // `\n` in the argument is Return, as a shell passes it unexpanded.
    type: async ([text]) => { await ask({ op: 'type', text: text.replace(/\\n/g, '\n') }); return `typed ${JSON.stringify(text)}`; },
    drag: async ([from, to, velocity]) => {
      const p = from.split(',').map(Number), q = to.split(',').map(Number);
      await drag(p, q, Number(velocity ?? 1000));
      return `dragged ${from} to ${to}`;
    },
    swipe: async ([direction]) => {
      const { w, h, direct } = await display();
      // Without a portrait screen the app's own size bounds the swipe.
      const [W, H] = direct ? [w, h] : (await tree()).frame.slice(2);
      const [cx, cy] = [W / 2, H / 2], dx = W * 0.3, dy = H * 0.25;
      const ends = { up: [[cx, cy + dy], [cx, cy - dy]], down: [[cx, cy - dy], [cx, cy + dy]], left: [[cx + dx, cy], [cx - dx, cy]], right: [[cx - dx, cy], [cx + dx, cy]] }[direction];
      await drag(...ends.map((p) => p.map(Math.round)), 1500);
      return `swiped ${direction}`;
    },
    wait: async ([ms]) => { await sleep(Number(ms)); return `waited ${ms} ms`; },
    screenshot: ([path]) => {
      const out = resolve(path);
      mkdirSync(dirname(out), { recursive: true });
      simctl(['io', udid, 'screenshot', out], 'screenshot');
      return `saved ${out}`;
    },
    appearance: ([mode]) => { simctl(['ui', udid, 'appearance', mode], 'appearance'); return `appearance ${mode}`; },
    textsize: ([size]) => { const s = SIZE_ALIASES[size] ?? size; simctl(['ui', udid, 'content_size', s], 'textsize'); return `text size ${s}`; },
  };

  let failed = null;
  try {
    for (const [name, args] of ops) {
      const started = performance.now();
      try { console.log(await run[name](args) + (timing ? ` (${Math.round(performance.now() - started)} ms)` : '')); }
      catch (e) { failed = `${name}${args.length ? ' ' + args.map((a) => JSON.stringify(a)).join(' ') : ''}: ${e.message}`; break; }
    }
  } finally {
    await closeSession().catch((e) => console.error(`note: closing the touch runner: ${e.message}`));
  }
  if (failed) { console.error(`error: ${failed}`); process.exitCode = 1; }
}

main().catch((e) => {
  console.error(`error: ${e instanceof Refusal ? e.message : e.stack}`);
  process.exit(1);
});
