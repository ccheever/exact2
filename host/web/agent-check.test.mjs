// @ref LLP 1116.003 D8 — `check`: what is wrong with the presented screen, in text (scripts/agent-check.mjs judging
// host/web-js/check.js's observations and a capture's pixels): one fixture view per finding class, found on the page in
// each scheme and absent from a page that has none; and a screenshot shows a transition where it ends, unless filmed.
import { test, expect } from 'bun:test';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { open } from '../../scripts/agent.mjs';
import { checkOp, judge, ratio } from '../../scripts/agent-check.mjs';
import { decodePng } from '../../scripts/png.mjs';
import { serveBuildTree } from './serve.mjs';

// One view per finding class, each named by its testId; `fits` and `ellipsis` are what must not be found.
const BROKEN = `component CheckFixture
  state on = false
  action toggle
    on = not on
  view
    column testId="root" gap=8 padding=16 accent-color="light-dark(#6652c5, #b5a4ff)"
      text testId="fits" "Fits"
      row testId="card" padding=12 border-radius=12 background-color="-exact-secondary-grouped-background"
        text testId="faint" color="#bbbbbb" "Faint text in a card"
      button testId="go" press=toggle buttonStyle="bordered-prominent"
        text "Go" color="white"
      text testId="clip" width=60 height=20 overflow="hidden" "A long label that will not fit"
      text testId="ellipsis" width=60 white-space="nowrap" overflow="hidden" text-overflow="ellipsis" "A long label with an ellipsis"
      column testId="stage" position="relative" height=60
        button testId="hidden" press=toggle
          text "Under"
        column testId="cover" position="absolute" top=0 left=0 width=300 height=60 background-color="-exact-secondary-background"
      row gap=0
        column testId="a" press=toggle width=100 height=44 background-color="-exact-secondary-background"
        column testId="b" press=toggle width=100 height=44 margin-left=-20 background-color="-exact-tertiary-fill"
      row gap=4
        column testId="tiny" press=toggle width=16 height=16 background-color="-exact-secondary-background"
        column testId="tiny2" press=toggle width=16 height=16 background-color="-exact-secondary-background"
      row testId="wide" width=600 height=20 background-color="-exact-secondary-background"
      row testId="options" width=120 gap=4
        button testId="opt-a" press=toggle flex=1 padding="8px 12px"
          text "Fifty"
        button testId="opt-b" press=toggle flex=1 padding="8px 12px"
          text "Seventy five"
      column testId="swatch" press=toggle width=100 height=100 background-color=(on ? "#ff0000" : "#0000ff") transition="background-color 400ms linear"
`;
// The same kinds of views, as the platform would have them: nothing to find in either scheme.
const CLEAN = `component CleanFixture
  state n = 0
  action bump
    n = n + 1
  view
    column testId="root" gap=12 padding=16
      text testId="title" font-size="-exact-title2" "A clean screen"
      column testId="card" padding=12 gap=4 border-radius=12 background-color="-exact-secondary-background"
        text testId="line" "Text in the label colour"
        text testId="detail" color="-exact-secondary-label" "Secondary detail"
      button testId="go" press=bump buttonStyle="bordered-prominent"
        text "Go"
      row gap=8
        button testId="one" press=bump
          text "One"
        button testId="two" press=bump
          text "Two"
`;
// LLP 1116.009 D5 (focus-timer, recipe-finder, round 11): a written percentage width and the padding it does not include,
// past what the parent gives it, inside the `scroll` that holds the overflow so nothing else sees it. `card` is the finding;
// each other box is what must not be one: border-box, a width that leaves room, a row's shrinking item, a pager that scrolls on
// purpose, a nowrap readout, a bar and a sheet the platform draws.
const PADDED = `routes nav
  home "/"

component PaddedFixture
  state n = 0
  action bump
    n = n + 1
  view
    main testId="app" navigationKey=\`\${top(nav).id}\` navigationBack="back" width="100%" height="100%"
      each e in stack(nav) key=e.id
        column navigationKey=\`\${e.id}\` navigationScroll="content" position="absolute" inset=0 display="flex" flex-direction="column"
          header display="flex" align-items="center" justify-content="space-between" padding="12px 16px"
            text "Padded" role="heading" aria-level=1
            button press=bump testId="bar-button"
              text "Add"
          scroll id="content" flex=1 min-height=0 testId="scroller"
            column testId="root" padding=16 gap=12
              column testId="card" width="100%" padding=24 background-color="-exact-secondary-background" border-radius=12
                text "Wider than its parent by its padding"
              column testId="boxed" width="100%" padding=24 box-sizing="border-box" background-color="-exact-secondary-background" border-radius=12
                text "Border-box: fits"
              column testId="narrow" width="50%" padding=8 background-color="-exact-secondary-background" border-radius=12
                text "Room to spare"
              row testId="chips" gap=8
                column testId="chip" width="100%" padding=12 background-color="-exact-secondary-background" border-radius=12
                  text "In a row"
              column testId="pager" overflow-x="auto"
                column testId="page" width="100%" padding=12
                  text "Scrolls on purpose"
              text testId="readout" white-space="nowrap" font-size="-exact-title2" "00:25:00"
`;
// A column held to the screen's height, its end past the screen's bottom: a line cut by it, a button out of reach.
const TALL = `component TallFixture
  state n = 0
  action bump
    n = n + 1
  view
    column testId="root" height="100vh" overflow="hidden"
      column testId="spacer" height=890 flex-shrink=0
      text testId="cut" flex-shrink=0 "This line straddles the bottom of the screen"
      button testId="unreachable" press=bump flex-shrink=0
        text "Save"
`;
// LLP 1116.006 D5: a text judged on the fill under it — its own box's (a rounded bubble, whose corners show the page), a
// bubble column's, the page's — in each scheme; and rows a scroller cuts, overlapping nothing past its edge.
const BUBBLES = `component BubbleFixture
  state n = 0
  action bump
    n = n + 1
  view
    column testId="root" gap=8 padding=16
      text testId="page" "Text on the page"
      text testId="own" padding="8px 12px" border-radius=18 background-color="light-dark(#5856d6, #8e8cf0)" color="white" "Own box"
      column testId="bubble" padding="8px 12px" border-radius=18 background-color="light-dark(#5856d6, #8e8cf0)"
        text testId="in" color="white" "In a bubble"
      text testId="own-light" padding="8px 12px" border-radius=18 background-color="#e9e9eb" color="-exact-label" "Label on a light bubble"
      column testId="light" padding="8px 12px" border-radius=18 background-color="#e9e9eb"
        text testId="dark-ink" color="#1c1c1e" "Dark on light"
      column testId="list" height=100 overflow-y="auto" flex-shrink=0
        column testId="row1" press=bump height=44 flex-shrink=0
          text "Row one"
        column testId="row2" press=bump height=44 flex-shrink=0
          text "Row two"
        column testId="row3" press=bump height=44 flex-shrink=0
          text "Row three"
      row testId="bar" gap=0 flex-shrink=0
        column testId="tab1" press=bump width=100 height=44
          text "One"
        column testId="tab2" press=bump width=100 height=44
          text "Two"
`;

