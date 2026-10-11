// LLP 1116.004 D7: `bun exact.mjs verify` is one call a builder can stop on: a clean run in a few lines, a
// failing test that fails it, and a check finding said once, reported but not failing it.
// `bun test ./scripts/verify.test.mjs` (the second test builds and drives a new app in Chrome: about a minute).
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { createApp } from '../game/new.mjs';
import { chromium } from './agent-launch.mjs';
import { groups, report } from './verify.mjs';

const SIZES = [[390, 844], [1280, 900]];
const runs = (lines) => SIZES.flatMap((size) => ['light', 'dark'].map((scheme) => ({ size, scheme, asked: scheme, lines: lines(size, scheme), hidden: 0 })));
const shots = SIZES.flatMap(([w, h]) => ['light', 'dark'].map((scheme) => `/app/.exact/verify/web-${w}x${h}-${scheme}.png`));

test('the report: clean in four lines; a failed test fails it; a finding is said once, where it was', () => {
  const clean = report({ host: 'web', built: null, tested: { passed: 2, failed: 0, results: [] }, checks: runs(() => []), shots, ms: 4000, cwd: '/app' });
  assert.deepEqual(clean.lines, [
    'tests: 2 passed, 0 failed',
    'check 390x844, 1280x900 · light, dark: no findings',
    'screenshots (.exact/verify): web-390x844-light.png web-390x844-dark.png web-1280x900-light.png web-1280x900-dark.png',
    'verify: clean in 4.0 s',
  ]);
  assert.equal(clean.exit, 0);

  // The clipped text is at every width and scheme, its box moved by the width; the contrast only in light.
  const clipped = ([w]) => `  clipped  Text "Total"#9 [16,96 ${w - 330}×18]: its text is 40 px wider than its box, which clips it`;
  const contrast = '  contrast Text "Due"#12 [16,78 60×18]: 1.3:1 (#dddddd on #ffffff), needs 4.5:1';
  const failing = report({
    host: 'web', built: { ms: 900, warnings: [] }, ms: 6500, cwd: '/app', shots,
    tested: { passed: 1, failed: 1, results: [{ name: 'adds', failures: ['line 4: text of "left" is "1 items left", expected "1 item left"\n  more'] }, { name: 'loads', failures: [] }] },
    checks: runs((size, scheme) => [clipped(size), ...(scheme === 'light' ? [contrast] : [])]),
  });
  assert.deepEqual(failing.lines.slice(0, 6), [
    'built web (0.9 s)',
    'tests: 1 passed, 1 failed',
    '  FAIL "adds": line 4: text of "left" is "1 items left", expected "1 item left"',
    'check 390x844, 1280x900 · light, dark: 2 findings',
    clipped(SIZES[0]),
    '  (light only) contrast Text "Due"#12 [16,78 60×18]: 1.3:1 (#dddddd on #ffffff), needs 4.5:1',
  ]);
  assert.equal(failing.lines.at(-1), 'verify: 3 problems (1 failed test, 2 findings) in 6.5 s');
  assert.equal(failing.exit, 1);

  // Findings alone are counted, never fail it; a launch that could not drive does.
  const found = report({ host: 'web', built: null, tested: { passed: 2, failed: 0, results: [] }, checks: runs(([w]) => (w === 1280 ? [contrast] : [])), shots, ms: 5000, cwd: '/app' });
  assert.equal(found.lines[2], '  (1280x900 only) contrast Text "Due"#12 [16,78 60×18]: 1.3:1 (#dddddd on #ffffff), needs 4.5:1');
  assert.equal(found.lines.at(-1), 'verify: 1 problem (1 finding; the tests passed) in 5.0 s');
  assert.equal(found.exit, 0);
  const stuck = report({ host: 'web', built: null, tested: { none: true }, checks: [{ size: null, label: '390x844', error: 'the page never drew\nstack' }], shots: [], ms: 9000 });
  assert.deepEqual(stuck.lines, ['tests: none (no app.test.contract)', 'check 390x844: could not drive: the page never drew', 'verify: 1 problem (1 failure to compile or drive) in 9.0 s']);
  assert.equal(stuck.exit, 1);
});

