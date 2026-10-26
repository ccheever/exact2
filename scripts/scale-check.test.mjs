// scale-check.mjs's reading of a corner and a padding: a rounded box drawn
// here with supersampled coverage, then fitted back to its radius.
import { expect, test } from 'bun:test';
import { check, edges, fitRadius, parse, reduced, rgb } from './scale-check.mjs';

/** A `width`×`height` RGBA image, white, with a box of `fill` and corner
 * radius `r` (CSS-reduced) at x, y, w, h (device pixels, fractional), each
 * pixel's coverage taken from 8×8 samples. */
function drawn(width, height, { x, y, w, h }, r, fill) {
  const data = new Uint8Array(width * height * 4).fill(255);
  r = reduced(r, w, h);
  const inside = (px, py) => {
    if (px < x || py < y || px > x + w || py > y + h) return false;
    const cx = Math.min(Math.max(px, x + r), x + w - r), cy = Math.min(Math.max(py, y + r), y + h - r);
    return (px - cx) ** 2 + (py - cy) ** 2 <= r * r;
  };
  for (let py = 0; py < height; py++) for (let px = 0; px < width; px++) {
    let n = 0;
    for (let s = 0; s < 64; s++) n += inside(px + ((s % 8) + 0.5) / 8, py + (Math.floor(s / 8) + 0.5) / 8);
    const c = n / 64, i = (py * width + px) * 4;
    for (let k = 0; k < 3; k++) data[i + k] = Math.round(255 + (fill[k] - 255) * c);
  }
  return { width, height, data };
}

const red = rgb('#ff0000'), white = rgb('#ffffff');
const fit = (image, box) => fitRadius(edges(image, box, red, white), Math.max(box.w, box.h) / 2 + 1);

test('a corner is fitted to its radius within a third of a pixel, square or round, on whole or fractional edges', () => {
  for (const origin of [10, 10.25, 10.5, 10.75]) {
    for (const r of [0, 4, 8, 12, 16, 26]) {
      const box = { x: origin, y: origin + 0.3, w: 120, h: 80 };
      expect(Math.abs(fit(drawn(160, 110, box, r, red), box) - r)).toBeLessThan(0.3);
    }
  }
});

test('a pill on a large box at 3x is fitted past any fixed ceiling', () => {
  const box = { x: 3, y: 3, w: 900, h: 900 };
  expect(Math.abs(fit(drawn(906, 906, box, 9999, red), box) - 450)).toBeLessThan(1);
});

const layout = { viewport: { w: 100 }, nodes: [
  { id: 1, x: 0, y: 0, w: 100, h: 60, testId: 'padded' }, { id: 2, x: 16, y: 16, w: 20, h: 20, testId: 'inner' },
  { id: 3, x: 10, y: 20, w: 60, h: 24, testId: 'pill' }, { id: 4, x: 0, y: 70, w: 30, h: 0, testId: 'empty' },
  { id: 5, x: 40, y: 70, w: 10, h: 10, testId: 'offside' }, { id: 6, x: 56, y: 72, w: 4, h: 4, testId: 'tall' },
] };
// At 2x, as a phone's screenshot is.
const image = drawn(200, 200, { x: 20, y: 40, w: 120, h: 48 }, 9999, rgb('#0000ff'));

test('a radius over half the box is a pill, and a padding is its named child\'s offset on both axes', () => {
  const rows = check('linux', layout, image, { boxes: [
    { testId: 'padded', child: 'inner', padding: { linux: 16, default: 10 } },
    { testId: 'pill', radius: 9999, fill: '#0000ff' },
  ] });
  expect(rows.map(r => r.ok)).toEqual([true, true]);
  expect(rows[1].expected).toBe(12);
});

test('what cannot be read or is not expected fails, never passes', () => {
  const rows = check('linux', layout, image, { boxes: [
    // Wrong on one axis.
    { testId: 'offside', child: 'tall', padding: 16 },
    // No child named.
    { testId: 'padded', padding: 16 },
    // Expected for another host only.
    { testId: 'padded', child: 'inner', padding: { ios: 16 } },
    // A zero-sized box: no corner to read.
    { testId: 'empty', radius: 9999, fill: '#0000ff' },
    // No fill there.
    { testId: 'offside', radius: 4, fill: '#0000ff' },
    // The wrong radius.
    { testId: 'pill', radius: 4, fill: '#0000ff' },
  ] });
  expect(rows.map(r => r.ok)).toEqual([false, false, false, false, false, false]);
  expect(rows[2].why).toContain('no padding expected for linux');
  expect(rows[3].why).toContain('could not be read');
});

