// @ref LLP 1012 §1 `tap` — a tap addressed by id presses what it names (scripts/agent-aim.mjs): a row with a press of
// its own is clicked beside the link card at its middle, a box without one never presses the control inside it, and
// `tap <target> at <x> <y>` keeps a point's semantics (the Bluesky clone's likes on real people's posts, 2026-10-09).
import { test, expect } from 'bun:test';
import { mkdtempSync, writeFileSync, existsSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { open } from '../../scripts/agent.mjs';
import { namedRefusal, pageAim } from '../../scripts/agent-aim.mjs';
import { tapWords } from '../../scripts/agent-keys.mjs';
import { serveBuildTree } from './serve.mjs';

const CONTRACT = `component AimFixture
  state post = 0
  state card = 0
  state like = 0
  action openPost
    post = post + 1
  action openCard
    card = card + 1
  action liked
    like = like + 1
  view
    column testId="root" gap=8 width=300
      column testId="post-0" press=openPost padding-left=70 padding-right=70 padding-top=30 padding-bottom=30 background-color="#eeeeee"
        column testId="card-0" press=openCard width=160 height=60 background-color="#cccccc"
      column testId="wrap-0" padding-left=100 padding-right=100 padding-top=20 padding-bottom=20
        button testId="like-0" press=liked width=100 height=40
          text "Like"
      column testId="post-1" press=openPost
        column testId="cover-1" press=openCard width=300 height=60 background-color="#dddddd"
      column testId="post-2" press=openPost padding-left=100 padding-right=100 padding-top=10 padding-bottom=10
        button testId="like-2" press=liked disabled=true width=100 height=40
          text "Like"
`;

test('a refusal names nodes by testId, and `tap <target> at <x> <y>` is a press at a point', () => {
  const nodes = [{ id: 4, props: { testId: 'wrap-0' } }, { id: 5, props: { testId: 'like 0' } }];
  expect(namedRefusal('tap #4 would press #5 inside it; tap #5, or tap #4 at <x> <y>', nodes)).toBe('tap wrap-0 would press "like 0" inside it; tap "like 0", or tap wrap-0 at <x> <y>');
  expect(namedRefusal('tap #4 at (1, 2): node #5 covers its middle', nodes)).toBe('tap #4 at (1, 2): node #5 covers its middle');
  expect(tapWords(['post-0', 'at', '10', '20'])).toEqual(['post-0', { at: [10, 20] }]);
  expect(tapWords(['post-0', 'at', '10', '20', 'modifiers', 'Shift'])).toEqual(['post-0', { at: [10, 20], modifiers: 'Shift' }]);
  expect(() => tapWords(['post-0', 'at', '10'])).toThrow(/takes a number/);
});

const built = (() => {
  let ready;
  return () => ready ??= (async () => {
    const dir = mkdtempSync(join(tmpdir(), 'exact-aim-')), contract = join(dir, 'app.contract'), plan = join(dir, 'app.plan'), dist = join(dir, 'dist');
    writeFileSync(contract, CONTRACT);
    const compile = spawnSync('cargo', ['run', '-q', '-p', 'contract', '--', 'build', contract, '-o', plan], { encoding: 'utf8' });
    if (compile.status !== 0) throw new Error(compile.stderr);
    const build = spawnSync(process.execPath, ['host/web-js/build.mjs', 'caltrain', '--plan', plan, '--out', dist, '--render', 'none'], { cwd: new URL('../../', import.meta.url).pathname, encoding: 'utf8' });
    if (build.status !== 0) throw new Error(build.stderr);
    const server = createServer((request, response) => serveBuildTree(dist, request, response));
    await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
    server.unref();
    return `http://127.0.0.1:${server.address().port}/`;
  })();
})();

const drive = async (session) => {
  const slots = async () => (await session.state()).slots;
  const row = await session.tap('post-0');
  expect(await slots()).toMatchObject({ post: 1, card: 0 });
  expect(row.avoided?.pressing).toBeGreaterThan(0);
  await expect(session.tap('wrap-0')).rejects.toThrow('tap wrap-0 would press like-0 inside it');
  await expect(session.tap('post-1')).rejects.toThrow(/tap post-1 would press cover-1 inside it, at its middle; no point of post-1/);
  await session.tap('post-2'); // a disabled Like at its middle presses nothing: the row, beside it
  expect(await slots()).toMatchObject({ post: 2, card: 0, like: 0 });
  await session.tap('post-0', { at: [150, 60] }); // a point: the card a finger there reaches
  await session.tap('wrap-0', { at: [150, 40] });
  await session.tap('card-0');
  expect(await slots()).toMatchObject({ post: 2, card: 2, like: 1 });
};

test('a plain web tap presses what it names: beside the card, never Like in a box without a press, refused when covered', async () => {
  const session = await open({ host: 'web', url: await built() });
  try {
    await drive(session);
    // In the page: a surface's action button over its canvas is a recipient of its own (gpu-glue.js), and a reachable
    // perimeter 2 px wide around a child that covers the rest is found (Astra, round 1).
    const probe = await session.carrier.evaluate(`(() => {
      const box = (css, attrs = {}) => { const e = document.createElement(attrs.tag ?? 'div'); e.style.cssText = css; for (const [k, v] of Object.entries(attrs)) if (k !== 'tag') e.setAttribute(k, v); return e; };
      const canvas = box('position:fixed;left:0;top:400px;width:200px;height:100px;z-index:9', { 'data-gpu-input': '' });
      canvas.append(box('position:absolute;left:50px;top:25px;width:100px;height:50px', { tag: 'button', 'data-action': 'fire' }));
      const ring = box('position:fixed;left:220px;top:400px;width:100px;height:100px;z-index:9', { 'data-exact-on': 'press' });
      ring.append(box('position:absolute;left:2px;top:2px;width:96px;height:96px', { 'data-exact-on': 'press' }));
      document.body.append(canvas, ring);
      exact.views.set(990001, canvas); exact.views.set(990002, ring);
      const aim = (${pageAim})({ id: 990001, x: 100, y: 450 }), edge = (${pageAim})({ id: 990002, x: 270, y: 450 });
      exact.views.delete(990001); exact.views.delete(990002); canvas.remove(); ring.remove();
      return { aim, edge };
    })()`);
    expect(probe.aim?.at).toBeDefined();
    expect(probe.aim.at[0] < 50 || probe.aim.at[0] >= 150 || probe.aim.at[1] < 425 || probe.aim.at[1] >= 475).toBe(true);
    expect(probe.edge?.at).toBeDefined();
    const [ex, ey] = probe.edge.at;
    expect(ex < 222 || ex >= 318 || ey < 402 || ey >= 498).toBe(true);
  } finally { await session.close(); }
}, 900000);

const { firefox } = await import('playwright-core');
test.skipIf(!existsSync(firefox.executablePath()))('the same on Firefox (Playwright; install it: bunx playwright@1.63.0 install firefox webkit)', async () => {
  const session = await open({ host: 'web', browser: 'firefox', url: await built() });
  try { await drive(session); } finally { await session.close(); }
}, 900000);
