import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, utimesSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { sweep } from './sweep.mjs';

const DAY = 86_400_000;
const A = '0123456789abcdef', B = 'fedcba9876543210';

function target() {
  const t = mkdtempSync(resolve(tmpdir(), 'exact-sweep-')), d = resolve(t, 'debug');
  for (const [pkg, hash] of [['a', A], ['b', B]]) {
    mkdirSync(resolve(d, '.fingerprint', `${pkg}-${hash}`), { recursive: true });
    writeFileSync(resolve(d, '.fingerprint', `${pkg}-${hash}`, `lib-${pkg}`), hash);
    mkdirSync(resolve(d, 'deps'), { recursive: true });
    writeFileSync(resolve(d, 'deps', `lib${pkg}-${hash}.rlib`), '');
    writeFileSync(resolve(d, 'deps', `${pkg}-${hash}.cgu.0.rcgu.o`), '');
    mkdirSync(resolve(d, 'build', `${pkg}-${hash}`, 'out'), { recursive: true });
  }
  mkdirSync(resolve(d, 'incremental', 'a-2ajo1mk2vrgcf', 's-old'), { recursive: true });
  return t;
}
const has = (t, ...p) => existsSync(resolve(t, 'debug', ...p));

test('a unit no build has read for a week goes; one read since the last sweep stays', async () => {
  const t = target(), now = Date.now();
  try {
    assert.deepEqual(await sweep(t, now), {}); // first sight: everything is new
    readFileSync(resolve(t, 'debug/.fingerprint', `b-${B}`, 'lib-b')); // a build considers b
    const old = new Date(now - 8 * DAY);
    utimesSync(resolve(t, 'debug/incremental/a-2ajo1mk2vrgcf/s-old'), old, old);
    utimesSync(resolve(t, 'debug/incremental/a-2ajo1mk2vrgcf'), old, old);
    assert.deepEqual(await sweep(t, now + 8 * DAY), { debug: 2 });
    assert.ok(!has(t, '.fingerprint', `a-${A}`) && !has(t, 'deps', `liba-${A}.rlib`) && !has(t, 'deps', `a-${A}.cgu.0.rcgu.o`) && !has(t, 'build', `a-${A}`));
    assert.ok(!has(t, 'incremental', 'a-2ajo1mk2vrgcf'));
    assert.ok(has(t, '.fingerprint', `b-${B}`) && has(t, 'deps', `libb-${B}.rlib`) && has(t, 'build', `b-${B}`));
    assert.ok(!existsSync(resolve(t, '.exact-swept')));
  } finally { rmSync(t, { recursive: true, force: true }); }
});

test('a profile directory a build holds is left alone, and its units keep their standing', async () => {
  const t = target(), now = Date.now();
  await sweep(t, now);
  // Hold `debug/.cargo-lock` the way Cargo does (flock), from another process.
  const holder = spawn('perl', ['-MFcntl=:flock', '-e', 'open my $f, ">>", $ARGV[0] or die; flock($f, LOCK_EX) or die; print "held\\n"; $| = 1; sleep 30', resolve(t, 'debug/.cargo-lock')], { stdio: ['ignore', 'pipe', 'inherit'] });
  try {
    await new Promise(ok => holder.stdout.once('data', ok));
    assert.deepEqual(await sweep(t, now + 8 * DAY), {});
    assert.ok(has(t, '.fingerprint', `a-${A}`));
    const units = JSON.parse(readFileSync(resolve(t, '.exact-sweep.json'), 'utf8')).units;
    assert.equal(units[`debug#${A}`], now);
    holder.kill();
    await new Promise(ok => holder.once('exit', ok));
    assert.deepEqual(await sweep(t, now + 9 * DAY), { debug: 3 }); // a and b unread since, and a week past the cache
    assert.deepEqual(readdirSync(resolve(t, 'debug/.fingerprint')), []);
  } finally { holder.kill(); rmSync(t, { recursive: true, force: true }); }
});