test('the command line is refused when malformed', () => {
  expect(parse(['a.contract', 'e.json', '--host'])).toContain('needs a value');
  expect(parse(['a.contract', 'e.json', '--hots', 'ios'])).toContain('unknown flag');
  expect(parse(['a.contract'])).toContain('a plan and an expectation file');
  expect(parse(['a.contract', 'e.json', '--out', '/tmp/a b'])).toContain('no spaces');
  expect(parse(['a.contract', 'e.json', '--host', 'web', '--host', 'macos']).hosts).toEqual(['web', 'macos']);
  // The reader runs an app's APK, the plan pushed to it: the app is named.
  expect(parse(['a.contract', 'e.json', '--host', 'android'])).toContain('needs --app');
  expect(parse(['a.contract', 'e.json', '--host', 'android', '--app', 'unsaid']).app).toBe('unsaid');
});

test('a corner with another colour in it (a shadow, a border) is not read', () => {
  const box = { x: 10, y: 10, w: 120, h: 80 };
  const image = drawn(160, 110, box, 12, red);
  // A dark pixel where the corner's cut-out is.
  const i = (12 * 160 + 11) * 4; image.data[i] = image.data[i + 1] = image.data[i + 2] = 40;
  expect(edges(image, box, red, white)).toBeNull();
  // A fill too close to the background to read.
  expect(edges(image, box, rgb('#fefefe'), white)).toBeNull();
});

test('a small radius on a fractional edge, a covered left edge, an unnamed child and a value that is no number', () => {
  // 1.2 px at 1x, its top 0.01 px under a pixel's: no row read shows it, so
  // it reads as square, and square is within it.
  const small = { x: 10, y: 10.01, w: 60, h: 40 };
  const rows = check('linux', { viewport: { w: 100 }, nodes: [{ id: 1, ...small, testId: 'small' }, { id: 2, x: 26, y: 26.01, w: 4, h: 4 }] },
    drawn(100, 60, small, 1.2, red), { boxes: [
      { testId: 'small', radius: 1.2, fill: '#ff0000' },
      { testId: 'small', padding: 16 },
      { testId: 'small', radius: '12px', fill: '#ff0000' },
      { testId: 'small', child: '', padding: Number.NaN },
    ] });
  expect(rows.map(r => r.ok)).toEqual([true, false, false, false]);
  // The middle row's first pixels covered by the background: not read.
  const box = { x: 10, y: 10, w: 120, h: 80 };
  const covered = drawn(160, 110, box, 12, red);
  for (let x = 10; x < 22; x++) { const i = (50 * 160 + x) * 4; covered.data[i] = covered.data[i + 1] = covered.data[i + 2] = 255; }
  expect(edges(covered, box, red, white)).toBeNull();
});

