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
import { openTouches } from '../../host/apple/touches.mjs';

const USAGE = `usage: bun bench/bakeoff/drive.mjs --udid <simulator udid> --app <bundle id> [--timing] <op> [args] …
ops: launch · relaunch · terminate · home · tree · tap x,y · tap "label" · type "text"
     drag x1,y1 x2,y2 [velocity] · swipe up|down|left|right · wait ms · screenshot file.png
     appearance light|dark · textsize default|largest|<content size category>`;

const SIZES = ['extra-small', 'small', 'medium', 'large', 'extra-large', 'extra-extra-large', 'extra-extra-extra-large',
  'accessibility-medium', 'accessibility-large', 'accessibility-extra-large', 'accessibility-extra-extra-large', 'accessibility-extra-extra-extra-large'];
const SIZE_ALIASES = { default: 'large', largest: 'accessibility-extra-extra-extra-large' };
// Elements that only group others: left out of the outline when they carry no text.
const SILENT = new Set(['Other', 'Group', 'Window', 'LayoutArea', 'LayoutItem']);
// What a person taps: of a match nested in another, the outer one when it is one of these.
const TAPPABLE = new Set(['Button', 'Cell', 'Link', 'Switch', 'Toggle', 'TextField', 'SecureTextField', 'SearchField', 'TextView',
  'Slider', 'Stepper', 'SegmentedControl', 'Tab', 'MenuItem', 'PopUpButton', 'MenuButton', 'ToolbarButton', 'CheckBox', 'RadioButton']);
const POINT = /^(-?\d+(?:\.\d+)?),(-?\d+(?:\.\d+)?)$/;

class Refusal extends Error {}
const refuse = (message) => { throw new Refusal(message); };

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
  // An app launched while no session was open (not by `launch`) can refuse the first reads (kAXErrorAPIDisabled).
  // Identifiers a toolkit generates (Exact's `exact-…`, anything carrying a UUID) are dropped: they would tell a
  // blind grader how an app was built, and no person sees them.
  const generated = /^exact-|[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-/i;
  const tree = async () => {
    for (const until = Date.now() + 8000; ;) {
      try {
        const t = await ask({ op: 'tree' });
        for (const e of t.elements) if (e.identifier != null && generated.test(e.identifier)) e.identifier = null;
        return t;
      }
      catch (e) { if (!(e instanceof Refusal && /APIDisabled/.test(e.message)) || Date.now() > until) throw e; }
      await sleep(250);
    }
  };
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
      const t = await tree();
      const { e, i } = find(t.elements, target), f = aim(t.elements, i);
      const [fx, fy, fw, fh] = t.frame;
      const [x0, y0] = [Math.max(f[0], fx), Math.max(f[1], fy)];
      const [x1, y1] = [Math.min(f[0] + f[2], fx + fw), Math.min(f[1] + f[3], fy + fh)];
      if (x1 <= x0 || y1 <= y0) refuse(`${line(e)} is off screen; swipe to bring it into view`);
      const [x, y] = [Math.round((x0 + x1) / 2), Math.round((y0 + y1) / 2)];
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

/** One element as the outline shows it. */
function line(e) {
  const [x, y, w, h] = e.frame.map(Math.round);
  const text = (s) => JSON.stringify(s.length > 80 ? s.slice(0, 80) + '…' : s);
  return [e.type, e.label != null ? text(e.label) : null, e.identifier != null ? `id=${text(e.identifier)}` : null,
    e.value != null ? `value=${text(e.value)}` : null, e.placeholder != null ? `placeholder=${text(e.placeholder)}` : null,
    e.enabled ? null : 'disabled', e.selected ? 'selected' : null, e.hasFocus ? 'focused' : null, `[${x},${y} ${w}x${h}]`].filter(Boolean).join(' ');
}

const silent = (e) => SILENT.has(e.type) && e.label == null && e.identifier == null && e.value == null && e.placeholder == null;

/** The tree as an indented outline: wrappers with no text are left out, a keyboard is one line. */
function outline({ elements, truncated }) {
  const out = [], stack = [], seen = new Set();
  let keyboard = null;
  for (const e of elements) {
    if (keyboard != null && e.depth > keyboard) continue;
    keyboard = null;
    while (stack.length && stack.at(-1).depth >= e.depth) stack.pop();
    const shown = !silent(e);
    const text = '  '.repeat(stack.filter((s) => s.shown).length) + line(e);
    // An element some apps expose twice (a scroll bar, a keyboard's button) is shown once.
    if (shown && !seen.has(text)) out.push(text), seen.add(text);
    stack.push({ depth: e.depth, shown });
    if (e.type === 'Keyboard') keyboard = e.depth;
  }
  if (truncated) out.push(`… (cut at ${elements.length} elements)`);
  return out.join('\n');
}

/** The element `target` names, with its index: by label, identifier, placeholder or title, exactly, then ignoring
 * case, then as part of a label. Of two nested matches the outer is kept when a person would tap it (a button
 * around its text), else the inner; keyboard keys only when nothing else matches. */
function find(elements, target) {
  const lower = target.toLowerCase();
  const tests = [
    (e) => [e.label, e.identifier, e.placeholder, e.title].includes(target),
    (e) => [e.label, e.identifier, e.placeholder, e.title].some((s) => s?.toLowerCase() === lower),
    (e) => e.label?.toLowerCase().includes(lower),
  ];
  for (const test of tests) {
    let found = elements.map((e, i) => ({ e, i })).filter(({ e }) => test(e));
    if (!found.length) continue;
    if (found.some(({ e }) => e.type !== 'Key')) found = found.filter(({ e }) => e.type !== 'Key');
    const losers = new Set();
    for (const a of found) for (const d of found) {
      if (d.i > a.i && within(elements, a.i, d.i)) losers.add(TAPPABLE.has(a.e.type) ? d.i : a.i);
    }
    const kept = found.filter(({ i }) => !losers.has(i));
    if (kept.length === 1) return kept[0];
    refuse(`${kept.length} elements match ${JSON.stringify(target)}; tap one by its point:\n${kept.slice(0, 12).map(({ e }) => '  ' + line(e)).join('\n')}`);
  }
  refuse(`nothing labelled ${JSON.stringify(target)}; run tree to see the labels, or tap x,y`);
}

/** Whether element `d` is inside element `a` (indices into the flattened tree). */
const within = (elements, a, d) => elements.slice(a + 1, d + 1).every((x) => x.depth > elements[a].depth);

/** Where a person taps element `i`: its middle, or for a switch, the middle of the switch control inside it (a
 * SwiftUI toggle's row is labelled as a switch, but only the control turns it). */
function aim(elements, i) {
  let e = elements[i];
  if (e.type === 'Switch' || e.type === 'Toggle') {
    for (let j = i + 1; j < elements.length && elements[j].depth > e.depth; j++) {
      if (elements[j].type === e.type) { e = elements[j]; break; }
    }
  }
  return e.frame;
}

main().catch((e) => {
  console.error(`error: ${e instanceof Refusal ? e.message : e.stack}`);
  process.exit(1);
});