const built = (() => {
  const ready = new Map();
  return (name, source) => {
    if (!ready.has(name)) ready.set(name, (async () => {
      const dir = mkdtempSync(join(tmpdir(), `exact-check-${name}-`)), contract = join(dir, 'app.contract'), plan = join(dir, 'app.plan'), dist = join(dir, 'dist');
      writeFileSync(contract, source);
      const compile = spawnSync('cargo', ['run', '-q', '-p', 'contract', '--', 'build', contract, '-o', plan], { encoding: 'utf8' });
      if (compile.status !== 0) throw new Error(compile.stderr);
      const build = spawnSync(process.execPath, ['host/web-js/build.mjs', 'caltrain', '--plan', plan, '--out', dist, '--render', 'none'], { cwd: new URL('../../', import.meta.url).pathname, encoding: 'utf8' });
      if (build.status !== 0) throw new Error(build.stderr);
      const server = createServer((request, response) => serveBuildTree(dist, request, response));
      await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
      server.unref();
      return `http://127.0.0.1:${server.address().port}/`;
    })());
    return ready.get(name);
  };
})();

/** testId → the kinds found on it, per scheme: `<control>'s label` findings count as the control's. */
const kinds = (run) => {
  const out = {};
  for (const f of run.findings) { const who = f.node.replace(/'s label .*$/, ''); (out[who] ??= []).push(f.kind); }
  return out;
};

test('judging: WCAG ratios, the floors, a clamp asked for, a reachable cover, the 24 px spacing exception and an invisible card', () => {
  expect(ratio([255, 255, 255], [0, 0, 0])).toBeCloseTo(21, 5);
  expect(ratio([187, 187, 187], [255, 255, 255])).toBeCloseTo(1.92, 2);
  // A 40×20 capture, white, its right half grey with a white 12×12 card drawn on it: a text and a card on white, a card on grey.
  const png = { width: 40, height: 20, data: new Uint8Array(40 * 20 * 4).fill(255) };
  for (let y = 0; y < 20; y++) for (let x = 20; x < 40; x++) if (!(x >= 24 && x < 36 && y >= 4 && y < 16)) png.data.set([128, 128, 128, 255], (y * 40 + x) * 4);
  const obs = (items, extra = {}) => ({ viewport: { w: 40, h: 20 }, scheme: 'light', target: 24, items, ...extra });
  const text = { id: 1, x: 2, y: 2, w: 16, h: 16, text: 'Hi', size: 16, weight: 400 };
  const found = (items, extra) => judge(obs(items, extra), png).map((f) => `${f.kind}:${f.id}`);
  expect(found([{ ...text, fg: [118, 118, 118, 1] }])).toEqual([]); // #767676 on white: 4.54:1
  expect(found([{ ...text, fg: [130, 130, 130, 1] }])).toEqual(['contrast:1']);
  expect(found([{ ...text, fg: [130, 130, 130, 1], size: 24 }])).toEqual([]); // large: 3:1
  expect(found([{ ...text, fg: [130, 130, 130, 1], fgRole: 'secondary-label' }])).toEqual([]); // the platform's own secondary label
  expect(found([{ ...text, fg: [130, 130, 130, 1], fgRole: 'AccentColorText' }])).toEqual([]); // the ink the platform chose for the accent, at 3:1
  expect(found([{ ...text, fg: [60, 60, 67, 0.3], fgRole: 'tertiaryLabelColor' }])).toEqual([]); // and its tertiary, not judged
  expect(found([{ ...text, fg: [197, 197, 198, 1], inactive: true }])).toEqual([]); // a disabled control's label (WCAG's inactive component)
  expect(found([{ ...text, fg: [60, 60, 67, 0.6] }])).toEqual([]); // the secondary label's value, unnamed (a section footer's), is its
  expect(found([{ ...text, fg: [255, 56, 60, 1] }])).toEqual([]); // the grouped sheet's red (a destructive row's title), UIKit's at 3:1
  expect(found([{ ...text, fg: [0, 0, 0, 1], over: [30, 0], clips: true }])).toEqual(['clipped:1']);
  expect(found([{ ...text, fg: [0, 0, 0, 1], over: [30, 0], clips: true, clamp: true }])).toEqual([]);
  // A card on white whose fill is white shows nothing; on grey it shows; with an edge it shows.
  const card = { id: 2, x: 4, y: 4, w: 12, h: 12, fill: [255, 255, 255, 1], radius: 8, fillRole: 'secondary-grouped-background' };
  const invisible = judge(obs([card]), png);
  expect(invisible.map((f) => f.kind)).toEqual(['card']);
  expect(invisible[0].hint).toContain('-exact-secondary-background');
  expect(found([{ ...card, x: 24 }])).toEqual([]);
  expect(found([{ ...card, edge: true }])).toEqual([]);
  // Two 16 px targets: under 24 px and closer than 24 px apart, both; far apart, neither (WCAG 2.5.8's spacing); the platform's own, never.
  const small = (id, x) => ({ id, x, y: 2, w: 16, h: 16, control: 'press', sized: 'author' });
  expect(found([small(3, 0), small(4, 18)])).toEqual(['target:3', 'target:4']);
  expect(found([small(3, 0), small(4, 30)], { viewport: { w: 60, h: 20 } })).toEqual([]);
  expect(judge(obs([small(3, 0), small(4, 18)], { target: 44 }), png, { host: 'ios' }).length).toBe(2);
  expect(found([{ ...small(3, 0), sized: 'platform' }, { ...small(4, 18), sized: 'platform' }])).toEqual([]);
  // Covered at rest is a finding; the host leaves out what a scroller could move clear.
  expect(found([{ ...small(5, 0), w: 30, h: 30, cover: { by: 9, kind: 'node' } }])).toEqual(['covered:5']);
  // LLP 1116.006 D5: a text on a fill the host found is judged on it, not on the pixels (here white): white on indigo reads,
  // white on a pale indigo does not, and the finding names the fill's owner.
  const white = { ...text, fg: [255, 255, 255, 1] };
  expect(found([white])).toEqual(['contrast:1']);
  expect(found([{ ...white, bg: [88, 86, 214], on: 'own' }])).toEqual([]);
  const pale = judge(obs([{ ...white, bg: [142, 140, 240], on: 7 }]), png);
  expect(pale.map((f) => [f.kind, f.detail.bg, f.detail.on])).toEqual([['contrast', '#8e8cf0', 7]]);
  expect(judge(obs([{ ...white, bg: [142, 140, 240], on: 'own' }]), null).length).toBe(1); // no capture needed
  // Overlap is judged on what is presented of each box: a row its scroller cuts at y = 10 overlaps nothing below.
  const row = { id: 6, x: 0, y: 0, w: 40, h: 20, control: 'press', sized: 'platform' }, tab = { id: 7, x: 0, y: 10, w: 40, h: 20, control: 'press', sized: 'platform' };
  expect(found([row, tab])).toEqual(['overlap:6']);
  expect(found([{ ...row, vis: { x: 0, y: 0, w: 40, h: 10 } }, tab])).toEqual([]);
  // LLP 1116.007 D3.8: what a bar is drawn over is covered, not presented (`vis` empty): a text wholly under it is not judged for
  // contrast, nor a control for size; one partly under it is judged on the fill under the part that shows (here the grey).
  const under = { ...text, fg: [130, 130, 130, 1] };
  expect(found([{ ...under, vis: { x: 2, y: 2, w: 16, h: 0 } }])).toEqual([]);
  expect(found([{ id: 8, x: 0, y: 2, w: 16, h: 16, control: 'press', sized: 'author', vis: { x: 0, y: 2, w: 16, h: 0 } }])).toEqual([]);
  const part = judge(obs([{ ...under, w: 36, vis: { x: 22, y: 2, w: 16, h: 16 } }]), png);
  expect(part.map((f) => [f.kind, f.detail.bg])).toEqual([['contrast', '#808080']]);
  // LLP 1116.007 D3 (unit-converter): the keyboard covers like a bar. A card under it shows the keyboard in the capture, not its
  // edge, so it is not "invisible"; one partly under it is judged on the edges that show.
  expect(found([card], { keyboard: 4 })).toEqual([]); // wholly under the keyboard
  expect(found([card], { keyboard: 10 })).toEqual(['card:2']); // its top half shows, and is white on white
  expect(found([{ ...text, fg: [130, 130, 130, 1] }], { keyboard: 2 })).toEqual([]); // a text under it is not judged for contrast
  expect(found([row, tab], { keyboard: 10 })).toEqual([]); // nor are two controls overlapping beneath it
  // A flex item past its container's side is an overflow of its own (tip-splitter's fifth radio), named for the row it spills.
  const spill = judge(obs([{ id: 9, x: 24, y: 2, w: 20, h: 16, control: 'radio', sized: 'platform', spill: { by: 8, dx: 6 } }], { viewport: { w: 60, h: 20 } }), png);
  expect(spill.map((f) => [f.kind, f.id, f.detail.past, f.detail.of])).toEqual([['overflow', 9, 6, 8]]);
});

// LLP 1116.009 D5: the judge names the cause: the written width, the padding, and the parent it runs past.
test('a padded percentage width past its parent is an overflow that names its cause and its repair', () => {
  const png = { width: 40, height: 20, data: new Uint8Array(40 * 20 * 4).fill(255) };
  const padded = { id: 4, x: 0, y: 2, w: 54, h: 16, padded: { by: 3, dx: 14, pad: 14, width: '100%' } };
  const f = judge({ viewport: { w: 40, h: 20 }, scheme: 'light', target: 24, items: [padded] }, png);
  expect(f.map((x) => [x.kind, x.id, x.detail.past, x.detail.of, x.detail.pad, x.detail.written])).toEqual([['overflow', 4, 14, 3, 14, '100%']]);
  expect(f[0].hint).toContain('border-box');
  // iOS is told no width (the kernel sized the box): the finding names the padding and the parent, and says no width.
  const ios = judge({ viewport: { w: 40, h: 20 }, scheme: 'light', target: 44, items: [{ id: 5, x: 0, y: 2, w: 54, h: 16, padded: { by: 3, dx: 14, pad: 14 } }] }, png, { host: 'ios' });
  expect(ios.map((x) => [x.kind, x.detail.past, x.detail.of, x.detail.pad, x.detail.written, x.detail.unit])).toEqual([['overflow', 14, 3, 14, undefined, 'pt']]);
  // The viewport's own overflow is still there beside it, not instead: the cause is named first, once.
  expect(judge({ viewport: { w: 40, h: 20 }, scheme: 'light', target: 24, items: [{ ...padded, outX: 14 }] }, png)).toHaveLength(1);
});

test('a page with every finding class has each found, by testId, in the scheme it shows in; a clean page has none', async () => {
  const session = await open({ host: 'web', url: await built('broken', BROKEN) });
  try {
    const both = await checkOp(session, ['both']);
    const [light, dark] = both.check;
    expect([light.scheme, dark.scheme]).toEqual(['light', 'dark']);
    const l = kinds(light), d = kinds(dark);
    expect(l.clip).toEqual(['clipped']);
    expect(l.hidden).toEqual(['covered']); // its label "Under" is the button's finding, not a second one
    expect(l.a).toEqual(['overlap']);
    expect(l.wide).toEqual(['overflow']);
    expect(l['opt-b']).toEqual(['overflow']); // a flex item wider than its row, within the viewport (LLP 1116.007 D3)
    expect(l['opt-a']).toBeUndefined();
    expect(l.faint).toEqual(['contrast']);
    expect(l.card).toEqual(['card']);
    expect(l.tiny).toEqual(['target']);
    expect(l.fits).toBeUndefined();
    expect(l.ellipsis).toBeUndefined(); // an ellipsis the author asked for
    expect(l.go).toBeUndefined(); // white on the light accent reads
    // In dark the card shows and the faint text reads on it; the author's white label on the pale
    // dark accent does not (unsaid, the web picks black there: LLP 1116.003 D3.1).
    expect(d.card).toBeUndefined();
    expect(d.faint).toBeUndefined();
    expect(d.go).toEqual(['contrast']);
    expect(both.text).toMatch(/^check web · light: \d+ findings\n/);
    expect(both.text).toContain('go\'s label "Go"');
    // The scheme is put back: the page draws light again.
    expect((await session.op({ op: 'check' })).scheme).toBe('light');
  } finally { await session.close(); }
  const wide = await open({ host: 'web', url: await built('padded', PADDED), size: [390, 844] });
  try {
    const run = (await checkOp(wide, ['light'])).check[0];
    expect(kinds(run)).toEqual({ card: ['overflow'] });
    expect(run.findings[0].detail).toMatchObject({ past: 48, pad: 48, written: '100%' });
    // At every width the parent gives it (the box has no `max-width` to keep it inside a wide window's), and nothing else is found.
    for (const width of [320, 360, 1280]) {
      await wide.resize(width, 844); await wide.clock('settle');
      const at = (await checkOp(wide, ['both'])).check;
      expect(at.map((r) => kinds(r))).toEqual([{ card: ['overflow'] }, { card: ['overflow'] }]);
      expect(at[0].findings[0].detail.past).toBe(48);
    }
  } finally { await wide.close(); }
  const tall = await open({ host: 'web', url: await built('tall', TALL) });
  try {
    const t = kinds((await checkOp(tall, [])).check[0]);
    expect(t).toEqual({ cut: ['clipped'], unreachable: ['covered'] });
  } finally { await tall.close(); }
  const clean = await open({ host: 'web', url: await built('clean', CLEAN) });
  try {
    const r = await checkOp(clean, ['both']);
    expect(r.check.map((run) => run.findings.map((f) => `${f.kind} ${f.node}`))).toEqual([[], []]);
    expect(r.text.split('\n')).toHaveLength(2);
    expect(r.text).toContain('check web · light: no findings');
  } finally { await clean.close(); }
}, 900000);

test('a text is judged on the fill under it in each scheme, and a row its scroller cuts overlaps nothing past it', async () => {
  const session = await open({ host: 'web', url: await built('bubbles', BUBBLES) });
  try {
    const dir = mkdtempSync(join(tmpdir(), 'exact-check-bubbles-'));
    const [light, dark] = (await checkOp(session, ['both'], { shots: (_, scheme) => join(dir, `${scheme}.png`) })).check;
    expect(kinds(light)).toEqual({}); // white on the indigo bubble reads, though its box's corners show the white page
    expect(kinds(dark)).toEqual({ own: ['contrast'], in: ['contrast'], 'own-light': ['contrast'] }); // on the pale bubbles, not the black page
    const on = (run, id) => run.findings.find((f) => f.node === id)?.detail;
    expect(on(dark, 'own')).toMatchObject({ bg: '#8e8cf0', on: 'own' });
    expect(on(dark, 'in')).toMatchObject({ bg: '#8e8cf0' });
    expect(on(dark, 'own-light')).toMatchObject({ fg: '#ffffff', bg: '#e9e9eb', on: 'own' });
    const obs = await session.op({ op: 'check' }), ids = new Map(((await session.op({ op: 'tree' })).nodes ?? []).map((n) => [n.props?.testId, n.id]));
    const item = (testId, pick = (it) => it.id === ids.get(testId)) => obs.items.find(pick);
    expect(item('in').on).toBe(ids.get('bubble'));
    // Its box's corners alone (round 8's chat bubbles) judged white on the white page.
    const pixels = judge({ ...obs, items: obs.items.map(({ bg, ...it }) => it) }, decodePng(readFileSync(join(dir, 'light.png'))));
    expect(pixels.filter((f) => f.kind === 'contrast').map((f) => f.id)).toEqual([ids.get('own')]);
    expect([item('page').bg, item('page').on]).toEqual([[255, 255, 255], 'page']); // the canvas, which nothing above paints
    expect(item('row3').vis.h).toBeLessThan(item('row3').h); // the list cuts it
    const whole = { ...obs, items: obs.items.map(({ vis, ...it }) => it) }; // judged on its whole box, it overlapped the tabs
    expect(judge(whole, null).filter((f) => f.kind === 'overlap').map((f) => f.id)).toContain(ids.get('row3'));
  } finally { await session.close(); }
}, 900000);

test('a screenshot shows a transition where it ends, the clock unmoved; a film\'s frames are the clock\'s', async () => {
  const session = await open({ host: 'web', url: await built('broken', BROKEN) });
  try {
    const dir = mkdtempSync(join(tmpdir(), 'exact-check-shot-'));
    const swatch = (await session.layout('swatch')).node.space.viewport;
    const at = (file) => { const png = decodePng(readFileSync(file)), x = Math.floor(swatch.x + swatch.w / 2), y = Math.floor(swatch.y + swatch.h / 2), i = (y * png.width + x) * 4; return [...png.data.slice(i, i + 3)]; };
    await session.tap('swatch');
    const clock = session.now;
    await session.screenshot(join(dir, 'after.png'));
    const [r, , b] = at(join(dir, 'after.png'));
    expect(r).toBeGreaterThan(240); // red, where the 400 ms transition ends
    expect(b).toBeLessThan(15);
    expect((await session.op({ op: 'tags' })).clock).toBe(clock); // the app's clock did not move for the picture
    await session.clock('settle'); // red now on the clock too
    await session.tap('swatch'); // back toward blue: a film's first frame is where the clock stands, its start
    const film = await session.screenshot(join(dir, 'film.png'), { over: 400, every: 200 });
    const frame = (i) => at(join(film.dir, `${String(i).padStart(3, '0')}-${film.at[i]}ms.png`));
    expect(frame(0)[0]).toBeGreaterThan(240); // still red at the clock it starts on
    expect(frame(1)[0]).toBeLessThan(200); // half way
    expect(frame(2)[2]).toBeGreaterThan(240); // blue where it ends
    // LLP 1116.007 D3.6: after `resize` a screenshot is the new viewport, in its reply as in its pixels.
    await session.resize(390, 844);
    const small = await session.screenshot(join(dir, 'small.png')), png = decodePng(readFileSync(join(dir, 'small.png')));
    expect([small.w, small.h, png.width, png.height]).toEqual([390, 844, 390, 844]);
  } finally { await session.close(); }
}, 900000);
