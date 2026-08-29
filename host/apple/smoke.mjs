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
const out = (r.stdout ?? '') + (r.stderr ?? '');
process.stdout.write(out);
const failures = [];
if (!/^boot [\d.]+ ms/m.test(out)) failures.push('no boot line');
if (!/error none/.test(out)) failures.push('the first batch reported an error');
for (const id of ['caltrain-main', 'station-name', 'board-north', 'board-south', 'change-station']) if (!out.includes(id)) failures.push(`missing testId ${id}`);
if (!/station Mountain View/.test(out)) failures.push('station name not presented');
if (!/smoke ok/.test(out)) failures.push(`the app did not finish (exit ${r.status}, signal ${r.signal})`);
if (/gpu: not loaded: (?!no canvas)/.test(out)) failures.push('a canvas is on screen but the GPU module did not load');
if (/gpu: module loaded/.test(out) && !/[1-9]\d* renders/.test(out)) failures.push('the GPU module loaded but rendered nothing');
if (failures.length) { for (const f of failures) console.error('  ' + f); process.exit(1); }
console.log('macos smoke: ok');
