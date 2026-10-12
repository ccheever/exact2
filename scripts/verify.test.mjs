// LLP 1116.004 D7, 1116.005 D7: `bun exact.mjs verify` is one call a builder can stop on: a clean run in a few
// lines, a failing test that fails it, a check finding or a compiler warning said once, reported but not failing
// it, each test's end screen checked under its name, and one relaunch on a test's store.
// `bun test ./scripts/verify.test.mjs` (the last test builds and drives a new app in Chrome: about a minute).
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { applyPattern, createApp } from '../game/new.mjs';
import { chromium } from './agent-launch.mjs';
import { appServer, groups, judgeRelaunch, pinnedLaunch, relaunchStore, report, serverUrl, shotName } from './verify.mjs';

const SIZES = [[390, 844], [1280, 900]];
const runs = (lines, screen) => SIZES.flatMap((size) => ['light', 'dark'].map((scheme) => ({ ...(screen ? { screen } : {}), size, scheme, asked: scheme, lines: lines(size, scheme), hidden: 0 })));
const shots = SIZES.flatMap(([w, h]) => ['light', 'dark'].map((scheme) => `/app/.exact/verify/web-${w}x${h}-${scheme}.png`));

test('the report: clean in four lines; a failed test fails it; a finding is said once, where it was', () => {
  const clean = report({ host: 'web', built: null, tested: { passed: 2, failed: 0, results: [] }, checks: runs(() => []), shots, ms: 4000, cwd: '/app' });
  assert.deepEqual(clean.lines, [
    'tests: 2 passed, 0 failed',
    'check first screen · 390x844, 1280x900 · light, dark: no findings',
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
    'check first screen · 390x844, 1280x900 · light, dark: 2 findings',
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

// LLP 1116.005 D7: each test's end screen is in the one report, its findings under the test's name and said once
// with the first screen's; the relaunch is a line of its own, and what it lost a finding.
test('the journeys: a test end\'s finding under its name, deduplicated across screens; the relaunch line', () => {
  const clipped = ([w]) => `  clipped  Text "Total"#9 [16,96 ${w - 330}×18]: its text is 40 px wider than its box, which clips it`;
  const contrast = (y) => `  contrast summary [16,${y} 300×20]: 2.1:1 (#999999 on #ffffff), needs 4.5:1`;
  const ends = ['web-t1-adds-a-card.png', 'web-t2-studies-the-deck.png'].map((f) => `/app/.exact/verify/${f}`);
  const relaunched = { test: 'adds a card', ms: 400, shows: ['Everyday French'], problems: [], said: 'the first screen shows what it stored: "Everyday French"' };
  const clean = report({ host: 'web', built: null, tested: { passed: 2, failed: 0, results: [] }, ms: 9000, cwd: '/app', relaunched,
    checks: [...runs(() => []), ...runs(() => [], 'adds a card'), ...runs(() => [], 'studies the deck'), ...runs(() => [], 'relaunch')], shots: [...shots, ...ends, '/app/.exact/verify/web-relaunch.png'] });
  assert.deepEqual(clean.lines, [
    'tests: 2 passed, 0 failed',
    'check first screen, 2 test ends, relaunch · 390x844, 1280x900 · light, dark: no findings',
    'relaunch on "adds a card"\'s store (0.4 s of real time): the first screen shows what it stored: "Everyday French"',
    'screenshots (.exact/verify): web-390x844-light.png web-390x844-dark.png web-1280x900-light.png web-1280x900-dark.png',
    '  test ends: web-t1-adds-a-card.png web-t2-studies-the-deck.png; relaunch: web-relaunch.png',
    'verify: clean in 9.0 s',
  ]);
  assert.equal(clean.exit, 0);

  // The summary's contrast only on the second test's end, in dark; the clipped total on the first screen and both ends.
  const found = report({ host: 'web', built: null, tested: { passed: 2, failed: 0, results: [] }, ms: 9000, cwd: '/app', shots: [], relaunched: { ...relaunched, problems: ['  lost     its first screen and resources are a fresh launch\'s, …'] },
    checks: [...runs((size) => [clipped(size)]), ...runs((size) => [clipped(size)], 'adds a card'), ...runs((size, scheme) => [clipped(size), ...(scheme === 'dark' ? [contrast(size[0] === 390 ? 210 : 140)] : [])], 'studies the deck'), ...runs(() => [], 'relaunch')] });
  assert.deepEqual(found.lines.slice(1, 4), [
    'check first screen, 2 test ends, relaunch · 390x844, 1280x900 · light, dark: 2 findings',
    `  (first screen, "adds a card", "studies the deck" only) ${clipped(SIZES[0]).trim()}`,
    `  ("studies the deck", dark only) ${contrast(210).trim()}`,
  ]);
  assert.deepEqual(found.lines.slice(4, 6), ['relaunch on "adds a card"\'s store (0.4 s of real time): 1 problem', '  lost     its first screen and resources are a fresh launch\'s, …']);
  assert.equal(found.lines.at(-1), 'verify: 3 problems (3 findings; the tests passed) in 9.0 s');
  assert.equal(found.exit, 0);

  // A test end or the relaunch that could not be driven fails it, as a launch does.
  const stuck = report({ host: 'web', built: null, tested: { passed: 1, failed: 0, results: [] }, ms: 9000, shots: [], relaunched: { test: 'adds a card', error: 'the page never drew\nstack' },
    checks: [...runs(() => []), { size: null, label: '"adds a card" end', error: 'resize: no page' }] });
  assert.deepEqual(stuck.lines.slice(1), ['check "adds a card" end: could not drive: resize: no page', 'check first screen · 390x844, 1280x900 · light, dark: no findings', 'relaunch: could not drive: the page never drew', 'verify: 2 problems (2 failures to compile or drive) in 9.0 s']);
  assert.equal(stuck.exit, 1);
  assert.equal(shotName('web', 1, "the header's button likes it, then unlikes it"), 'web-t2-the-header-s-button-likes-it.png');
});

test('the relaunch: the store of a passing test that wrote the most; what came back, or that it was lost', () => {
  const fresh = { texts: new Set(['Decks', 'No decks yet.']), resources: { decks: [] }, slots: { draft: '' }, ops: 1 };
  const end = (resources, ops, reload = false) => ({ keep: true, resources, ops, reload });
  const results = [
    { name: 'reads', failures: [], store: 's0', end: end({ decks: [{ id: '1', name: 'Seeded deck with a long name' }] }, 1) },
    { name: 'adds', failures: [], store: 's1', end: end({ decks: [{ id: '1', name: 'French' }] }, 3) },
    { name: 'adds two', failures: ['line 9: …'], store: 's2', end: end({ decks: [{ id: '1' }, { id: '2' }] }, 5) },
  ];
  assert.equal(relaunchStore(results, fresh).name, 'adds'); // it wrote; the longer resources only read; the third failed
  assert.equal(relaunchStore(results.slice(0, 1), fresh).name, 'reads');
  assert.equal(relaunchStore([results[2]], fresh), null);

  const after = (texts, resources = { decks: [] }, slots = { draft: '' }) => ({ texts: new Set(texts), resources, slots });
  const back = judgeRelaunch(results[1], fresh, after(['Decks', 'French', '1 card']), { busy: [], failed: {} });
  assert.deepEqual([back.shows, back.problems, back.said], [['French', '1 card'], [], 'the first screen shows what it stored: "French", "1 card"']);
  const lost = judgeRelaunch(results[1], fresh, after(['Decks', 'No decks yet.']), {});
  assert.equal(lost.problems.length, 1);
  assert.match(lost.problems[0], /^ {2}lost {5}.*what it stored did not come back$/);
  // A test that wrote nothing has nothing to lose; one whose resources came back off this screen lost nothing either.
  assert.deepEqual(judgeRelaunch(results[0], fresh, after(['Decks', 'No decks yet.']), {}), { shows: [], problems: [], said: 'the first screen is a fresh launch\'s; that test wrote no storage' });
  assert.deepEqual(judgeRelaunch(results[1], fresh, after(['Decks', 'No decks yet.'], { decks: [{ id: '1' }] }), {}).problems, []);
  const slow = judgeRelaunch(results[1], fresh, after(['Decks']), { busy: ['decks'], failed: { cards: 'a refused shape' } });
  assert.deepEqual(slow.problems.map((p) => p.trim().split(/\s+/)[0]), ['loading', 'failed']);
});

// LLP 1116.007 D3.4: a test that pinned its date left data a month-filtered screen shows only under that date, so the
// relaunch is made under the test's launch facts and compared with a fresh launch under them; only a test whose facts
// are not the driver's own asks for that second fresh launch.
test('the relaunch keeps a test\'s pinned date, zone, locale and seed', () => {
  const defaults = { epoch: Date.parse('2026-01-01T00:00:00Z'), timeZone: 'UTC', locale: 'en-US', seed: 1 };
  const test = (launch) => ({ name: 'saves', failures: [], store: 's', end: { keep: true, resources: {}, ops: 3, launch } });
  assert.equal(pinnedLaunch(test(defaults), defaults), false);
  assert.equal(pinnedLaunch(test({ ...defaults, epoch: Date.parse('2026-10-12T12:00:00Z') }), defaults), true);
  assert.equal(pinnedLaunch(test({ ...defaults, timeZone: 'Pacific/Auckland' }), defaults), true);
  assert.equal(pinnedLaunch(test({ ...defaults, locale: 'fr-FR' }), defaults), true);
  assert.equal(pinnedLaunch(test({ ...defaults, seed: 7 }), defaults), true);
  assert.equal(pinnedLaunch({ name: 'saves', end: { keep: true } }, defaults), false); // a test run without launch facts

  // The relaunch under October is compared with a fresh launch under October: the month's title is October's in both, so
  // the entry's coming back, or its loss, is all that differs.
  const october = (month) => ({ texts: new Set(['October 2026', ...month]), resources: { month: { title: 'October 2026', count: month.length } }, slots: {}, ops: 1 });
  const saved = { ...test(defaults), end: { ...test(defaults).end, resources: october(['A fresh start.']).resources } };
  assert.deepEqual(judgeRelaunch(saved, october([]), october(['A fresh start.'])).problems, []);
  assert.equal(judgeRelaunch(saved, october([]), october([])).problems.length, 1);
});

test('the compiler\'s warnings: a group before the tests, capped as findings are, counted as findings', () => {
  const warning = (n) => `app.contract:${n}:23 warning [lower-colour-one-scheme] \`#336699\` on \`color\` stays the same in dark mode`;
  const one = report({ host: 'web', built: null, compiler: [warning(25)], tested: { passed: 2, failed: 0, results: [] }, checks: runs(() => []), shots: [], ms: 4000 });
  assert.deepEqual(one.lines, ['compiler: 1 warning', `  ${warning(25)}`, 'tests: 2 passed, 0 failed', 'check first screen · 390x844, 1280x900 · light, dark: no findings', 'verify: 1 problem (1 finding; the tests passed) in 4.0 s']);
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

// LLP 1116.006 D3: the server an app calls, found where the rest pattern and the builders put it.
test('the app\'s server: its entry, the URL app.ts calls, and why it cannot be run on an empty database', () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-server-'));
  const put = (files) => { rmSync(dir, { recursive: true, force: true }); for (const [path, text] of Object.entries(files)) { mkdirSync(resolve(dir, path, '..'), { recursive: true }); writeFileSync(resolve(dir, path), text); } };
  const reads = "const db = new Database(process.env.DB ?? 'data.db');\n";
  try {
    put({ 'app.ts': "const API = 'http://127.0.0.1:6810'; // a phone reaches http://192.168.1.2:6810\n", 'server/index.ts': reads });
    assert.deepEqual(appServer(dir), { entry: 'server/index.ts', url: 'http://127.0.0.1:6810', port: 6810, why: null });
    put({ 'app.ts': "export const API = 'http://127.0.0.1:8787';\n", 'server.ts': reads, 'srv/main.ts': reads, 'app.json': JSON.stringify({ commands: { server: ['bun', '--watch', 'srv/main.ts'] } }) });
    assert.equal(appServer(dir).entry, 'srv/main.ts');
    put({ 'app.ts': "import type { Answer } from './app.contract.d.ts';\nimport { API } from './api';\n", 'api.ts': "export const API = 'http://localhost:3000/v1/';\n", 'server/server.ts': 'const DB = Bun.env.DB;\n' });
    assert.deepEqual([serverUrl(dir), appServer(dir).port, appServer(dir).why], ['http://localhost:3000/v1', 3000, null]);
    put({ 'app.ts': "const API = 'http://127.0.0.1:8787';\n", 'server.ts': "new Database('data.db');\n" });
    assert.match(appServer(dir).why, /^server\.ts is left alone: it does not read its database's path from DB/);
    put({ 'app.ts': "const API = 'https://api.example.com';\n", 'server.ts': reads });
    assert.match(appServer(dir).why, /^server\.ts is left alone: app\.ts calls no local URL/);
    put({ 'app.ts': "const API = 'http://127.0.0.1:8787';\n" });
    assert.equal(appServer(dir), null);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

// The rest pattern, verified as a builder would with a data.db of their own: verify runs its server on an empty
// database and leaves theirs as it was; with their dev server up it refuses in one line and leaves that one running.
test.skipIf(!!chromium().unavailable)('verify on a server app: its own server on an empty database, the builder\'s data untouched, a dev server refused', async () => {
  const parent = mkdtempSync(resolve(tmpdir(), 'exact-verify-server-'));
  try {
    const dir = resolve(parent, 'shared-list');
    createApp(dir);
    applyPattern(dir, 'rest');
    const { Database } = await import('bun:sqlite');
    const theirs = new Database(resolve(dir, 'data.db'));
    theirs.run('CREATE TABLE items (id INTEGER PRIMARY KEY, title TEXT NOT NULL, notes TEXT NOT NULL, by TEXT NOT NULL, locked INTEGER NOT NULL DEFAULT 0)');
    theirs.run("INSERT INTO items (title, notes, by) VALUES ('Theirs', '', 'Ada')");
    theirs.close();
    const before = readFileSync(resolve(dir, 'data.db'));
    const verify = () => { const r = spawnSync(process.execPath, [resolve(dir, 'exact.mjs'), 'verify'], { cwd: dir, encoding: 'utf8', timeout: 240_000 }); return { ...r, lines: r.stdout.trim().split('\n') }; };
    const clean = verify();
    assert.equal(clean.status, 0, clean.stdout + clean.stderr);
    assert.equal(clean.lines[0], 'server: bun server.ts at http://127.0.0.1:8787, on an empty database (.exact/verify/db.sqlite, removed after)', clean.stdout);
    assert.ok(clean.lines.includes('tests: 1 passed, 0 failed'), clean.stdout);
    assert.match(clean.stdout, /^relaunch on .*: the first screen shows what it stored: /m);
    assert.ok(readFileSync(resolve(dir, 'data.db')).equals(before), 'the builder\'s data.db is as it was');
    assert.deepEqual(['data.db-wal', '.exact/verify/db.sqlite'].filter((f) => existsSync(resolve(dir, f))), []);
    // LLP 1116.007 D2: `test` runs the same server on the same terms (it was the builder's to start, and `api-test`'s to know),
    // and an older app's api-test, whose test.mjs runs `test` under `withAppServer`, still passes on the one server it started.
    const plain = (verb = 'test') => spawnSync(process.execPath, [resolve(dir, 'exact.mjs'), verb, 'web'], { cwd: dir, encoding: 'utf8', timeout: 240_000 });
    const tested = plain();
    assert.equal(tested.status, 0, tested.stdout + tested.stderr);
    assert.match(tested.stderr, /^test: bun server\.ts at http:\/\/127\.0\.0\.1:8787, on an empty database \(\.exact\/test\/db\.sqlite, removed after\)$/m);
    assert.match(tested.stdout, /^1 passed, 0 failed$/m);
    assert.ok(readFileSync(resolve(dir, 'data.db')).equals(before), 'test leaves the builder\'s data.db as it was');
    writeFileSync(resolve(dir, 'test.mjs'), "const { withAppServer } = await import(`${process.env.EXACT2}/scripts/verify.mjs`);\nprocess.exit(await withAppServer(import.meta.dir, () =>\n  Bun.spawn(['bun', 'exact.mjs', 'test', ...process.argv.slice(2)], { cwd: import.meta.dir, stdio: ['inherit', 'inherit', 'inherit'] }).exited));\n");
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ ...JSON.parse(readFileSync(resolve(dir, 'app.json'), 'utf8')), commands: { 'api-test': ['bun', 'test.mjs'] } }, null, 2));
    const old = plain('api-test');
    assert.equal(old.status, 0, old.stdout + old.stderr);
    assert.match(old.stdout, /^1 passed, 0 failed$/m);
    assert.deepEqual(['data.db-wal', '.exact/test/db.sqlite', '.exact/api-test/db.sqlite', '.exact/api-test/owner'].filter((f) => existsSync(resolve(dir, f))), []);
    // Their dev server (started here, so this test may stop it) holds the port: one line naming it, and it runs on.
    const dev = Bun.spawn([process.execPath, 'server.ts'], { cwd: dir, stdout: 'ignore', stderr: 'ignore' });
    try {
      for (let i = 0; !(await fetch('http://127.0.0.1:8787/health').then((r) => r.ok, () => false)); i++) { assert.ok(i < 100, 'the dev server answers'); await Bun.sleep(100); }
      const refused = verify();
      assert.equal(refused.status, 1, refused.stdout + refused.stderr);
      assert.equal(refused.lines.length, 1, refused.stdout);
      assert.match(refused.lines[0], new RegExp(`^verify: http://127\\.0\\.0\\.1:8787 already answers \\(pid ${dev.pid}: .*server\\.ts\\), and the tests would write into that server's database: stop it \\(kill ${dev.pid}\\) and run this again, which starts server\\.ts itself on an empty one$`));
      assert.equal(dev.exitCode, null, 'the dev server runs on');
      const held = plain();
      assert.equal(held.status, 1, held.stdout + held.stderr);
      assert.match(held.stderr, new RegExp(`^test: http://127\\.0\\.0\\.1:8787 already answers \\(pid ${dev.pid}: .*server\\.ts\\)`, 'm'));
      assert.equal(dev.exitCode, null, 'the dev server runs on');
    } finally { dev.kill(); await dev.exited; }
  } finally { rmSync(parent, { recursive: true, force: true }); }
}, 300_000);

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
    assert.ok(clean.lines.length <= 8, clean.stdout);
    assert.ok(!clean.lines.some((l) => l.startsWith('compiler:')), clean.stdout);
    assert.ok(clean.lines.includes('tests: 2 passed, 0 failed'), clean.stdout);
    assert.ok(clean.lines.includes('check first screen, 2 test ends, relaunch · 390x844, 1280x900 · light, dark: no findings'), clean.stdout);
    // The starter keeps nothing: the relaunch on the second test's store opens as a fresh launch does, and says so.
    assert.match(clean.stdout, /^relaunch on "the header's button likes it"'s store \(\d+\.\d s of real time\): the first screen is a fresh launch's; that test wrote no storage$/m);
    assert.match(clean.lines.at(-1), /^verify: clean in \d+\.\d s$/);
    for (const [w, h] of SIZES) for (const scheme of ['light', 'dark']) assert.ok(existsSync(resolve(dir, `.exact/verify/web-${w}x${h}-${scheme}.png`)), `${w}x${h} ${scheme}`);
    for (const f of ['web-t1-the-greeting-loads.png', 'web-t2-the-header-s-button-likes-it.png', 'web-relaunch.png']) assert.ok(existsSync(resolve(dir, '.exact/verify', f)), f);

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
    assert.match(found.stdout, /^ {2}\(light only\) contrast Text "Faint"#\d+ \[.*\]: 1\.\d:1 \(#dddddd on #ffffff(?:, [^)]*)?\), needs 4\.5:1/m);
    assert.match(found.lines.at(-1), /^verify: 2 problems \(2 findings; the tests passed\) in /);

    const json = verify('--json');
    assert.equal(json.status, 0, json.stderr);
    const r = JSON.parse(json.stdout);
    assert.deepEqual([r.host, r.built, r.tests.passed, r.tests.failed, r.problems, r.exit, r.screenshots.length, r.compiler.length], ['web', null, 2, 0, 2, 0, 7, 1]);
    // Each screen at each width and scheme: the first screen, the two test ends (in their own sessions), the relaunch.
    const onScreen = (screen) => r.checks.filter((c) => c.screen === screen).map((c) => `${c.size.join('x')} ${c.scheme} ${c.findings.map((f) => f.kind).join(',')}`);
    for (const screen of ['first screen', 'the greeting loads', "the header's button likes it", 'relaunch']) assert.deepEqual(onScreen(screen), ['390x844 light contrast', '390x844 dark ', '1280x900 light contrast', '1280x900 dark '], screen);
    assert.deepEqual([r.relaunch.test, r.relaunch.problems, r.tests.results.map((t) => t.shot.split('/').at(-1))], ["the header's button likes it", [], ['web-t1-the-greeting-loads.png', 'web-t2-the-header-s-button-likes-it.png']]);
  } finally { rmSync(parent, { recursive: true, force: true }); }
}, 300_000); // A new app's offline Cargo resolution and its first web build, then four runs.
