// scripts/stage12-gate.mjs: the command line, a screen's ops, the pixel
// diff and its allowlist, and that every line says WAITS-ON before its parts.
import { expect, test } from 'bun:test';
import { encodePng } from './png.mjs';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { allowOf, beforeDrift, flakyOf, durationMs, slotCheckTimes, conformFailures, conformRead, conformReport, conformSame, renderedIds, renderedLibrary, viewportTop, BASELINE_TAG, comparePixels, expected, lines, releaseIdentity, parse, pixelDiff, RESTYLE, SCREENS, screenOps, WEB_NOISE } from './stage12-gate.mjs';

test('the command line', () => {
  expect(parse(['baseline', '--copy', '/c'])).toBe('baseline needs --copy and --out');
  expect(parse(['baseline', '--copy', '/c', '--out', '/o', '--host', 'tvos'])).toMatch(/^--host tvos/);
  expect(parse(['baseline', '--copy', '/c', '--out', '/o', '--host', 'terminal'])).toMatch(/^--host terminal/);
  expect(parse(['--host', 'terminal']).hosts).toEqual(['terminal']);
  expect(parse(['--only', 'nope'])).toMatch(/^--only nope/);
  expect(parse(['--bogus', 'x'])).toBe('unknown argument --bogus');
  const b = parse(['baseline', '--copy', '/c', '--out', '/o', '--host', 'web']);
  expect([b.mode, b.copy, b.out, b.hosts]).toEqual(['baseline', '/c', '/o', ['web']]);
});

test('a screen is reached by its taps, each settled, the scheme first', () => {
  for (const [name, taps] of SCREENS) {
    const ops = screenOps(name, 'dark', '/s.png');
    expect(ops.slice(0, 3)).toEqual(['clock +1000 real', 'clock settle', 'prefer prefers-color-scheme dark']);
    expect(ops.slice(ops.indexOf('screenshot /s.png'))).toEqual(['screenshot /s.png', 'logs', 'layout', ...(name === 'composer' ? ['layout compose-text'] : [])]);
    for (const t of taps) expect(ops[ops.indexOf(t) + 1]).toBe('clock settle');
  }
});

test('the pixel diff counts what differs outside the allowlist', () => {
  const img = (w, h, f) => { const data = new Uint8Array(w * h * 4); for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) data.set(f(x, y), (y * w + x) * 4); return encodePng({ width: w, height: h, data }); };
  const a = img(4, 4, () => [0, 0, 0, 255]);
  const b = img(4, 4, (x, y) => (x === 1 && y === 1) || (x === 3 && y === 0) ? [255, 0, 0, 255] : [0, 0, 0, 255]);
  expect(pixelDiff(a, a)).toEqual({ differ: 0, total: 16 });
  expect(pixelDiff(a, b)).toEqual({ differ: 2, total: 16 });
  expect(pixelDiff(a, b, [[1, 1, 1, 1]])).toEqual({ differ: 1, total: 16 });
  expect(pixelDiff(a, img(4, 5, () => [0, 0, 0, 255])).differ).toBe(16);
  // The web's noise: up to its floor on an anti-aliased edge pixel, never in
  // a flat area, where one level fails.
  const flat = img(4, 4, (x, y) => x === 1 && y === 1 ? [1, 0, 0, 255] : [0, 0, 0, 255]);
  expect([pixelDiff(a, flat).differ, pixelDiff(a, flat, [], WEB_NOISE).differ]).toEqual([1, 1]);
  const edgy = img(4, 4, (x) => x < 2 ? [0, 0, 0, 255] : [200, 200, 200, 255]);
  const edgy2 = img(4, 4, (x, y) => x === 1 && y === 1 ? [2, 2, 0, 255] : x < 2 ? [0, 0, 0, 255] : [200, 200, 200, 255]);
  const edgy3 = img(4, 4, (x, y) => x === 1 && y === 1 ? [3, 0, 0, 255] : x < 2 ? [0, 0, 0, 255] : [200, 200, 200, 255]);
  expect([pixelDiff(edgy, edgy2).differ, pixelDiff(edgy, edgy2, [], WEB_NOISE).differ, pixelDiff(edgy, edgy3, [], WEB_NOISE).differ]).toEqual([1, 0, 1]);
});

