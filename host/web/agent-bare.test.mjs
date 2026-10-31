// @ref LLP 1104 D2 — the driver's shared layout names why the default is bare.
import { test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';
import { sourceMapReader, render, identifyLayoutNodes } from '../../scripts/agent-inspect.mjs';

test('the JS agent tree lists authored children but not a hatch part as a view', () => {
  const source = readFileSync(new URL('../web-js/agent.js', import.meta.url), 'utf8');
  const declaration = source.match(/^  const kids = .*;$/m)[0];
  const kids = vm.runInNewContext(`const flowed = () => false; ${declaration}; kids`);
  const child = exact => ({ $exactTree: exact, hasAttribute: () => false });
  const authored = child(true), seal = child(false);
  const node = { getAttribute: name => name === 'data-hatch' ? 'badge' : null, children: [authored, seal] };
  expect(kids(node)).toEqual([authored]);
  node.getAttribute = () => null;
  expect(kids(node)).toEqual([authored, seal]);
});

test('layout joins a bare reason only to the matching plan site on every carrier', () => {
  const digest = 'a'.repeat(64), reader = sourceMapReader(null);
  const at = { file: '/app/app.contract', line: 4, col: 5, end_col: 11, component: 'App', chain: [], bindings: [] };
  reader.add({ digest, nodes: [{ ...at, bare_reason: 'background-color' }, at] });
  for (const host of ['web', 'macos', 'ios', 'linux', 'android', 'host', 'host-ios']) {
    const node = { id: 1, type: 'Pressable', site: 0, planDigest: digest, props: {}, epoch: 1, incarnation: 1 };
    reader.attach(node);
    const reply = { host, viewport: { w: 100, h: 100 }, clock: 0, nodes: [{ ...node, x: 0, y: 0, w: 50, h: 20 }] };
    expect(render('layout', reply)).toContain('Pressable · bare: `background-color`');
    reply.node = node;
    expect(render('layout', reply)).toContain('  bare: `background-color`');
    node.planDigest = 'b'.repeat(64);
    reader.attach(node);
    expect(node.bare_reason).toBeUndefined();
    expect(render('layout', { ...reply, nodes: [] })).not.toContain('bare:');
    node.planDigest = digest; node.site = 1;
    reader.attach(node);
    expect(node.bare_reason).toBeUndefined(); // Explicit none and native buttons have no default refusal.
    node.site = 0; delete node.planDigest;
    reader.attach(node);
    expect(node.bare_reason).toBeUndefined();
  }
});

test('an `:active` row lowered to the press paint carries its part and where it is written (LLP 1121 D8)', () => {
  const digest = 'c'.repeat(64), reader = sourceMapReader(null);
  const part = { file: '/app/app.contract', line: 6, col: 9, end_col: 20 };
  reader.add({ digest, nodes: [{ file: '/app/app.contract', line: 4, col: 5, end_col: 11, component: 'App', chain: [],
    bindings: [{ row: 'press_background_color', states: [{ state: 'active', origin: 'own', at: part }] }, { row: 'background_color', origin: 'own' }] }] });
  const node = { id: 1, type: 'Pressable', site: 0, planDigest: digest, props: {}, style: {
    press_background_color: { value: '#00f', source: 'authored' }, background_color: { value: '#fff', source: 'authored' } } };
  reader.attach(node);
  expect(node.style.press_background_color).toMatchObject({ origin: 'own', state: 'active', at: part });
  expect(node.style.background_color.state).toBeUndefined();
  // A conditional class's part says no place (either style's): its state still, and no `at`.
  const either = 'd'.repeat(64);
  reader.add({ digest: either, nodes: [{ file: '/app/app.contract', line: 4, col: 5, end_col: 11, component: 'App', chain: [],
    bindings: [{ row: 'press_background_color', states: [{ state: 'active', origin: 'class:A/B' }] }] }] });
  const other = { id: 2, type: 'Pressable', site: 0, planDigest: either, props: {}, style: { press_background_color: { value: '#00f', source: 'authored' } } };
  reader.attach(other);
  expect(other.style.press_background_color).toMatchObject({ origin: 'class:A/B', state: 'active' });
  expect(other.style.press_background_color.at).toBeUndefined();
});

test('malformed bare metadata cannot supply a reason', () => {
  const digest = 'a'.repeat(64), reader = sourceMapReader(null);
  const at = { file: '/app/app.contract', line: 4, col: 5, end_col: 11, component: 'App', chain: [], bindings: [] };
  for (const bare_reason of [null, 17, {}, '', 'x'.repeat(1025)]) {
    reader.add({ digest, nodes: [{ ...at, bare_reason }] });
    const node = { id: 1, site: 0, planDigest: digest, bare_reason: 'old' };
    reader.attach(node);
    expect(node.bare_reason).toBeUndefined();
  }
});


test("a listing does not attach the next settlement's reason to old geometry", () => {
  const calls = [], maps = { attach: node => calls.push(node.site) };
  const layout = { epoch: 1, incarnation: 1, nodes: [{ id: 4 }] };
  const tree = { epoch: 2, incarnation: 1, planDigest: 'a'.repeat(64), nodes: [{ id: 4, site: 9, type: 'Pressable', props: { testId: 'action' } }] };
  identifyLayoutNodes(layout, tree, maps);
  expect(calls).toEqual([]);
  tree.epoch = 1; tree.incarnation = 2;
  identifyLayoutNodes(layout, tree, maps);
  expect(calls).toEqual([]);
  tree.incarnation = 1;
  identifyLayoutNodes(layout, tree, maps);
  expect(calls).toEqual([9]);
  expect(layout.nodes[0].planDigest).toBe(tree.planDigest);
});

test('driver deadlines release their timer on success, rejection and timeout', async () => {
  // (The driver's carrier, scripts/agent-wire.mjs since 416f51f1f4.)
  const driver = readFileSync(new URL('../../scripts/agent-wire.mjs', import.meta.url), 'utf8');
  const implementation = driver.match(/async function waitAtMost\([^]*?\n\}/)[0];
  const timers = new Set();
  const wait = vm.runInNewContext(`(${implementation})`, {
    setTimeout(callback) { timers.add(callback); return callback; },
    clearTimeout(callback) { timers.delete(callback); },
  });
  expect(await wait(Promise.resolve('ready'), 20000)).toBe('ready');
  expect(timers.size).toBe(0);
  const failed = new Error('app exited');
  await expect(wait(Promise.reject(failed), 20000)).rejects.toBe(failed);
  expect(timers.size).toBe(0);
  const pending = new Promise(() => {});
  const closed = wait(pending, 2000);
  expect(timers.size).toBe(1);
  [...timers][0]();
  expect(await closed).toBeUndefined();
  expect(timers.size).toBe(0);
  const late = wait(pending, 20000, () => { throw failed; });
  [...timers][0]();
  await expect(late).rejects.toBe(failed);
  expect(timers.size).toBe(0);
});

// Agent input/geometry stays outside the two-module first-paint graph. Its
// first request waits once; readiness leaves subsequent reads synchronous.
test('the wasm agent loads its input helpers once and keeps settled reads synchronous', async () => {
  const source = readFileSync(new URL('./glue.js', import.meta.url), 'utf8');
  const declaration = name => source.match(new RegExp(`function ${name}\\([^]*?\\n\\}`))[0];
  let finish, loads = 0;
  const context = vm.createContext({
    stageLoaded: () => true, agentMode: false, agentNow: request => ({ op: request.op }),
    loadAfterPaint: (file, exported) => {
      expect([file, exported]).toEqual(['./agent-input.js', 'agentInput']);
      loads++;
      return new Promise(resolve => { finish = resolve; });
    },
  });
  vm.runInContext(`let guestOutline, guestTap, guestType, reveal, backButton, viewBox, layoutBoxes, scrollRange, agentInputLoading;
${declaration('agentInputReady')}
${declaration('agent')}
this.agent = agent; this.ready = agentInputReady;`, context);
  const ready = context.ready(), first = context.agent({ op: 'tree' });
  expect(typeof first.then).toBe('function');
  finish({ viewBox: () => ({}) });
  await ready;
  expect(await first).toEqual({ op: 'tree' });
  expect(context.agent({ op: 'layout' })).toEqual({ op: 'layout' });
  expect(loads).toBe(1);
});
