import { test, expect } from 'bun:test';
import { spawn } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { audioProof } from '../../bench/probes/audio.mjs';

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
  // Execute the actual exclusion condition used by the proof, not a copied regex.
  const source = readFileSync(resolve(import.meta.dir, '../../proof.mjs'), 'utf8');
  const condition = source.match(/if \((\(?\/\^\(game[\s\S]*?)\) continue;/)[1];
  const excluded = new Function('file', 'appPrefix', 'existsSync', 'resolve', 'root', `return (${condition});`);
  const root = resolve(import.meta.dir, '../../..');
  expect(excluded('game/bench/probes/audio.mjs', 'game/games/greybox/', existsSync, resolve, root)).toBe(true);
  expect(excluded('game/games/greybox/logic/src/lib.rs', 'game/games/greybox/', existsSync, resolve, root)).toBe(false);
  expect(excluded('game/bench/cubes/logic/src/lib.rs', 'game/bench/cubes/', existsSync, resolve, root)).toBe(false);
  expect(excluded('game/bench/probes/audio.mjs', 'game/bench/cubes/', existsSync, resolve, root)).toBe(true);
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
    document.hidden = false; listeners.pageshow();
    expect(calls).toEqual([[11,0],[22,0],[11,0],[22,0],[11,1],[22,1]]);
    expect(canceled).toEqual(seekable ? [] : [9]);
    expect(scheduled).toBe(1);
  }
});
