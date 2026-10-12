// LLP 1116.004 D7: `bun exact.mjs verify` is one call a builder can stop on: a clean run in a few lines, a
// failing test that fails it, and a check finding or a compiler warning said once, reported but not failing it.
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

test('the compiler\'s warnings: a group before the tests, capped as findings are, counted as findings', () => {
  const warning = (n) => `app.contract:${n}:23 warning [lower-colour-one-scheme] \`#336699\` on \`color\` stays the same in dark mode`;
  const one = report({ host: 'web', built: null, compiler: [warning(25)], tested: { passed: 2, failed: 0, results: [] }, checks: runs(() => []), shots: [], ms: 4000 });
  assert.deepEqual(one.lines, ['compiler: 1 warning', `  ${warning(25)}`, 'tests: 2 passed, 0 failed', 'check 390x844, 1280x900 · light, dark: no findings', 'verify: 1 problem (1 finding; the tests passed) in 4.0 s']);
  assert.equal(one.exit, 0);
  const many = report({ host: 'ios', built: { ms: 1200, said: ['iOS: app.contract changed; running the new plan on the existing build (no native rebuild)'], warnings: [] },
    compiler: Array.from({ length: 8 }, (_, i) => warning(i + 1)), tested: { none: true }, checks: [], shots: [], ms: 9000 });
  assert.deepEqual(many.lines.slice(1, 9), ['compiler: 8 warnings', ...Array.from({ length: 6 }, (_, i) => `  ${warning(i + 1)}`), '  … and 2 more (bun exact.mjs contract build app.contract prints them all)']);
  assert.equal(many.lines.at(-1), 'verify: 8 problems (8 findings; the tests passed) in 9.0 s');
  assert.equal(many.exit, 0);
});

test('check\'s text splits by scheme, counting what its own cap left out', () => {
  const text = ['check web · light: 13 findings', ...Array.from({ length: 12 }, (_, i) => `  target   button#${i} [0,0 20×20]: small`), '  … and 1 more (--json lists them all)', 'check web · dark: no findings (text fits)'].join('\n');
  const [light, dark] = groups(text);
  assert.equal(light.lines.length, 12);
  assert.equal(light.hidden, 1);
  assert.deepEqual([dark.lines, dark.hidden], [[], 0]);
  // Advice (LLP 1116.004 D1.3) is kept apart, never a finding: a check with only advice is clean.
  const [wide] = groups('check web · light: no findings (text fits)\n  advice   tabs: a bottom tab bar 1280 px wide');
  assert.deepEqual([wide.lines, wide.advice], [[], ['  advice   tabs: a bottom tab bar 1280 px wide']]);
  const r = report({ host: 'web', built: { ms: 1000, said: [], warnings: [] }, compiler: [], tested: { passed: 1, failed: 0, results: [] },
    checks: [{ size: [1280, 900], scheme: 'light', lines: [], hidden: 0, advice: wide.advice }], shots: [], ms: 2000 });
  assert.equal(r.lines.at(-1), 'verify: clean in 2.0 s');
  assert.ok(r.lines.includes('  advice   tabs: a bottom tab bar 1280 px wide'));
});

// A new app, built and driven as a builder would: clean; a failing test fails it; a finding does not; a
// compiler warning reaches the builder from `test web`'s build and from verify, and a clean app says none.
test.skipIf(!!chromium().unavailable)('verify on a new app: clean, a failing test, a check finding, a compiler warning, --json', () => {
  const parent = mkdtempSync(resolve(tmpdir(), 'exact-verify-'));
  try {
    const dir = resolve(parent, 'field-log');
    createApp(dir);
    const exact = (verb, ...args) => {
      const r = spawnSync(process.execPath, [resolve(dir, 'exact.mjs'), verb, ...args], { cwd: dir, encoding: 'utf8', timeout: 180_000 });
      return { ...r, lines: r.stdout.trim().split('\n') };
    };
    const verify = (...args) => exact('verify', ...args);
    const warned = (text) => text.split('\n').filter((l) => / warning \[/.test(l));
    // The build `test web` runs first says its count, and here names nothing; its drive's stdout is the tests'.
    const quiet = exact('test', 'web');
    assert.equal(quiet.status, 0, quiet.stdout + quiet.stderr);
    assert.match(quiet.stderr, /app\.css \d+ B, 0 warnings$/m);
    assert.deepEqual([warned(quiet.stderr), warned(quiet.stdout)], [[], []]);
    const clean = verify();
    assert.equal(clean.status, 0, clean.stdout + clean.stderr);
    assert.ok(clean.lines.length <= 6, clean.stdout);
    assert.ok(!clean.lines.some((l) => l.startsWith('compiler:')), clean.stdout);
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
    const line = source.slice(0, source.indexOf(at[0])).split('\n').length + 1, column = at[1].length + 'text "Faint" '.length + 1; // the attribute's name
    const warning = new RegExp(`^app\\.contract:${line}:${column} warning \\[lower-colour-one-scheme\\] \`#dddddd\` on \`color\` stays the same in dark mode`);
    // The hex colour: `test web`'s build names the compiler's warning, as `contract build` does, and counts it.
    const tested = exact('test', 'web');
    assert.equal(tested.status, 0, tested.stdout + tested.stderr);
    assert.match(tested.stderr, /app\.css \d+ B, 1 warning$/m);
    assert.equal(warned(tested.stderr).length, 1, tested.stderr);
    assert.match(warned(tested.stderr)[0], warning);
    assert.deepEqual(warned(tested.stdout), []);
    const found = verify();
    assert.equal(found.status, 0, found.stdout + found.stderr);
    assert.equal(found.lines[0], 'compiler: 1 warning', found.stdout);
    assert.match(found.lines[1].trim(), warning);
    assert.match(found.stdout, /^ {2}\(light only\) contrast Text "Faint"#\d+ \[.*\]: 1\.\d:1 \(#dddddd on #ffffff\), needs 4\.5:1/m);
    assert.match(found.lines.at(-1), /^verify: 2 problems \(2 findings; the tests passed\) in /);

    const json = verify('--json');
    assert.equal(json.status, 0, json.stderr);
    const r = JSON.parse(json.stdout);
    assert.deepEqual([r.host, r.built, r.tests.passed, r.tests.failed, r.problems, r.exit, r.screenshots.length, r.compiler.length], ['web', null, 2, 0, 2, 0, 4, 1]);
    assert.deepEqual(r.checks.map((c) => `${c.size.join('x')} ${c.scheme} ${c.findings.map((f) => f.kind).join(',')}`), ['390x844 light contrast', '390x844 dark ', '1280x900 light contrast', '1280x900 dark ']);
  } finally { rmSync(parent, { recursive: true, force: true }); }
}, 300_000); // A new app's offline Cargo resolution and its first web build, then four runs.
