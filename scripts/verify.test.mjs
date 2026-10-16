// LLP 1116.004 D7, 1116.005 D7: `bun exact.mjs verify` is one call a builder can stop on: a clean run in a few
// lines, a failing test that fails it, a check finding or a compiler warning said once, reported but not failing
// it, each test's end screen checked under its name, and one relaunch on a test's store.
// `bun test ./scripts/verify.test.mjs` (the last test builds and drives a new app in Chrome: about a minute).
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { applyPattern, createApp } from '../game/new.mjs';
import { chromium } from './agent-launch.mjs';
import { appServer, draftFields, groups, judgeRelaunch, judgeReload, movedClock, persistedStates, pinnedLaunch, refusalsOf, relaunchFacts, relaunchStore, reloadLine, reloadNote, report, savedChanges, screenTexts, serverUrl, shotName, storedTexts } from './verify.mjs';

const SIZES = [[390, 844], [1280, 900]];
/** A port nothing holds now, put in a rest-pattern app in place of 8787, which another run on this machine may hold. */
function onPort(dir) {
  const probe = Bun.serve({ port: 0, fetch: () => new Response('') }), port = probe.port;
  probe.stop(true);
  for (const file of ['app.ts', 'app.test.contract']) writeFileSync(resolve(dir, file), readFileSync(resolve(dir, file), 'utf8').replaceAll('8787', String(port)));
  return port;
}
const runs = (lines, screen) => SIZES.flatMap((size) => ['light', 'dark'].map((scheme) => ({ ...(screen ? { screen } : {}), size, scheme, asked: scheme, lines: lines(size, scheme), hidden: 0 })));
const shots = SIZES.flatMap(([w, h]) => ['light', 'dark'].map((scheme) => `/app/.exact/verify/web-${w}x${h}-${scheme}.png`));