test('a line whose parts are missing waits on them', async () => {
  const waiting = ['consumer-pixels'];
  for (const [id, , check] of lines({ baseline: null, hosts: [] }).filter(([id]) => waiting.includes(id))) expect([id, (await check()).status]).toEqual([id, expect.stringMatching(/^WAITS-ON /)]);
});

test('the composer\'s caret strip comes from its field\'s layout', () => {
  const out = ['screenshot x', JSON.stringify({ node: { testId: 'compose-text', space: { viewport: { x: 70, y: 75, w: 334, h: 128 }, capture: { scale: 1 } } } })].join('\n');
  expect(allowOf('composer', out)).toEqual([{ rect: [70, 75, 4, 40], reason: expect.stringContaining('caret') }]);
  expect(allowOf('composer', 'no layout')).toBe(null);
  // With the system keyboard up, its rectangle too, from the layout's inset.
  const kb = JSON.stringify({ viewport: { w: 402, h: 546 }, env: { 'keyboard-inset-height': 328 }, node: { testId: 'compose-text', space: { viewport: { x: 70, y: 124, w: 320, h: 120 } } } });
  expect(allowOf('composer', kb).map(r => r.rect)).toEqual([[70, 124, 4, 40], [0, 546, 402, 328]]);
  expect(allowOf('discover', '')).toEqual([]);
  expect(screenOps('composer', 'light', '/s.png').at(-1)).toBe('layout compose-text');
});

test('before and after compare shot for shot, strict, with both allowlists', () => {
  const dir = mkdtempSync(join(tmpdir(), 's12e-'));
  const img = (name, f) => { const data = new Uint8Array(4 * 4 * 4); for (let i = 0; i < 16; i++) data.set(f(i % 4, Math.floor(i / 4)), i * 4); const p = join(dir, name); writeFileSync(p, encodePng({ width: 4, height: 4, data })); return p; };
  const black = () => [0, 0, 0, 255];
  const before = { shots: [{ host: 'web', scheme: 'light', screen: 'discover', ok: true, png: img('b1.png', black), allow: [] },
    { host: 'web', scheme: 'light', screen: 'composer', ok: true, png: img('b2.png', black), allow: [{ rect: [0, 0, 1, 4], reason: 'caret' }] }] };
  const after = { shots: [{ host: 'web', scheme: 'light', screen: 'discover', ok: true, png: img('a1.png', (x, y) => x === 2 && y === 2 ? [0, 0, 1, 255] : [0, 0, 0, 255]), allow: [] },
    { host: 'web', scheme: 'light', screen: 'composer', ok: true, png: img('a2.png', (x) => x === 0 ? [255, 0, 0, 255] : [0, 0, 0, 255]), allow: [] }] };
  const r = comparePixels(before, after);
  expect(r.map(x => [x.shot, x.ok, x.differ])).toEqual([['web light discover', false, 1], ['web light composer', true, 0]]);
  expect(comparePixels(before, { shots: [after.shots[1]] })[0]).toMatchObject({ ok: false, said: 'no after shot' });
});

test('the before differs from the baseline\'s tag only in its lock', () => {
  const dir = mkdtempSync(join(tmpdir(), 's12e-git-'));
  const git = (...a) => spawnSync('git', a, { cwd: dir, encoding: 'utf8' });
  git('init', '-q'); git('config', 'user.email', 't@t'); git('config', 'user.name', 't');
  writeFileSync(join(dir, 'Cargo.lock'), 'a'); writeFileSync(join(dir, 'app.contract'), 'a');
  git('add', '-A'); git('commit', '-qm', 'base'); git('tag', BASELINE_TAG);
  writeFileSync(join(dir, 'Cargo.lock'), 'b'); git('commit', '-qam', 'lock');
  expect(beforeDrift(dir)).toEqual([]);
  writeFileSync(join(dir, 'app.contract'), 'b'); git('commit', '-qam', 'more');
  expect(beforeDrift(dir)).toEqual(['app.contract']);
});

