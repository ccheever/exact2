#!/usr/bin/env bun
// The fixture under the agent (LLP 1133.029 §9.1): each case's taps, then
// `state.orientationlock` checked, in one session. On an iOS simulator the
// device is turned between steps (`xcrun devicectl device orientation set`)
// while the session waits on the real clock, and the scene's orientation
// after the turn is the verdict (`applied.orientation`, read when the state
// is). Elsewhere the turns are skipped and the holders are checked.
//
//   bun apps/orientation-fixture/test/drive.mjs <linux|macos|ios|web> [--sim <udid>] [agent flags…]
//
// An iPad (`--ipad`) applies no type and holds `current` alone (D5).
import { spawn, spawnSync } from 'node:child_process';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(fileURLToPath(new URL('../../..', import.meta.url)));
const [host, ...rest] = process.argv.slice(2);
const ipad = rest.includes('--ipad');
const flags = rest.filter(a => a !== '--ipad');
const sim = flags.includes('--sim') ? flags[flags.indexOf('--sim') + 1] : process.env.EXACT_SIM;
const turns = host === 'ios' && sim;

const portrait = 'portrait-primary', landscape = ['landscape-primary', 'landscape-secondary'];
const isLandscape = o => landscape.includes(o);
// The scene's orientation is judged only where the drive turns the device.
const seen = (o, judged) => !turns || judged;
// The Linux host opens no `dialog` headless, so its steps are skipped there.
const dialog = ops => (host === 'linux' ? [] : ops);
// Each step: the taps, a turn of the device (iOS), and what `state.orientationlock` says after.
const steps = [
  { name: 'nothing held', ops: [], expect: s => !s.held && s.words.length === 0 },
  { name: 'the page locks portrait', ops: ['tap page-portrait'], expect: s => s.held && s.words.join() === 'portrait' },
  { name: 'a type holds against a turn', turn: 'landscapeLeft', expect: (s, o) => s.held && seen(o, ipad ? isLandscape(o) : o === portrait) },
  { name: 'released, the scene turns', ops: ['tap page-none'], expect: (s, o) => !s.held && seen(o, isLandscape(o)) },
  { name: 'current captures the turned scene', ops: ['tap page-current'], expect: s => s.held && s.words.join() === 'current' },
  { name: 'current holds against a turn back', turn: 'portrait', expect: (s, o) => s.held && seen(o, isLandscape(o)) },
  // On iOS 26 and later, lifting `prefersInterfaceOrientationLocked` leaves
  // the scene until the device next turns (the iOS 27 simulator); before,
  // lifting the mask turns it at once. Either is UIKit's own answer.
  { name: 'released again', ops: ['tap page-none'], expect: s => !s.held },
  { name: 'unlocked, the scene follows the next turn', turn: 'landscapeRight', expect: (s, o) => !s.held && seen(o, o === 'landscape-secondary') },
  { name: 'and the one after', turn: 'portrait', expect: (s, o) => !s.held && seen(o, o === portrait) },
  { name: 'a type turns the scene', ops: ['tap page-landscape'], expect: (s, o) => s.held && seen(o, ipad ? o === portrait : isLandscape(o)) },
  { name: 'a dialog over it leaves the lock', dialog: true, ops: ['tap open-dialog', 'clock settle'], expect: (s, o) => s.held && s.words.join() === 'landscape' && seen(o, ipad || isLandscape(o)) },
  { name: 'the dialog closed', ops: [...dialog(['tap close-dialog', 'clock settle']), 'tap page-portrait'], expect: s => s.held && s.words.join() === 'portrait' },
  { name: "a sheet's lock wins", ops: ['tap open-sheet', 'clock settle'], expect: (s, o) => s.words.join() === 'landscape' && seen(o, ipad || isLandscape(o)) },
  { name: 'a sheet with none leaves the page', ops: ['tap sheet-none'], expect: (s, o) => s.words.join() === 'portrait' && seen(o, ipad || o === portrait) },
  { name: 'a sheet over a locked sheet', ops: ['tap sheet-landscape', 'wait', 'tap open-over', 'clock settle'], expect: s => s.words.join() === 'landscape' },
  { name: 'the sheets closed', ops: ['tap close-over', 'clock settle', 'tap close-sheet', 'clock settle'], expect: s => s.words.join() === 'portrait' },
  { name: 'nesting: a pushed landscape screen over a portrait root', ops: ['tap page-none', 'tap root-portrait', 'tap open-landscape', 'clock settle'], expect: s => s.words.join() === 'landscape' },
  { name: 'popped: the root again', ops: ['tap landscape-back', 'clock settle'], expect: s => s.words.join() === 'portrait' },
  { name: '`any` frees the root', ops: ['tap page-any'], expect: s => s.words.join() === 'any' },
  { name: 'a conflict holds none', ops: ['tap page-portrait', 'tap side-landscape'], expect: s => !s.held && s.conflict?.length === 2 },
  { name: 'a computed word outside the table holds nothing', ops: ['tap side-none', 'tap page-sideways'], expect: s => s.words.join() === 'portrait' },
];

if (host === 'linux') steps.splice(steps.findIndex(s => s.dialog), 1);
const ops = [];
const marks = []; // the index of each step's `state` among the replies
let replies = 0;
for (const step of steps) {
  for (const op of step.ops ?? []) {
    // `wait`: a lock that turns the scene animates before the next tap can land.
    if (op === 'wait' && !turns) continue;
    ops.push(op === 'wait' ? 'clock +1500 real' : op); replies++;
  }
  if (step.turn && turns) { step.at = replies; ops.push('clock +4000 real'); replies++; }
  // A lock that turns the scene, or a release, animates first.
  else if (turns) { ops.push('clock +1500 real'); replies++; }
  ops.push('state'); marks.push(replies++);
}

const agent = spawn('bun', [resolve(ROOT, 'scripts/agent.mjs'), host, '--app', 'orientation-fixture', ...flags, '--json', ...ops], { cwd: ROOT, stdio: ['ignore', 'pipe', 'inherit'] });
let count = 0, buffer = '', failed = 0;
const states = [];
agent.stdout.on('data', chunk => {
  buffer += chunk;
  let nl;
  while ((nl = buffer.indexOf('\n')) >= 0) {
    const line = buffer.slice(0, nl); buffer = buffer.slice(nl + 1);
    let reply;
    try { reply = JSON.parse(line); } catch { continue; }
    // The turn starts as the session begins its real wait, and lands inside it.
    const step = steps.find(s => s.at === count + 1);
    if (step) spawnSync('xcrun', ['devicectl', 'device', 'orientation', 'set', '-d', sim, step.turn], { stdio: 'ignore' });
    if (marks.includes(count)) states.push(reply);
    count++;
  }
});
await new Promise(ok => agent.on('exit', ok));
for (const [i, step] of steps.entries()) {
  const s = states[i]?.orientationlock;
  const o = s?.applied?.orientation;
  const pass = s && step.expect(s, o);
  if (!pass) failed++;
  console.log(`${pass ? 'ok  ' : 'FAIL'} ${step.name}: held ${s?.held} words ${JSON.stringify(s?.words)} conflict ${JSON.stringify(s?.conflict)} applied ${JSON.stringify(s?.applied)}`);
}
console.log(failed ? `${failed} of ${steps.length} failed` : `all ${steps.length} passed`);
process.exit(failed ? 1 : 0);
