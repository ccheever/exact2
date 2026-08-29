#!/usr/bin/env node
// Launch the built macOS app in smoke mode: it prints its boot time and a
// summary of the first frames, then exits. Asserts the app's landmarks. Not
// a blocking check (it needs a window server and Xcode's toolchain); run by
// hand after `node host/apple/build.mjs`.
import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';

const root = resolve(new URL('../..', import.meta.url).pathname);
const bin = resolve(root, 'host/apple/macos/.build/release/ExactMac');
if (!existsSync(bin)) { console.error('run node host/apple/build.mjs first'); process.exit(2); }
const r = spawnSync(bin, [], { encoding: 'utf8', env: { ...process.env, EXACT_SMOKE: '1' }, timeout: 20000 });
const fixture = Boolean(process.env.EXACT_PLAN); // a plan file, not the app: landmarks differ
const out = (r.stdout ?? '') + (r.stderr ?? '');
process.stdout.write(out);
const failures = [];
if (!/^boot [\d.]+ ms/m.test(out)) failures.push('no boot line');
if (!/error none/.test(out)) failures.push('the first batch reported an error');
if (!fixture) {
  for (const id of ['caltrain-main', 'station-name', 'board-north', 'board-south', 'change-station']) if (!out.includes(id)) failures.push(`missing testId ${id}`);
  if (!/station Mountain View/.test(out)) failures.push('station name not presented');
}
// Scrolling (LLP 1008 §5): a wheel over a scroll node reaches the page when
// the node cannot use it; an overflowing node scrolls itself first, then chains.
const sc = /^scroll: .*inner document (\d+) in (\d+);.*via hit view \(([\w.]+)\): page (\d+)→(\d+), inner (\d+)→(\d+);.*after 200 more: inner (\d+)→(\d+) \(limit (\d+)\), page (\d+)→(\d+) \(limit (\d+)\).*viewport (\d+) wide/m.exec(out);
if (!sc) failures.push('no scroll report');
else {
  const [docH, boxH, , p2, p3, i2, i3, i0, i1, iMax, p4, p5, pMax, viewportW] = sc.slice(1).map((x) => (isNaN(Number(x)) ? x : Number(x)));
  // The root fills the viewport's width (the kernel's block rule for a root).
  const rootW = Number(/root (\d+)x\d+/.exec(out)?.[1]);
  if (rootW !== viewportW) failures.push(`the root is ${rootW} wide in a ${viewportW} viewport`);
  if (docH > boxH && i1 !== iMax) failures.push(`the scroll node stopped at ${i1}, not its limit ${iMax}`);
  if (!(docH > boxH) && p5 !== pMax) failures.push(`the page stopped at ${p5}, not its limit ${pMax}`);
  // One event moves exactly one scroll view: the node when it can use it, else the page.
  if (docH > boxH) {
    if (!(i3 > i2)) failures.push('an overflowing scroll node did not scroll itself for a wheel over it');
    if (p3 !== p2) failures.push('the page moved for a wheel the scroll node consumed');
    if (!(i1 > i0)) failures.push('an overflowing scroll node did not keep scrolling');
    if (!(p5 > p4)) failures.push('a scroll node at its edge did not chain to the page');
    if (/page moved before the inner limit: yes/.test(out)) failures.push('the page moved while the scroll node could still scroll');
  } else {
    if (!(p3 > p2)) failures.push('a wheel over a scroll node with nothing to scroll did not reach the page');
    if (i3 !== i2) failures.push('a scroll node with nothing to scroll moved');
  }
}
if (!/smoke ok/.test(out)) failures.push(`the app did not finish (exit ${r.status}, signal ${r.signal})`);
if (/gpu: not loaded: (?!no canvas)/.test(out)) failures.push('a canvas is on screen but the GPU module did not load');
if (/gpu: module loaded/.test(out) && !/[1-9]\d* renders/.test(out)) failures.push('the GPU module loaded but rendered nothing');
if (failures.length) { for (const f of failures) console.error('  ' + f); process.exit(1); }
console.log('macos smoke: ok');