test('a length named is the design\'s own, else a base name\'s in the kernel table\'s column for the host', async () => {
  const { baseLength, overridesOf, RADIUS_FULL } = await import('./scale-check.mjs');
  const schema = JSON.parse(await Bun.file(new URL('../kernel/tables/schema.json', import.meta.url)).text());
  const none = {};
  // A metric's reference, read in the host's column (the Mac's grouped row inset is 10).
  expect([baseLength(schema, none, 'space.md', 'macos'), baseLength(schema, none, 'space.md', 'linux')]).toEqual([10, 16]);
  // A neutral-only metric row (the neutral look's control radius), every web build and Linux.
  expect([baseLength(schema, none, 'radius.md', 'web'), baseLength(schema, none, 'radius.md', 'linux')]).toEqual([8, 8]);
  // A metric row with no Material cell reads the web's first look's there.
  expect(baseLength(schema, none, 'space.sm', 'android')).toBe(8);
  // A stretch reads its metric (stage 13 D5): the Mac's space.xl is grouped-label-inset's 30.
  expect(baseLength(schema, none, 'space.xl', 'macos')).toBe(30);
  // A measured cell, a guideline's, radius.full's one length.
  expect(baseLength(schema, none, 'space.xs', 'ios')).toBe(5);
  expect(baseLength(schema, none, 'space.lg', 'android')).toBe(24);
  expect(baseLength(schema, none, 'radius.full', 'ios')).toBe(RADIUS_FULL);
  // A name neither has; a host with no column (the terminal has no agent host yet; `host` is the Mac's sample).
  expect(baseLength(schema, none, 'space.header', 'web')).toBeUndefined();
  expect(baseLength(schema, none, 'space.md', 'host')).toBeUndefined();
  // The design's own, as the compiler reads it (`contract tamber ir`), wins on every host.
  const names = schema.scale.map(r => r[0]);
  const ir = (tokens) => ({ tokens });
  const own = overridesOf(ir([
    { name: 'space.md', kind: 'length', values: ['12px'] },
    { name: 'space.header', kind: 'length', values: ['44px', '40px'] },
    { name: 'probe.fill', kind: 'color', values: ['#0000ff'] },
  ]), names);
  expect(own).toEqual({ 'space.md': 12 });
  expect(baseLength(schema, own, 'space.md', 'macos')).toBe(12);
  // A base name the design varies, or sets in another unit, is refused.
  expect(() => overridesOf(ir([{ name: 'space.md', kind: 'length', values: ['12px', '8px'] }]), names)).toThrow('not one length in px');
  expect(() => overridesOf(ir([{ name: 'radius.lg', kind: 'length', values: ['1rem'] }]), names)).toThrow('not one length in px');
});

test('a box\'s gap, height and named lengths are checked through the names given', () => {
  const layout = { viewport: { w: 100 }, nodes: [
    { id: 1, x: 0, y: 0, w: 100, h: 40, testId: 'row' }, { id: 2, x: 0, y: 0, w: 20, h: 10, testId: 'a' },
    { id: 3, x: 32, y: 0, w: 20, h: 10, testId: 'b' }, { id: 4, x: 0, y: 50, w: 20, h: 44, testId: 'header' },
  ] };
  const image = drawn(100, 100, { x: 0, y: 0, w: 1, h: 1 }, 0, red);
  const names = { 'space.md': 12, 'space.header': 44 };
  const rows = check('linux', layout, image, { boxes: [
    { testId: 'row', between: ['a', 'b'], gap: 'space.md' },
    { testId: 'header', height: 'space.header' },
    { testId: 'row', between: ['a', 'b'], gap: 'space.lg' },
    // Aligned on neither axis: refused unless its axis is named.
    { testId: 'row', between: ['b', 'header'], gap: 40 },
    { testId: 'row', between: ['b', 'header'], gap: 40, axis: 'column' },
  ] }, (name) => names[name]);
  expect(rows.map(r => r.ok)).toEqual([true, true, false, false, true]);
  expect(rows[2].why).toContain('is no number');
  expect(rows[3].why).toContain('aligned on neither axis');
});

test('a screenshot taller than the page (the reader\'s, with its bars) is read from where the page begins', async () => {
  const { pageTop } = await import('./scale-check.mjs');
  // The page at 2x under a 30 px bar; a fill-coloured stroke (a focus ring)
  // above the topmost box, and a second box below it in the same column.
  const layout = { viewport: { w: 100, h: 100 }, nodes: [
    { id: 1, x: 10, y: 20, w: 60, h: 24, testId: 'a' }, { id: 2, x: 10, y: 60, w: 60, h: 24, testId: 'b' },
  ] };
  const image = drawn(200, 260, { x: 20, y: 70, w: 120, h: 48 }, 0, red);
  const second = drawn(200, 260, { x: 20, y: 150, w: 120, h: 48 }, 0, red);
  for (let i = 150 * 200 * 4; i < 200 * 200 * 4; i++) image.data[i] = second.data[i];
  for (let x = 0; x < 200; x++) { const i = (40 * 200 + x) * 4; image.data[i] = 255; image.data[i + 1] = image.data[i + 2] = 0; }
  const boxes = [{ testId: 'b', fill: '#ff0000' }, { testId: 'a', fill: '#ff0000' }];
  expect(Math.abs(pageTop(image, layout, { boxes }, 2) - 30)).toBeLessThan(0.05);
  // A page whose top falls inside a pixel: read to the fraction.
  const frac = drawn(200, 260, { x: 20, y: 70.4, w: 120, h: 48 }, 0, red);
  expect(Math.abs(pageTop(frac, layout, { boxes: [{ testId: 'a', fill: '#ff0000' }] }, 2) - 30.4)).toBeLessThan(0.05);
  // A page offset across (a side bar on the left), or narrowed (one on the
  // right, which makes the width's pixels a point wrong): refused.
  const side = drawn(200, 260, { x: 26, y: 70, w: 120, h: 48 }, 0, red);
  expect(() => pageTop(side, layout, { boxes: [{ testId: 'a', fill: '#ff0000' }] }, 2)).toThrow("not the screenshot's width");
  // (Its left edge where the layout puts it, its right a few pixels short, inside a pixel.)
  const narrow = drawn(200, 260, { x: 20, y: 70, w: 113.6, h: 48 }, 0, red);
  expect(() => pageTop(narrow, layout, { boxes: [{ testId: 'a', fill: '#ff0000' }] }, 2)).toThrow("not the screenshot's width");
  // A screenshot that is the page: no offset.
  expect(pageTop(drawn(200, 200, { x: 0, y: 0, w: 1, h: 1 }, 0, red), layout, { boxes }, 2)).toBe(0);
});

