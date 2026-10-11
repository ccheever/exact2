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
        text "Go"
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
  expect(found([{ ...text, fg: [60, 60, 67, 0.3], fgRole: 'tertiaryLabelColor' }])).toEqual([]); // and its tertiary, not judged
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
    expect(l.faint).toEqual(['contrast']);
    expect(l.card).toEqual(['card']);
    expect(l.tiny).toEqual(['target']);
    expect(l.fits).toBeUndefined();
    expect(l.ellipsis).toBeUndefined(); // an ellipsis the author asked for
    expect(l.go).toBeUndefined(); // white on the light accent reads
    // In dark the card shows and the faint text reads on it; the label on the pale dark accent does not.
    expect(d.card).toBeUndefined();
    expect(d.faint).toBeUndefined();
    expect(d.go).toEqual(['contrast']);
    expect(both.text).toMatch(/^check web · light: \d+ findings\n/);
    expect(both.text).toContain('go\'s label "Go"');
    // The scheme is put back: the page draws light again.
    expect((await session.op({ op: 'check' })).scheme).toBe('light');
  } finally { await session.close(); }
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
  } finally { await session.close(); }
}, 900000);