test('a shot with bars above its viewport: the top mark, not the markers, says where the viewport starts', () => {
  // A 10x14 shot of a 10x10 viewport, 3 rows of bars on top; the mark is
  // laid out at (2, 1), 4x3, so it shows from row 4.
  const img = (mark) => {
    const data = new Uint8Array(10 * 14 * 4).fill(255);
    if (mark) for (let y = 4; y < 7; y++) for (let x = 2; x < 6; x++) data.set([255, 0, 255, 255], (y * 10 + x) * 4);
    return { width: 10, height: 14, data };
  };
  const layout = { viewport: { w: 10, h: 10 }, nodes: [{ testId: 'gate-top', x: 2, y: 1, w: 4, h: 3 }] };
  expect(viewportTop(img(true), layout)).toBe(3);
  expect(viewportTop(img(true), { ...layout, viewport: { w: 10, h: 14 } })).toBe(0);
  expect(() => viewportTop(img(false), layout)).toThrow(/top mark/);
});

test('the restyle line expects the value, or its change from the base drive, in whole cells on the terminal', () => {
  const is = { is: true, axis: 'w' }, twice = { twice: true, axis: 'w' }, once = { once: true, axis: 'h' };
  expect(expected(is, 24, 12, 12, null)).toBe(24);
  expect(expected(twice, 24, 12, 80, null)).toBe(104);
  expect(expected(once, 24, 12, 50, null)).toBe(62);
  // A cell 8 wide and 16 tall: 24 is 3 columns, 12 one (a half rounds
  // down), and 20 and 12 are both one row.
  expect(expected(is, 24, 12, 16, { w: 8, h: 16 })).toBe(24);
  expect(expected(twice, 24, 12, 80, { w: 8, h: 16 })).toBe(112);
  expect(expected(once, 20, 12, 32, { w: 8, h: 16 })).toBe(32);
  // An instance reading another token stays where it was.
  expect(expected({ same: true, axis: 'w' }, 24, 12, 8, null)).toBe(8);
  // Each case names what it sets and how it is removed.
  for (const c of RESTYLE) {
    expect(c.set).toMatch(/^\d+px$/);
    expect(['remove-token', 'unset-param']).toContain(c.unset.op);
    expect(c.reads.length).toBeGreaterThan(0);
    for (const r of c.reads) expect(r.ids.length).toBeGreaterThan(0);
  }
  expect(new Set(RESTYLE.flatMap(c => c.reads.map(r => r.who)))).toEqual(new Set(['PostCard', 'ActionRow', 'AtCard', 'PostButton']));
});

test('release identity: every source compared and identical in every run, but library-gate\'s own and the named alike failures', () => {
  const dir = mkdtempSync(join(tmpdir(), 's12e-rel-'));
  const row = (source, status, run = 'a..b') => ({ run, commits: ['Stage 12 E: x'], source, status });
  const file = rows => { const f = join(dir, `${Math.random()}.json`); writeFileSync(f, JSON.stringify(rows)); return f; };
  const sources = ['apps/a/app.contract', 'apps/expose/app.contract'];
  const id = (f, src = sources, runs = ['a..b']) => releaseIdentity(f, src, runs);
  expect(id(null).status).toBe('WAITS');
  expect(id(file([])).status).toBe('FAIL');
  expect(id(file([row('apps/a/app.contract', 'identical'), row('apps/expose/app.contract', 'both failed'), row('apps/library-gate/app.contract', 'build failed')])).status).toBe('PASS');
  const moved = id(file([row('apps/a/app.contract', 'DIFFERS'), row('apps/expose/app.contract', 'both failed')]));
  expect(moved.status).toBe('FAIL');
  expect(moved.evidence[1]).toContain('MOVED apps/a/app.contract');
  // Failing alike counts only for the named; a run that leaves a source out fails.
  expect(id(file([row('apps/a/app.contract', 'both failed'), row('apps/expose/app.contract', 'both failed')])).status).toBe('FAIL');
  const short = id(file([row('apps/a/app.contract', 'identical'), row('apps/expose/app.contract', 'both failed'), row('apps/a/app.contract', 'identical', 'c..d')]));
  expect(short.status).toBe('FAIL');
  expect(short.evidence.join('\n')).toContain('NOT COMPARED apps/expose/app.contract over c..d');
  expect(short.evidence.join('\n')).toContain('NOT A RUN OF STAGE 12 E c..d');
  // A run the history has that the file leaves out fails; so does a row with no commits.
  const ok = [row('apps/a/app.contract', 'identical'), row('apps/expose/app.contract', 'both failed')];
  expect(id(file(ok), sources, ['a..b', 'e..f']).evidence.join('\n')).toContain('RUN NOT COMPARED e..f');
  expect(id(file([{ ...ok[0], commits: [] }, ok[1]])).status).toBe('FAIL');
});

