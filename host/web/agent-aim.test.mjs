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
import { namedRefusal } from '../../scripts/agent-aim.mjs';
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
`;

test('a refusal names nodes by testId, and `tap <target> at <x> <y>` is a press at a point', () => {
  const nodes = [{ id: 4, props: { testId: 'wrap-0' } }, { id: 5, props: { testId: 'like 0' } }];
  expect(namedRefusal('tap #4 would press #5 inside it; tap #5, or tap #4 at <x> <y>', nodes)).toBe('tap wrap-0 would press "like 0" inside it; tap "like 0", or tap wrap-0 at <x> <y>');
  expect(namedRefusal('tap #4 at (1, 2): node #5 covers its middle', nodes)).toBe('tap #4 at (1, 2): node #5 covers its middle');
  expect(tapWords(['post-0', 'at', '10', '20'])).toEqual(['post-0', { at: [10, 20] }]);
  expect(tapWords(['post-0', 'at', '10', '20', 'modifiers', 'Shift'])).toEqual(['post-0', { at: [10, 20], modifiers: 'Shift' }]);
  expect(() => tapWords(['post-0', 'at', '10'])).toThrow(/takes a number/);
});

test('a plain web tap presses what it names: beside the card, never Like in a box without a press, refused when covered', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-aim-')), contract = join(dir, 'app.contract'), plan = join(dir, 'app.plan'), dist = join(dir, 'dist');
  writeFileSync(contract, CONTRACT);
  const compile = spawnSync('cargo', ['run', '-q', '-p', 'contract', '--', 'build', contract, '-o', plan], { encoding: 'utf8' });
  expect(compile.status, compile.stderr).toBe(0);
  const build = spawnSync(process.execPath, ['host/web-js/build.mjs', 'caltrain', '--plan', plan, '--out', dist, '--render', 'none'], { cwd: new URL('../../', import.meta.url).pathname, encoding: 'utf8' });
  expect(build.status, build.stderr).toBe(0);
  const server = createServer((request, response) => serveBuildTree(dist, request, response));
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const url = `http://127.0.0.1:${server.address().port}/`;
  const drive = async (session) => {
    const slots = async () => (await session.state()).slots;
    const row = await session.tap('post-0');
    expect(await slots()).toMatchObject({ post: 1, card: 0 });
    expect(row.avoided?.pressing).toBeGreaterThan(0);
    await expect(session.tap('wrap-0')).rejects.toThrow('tap wrap-0 would press like-0 inside it');
    await expect(session.tap('post-1')).rejects.toThrow(/tap post-1 would press cover-1 inside it, at its middle; no point of post-1/);
    expect(await slots()).toMatchObject({ post: 1, card: 0, like: 0 });
    await session.tap('post-0', { at: [150, 60] }); // a point: the card a finger there reaches
    await session.tap('wrap-0', { at: [150, 40] });
    await session.tap('card-0');
    expect(await slots()).toMatchObject({ post: 1, card: 2, like: 1 });
  };
  let session;
  try {
    session = await open({ host: 'web', url });
    await drive(session);
    await session.close(); session = null;
    const { firefox } = await import('playwright-core');
    if (!existsSync(firefox.executablePath())) { console.log('skip firefox: bunx playwright@1.63.0 install firefox webkit'); return; }
    session = await open({ host: 'web', browser: 'firefox', url });
    await drive(session);
  } finally {
    await session?.close?.();
    server.close();
  }
}, 900000);
