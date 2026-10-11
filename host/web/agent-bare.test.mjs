// @ref LLP 1104 D2 — the driver's shared layout names why the default is bare.
import { test, expect } from 'bun:test';
import { sourceMapReader, render, identifyLayoutNodes } from '../../scripts/agent-inspect.mjs';

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