test('check\'s text splits by scheme, counting what its own cap left out', () => {
  const text = ['check web · light: 13 findings', ...Array.from({ length: 12 }, (_, i) => `  target   button#${i} [0,0 20×20]: small`), '  … and 1 more (--json lists them all)', 'check web · dark: no findings (text fits)'].join('\n');
  const [light, dark] = groups(text);
  assert.equal(light.lines.length, 12);
  assert.equal(light.hidden, 1);
  assert.deepEqual([dark.lines, dark.hidden], [[], 0]);
});

// A new app, built and driven as a builder would: clean; a failing test fails it; a finding does not.
test.skipIf(!!chromium().unavailable)('verify on a new app: clean, a failing test, a check finding, --json', () => {
  const parent = mkdtempSync(resolve(tmpdir(), 'exact-verify-'));
  try {
    const dir = resolve(parent, 'field-log');
    createApp(dir);
    const verify = (...args) => {
      const r = spawnSync(process.execPath, [resolve(dir, 'exact.mjs'), 'verify', ...args], { cwd: dir, encoding: 'utf8', timeout: 180_000 });
      return { ...r, lines: r.stdout.trim().split('\n') };
    };
    const clean = verify();
    assert.equal(clean.status, 0, clean.stdout + clean.stderr);
    assert.ok(clean.lines.length <= 6, clean.stdout);
    assert.ok(clean.lines.includes('tests: 2 passed, 0 failed'), clean.stdout);
    assert.ok(clean.lines.includes('check 390x844, 1280x900 · light, dark: no findings'), clean.stdout);
    assert.match(clean.lines.at(-1), /^verify: clean in \d+\.\d s$/);
    for (const [w, h] of SIZES) for (const scheme of ['light', 'dark']) assert.ok(existsSync(resolve(dir, `.exact/verify/web-${w}x${h}-${scheme}.png`)), `${w}x${h} ${scheme}`);

    const tests = resolve(dir, 'app.test.contract'), authored = readFileSync(tests, 'utf8');
    writeFileSync(tests, authored.replace('expect state liked == true', 'expect state liked == false'));
    const failing = verify();
    assert.equal(failing.status, 1, failing.stdout + failing.stderr);
    assert.ok(failing.lines.includes('tests: 1 passed, 1 failed'), failing.stdout);
    assert.match(failing.stdout, /^ {2}FAIL "the header's button likes it": line \d+: /m);
    assert.match(failing.lines.at(-1), /^verify: 1 problem \(1 failed test\) in /);

    writeFileSync(tests, authored);
    const view = resolve(dir, 'app.contract'), source = readFileSync(view, 'utf8');
    const at = source.match(/^( *)text greeting\.text testId="greeting"$/m);
    assert.ok(at, 'the starter shows its greeting');
    writeFileSync(view, source.replace(at[0], `${at[0]}\n${at[1]}text "Faint" color="#dddddd"`));
    const found = verify();
    assert.equal(found.status, 0, found.stdout + found.stderr);
    assert.match(found.lines[0], /^built web \(\d+\.\d s\)$/);
    assert.match(found.stdout, /^ {2}\(light only\) contrast Text "Faint"#\d+ \[.*\]: 1\.\d:1 \(#dddddd on #ffffff\), needs 4\.5:1/m);
    assert.match(found.lines.at(-1), /^verify: 1 problem \(1 finding; the tests passed\) in /);

    const json = verify('--json');
    assert.equal(json.status, 0, json.stderr);
    const r = JSON.parse(json.stdout);
    assert.deepEqual([r.host, r.built, r.tests.passed, r.tests.failed, r.problems, r.exit, r.screenshots.length], ['web', null, 2, 0, 1, 0, 4]);
    assert.deepEqual(r.checks.map((c) => `${c.size.join('x')} ${c.scheme} ${c.findings.map((f) => f.kind).join(',')}`), ['390x844 light contrast', '390x844 dark ', '1280x900 light contrast', '1280x900 dark ']);
  } finally { rmSync(parent, { recursive: true, force: true }); }
}, 300_000); // A new app's offline Cargo resolution and its first web build, then four runs.