test('what a screen renders: the testIds only the packages write, from its whole layout', () => {
  const at = { x: 10, y: 10, w: 20, h: 20 };
  const out = ['screenshot x', JSON.stringify({ viewport: { w: 400, h: 800 }, nodes: [{ testId: 'post-1', ...at }, { testId: 'postcard-content-1', ...at }, { props: { testId: 'actions-1' }, ...at }, { testId: 'compose-post', ...at }, { testId: 'compose-post-2', ...at }, { testId: 'atcard-visit-3', ...at },
    // Shown nowhere: zero size, below the screen.
    { testId: 'postcard-content-2', x: 10, y: 10, w: 0, h: 0 }, { testId: 'actions-2', x: 10, y: 900, w: 20, h: 20 }] }), JSON.stringify({ node: { testId: 'compose-text' } })].join('\n');
  expect(renderedIds(out).sort()).toEqual(['actions-1', 'atcard-visit-3', 'compose-post', 'postcard-content-1']);
  expect(renderedIds('no layout')).toBe(null);
});

test('conform on the clone: the adoption fails nothing the baseline does not', () => {
  const out = 'x\nFAIL app tap a: layout b.w: wasm 1 js 2 (delta 1 px, tolerance 0.5 px)\nFAIL app clock +1: screenshot: 1.50% of pixels differ (clock-1-side-by-side.png)\n2 failures across 1 targets; r.md';
  expect(conformFailures(out)).toEqual(['FAIL app tap a: layout b.w: wasm 1 js 2 (delta 1 px, tolerance 0.5 px)', 'FAIL app clock +1: screenshot: 1.50% of pixels differ']);
  const before = { failures: conformFailures(out) };
  expect(conformSame({ before, after: { failures: before.failures.slice(1) } })).toBe(true);
  expect(conformSame({ before, after: { failures: [...before.failures, 'FAIL app new'] } })).toBe(false);
  expect(conformSame({ before: null, after: { failures: [] } })).toBe(false);
});

test('a conform run counts only when it finished, compared steps and printed every failure it counts', () => {
  const table = n => `| target | JS build | steps compared | steps equal | failures |\n|---|---|---|---|---|\n| app | ok | ${n} | 1 | 1 |`;
  const out = n => `${table(n)}\n\n1 failures across 1 targets; r.md\nFAIL app tap a: layout b.w: wasm 1 js 2`;
  expect(conformRead(out(3), 'app')).toMatchObject({ compared: 3, failures: ['FAIL app tap a: layout b.w: wasm 1 js 2'] });
  expect(conformRead(out(0), 'app')).toBe(null);
  expect(conformRead(out(3).replace(/\nFAIL.*$/, ''), 'app')).toBe(null);
  expect(conformRead(out(3), 'app', new Error('timed out'))).toBe(null);
});

test('the components a source map\'s view nodes come from, by declared name', () => {
  const map = { sources: [{ file: '/n/cards/at-card.contract', home: '@exact/cards/at-card.contract' }, { file: '/a/post.contract', home: 'post.contract' }],
    nodes: [{ file: '/n/cards/at-card.contract', component: 'AtCard__at_card' }, { file: '/n/cards/at-card.contract', component: 'AtCard' }, { file: '/a/post.contract', component: 'AtCard' }] };
  expect(renderedLibrary(map)).toEqual(['@exact/cards/at-card.contract#AtCard']);
});