test('the report: clean in four lines; a failed test fails it; a finding is said once, where it was', () => {
  const clean = report({ host: 'web', built: null, tested: { passed: 2, failed: 0, results: [] }, checks: runs(() => []), shots, ms: 4000, cwd: '/app' });
  assert.deepEqual(clean.lines, [
    'tests: 2 passed, 0 failed',
    'check first screen · 390x844, 1280x900 · light, dark: no findings',
    'screenshots (.exact/verify): web-390x844-light.png web-390x844-dark.png web-1280x900-light.png web-1280x900-dark.png',
    'open it: bun exact.mjs web (serves what verify checked)',
    'verify: clean in 4.0 s',
  ]);
  assert.equal(clean.exit, 0);
  // `--shots <dir>` (LLP 1116.008 D2): the line says where they were copied too.
  assert.equal(report({ host: 'web', built: null, tested: { passed: 2, failed: 0, results: [] }, checks: runs(() => []), shots, copied: '/app/shots', ms: 4000, cwd: '/app' }).lines[2],
    'screenshots (.exact/verify, copied to shots): web-390x844-light.png web-390x844-dark.png web-1280x900-light.png web-1280x900-dark.png');

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
  assert.ok(!failing.lines.some((l) => l.startsWith('open it')), 'a run whose test failed does not point at the app');
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
    'open it: bun exact.mjs web (serves what verify checked)',
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

// LLP 1116.009 D1: a `reload` relaunches at the virtual time the test had reached, and so does verify's relaunch; a test
// that only moved the clock pinned nothing, but its first screen may say another day, so the comparison is made at that day.
test('the relaunch starts at the date the test reached, which moves the comparison with it and pins nothing', () => {
  const defaults = { epoch: Date.parse('2026-01-01T00:00:00Z'), timeZone: 'UTC', locale: 'en-US', seed: 1 };
  const reached = defaults.epoch + 86_400_000;
  const test = (end) => ({ name: 'saves', failures: [], store: 's', end: { keep: true, resources: {}, ops: 3, launch: defaults, ...end } });
  assert.deepEqual(relaunchFacts(test({ reached })), { ...defaults, epoch: reached });
  assert.deepEqual(relaunchFacts(test({})), defaults); // a test run without a reached date relaunches as it launched
  assert.equal(movedClock(test({ reached })), true);
  assert.equal(movedClock(test({ reached: defaults.epoch })), false);
  assert.equal(movedClock(test({})), false);
  assert.equal(pinnedLaunch(test({ reached }), defaults), false);
});

test('the compiler\'s warnings: a group before the tests, capped as findings are, counted as findings', () => {
  const warning = (n) => `app.contract:${n}:23 warning [lower-colour-one-scheme] \`#336699\` on \`color\` stays the same in dark mode`;
  const one = report({ host: 'web', built: null, compiler: [warning(25)], tested: { passed: 2, failed: 0, results: [] }, checks: runs(() => []), shots: [], ms: 4000 });
  assert.deepEqual(one.lines, ['compiler: 1 warning', `  ${warning(25)}`, 'tests: 2 passed, 0 failed', 'check first screen · 390x844, 1280x900 · light, dark: no findings', 'open it: bun exact.mjs web (serves what verify checked)', 'verify: 1 problem (1 finding; the tests passed) in 4.0 s']);
  assert.equal(one.exit, 0);
  const many = report({ host: 'ios', built: { ms: 1200, said: ['iOS: app.contract changed; running the new plan on the existing build (no native rebuild)'], warnings: [] },
    compiler: Array.from({ length: 8 }, (_, i) => warning(i + 1)), tested: { none: true }, checks: [], shots: [], ms: 9000 });
  assert.deepEqual(many.lines.slice(1, 9), ['compiler: 8 warnings', ...Array.from({ length: 6 }, (_, i) => `  ${warning(i + 1)}`), '  … and 2 more (bun exact.mjs contract build app.contract prints them all)']);
  assert.equal(many.lines.at(-1), 'verify: 8 problems (8 findings; the tests passed) in 9.0 s');
  assert.equal(many.lines.at(-2), 'open it: bun exact.mjs ios --run (launches what verify checked)');
  assert.equal(many.exit, 0);
});

// LLP 1116.010 D6: a screen survives a reload. Each test's end screen is reloaded where it stands and its texts compared;
// round 12's flashcards study screen reloaded into "0 / 0", focus-timer's settings into 25 and 5 over the saved 30 and 10.
// LLP 1116.011 D5: the first is a note (a session in progress lives in memory), the second a finding (a saved value came back different).
test('a reload: the texts a screen shows, what it forgives, and the note that names the route', () => {
  const node = (depth, props, extra = {}) => ({ id: 1, type: 'Text', depth, props, ...extra });
  const nodes = [
    node(0, { testId: 'app' }, { type: 'View' }),
    node(1, { text: 'Recall' }),
    node(1, { text: 'covered' }, { inactive: true }), // a screen under another is not shown
    node(1, { text: 'Saved!', accessibilityRole: 'status' }), // a live region says its message once
    node(2, { text: 'inside the region' }, { type: 'View' }),
    node(1, { testId: 'focus-minutes', value: '30' }, { type: 'TextInput' }),
    node(1, { testId: 'lock', accessibilityLabel: 'Locked', checked: true }, { type: 'Control' }),
    node(1, { text: '  ' }),
  ];
  assert.deepEqual(screenTexts(nodes), ['Recall', '30', 'Locked: on']);
  assert.deepEqual(screenTexts(nodes, new Set(['focus-minutes'])), ['Recall', 'Locked: on']); // a field left out by its id
  assert.deepEqual(screenTexts(undefined), []);

  // A field the test typed into is a draft, left out of the comparison, unless the app kept what it showed and the reloaded
  // field shows something it did not keep: focus-timer's settings came back as 25 over the saved 30, the water tracker's
  // refused 0 as the stored 2000, a saved bill as an empty field, a name the app never stored.
  const field = (testId, value) => node(1, { testId, value }, { type: 'TextInput' });
  const state = { slots: { focusMinutes: 30, draft: '', tab: 'home', today: 0 }, resources: { goal: { ml: 2000 }, items: [{ title: 'Milk', price: 100.5 }] } };
  assert.deepEqual([...storedTexts(state)].sort(), ['', '0', '100.5', '2000', '30', 'Milk', 'home'].sort());
  // What the app kept is what both its states hold: the settings form's draft slot says 30 before and the default 25 after.
  const states = { before: { ...state, slots: { ...state.slots, focusDraft: '30' } }, after: { ...state, slots: { ...state.slots, focusDraft: '25' } } };
  const was = [field('focus-minutes', '30'), field('goal', '0'), field('bill', '100.50'), field('name', 'x')];
  const now = [field('focus-minutes', '25'), field('goal', '2000'), field('bill', ''), field('name', '')];
  const touched = ['focus-minutes', 'goal', 'bill', 'name'];
  assert.deepEqual([...draftFields(was, now, touched, states)].sort(), ['goal', 'name']); // the stored 30 and 100.5 came back as 25 and nothing
  assert.deepEqual([...draftFields(was, now, ['goal', 'name'], states)].sort(), ['goal', 'name']);
  assert.deepEqual([...draftFields(was, now, [], states)], []); // a field the test never typed into is the app's to show
  assert.deepEqual([...draftFields(was, [], ['focus-minutes'], states)], []); // gone after: the stored text it showed is compared as lost
  const shown = (url, ...texts) => ({ url, texts });
  // Nothing differs; a running timer's clock face and the texts around it are masked; the date a test reached is pinned, so a date is the same.
  assert.equal(judgeReload(shown('/', 'Focus', '24:49', 'Last studied Oct 12, 2026'), shown('/', 'Focus', '25:00', 'Last studied Oct 12, 2026')), null);
  assert.equal(judgeReload(shown('/', 'Elapsed 01:02:03.5'), shown('/', 'Elapsed 00:00:00.0')), null);
  // A message that went (a toast, a validation error, an open dialog's words): texts taken away, none put in their place.
  assert.equal(judgeReload(shown('/new', 'New deck', 'Give your deck a name.'), shown('/new', 'New deck')), null);
  assert.equal(judgeReload(shown('/', 'Milk', 'Deleted', 'Undo'), shown('/', 'Milk')), null);
  // A route that reloads into another state: flashcards' study screen, focus-timer's settings, a draft that was kept in memory.
  const study = judgeReload(shown('/deck/1/study', 'Session complete', '0 / 1', 'Keep practicing.'), shown('/deck/1/study', 'Session complete', '0 / 0', 'Every card remembered.'));
  assert.deepEqual(study, { route: '/deck/1/study', lost: ['0 / 1', 'Keep practicing.'], gained: ['0 / 0', 'Every card remembered.'] });
  assert.equal(reloadNote(study, 'early finish'), '  note     /deck/1/study reloads into another state: "0 / 1", "Keep practicing." → "0 / 0", "Every card remembered." ("early finish")');
  assert.deepEqual(judgeReload(shown('/settings', 'Focus', '30', '10'), shown('/settings', 'Focus', '25', '5')).lost, ['30', '10']);
  const draft = judgeReload(shown('/', 'Squat', '5 × 135', 'Add set', 'Finish workout'), shown('/', 'Start workout'));
  assert.deepEqual([draft.lost.length, draft.gained], [4, ['Start workout']]);
  // More than a message gone with nothing back is a list that is empty now; a text that only appeared is a state too; so is a URL that moved.
  assert.deepEqual(judgeReload(shown('/', 'a', 'b', 'c', 'd'), shown('/')).lost, ['a', 'b', 'c', 'd']);
  assert.match(reloadNote(judgeReload(shown('/', 'a', 'b', 'c', 'd', 'e'), shown('/')), 'x'), /^ {2}note {5}\/ reloads into another state: loses "a", "b", "c", … \(2 more\) /);
  assert.match(reloadNote(judgeReload(shown('/', 'Home'), shown('/', 'Home', 'Welcome back')), 'x'), /: gains "Welcome back" /);
  assert.equal(judgeReload(shown('/settings', 'Home', 'x'), shown('/', 'Home', 'y')).now, '/');
  assert.equal(judgeReload(shown('/', 'Home'), shown('/other', 'Home')), null); // the same words on another URL are nothing to say
});

// LLP 1116.011 D5: a reload that changes the screen is a note; it is a finding only when a saved value came back different.
test('a reload: the persisted states, and the evidence that something saved was lost', () => {
  assert.deepEqual(persistedStates('/no/such/app'), []);
  const dir = mkdtempSync(resolve(tmpdir(), 'verify-persist-'));
  try {
    writeFileSync(resolve(dir, 'app.contract'), 'component App\n  state goal = 2000 persist\n  state last-seen = "" persist // kept\n  state draft = ""\n  state label = "persist"\n  state note = "x" // persist\n  state tab = "home"  persist  \n  view\n    text "x"\n');
    assert.deepEqual(persistedStates(dir), ['goal', 'last-seen', 'tab']);
  } finally { rmSync(dir, { recursive: true, force: true }); }

  const node = (testId, value, extra = {}) => ({ id: 1, type: 'TextInput', depth: 1, props: { testId, value }, ...extra });
  const held = { slots: { persona: 'Grace', goal: 2000, saved: ['a', 'b'], round: 3 }, resources: { questions: [{ q: 'Capital of France?' }] } };
  const states = (before, after) => ({ before: { ...held, slots: { ...held.slots, ...before } }, after: { ...held, slots: { ...held.slots, ...after } } });
  const same = savedChanges({ persisted: ['persona', 'goal', 'saved'], states: states({}, { round: 0 }), nodes: { before: [], after: [] }, touched: [] });
  // A game in progress (round 3, a question on the screen) came back as round 0: memory, not a saved value.
  assert.deepEqual(same, []);
  // A persisted state that came back different: the saved persona overwritten, a list shortened.
  assert.deepEqual(savedChanges({ persisted: ['persona', 'goal', 'saved', 'gone'], states: states({}, { persona: 'Ada', saved: ['a'] }), nodes: { before: [], after: [] }, touched: [] }),
    [{ kind: 'state', name: 'persona', was: 'Grace', now: 'Ada' }, { kind: 'state', name: 'saved', was: ['a', 'b'], now: ['a'] }]);
  // A state that is not persisted is not judged, whatever it did.
  assert.deepEqual(savedChanges({ persisted: ['goal'], states: states({}, { persona: 'Ada', round: 0 }), nodes: { before: [], after: [] }, touched: [] }), []);
  // A field the test typed into that showed a kept value (the app holds 2000 both times) and shows another now: focus-timer's
  // settings. A draft (a value the app never held) and a field the test never touched are not saved values.
  const fields = savedChanges({ persisted: [], states: states({}, {}), touched: ['goal', 'draft'],
    nodes: { before: [node('goal', '2000'), node('draft', 'half-typed'), node('name', '2000')], after: [node('goal', '25'), node('draft', ''), node('name', '25')] } });
  assert.deepEqual(fields, [{ kind: 'field', id: 'goal', was: '2000', now: '25' }]);
  // A field that is gone after the reload is another screen, which the note says; it is not a saved value changed.
  assert.deepEqual(savedChanges({ persisted: [], states: states({}, {}), touched: ['goal'], nodes: { before: [node('goal', '2000')], after: [] } }), []);

  const finding = { route: '/settings', lost: ['2000'], gained: ['25'], saved: [...fields, { kind: 'state', name: 'persona', was: 'Grace', now: 'Ada' }] };
  assert.equal(reloadLine(finding, 'goal is saved'), '  reload   /settings a saved value came back different: the field "goal" showed "2000", which the app holds, and shows "25"; the saved state `persona` was "Grace" and is "Ada"; a reload must bring back what was saved ("goal is saved")');
  assert.match(reloadLine({ ...finding, now: '/' }, 'x'), /^ {2}reload {3}\/settings, landing on \/, a saved value came back different: /);
  assert.match(reloadLine({ route: '/', lost: [], gained: [], saved: [{ kind: 'state', name: 'saved', was: ['a', 'b'], now: undefined }] }, 'x'), /the saved state `saved` was "\[\\"a\\",\\"b\\"\]" and is "none"/);
});

test('the report: each test end reloaded; a saved value lost is a finding once a route and change, any other change a note', () => {
  const finding = { route: '/settings', lost: ['30'], gained: ['25'], saved: [{ kind: 'field', id: 'focus-minutes', was: '30', now: '25' }] };
  const note = { route: '/deck/1/study', lost: ['0 / 1'], gained: ['0 / 0'] };
  const base = { host: 'web', built: null, tested: { passed: 3, failed: 0, results: [] }, checks: runs(() => []), shots: [], ms: 4000 };
  const same = report({ ...base, tested: { passed: 2, failed: 0, results: [] },
    reloads: [{ test: 'a', ran: true, finding: null, note: null }, { test: 'b', ran: true, finding: null }, { test: 'drafts', ran: false }] });
  assert.deepEqual(same.lines.slice(1, 3), ['check first screen · 390x844, 1280x900 · light, dark: no findings', 'reload 2 test ends: the same screen after each']);
  assert.equal(same.lines.at(-1), 'verify: clean in 4.0 s');
  // Two tests ending on one route with one saved value lost name it once, the first test; the finding counts, and fails nothing.
  const found = report({ ...base, reloads: [{ test: 'sets the goal', ran: true, finding }, { test: 'sets it twice', ran: true, finding }, { test: 'adds', ran: true, finding: null, note: null }] });
  assert.deepEqual(found.lines.slice(2, 4), ['reload 3 test ends: 1 finding', reloadLine(finding, 'sets the goal')]);
  assert.equal(found.lines.at(-1), 'verify: 1 problem (1 finding; the tests passed) in 4.0 s');
  assert.equal(found.exit, 0);
  // A screen that reloaded into another state is a note under its own heading, with the advice once; it counts for nothing.
  const noted = report({ ...base, reloads: [{ test: 'finishes early', ran: true, finding: null, note }, { test: 'finishes twice', ran: true, finding: null, note }, { test: 'adds', ran: true, finding: null, note: null }] });
  assert.deepEqual(noted.lines.slice(2, 6), [
    'reload 3 test ends: no findings',
    'reload note: 1 screen reloads into another state (not counted)',
    reloadNote(note, 'finishes early'),
    '  a saved value must survive a reload; a game or a draft in progress may not',
  ]);
  assert.equal(noted.lines.at(-1), 'verify: clean in 4.0 s');
  assert.equal(noted.problems, 0);
  // Both: the finding counts, the note does not.
  const both = report({ ...base, reloads: [{ test: 'plays', ran: true, finding: null, note }, { test: 'sets the goal', ran: true, finding }] });
  assert.deepEqual(both.lines.slice(2, 7), ['reload 2 test ends: 1 finding', reloadLine(finding, 'sets the goal'), 'reload note: 1 screen reloads into another state (not counted)', reloadNote(note, 'plays'), '  a saved value must survive a reload; a game or a draft in progress may not']);
  assert.equal(both.lines.at(-1), 'verify: 1 problem (1 finding; the tests passed) in 4.0 s');
  // Notes past the shown ones are counted, not listed.
  const many = report({ ...base, reloads: Array.from({ length: 8 }, (_, i) => ({ test: `t${i}`, ran: true, finding: null, note: { route: `/r${i}`, lost: ['a'], gained: ['b'] } })) });
  assert.ok(many.lines.includes('reload note: 8 screens reload into another state (not counted)') && many.lines.includes('  … and 2 more (verify --json lists them all)'), many.lines.join('\n'));
  assert.equal(many.problems, 0);
  const stuck = report({ ...base, tested: { passed: 1, failed: 0, results: [] },
    reloads: [{ test: 'saves', ran: true, error: 'reload: the app\'s storage did not finish before the restart: x\nmore' }] });
  assert.deepEqual(stuck.lines.slice(2, 4), ['reload 1 test end: 1 finding', '  reload   "saves": could not reload the end screen: reload: the app\'s storage did not finish before the restart: x']);
  // The iOS report has no reload: a native host's cold launch is `/` (LLP 1038 §7).
  assert.ok(!report({ ...base, host: 'ios', reloads: [{ test: 'a', ran: false }] }).lines.some((l) => l.startsWith('reload')));
});

// LLP 1116.010 D5: what the host refused while the app ran is in the journal (and, on iOS, the host's stderr); verify shows it first.
test('run-time refusals: found in the journal and the host\'s lines, shown first, counted as findings', () => {
  const swipe = 'swipeContent on #243 is refused: "delete-1" names no one descendant of it; a row needs one full-size descendant and uniquely named descendant press controls with accessible names';
  assert.deepEqual(refusalsOf({
    lines: ['t=0 boot: 15 nodes, epoch 1', `t=12 ${swipe}`, 't=40 saveFile: refused: no app file to copy', 't=41 saveFile: refused: no app file to copy', 't=50 modal dismissal refused: closedby is none', 't=60 back refused: the Back control is disabled'],
    host: [`app: exact: ${swipe}`, 'console.error: Failed to load resource: net::ERR_CONNECTION_REFUSED', 'app: exact: showModal confirm refused: no dialog or popover has that id'],
  }), [swipe, 'saveFile: refused: no app file to copy', 'showModal confirm refused: no dialog or popover has that id']);
  assert.deepEqual(refusalsOf({}), []);
  const refused = [{ line: swipe, where: ['first screen', 'test "adds"', 'relaunch'] }, { line: 'saveFile: refused: no app file to copy', where: ['test "saves"'] }];
  const r = report({ host: 'ios', built: { ms: 900, said: [], warnings: [] }, tested: { passed: 1, failed: 0, results: [] }, checks: runs(() => []), shots: [], ms: 4000, refused });
  assert.deepEqual(r.lines.slice(0, 3), [
    'refused at run time: 2 (the host\'s journal, \'logs\'; each is a call or a wiring the app got wrong)',
    `  ${swipe} (first screen, test "adds" and 1 more)`,
    '  saveFile: refused: no app file to copy (test "saves")',
  ]);
  assert.equal(r.lines[3], 'built ios (0.9 s)');
  assert.equal(r.lines.at(-1), 'verify: 2 problems (2 findings; the tests passed) in 4.0 s');
  assert.equal(r.exit, 0);
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
// LLP 1116.008 D1: an entry that does not read DB (itself, or in a module it imports) is refused in one line naming it.
test('the app\'s server: its entry, the URL app.ts calls, and why it cannot be run on an empty database', () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-server-'));
  const put = (files) => { rmSync(dir, { recursive: true, force: true }); for (const [path, text] of Object.entries(files)) { mkdirSync(resolve(dir, path, '..'), { recursive: true }); writeFileSync(resolve(dir, path), text); } };
  const reads = "const db = new Database(process.env.DB ?? 'data.db');\n";
  try {
    put({ 'app.ts': "const API = 'http://127.0.0.1:6810'; // a phone reaches http://192.168.1.2:6810\n", 'server/index.ts': reads });
    assert.deepEqual(appServer(dir), { entry: 'server/index.ts', url: 'http://127.0.0.1:6810', port: 6810, why: null, refusal: null });
    put({ 'app.ts': "export const API = 'http://127.0.0.1:8787';\n", 'server.ts': reads, 'srv/main.ts': reads, 'app.json': JSON.stringify({ commands: { server: ['bun', '--watch', 'srv/main.ts'] } }) });
    assert.equal(appServer(dir).entry, 'srv/main.ts');
    put({ 'app.ts': "import type { Answer } from './app.contract.d.ts';\nimport { API } from './api';\n", 'api.ts': "export const API = 'http://localhost:3000/v1/';\n", 'server/server.ts': 'const DB = Bun.env.DB;\n' });
    assert.deepEqual([serverUrl(dir), appServer(dir).port, appServer(dir).refusal], ['http://localhost:3000/v1', 3000, null]);
    put({ 'app.ts': "const API = 'http://127.0.0.1:8787';\n", 'server.ts': "new Database('data.db');\n" });
    assert.match(appServer(dir).refusal, /^server\.ts does not read its database's path from DB, so it cannot be run on an empty one: write new Database\(process\.env\.DB \?\? 'data\.db'\) in it .*"commands": \{ "server": \["bun", "server\/index\.ts"\] \}$/);
    assert.equal(appServer(dir).why, null);
    // The entry app.json names is the one it looked at, and says so.
    put({ 'app.ts': "const API = 'http://127.0.0.1:8787';\n", 'server.ts': reads, 'srv/main.ts': 'Bun.serve({});\n', 'app.json': JSON.stringify({ commands: { server: ['bun', 'srv/main.ts'] } }) });
    assert.match(appServer(dir).refusal, /^srv\/main\.ts \(app\.json's commands\.server\) does not read /);
    put({ 'app.ts': "const API = 'https://api.example.com';\n", 'server.ts': reads });
    assert.match(appServer(dir).why, /^server\.ts is left alone: app\.ts calls no local URL/);
    put({ 'app.ts': "const API = 'http://127.0.0.1:8787';\n" });
    assert.equal(appServer(dir), null);
    // The spellings of reading DB, and a root shim that only imports the backend that reads it (book-club, polls, event-rsvp).
    for (const spelling of ['Bun.env.DB', 'process.env["DB"]', "process.env['DB'] ?? 'x.db'", 'env.DB', 'process.env?.DB', "const { DB } = process.env;", "const { DB: path = 'x.db', PORT } = Bun.env;", 'start(process.env.DB)']) {
      put({ 'app.ts': "const API = 'http://127.0.0.1:8787';\n", 'server.ts': `${spelling}\n` });
      assert.equal(appServer(dir).refusal, null, spelling);
    }
    for (const reader of ["process.env.DB_PATH", 'const DB = "data.db"', '// the DB is data.db']) {
      put({ 'app.ts': "const API = 'http://127.0.0.1:8787';\n", 'server.ts': `${reader}\n` });
      assert.match(appServer(dir).refusal, /^server\.ts does not read/, reader);
    }
    put({ 'app.ts': "const API = 'http://127.0.0.1:8787';\n", 'server.ts': "import './server/index.ts';\n", 'server/index.ts': "import { serve } from './polls';\nserve();\n", 'server/polls.ts': reads });
    assert.equal(appServer(dir).refusal, null);
    put({ 'app.ts': "const API = 'http://127.0.0.1:8787';\n", 'server.ts': "export * from './a';\n", 'a.ts': "import('./b');\n", 'b.ts': "import './a';\nconst x = require('./c');\n", 'c.js': 'new Database("data.db");\n' });
    assert.match(appServer(dir).refusal, /^server\.ts does not read/); // the cycle ends
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
    const port = onPort(dir); // a port of its own: other runs on this machine may hold 8787
    const { Database } = await import('bun:sqlite');
    const theirs = new Database(resolve(dir, 'data.db'));
    theirs.run('CREATE TABLE items (id INTEGER PRIMARY KEY, title TEXT NOT NULL, notes TEXT NOT NULL, by TEXT NOT NULL, locked INTEGER NOT NULL DEFAULT 0)');
    theirs.run("INSERT INTO items (title, notes, by) VALUES ('Theirs', '', 'Ada')");
    theirs.close();
    const before = readFileSync(resolve(dir, 'data.db'));
    const verify = () => { const r = spawnSync(process.execPath, [resolve(dir, 'exact.mjs'), 'verify'], { cwd: dir, encoding: 'utf8', timeout: 240_000 }); return { ...r, lines: r.stdout.trim().split('\n') }; };
    const clean = verify();
    assert.equal(clean.status, 0, clean.stdout + clean.stderr);
    assert.equal(clean.lines[0], `server: bun server.ts at http://127.0.0.1:${port}, on an empty database (.exact/verify/db.sqlite, removed after)`, clean.stdout);
    assert.ok(clean.lines.includes('tests: 1 passed, 0 failed'), clean.stdout);
    assert.match(clean.stdout, /^relaunch on .*: the first screen shows what it stored: /m);
    assert.ok(readFileSync(resolve(dir, 'data.db')).equals(before), 'the builder\'s data.db is as it was');
    assert.deepEqual(['data.db-wal', '.exact/verify/db.sqlite'].filter((f) => existsSync(resolve(dir, f))), []);
    // LLP 1116.007 D2: `test` runs the same server on the same terms (it was the builder's to start, and `api-test`'s to know),
    // and an older app's api-test, whose test.mjs runs `test` under `withAppServer`, still passes on the one server it started.
    const plain = (verb = 'test') => spawnSync(process.execPath, [resolve(dir, 'exact.mjs'), verb, 'web'], { cwd: dir, encoding: 'utf8', timeout: 240_000 });
    const tested = plain();
    assert.equal(tested.status, 0, tested.stdout + tested.stderr);
    assert.match(tested.stderr, new RegExp(`^test: bun server\\.ts at http://127\\.0\\.0\\.1:${port}, on an empty database \\(\\.exact/test/db\\.sqlite, removed after\\)$`, 'm'));
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
      for (let i = 0; !(await fetch(`http://127.0.0.1:${port}/health`).then((r) => r.ok, () => false)); i++) { assert.ok(i < 100, 'the dev server answers'); await Bun.sleep(100); }
      const refused = verify();
      assert.equal(refused.status, 1, refused.stdout + refused.stderr);
      assert.deepEqual([refused.stdout, refused.stderr.trim().split('\n').length], ['', 1], refused.stdout + refused.stderr); // one line, on stderr
      assert.match(refused.stderr, new RegExp(`^verify: http://127\\.0\\.0\\.1:${port} already answers \\(pid ${dev.pid}: .*server\\.ts\\), and the tests would write into that server's database: stop it \\(kill ${dev.pid}\\) and run this again, which starts server\\.ts itself on an empty one$`, 'm'));
      assert.equal(dev.exitCode, null, 'the dev server runs on');
      const held = plain();
      assert.equal(held.status, 1, held.stdout + held.stderr);
      assert.match(held.stderr, new RegExp(`^test: http://127\\.0\\.0\\.1:${port} already answers \\(pid ${dev.pid}: .*server\\.ts\\)`, 'm'));
      assert.equal(dev.exitCode, null, 'the dev server runs on');
    } finally { dev.kill(); await dev.exited; }
    // LLP 1116.008 D1: a server.ts that hardcodes its database stops both before anything builds or runs: one line on
    // stderr naming the entry and the fix, a non-zero exit, no output, and no server started.
    const server = readFileSync(resolve(dir, 'server.ts'), 'utf8');
    writeFileSync(resolve(dir, 'server.ts'), server.replace("process.env.DB ?? 'data.db'", "'data.db'"));
    const line = /^(test|verify): server\.ts does not read its database's path from DB, so it cannot be run on an empty one: write new Database\(process\.env\.DB \?\? 'data\.db'\) in it .*app\.json: "commands": \{ "server": \["bun", "server\/index\.ts"\] \}\n$/;
    for (const r of [plain(), verify()]) {
      assert.equal(r.status, 1, r.stdout + r.stderr);
      assert.deepEqual([r.stdout, line.test(r.stderr)], ['', true], r.stdout + r.stderr);
    }
    assert.deepEqual(['.exact/test/db.sqlite', '.exact/verify/db.sqlite'].filter((f) => existsSync(resolve(dir, f))), []); // nothing started
  } finally { rmSync(parent, { recursive: true, force: true }); }
}, 300_000);

// LLP 1116.008 D1: `agent` starts the app's server for a drive when nothing answers where app.ts calls it, on a database of
// its own, and the last of several overlapping drives stops it; a server the builder runs is used and left running.
test('agent\'s server: started on a database of its own, shared by overlapping drives, stopped by the last, the builder\'s left alone', async () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-drive-server-'));
  const probe = Bun.serve({ port: 0, fetch: () => new Response('') }), port = probe.port;
  probe.stop(true);
  const url = `http://127.0.0.1:${port}`, up = () => fetch(`${url}/health`).then((r) => r.ok, () => false);
  const serving = "import { Database } from 'bun:sqlite';\nconst db = new Database(process.env.DB!);\nBun.serve({ hostname: '127.0.0.1', port: Number(process.env.PORT ?? " + port + "), routes: { '/health': () => new Response('ok') } });\n";
  // One drive, as agent.mjs makes it: its own process, ending when its stdin does.
  const script = `const { driveServer } = await import(${JSON.stringify(resolve(import.meta.dir, 'verify.mjs'))});
    const served = await driveServer(process.argv[1], process.argv[2] === '-' ? undefined : process.argv[2]);
    console.log('up');
    for await (const line of console) break;
    await served?.stop();
    console.log('stopped');`;
  const drive = (storage) => {
    const child = Bun.spawn([process.execPath, '-e', script, dir, storage], { stdin: 'pipe', stdout: 'pipe', stderr: 'pipe' }), lines = child.stdout.values()[Symbol.asyncIterator]();
    const next = async () => new TextDecoder().decode((await lines.next()).value).trim();
    return { child, next, said: () => new Response(child.stderr).text(), end: async () => { child.stdin.write('\n'); await child.stdin.flush(); return next(); } };
  };
  try {
    mkdirSync(dir, { recursive: true });
    writeFileSync(resolve(dir, 'app.ts'), `export const API = '${url}';\n`);
    // Without a server entry there is nothing to start; one that does not read DB is refused, as test refuses it.
    const { driveServer } = await import('./verify.mjs');
    assert.equal(await driveServer(dir, 's'), null);
    writeFileSync(resolve(dir, 'server.ts'), "Bun.serve({ port: 1 });\n");
    await assert.rejects(driveServer(dir, 's'), /^Error: agent: server\.ts does not read its database's path from DB/);
    writeFileSync(resolve(dir, 'server.ts'), serving);
    assert.deepEqual([await up(), existsSync(resolve(dir, '.exact/agent-server'))], [false, false]);

    // Two drives that overlap: the first starts it on its storage's database, the second uses it and says whose it is.
    const a = drive('one');
    assert.equal(await a.next(), 'up');
    const b = drive('two');
    assert.equal(await b.next(), 'up');
    assert.equal(await up(), true);
    assert.equal(await a.end(), 'stopped');
    await Bun.sleep(300);
    assert.equal(await up(), true, 'the server runs on for the drive still using it');
    assert.equal(await b.end(), 'stopped');
    await Bun.sleep(100);
    assert.equal(await up(), false, 'the last drive out stops it');
    assert.equal(await a.said(), `agent: started server.ts at ${url} for this drive (database .exact/agent/one/db.sqlite, kept for --storage one); stopped after\n`);
    assert.equal(await b.said(), `agent: server.ts at ${url} is another drive's, on its database (.exact/agent/one/db.sqlite); this drive uses it\n`);
    assert.deepEqual(['.exact/agent/one/db.sqlite', '.exact/agent-server'].map((f) => existsSync(resolve(dir, f))), [true, false]);

    // Two that start together: one starts it, the other waits and uses it.
    const [d, e] = [drive('four'), drive('five')];
    assert.deepEqual([await d.next(), await e.next(), await up()], ['up', 'up', true]);
    assert.equal(await d.end(), 'stopped');
    assert.equal(await e.end(), 'stopped');
    await Bun.sleep(100);
    const said = [await d.said(), await e.said()];
    assert.deepEqual([await up(), said.filter((l) => l.includes('started server.ts')).length, said.filter((l) => l.includes('is another drive\'s')).length], [false, 1, 1], said.join());

    // Without a storage name: an empty database, gone after.
    const c = drive('-');
    assert.equal(await c.next(), 'up');
    assert.equal(existsSync(resolve(dir, '.exact/agent-server/db.sqlite')), true);
    assert.equal(await c.end(), 'stopped');
    assert.match(await c.said(), /^agent: started server\.ts at \S+ for this drive \(an empty database\); stopped after\n$/);
    assert.deepEqual([await up(), existsSync(resolve(dir, '.exact/agent-server'))], [false, false]);

    // The builder's own server answers: used, said nothing of, and still running after.
    const theirs = Bun.spawn([process.execPath, 'server.ts'], { cwd: dir, env: { ...process.env, DB: resolve(dir, 'theirs.db') }, stdout: 'ignore', stderr: 'ignore' });
    try {
      for (let i = 0; !(await up()); i++) { assert.ok(i < 100, 'their server answers'); await Bun.sleep(100); }
      const d = drive('three');
      assert.equal(await d.next(), 'up');
      assert.equal(await d.end(), 'stopped');
      assert.equal(await d.said(), '');
      assert.deepEqual([await up(), theirs.exitCode], [true, null]);
    } finally { theirs.kill(); await theirs.exited; }
  } finally { rmSync(dir, { recursive: true, force: true }); }
}, 60_000);

// The same on a real drive, in Chrome: the rest pattern's server started where no one runs it, the data one drive wrote
// there for the next on the same --storage, and nothing left running.
test.skipIf(!!chromium().unavailable)('agent on a server app: its server for the drive, on the storage\'s own database', async () => {
  const parent = mkdtempSync(resolve(tmpdir(), 'exact-agent-server-'));
  try {
    const dir = resolve(parent, 'shared-list');
    createApp(dir);
    applyPattern(dir, 'rest');
    const port = onPort(dir), url = `http://127.0.0.1:${port}`;
    const agent = (...args) => spawnSync(process.execPath, [resolve(dir, 'exact.mjs'), 'agent', 'web', ...args], { cwd: dir, encoding: 'utf8', timeout: 240_000 });
    const started = (storage) => new RegExp(`^agent: started server\\.ts at ${url} for this drive \\(database \\.exact/agent/${storage}/db\\.sqlite, kept for --storage ${storage}\\); stopped after$`, 'm');
    const added = agent('--storage', 's1', 'clock data', 'tap add-item', 'clock settle', 'type add-title Milk', 'tap add-save', 'clock data', 'state lib.items.0.title');
    assert.equal(added.status, 0, added.stdout + added.stderr);
    assert.match(added.stderr, started('s1'));
    assert.match(added.stdout, /^"Milk"$/m);
    assert.equal(await fetch(`${url}/health`).then((r) => r.ok, () => false), false, 'stopped after');
    assert.ok(existsSync(resolve(dir, '.exact/agent/s1/db.sqlite')) && !existsSync(resolve(dir, '.exact/agent-server')));
    // The next drive on the name sees it; another name, and none, do not.
    const again = agent('--storage', 's1', 'clock data', 'state lib.items.0.title');
    assert.match(again.stdout, /^"Milk"$/m, again.stdout + again.stderr);
    for (const flags of [['--storage', 's2'], []]) {
      const other = agent(...flags, 'clock data', 'state lib.items');
      assert.match(other.stdout, /^\[\]$/m, other.stdout + other.stderr);
    }
    // A server the builder runs is used, and runs on.
    const theirs = Bun.spawn([process.execPath, 'server.ts'], { cwd: dir, stdout: 'ignore', stderr: 'ignore' });
    try {
      for (let i = 0; !(await fetch(`${url}/health`).then((r) => r.ok, () => false)); i++) { assert.ok(i < 100, 'their server answers'); await Bun.sleep(100); }
      const used = agent('--storage', 's3', 'clock data', 'state lib.items');
      assert.equal(used.status, 0, used.stdout + used.stderr);
      assert.doesNotMatch(used.stderr, /^agent: /m);
      assert.equal(theirs.exitCode, null, 'their server runs on');
    } finally { theirs.kill(); await theirs.exited; rmSync(resolve(dir, 'data.db'), { force: true }); }
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
    const clean = verify('--shots', 'out/shots');
    assert.equal(clean.status, 0, clean.stdout + clean.stderr);
    assert.ok(clean.lines.length <= 8, clean.stdout);
    assert.ok(!clean.lines.some((l) => l.startsWith('compiler:')), clean.stdout);
    assert.ok(clean.lines.includes('tests: 2 passed, 0 failed'), clean.stdout);
    assert.ok(clean.lines.includes('check first screen, 2 test ends, relaunch · 390x844, 1280x900 · light, dark: no findings'), clean.stdout);
    // The starter keeps nothing: the relaunch on the second test's store opens as a fresh launch does, and says so.
    assert.match(clean.stdout, /^relaunch on "the header's button likes it"'s store \(\d+\.\d s of real time\): the first screen is a fresh launch's; that test wrote no storage$/m);
    assert.match(clean.lines.at(-1), /^verify: clean in \d+\.\d s$/);
    assert.equal(clean.lines.at(-2), 'open it: bun exact.mjs web (serves what verify checked)'); // how a person opens it, with no dev server of the builder's own
    for (const [w, h] of SIZES) for (const scheme of ['light', 'dark']) assert.ok(existsSync(resolve(dir, `.exact/verify/web-${w}x${h}-${scheme}.png`)), `${w}x${h} ${scheme}`);
    for (const f of ['web-t1-the-greeting-loads.png', 'web-t2-the-header-s-button-likes-it.png', 'web-relaunch.png']) assert.ok(existsSync(resolve(dir, '.exact/verify', f)), f);
    // `--shots out/shots` made the directory and copied every one, under the names verify keeps (LLP 1116.008 D2).
    const kept = readdirSync(resolve(dir, '.exact/verify')).filter((f) => f.endsWith('.png')).sort();
    assert.equal(kept.length, 7);
    assert.deepEqual(readdirSync(resolve(dir, 'out/shots')).sort(), kept);
    assert.ok(readFileSync(resolve(dir, 'out/shots/web-relaunch.png')).equals(readFileSync(resolve(dir, '.exact/verify/web-relaunch.png'))));
    assert.match(clean.stdout, /^screenshots \(\.exact\/verify, copied to out\/shots\): /m);
    const bare = verify('--shots');
    assert.deepEqual([bare.status, bare.stdout, bare.stderr], [2, '', 'verify: --shots takes a directory: the screenshots are copied there, under the names in .exact/verify/\n']);

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

    // A test's own `screenshot` step joins --shots, named by the test (LLP 1116.008 D2).
    writeFileSync(tests, authored.replace('  expect state liked == true', '  expect state liked == true\n  screenshot "own/after-like.png"'));
    const own = verify('--shots', 'out/shots');
    assert.equal(own.status, 0, own.stdout + own.stderr);
    assert.ok(existsSync(resolve(dir, 'out/shots/web-t2-after-like.png')) && existsSync(resolve(dir, 'own/after-like.png')));
    assert.match(own.stdout, /; its own: web-t2-after-like\.png$/m);
    writeFileSync(tests, authored);

    const json = verify('--json');
    assert.equal(json.status, 0, json.stderr);
    const r = JSON.parse(json.stdout);
    assert.deepEqual([r.host, r.built, r.tests.passed, r.tests.failed, r.problems, r.exit, r.screenshots.length, r.compiler.length], ['web', null, 2, 0, 2, 0, 7, 1]);
    // Each screen at each width and scheme: the first screen, the two test ends (in their own sessions), the relaunch.
    const onScreen = (screen) => r.checks.filter((c) => c.screen === screen).map((c) => `${c.size.join('x')} ${c.scheme} ${c.findings.map((f) => f.kind).join(',')}`);
    for (const screen of ['first screen', 'the greeting loads', "the header's button likes it", 'relaunch']) assert.deepEqual(onScreen(screen), ['390x844 light contrast', '390x844 dark ', '1280x900 light contrast', '1280x900 dark '], screen);
    assert.deepEqual([r.relaunch.test, r.relaunch.problems, r.tests.results.map((t) => t.shot.split('/').at(-1))], ["the header's button likes it", [], ['web-t1-the-greeting-loads.png', 'web-t2-the-header-s-button-likes-it.png']]);
    assert.deepEqual(r.reloads.map((x) => [x.ran, x.finding]), [[true, null], [true, null]]); // each end screen reloaded: the same

    // A call the host refuses is the report's first line (LLP 1116.010 D5), and a count kept only in memory is lost by a
    // reload: the page's URL and storage hold nothing of it (D6). The starter's greeting source goes with its view.
    const appId = readFileSync(resolve(dir, 'app.ts'), 'utf8').match(/appId = '([^']+)'/)[1];
    writeFileSync(resolve(dir, 'app.ts'), `import type { Answer, Sources } from './app.contract.d.ts';\nexport const appId = '${appId}';\nexport const grants = '';\nconst sources: Sources = {};\nexport const answer: Answer = (name, args, store, storage, native) => (sources as any)[name](args, store, storage, native);\n`);
    writeFileSync(view, 'component App\n  state which = "nope"\n  state count = 0\n  action poke\n    showPicker(which)\n    count = count + 1\n  view\n    column padding=16\n      button press=poke testId="poke"\n        text "Poke"\n      text `Pokes: ${count}` testId="count"\n');
    writeFileSync(tests, 'test "poking counts, and asks for a picker nobody has"\n  tap "poke"\n  tap "poke"\n  clock settle\n  expect text "count" == "Pokes: 2"\n');
    const poked = verify();
    assert.equal(poked.status, 0, poked.stdout + poked.stderr);
    assert.equal(poked.lines[0], "refused at run time: 1 (the host's journal, 'logs'; each is a call or a wiring the app got wrong)");
    assert.match(poked.lines[1], /^ {2}picker: refused: no file input with id "nope" \(test "poking counts, and asks for/);
    // LLP 1116.011 D5: the count lived in memory by design, so its reload is a note, shown and not counted.
    assert.match(poked.stdout, /^reload 1 test end: no findings\nreload note: 1 screen reloads into another state \(not counted\)\n {2}note {5}\/ reloads into another state: "Pokes: 2" → "Pokes: 0" \("poking counts[^\n]*\n {2}a saved value must survive a reload; a game or a draft in progress may not\n/m);
    assert.match(poked.lines.at(-1), /^verify: 1 problem \(1 finding; the tests passed\) in /);
  } finally { rmSync(parent, { recursive: true, force: true }); }
}, 300_000); // A new app's offline Cargo resolution and its first web build, then four runs.

// The euxy port (friction item 12): a faithful port's palette and keys are its design. app.json `verify.accept` names a
// finding by its kind and its node's testId (or its control's, for a label; `*` a prefix), and verify says those apart.
test('accepted findings: matched by kind and testId, said once as accepted, never counted; an entry that took nothing is named', async () => {
  const { accepts } = await import('./agent-check.mjs');
  const nodes = new Map([[234, { id: 234, props: { testId: 'mute-0' } }], [236, { id: 236, props: {} }], [300, { id: 300, props: { testId: 'pattern' } }]]);
  const entries = [{ finding: 'contrast', node: 'mute-*', why: "the original's dim letters" }, { finding: 'target', node: 'pattern', why: 'the reference trigger' }];
  assert.equal(accepts(entries, { kind: 'contrast', id: 236, of: 234 }, nodes), entries[0], "a label, by its control's testId");
  assert.equal(accepts(entries, { kind: 'target', id: 234 }, nodes), undefined, 'another kind on that node is still a finding');
  assert.equal(accepts(entries, { kind: 'target', id: 300 }, nodes), entries[1]);
  assert.equal(accepts(entries, { kind: 'target', id: 236 }, nodes), undefined, 'a node with no testId is never accepted');
  // A bare `*` (or a `*` inside) would accept a kind on every node: it matches nothing, and the manifest refuses it.
  for (const node of ['*', 'mute*0', 'mute-**']) assert.equal(accepts([{ finding: 'contrast', node, why: 'x' }], { kind: 'contrast', id: 236, of: 234 }, nodes), undefined, node);
  const { validate } = await import('./app.mjs'), schema = JSON.parse(readFileSync(resolve(import.meta.dir, 'app.schema.json'), 'utf8'));
  const problems = (node) => validate({ accept: [{ finding: 'contrast', node, why: 'x' }] }, schema.properties.verify, 'verify', schema);
  assert.deepEqual([problems('mute-*'), problems('mute-0')], [[], []]);
  for (const node of ['*', 'mute*0', 'mute-**']) assert.equal(problems(node).length, 1, node);
  const line = (x) => `  contrast mute-0's label "M" [${x},127 13×18]: 1.8:1 (#b4b4ba on #f6f4f4), needs 4.5:1; accepted: the original's dim letters`;
  const checks = runs(() => []).map((r) => ({ ...r, accepted: [{ line: line(r.size[0] - 73), accept: entries[0] }] }));
  const r = report({ host: 'ios', built: null, tested: { passed: 1, failed: 0, results: [] }, checks, shots: [], accept: [...entries, { finding: 'clipped', node: 'gone', why: 'x' }], ms: 1000 });
  assert.deepEqual(r.lines.slice(1, 6), [
    'check first screen · 390x844, 1280x900 · light, dark: no findings',
    'accepted (app.json verify.accept, not counted): 1 finding',
    line(317),
    '  accepts nothing: target pattern (no such finding in this run)',
    '  accepts nothing: clipped gone (no such finding in this run)',
  ]);
  assert.equal(r.lines.at(-1), 'verify: clean in 1.0 s');
});

test('a finding on a view only one scheme shows is accepted by the testId that scheme\'s tree gives it', async () => {
  const { settle } = await import('./agent-check.mjs');
  const finding = { kind: 'contrast', id: 7, text: 'Night', box: [0, 0, 40, 18], detail: { got: 1.2, need: 4.5, fg: '#222222', bg: '#000000' } };
  const runs = [{ scheme: 'light', target: 44, findings: [] }, { scheme: 'dark', target: 44, findings: [finding] }];
  const trees = [new Map(), new Map([[7, { id: 7, type: 'Text', props: { testId: 'night-badge', text: 'Night' } }]])];
  const r = settle(runs, trees, [{ finding: 'contrast', node: 'night-badge', why: 'the dark badge' }], 'ios');
  assert.deepEqual(r.check.map((run) => [run.findings.length, run.accepted.length]), [[0, 0], [0, 1]]);
  assert.equal(r.check[1].accepted[0].node, 'night-badge');
  assert.match(r.text, /check ios · dark \(1 accepted\): no findings/);
});