test('the terminal: its cells by its reply\'s grid, its corners from its reply, its padding the same rows above and below', async () => {
  const { baseLength } = await import('./scale-check.mjs');
  const schema = JSON.parse(await Bun.file(new URL('../kernel/tables/schema.json', import.meta.url)).text());
  const grid = { w: 8, h: 16 };
  // space.sm is a cell, a ch: the grid's width; with no grid, no length.
  expect(baseLength(schema, {}, 'space.sm', 'terminal', grid)).toBe(8);
  expect(baseLength(schema, {}, 'space.sm', 'terminal')).toBeUndefined();
  expect(baseLength(schema, {}, 'radius.full', 'terminal', grid)).toBe(0);
  const layout = { viewport: { w: 320, h: 160 }, cell: grid, nodes: [
    { id: 1, x: 0, y: 0, w: 176, h: 48, cells: { x: 0, y: 0, w: 22, h: 3 }, radius: [0, 0, 0, 0], testId: 'box' },
    { id: 2, x: 8, y: 8, w: 160, h: 32, cells: { x: 1, y: 1, w: 20, h: 2 }, radius: [0, 0, 0, 0], testId: 'inner' },
    { id: 3, x: 0, y: 60, w: 40, h: 40, cells: { x: 0, y: 4, w: 5, h: 2 }, radius: [8, 8, 8, 8], testId: 'round' },
  ] };
  const base = (name, host, g) => baseLength(schema, {}, name, host, g);
  const rows = check('terminal', layout, null, { boxes: [
    { testId: 'box', child: 'inner', padding: 'space.sm' },
    { testId: 'box', child: 'inner', padding: 'space.sm', even: true },
    { testId: 'box', radius: 'radius.full' },
    { testId: 'round', radius: 'radius.md' },
  ] }, base);
  // (Half a row of padding laid out and painted as a row above and none
  // below, as before stage 11 (c): not whole cells, nor even.)
  expect(rows.map(r => r.ok)).toEqual([false, false, true, false]);
  expect(rows[0].margins).toEqual({ top: 1, bottom: 0, left: 1, right: 1 });
  expect(rows[0].box.cells).toEqual({ x: 0, y: 0, w: 22, h: 3 });
  expect(rows[0].why).toContain('expected 0 rows and 1 columns');
  expect(rows[1].why).toContain('not equal rows above and below');
  expect(rows[3].why).toContain('radii 8, 8, 8, 8, expected 0');
  // As the kernel lays a terminal's spaces out, in whole cells (stage 11
  // (c)): half a row of padding is none, the box its child's two rows.
  const rounded = { ...layout, nodes: layout.nodes.map(n => (
    n.testId === 'inner' ? { ...n, y: 0, cells: { x: 1, y: 0, w: 20, h: 2 } }
      : n.testId === 'box' ? { ...n, h: 32, cells: { x: 0, y: 0, w: 22, h: 2 } } : n)) };
  const evened = check('terminal', rounded, null, { boxes: [{ testId: 'box', child: 'inner', padding: 'space.sm', even: true }] }, base);
  expect(evened.map(r => r.ok)).toEqual([true]);
  // A gap, in whole cells on its axis: 8 px down is none, 16 px a row, 24
  // px (a row and a half) a row, the tie toward zero; one laid out at 24 px
  // and painted two rows is refused twice over.
  const listed = { ...layout, nodes: [
    { id: 1, x: 0, y: 0, w: 40, h: 16, cells: { x: 0, y: 0, w: 5, h: 1 }, testId: 'a' },
    { id: 2, x: 0, y: 16, w: 40, h: 16, cells: { x: 0, y: 1, w: 5, h: 1 }, testId: 'b' },
    { id: 3, x: 0, y: 48, w: 40, h: 16, cells: { x: 0, y: 3, w: 5, h: 1 }, testId: 'c' },
    { id: 4, x: 0, y: 88, w: 40, h: 16, cells: { x: 0, y: 6, w: 5, h: 1 }, testId: 'd' },
  ] };
  const gaps = check('terminal', listed, null, { boxes: [
    { testId: 'a', between: ['a', 'b'], gap: 8 },
    { testId: 'b', between: ['b', 'c'], gap: 16 },
    { testId: 'c', between: ['c', 'd'], gap: 24 },
  ] }, base);
  expect(gaps.map(r => r.ok)).toEqual([true, true, false]);
  // A reply with no cells for a gap's children, or a padded box's, fails.
  const bare = { ...listed, nodes: listed.nodes.map(n => ({ ...n, cells: undefined })) };
  const unread = check('terminal', bare, null, { boxes: [{ testId: 'a', between: ['a', 'b'], gap: 8 }] }, base);
  expect(unread.map(r => r.ok)).toEqual([false]);
  expect(unread[0].why).toContain('not whole cells in the reply');
  const thinPad = check('terminal', { ...layout, nodes: layout.nodes.map(n => ({ ...n, cells: undefined })) }, null, { boxes: [{ testId: 'box', child: 'inner', padding: 'space.sm' }] }, base);
  expect(thinPad.map(r => r.ok)).toEqual([false]);
  // A terminal reply with no grid at all fails too.
  const gridless = check('terminal', { ...layout, cell: undefined, nodes: layout.nodes.map(n => ({ ...n, cells: undefined })) }, null, { boxes: [{ testId: 'box', child: 'inner', padding: 8, even: true }] }, base);
  expect(gridless.map(r => r.ok)).toEqual([false]);
  expect(gridless[0].why).toContain('no cell grid');
  expect(gaps[2].why).toBe('gap 24, expected 16; its gap paints 2 cells, expected 1');
  // A reply missing what is measured fails: no cells for an even box, radii not four lengths (an elliptical corner's pair).
  const thin = { ...layout, nodes: layout.nodes.map(n => ({ ...n, cells: undefined, radius: n.testId === 'round' ? [[8, 4], 8, 8, 8] : n.radius })) };
  const missing = check('terminal', thin, null, { boxes: [
    { testId: 'box', child: 'inner', padding: 'space.sm', even: true },
    { testId: 'round', radius: 'radius.md' },
  ] }, base);
  expect(missing.map(r => r.ok)).toEqual([false, false]);
  expect(missing[1].why).toContain('not four lengths');
  // A host that does not paint in cells (no grid in its reply) is held to an even box's px alone.
  const pictured = check('linux', { ...layout, cell: undefined, nodes: layout.nodes.map(n => ({ ...n, cells: undefined })) }, drawn(320, 160, { x: 0, y: 0, w: 1, h: 1 }, 0, red), { boxes: [
    { testId: 'box', child: 'inner', padding: 8, even: true },
  ] }, base);
  expect(pictured.map(r => r.ok)).toEqual([true]);
});

test('a terminal step keeps its text whole', async () => {
  const { terminalVerb } = await import('./scale-check.mjs');
  expect(terminalVerb('tap flip')).toEqual(['tap', 'flip']);
  expect(terminalVerb('type field hello  world')).toEqual(['type', 'field', 'hello  world']);
  expect(terminalVerb('paste a b')).toEqual(['paste', 'a b']);
  expect(terminalVerb('wheel list 3')).toEqual(['wheel', 'list', '3']);
  expect(() => terminalVerb('type field')).toThrow('short of its words');
});