test('a conform report\'s failures, keyed without the checkout\'s path', () => {
  const dir = mkdtempSync(join(tmpdir(), 's12e-conf-'));
  const f = join(dir, 'report.json');
  writeFileSync(f, JSON.stringify({ targets: { a: {}, b: {} }, failures: [{ target: 'a', step: 'contract-build', what: '/Users/x/exact2/host/t.contract:0:0 [id] m' }] }));
  expect(conformReport(f, '/Users/x/exact2')).toEqual({ targets: 2, failures: ['a | contract-build | <checkout>/host/t.contract:0:0 [id] m'] });
  // Only the checkout's own prefix: two paths under it stay apart.
  writeFileSync(f, JSON.stringify({ targets: { a: {} }, failures: [{ target: 'a', step: 's', what: '/r/exact2/host/a/exact2-one/t.contract' }, { target: 'a', step: 's', what: '/r/exact2/host/b/exact2-two/t.contract' }] }));
  expect(new Set(conformReport(f, '/r/exact2/').failures).size).toBe(2);
  // Not inside another path: /archive/r/exact2/… is no checkout path.
  writeFileSync(f, JSON.stringify({ targets: { a: {} }, failures: [{ target: 'a', step: 's', what: 'at /r/exact2/x and /archive/r/exact2/x' }] }));
  expect(conformReport(f, '/r/exact2').failures).toEqual(['a | s | at <checkout>/x and /archive/r/exact2/x']);
  expect(conformReport(join(dir, 'none.json'), '/r')).toBe(null);
});

test('the slot check\'s times, from rt1\'s tests\' lines, in milliseconds', () => {
  expect([durationMs('250ns'), durationMs('1.5µs'), durationMs('3.4ms'), durationMs('2s'), durationMs('soon')]).toEqual([0.00025, 0.0015, 3.4, 2000, NaN]);
  const out = 'x\ncheck_slots on the clone\'s design (146 tokens): one touched name 250ns, every chain 30.1µs (median of 5, warm)\ncheck_slots, the clone\'s design (146 tokens) with @exact/cards and @exact/actions (1 chains, 0 names): one touched name 291ns, every chain 1.2ms (median of 5, warm)';
  const t = slotCheckTimes(out);
  expect(t.map(x => [x.real, +x.one.toPrecision(6), +x.all.toPrecision(6)])).toEqual([[false, 0.00025, 0.0301], [true, 0.000291, 1.2]]);
});

test('a boot flake in conform: rerun alone three times, at most two, never twice running, and QUEUE\'d (theme2 03:25)', () => {
  const work = mkdtempSync(join(tmpdir(), 's12e-flaky-'));
  const base = { failures: ['synthetic-tabs | contract-build | x'] };
  const added = ['synthetic-startend | boot | state slots.atEnd: wasm true js false'];
  const queue = "- **Conformance's `startend` fixture fails now and then on the JS target under load** (…)";
  // Passes alone three times and is QUEUE'd: FLAKY, nothing failing.
  let r = flakyOf(added, base, work, () => true, queue);
  expect([r.flaky, r.failing]).toEqual([['synthetic-startend'], []]);
  expect(r.said[0]).toContain('FLAKY synthetic-startend');
  // The next run (its record says so), FLAKY again: a failure to chase.
  expect(r.seen).toEqual(['synthetic-startend']);
  writeFileSync(join(work, 'flaky-last.json'), JSON.stringify({ targets: r.seen }));
  r = flakyOf(added, base, work, () => true, queue);
  expect([r.flaky, r.failing]).toEqual([[], added]);
  // A rerun that fails is a failure; so is a target the base fails, or no QUEUE line.
  const fresh = () => mkdtempSync(join(tmpdir(), 's12e-flaky-'));
  expect(flakyOf(added, base, fresh(), (t, n) => n < 2, queue).failing).toEqual(added);
  expect(flakyOf(['synthetic-tabs | boot | y'], base, fresh(), () => true, "- **Conformance's `tabs` fixture fails now and then").failing).toEqual(['synthetic-tabs | boot | y']);
  // A QUEUE line merely mentioning the name is no entry for it.
  expect(flakyOf(added, base, fresh(), () => true, 'see `startend` above').failing).toEqual(added);
  // More than two: all fail.
  const three = ['a', 'b', 'c'].map(t => `synthetic-${t} | boot | z`);
  const q3 = ['a', 'b', 'c'].map(t => `- **Conformance's \`${t}\` fixture fails now and then`).join('\n');
  const r3 = flakyOf(three, base, fresh(), () => true, q3);
  expect([r3.failing.length, r3.seen.length]).toEqual([3, 3]);
});
