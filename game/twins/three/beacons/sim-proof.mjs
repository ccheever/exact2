#!/usr/bin/env bun
// Closest available substitute when Chrome cannot start. This does NOT prove the browser.
import { initial, key, step } from './simulation.js';
import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
const here = import.meta.dir;
const results = [];
function check(name, pass, detail) { results.push({ name, pass, detail }); console.log(`${pass ? 'PASS' : 'FAIL'} ${name}`); }
const advance = (s, n) => { for (let i = 0; i < n; i++) step(s); };
function walk(s, x, z) {
  for (let n = 0; n < 220; n++) {
    const dx = x - s.player.x, dz = z - s.player.z;
    if (Math.hypot(dx, dz) < .45) break;
    const desired = [];
    if (Math.abs(dx) > .16) desired.push(dx > 0 ? 'KeyD' : 'KeyA');
    if (Math.abs(dz) > .16) desired.push(dz > 0 ? 'KeyS' : 'KeyW');
    for (const c of [...s.keys]) if (!desired.includes(c)) key(s, c, false);
    for (const c of desired) key(s, c, true);
    advance(s, 12);
  }
  for (const c of [...s.keys]) key(s, c, false);
  advance(s, 90);
}
function prefix() {
  const s = initial(); s.mode = 'playing'; key(s, 'KeyW', true); advance(s, 180); key(s, 'KeyW', false);
  const q = Math.exp(-10 / 120), expected = -4 / 120 * (180 - q * (1 - q ** 180) / (1 - q));
  check('W position after 180 ticks against closed form within 1 mm', Math.abs(s.player.z - expected) < .001 && s.player.x === 0 && s.player.y === 0, { expected, actual: s.player.z });
  walk(s, 8, 0); key(s, 'KeyE', true); advance(s, 12); key(s, 'KeyE', false);
  check('One lit beacon and glow 0.104 at 0.1 s', s.beacons.filter(b => b.lit).length === 1 && Math.abs(s.beacons[0].glow - .104) < 1e-12);
  s.paused = true; const frozen = JSON.stringify(s); advance(s, 240);
  check('Pause freezes all simulation state for 240 ticks', frozen === JSON.stringify(s)); s.paused = false;
  key(s, 'KeyD', true); key(s, 'Space', true); advance(s, 7);
  check('Save point contains airborne motion, held input and unfinished easing', s.player.y > 0 && s.beacons[0].glow < 1 && s.keys.includes('KeyD'));
  return s;
}
function continuation(s) {
  advance(s, 41); key(s, 'Space', false); key(s, 'Space', true);
  const vy = s.player.vy; advance(s, 1);
  check('Airborne Space does not reset vertical velocity', Math.abs(s.player.vy - (vy - .1)) < 1e-12);
  key(s, 'Space', false); advance(s, 59);
  check('Glow is exactly 1 at 1 second', s.beacons[0].glow === 1);
  key(s, 'KeyD', false); advance(s, 90);
  return s;
}
if (process.argv.includes('--resume')) {
  const end = continuation(JSON.parse(readFileSync(`${here}/sim-save.json`, 'utf8')));
  writeFileSync(`${here}/sim-restored.json`, JSON.stringify(end));
  process.exit(results.every(r => r.pass) ? 0 : 1);
}
const started = new Date().toISOString();
const a = prefix(), save = JSON.stringify(a); writeFileSync(`${here}/sim-save.json`, save);
const endA = JSON.stringify(continuation(a));
const b = prefix(); check('Repeated prefix byte-identical', JSON.stringify(b) === save);
check('Repeated continuation byte-identical', JSON.stringify(continuation(b)) === endA);
const child = spawnSync(process.execPath, [import.meta.filename, '--resume'], { encoding: 'utf8' });
check('Fresh Bun process restores disk save and continues identically', child.status === 0 && readFileSync(`${here}/sim-restored.json`, 'utf8') === endA, { childOutput: child.stdout, stderr: child.stderr });
check('Seed repeats six crate positions', initial(42).crates.length === 6 && JSON.stringify(initial(42).crates) === JSON.stringify(initial(42).crates));
check('Different seed changes crate layout', JSON.stringify(initial(42).crates) !== JSON.stringify(initial(43).crates));
walk(a, -6, 7); key(a, 'KeyE', true); advance(a, 120); key(a, 'KeyE', false);
walk(a, 3, -9); key(a, 'KeyE', true); advance(a, 120);
check('Walking and E can complete the game', a.mode === 'won' && a.beacons.every(b => b.glow === 1));
const jump = initial(); jump.mode = 'playing'; key(jump, 'Space', true); let peak = 0;
for (let n = 0; n < 120; n++) { step(jump); peak = Math.max(peak, jump.player.y); }
check('Hop peaks at 1.2 m and lands; held Space does not repeat', Math.abs(peak - 1.2) < .001 && jump.player.grounded, { peak });
key(jump, 'Space', false); key(jump, 'ArrowUp', true); advance(jump, 180);
check('ArrowUp uses the same movement as W', Math.abs(jump.player.z - results[0].detail.expected) < .001);
const diagonal = initial(); diagonal.mode = 'playing'; key(diagonal, 'KeyD', true); key(diagonal, 'KeyW', true); advance(diagonal, 240);
check('Diagonal top speed is capped at 4 m/s', Math.abs(Math.hypot(diagonal.player.vx, diagonal.player.vz) - 4) < 1e-7);
key(diagonal, 'KeyD', false); key(diagonal, 'KeyW', false); const v = diagonal.player.vx; step(diagonal);
check('Stopping is smooth, not instantaneous', diagonal.player.vx > 0 && diagonal.player.vx < v);
const result = { browserProof: false, substitute: 'Bun simulation only; no DOM, WebGL, browser input, screenshot or browser restore asserted', started, finished: new Date().toISOString(), pass: results.every(r => r.pass), results };
writeFileSync(`${here}/sim-proof-result.json`, JSON.stringify(result, null, 2) + '\n');
process.exitCode = result.pass ? 0 : 1;
