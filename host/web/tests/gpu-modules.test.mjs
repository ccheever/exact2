import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

// The router (LLP 1009 D6) over recording stand-ins for gpu-glue.js
// instances: which artifact each call reaches, and what it joins.
async function fixture(pending = []) {
  const calls = [], frames = [], imported = [];
  const exact = { compat: { inputs: { gpuModules: { world: ['world', 'arena'] } } }, generation: 0, pendingSurfaces: pending };
  const host = stem => ({
    surface(view, name, values) { calls.push([stem, 'surface', view, name, values]); },
    destroy(view) { calls.push([stem, 'destroy', view]); },
    settled: async () => [`${stem} pending`],
    decorate(request, reply) { return { ...reply, world: [...(reply.world ?? []), stem] }; },
    clock: () => ({ pending: stem === 'gpu/world', settleAt: stem === 'gpu' ? 5 : 9, reply: { world: [{ stem }] } }),
    deviceLost() { calls.push([stem, 'deviceLost']); },
    swap(version) { calls.push([stem, 'swap', version]); return { ms: 1, errors: [] }; },
    surfaceWork: async name => `${stem}:${name}`,
    handle: request => ({ stem, id: request.id }),
    wantsInput: () => true,
    placementHidden: () => stem === 'gpu/world',
  });
  const source = readFileSync(new URL('../gpu-modules.js', import.meta.url), 'utf8')
    .replace('import(`./gpu-glue.js?artifact=${stem}`)', 'load(stem)');
  new Function('globalThis', 'requestAnimationFrame', 'load', 'console', source)(
    { exact }, fn => frames.push(fn),
    async stem => { imported.push(stem); exact.gpu.register(stem, host(stem)); },
    { error() {} });
  const paint = async () => { while (frames.length) frames.shift()(); await new Promise(r => setTimeout(r, 0)); };
  return { exact, gpu: exact.gpu, calls, imported, paint };
}

test('each surface reaches the artifact that owns its name, loaded only when needed', async () => {
  const f = await fixture([{ id: 2, name: 'night', values: [true], generation: 0 }, { id: 3, name: 'stale', values: [], generation: 9 }]);
  await f.paint();
  assert.deepEqual(f.imported, ['gpu'], 'the queued canvases load their artifact at once; another generation is dropped');
  assert.deepEqual(f.calls, [['gpu', 'surface', 2, 'night', [true]]]);
  assert.deepEqual(f.exact.pendingSurfaces, []);
  f.gpu.surface(15, 'world', [7]);
  f.gpu.surface(15, 'world', [8]);
  assert.deepEqual(f.imported, ['gpu'], 'a module waits for a rendering opportunity');
  await f.paint();
  assert.deepEqual(f.imported, ['gpu', 'gpu/world']);
  assert.deepEqual(f.calls.at(-1), ['gpu/world', 'surface', 15, 'world', [8]], 'only the latest inputs replay');
  f.gpu.surface(15, 'night', [false]);
  assert.deepEqual(f.calls.slice(-2), [['gpu/world', 'destroy', 15], ['gpu', 'surface', 15, 'night', [false]]], 'a view that changes artifact leaves the old one');
  f.gpu.destroy(15);
  assert.deepEqual(f.calls.at(-1), ['gpu', 'destroy', 15]);
});

test('replies join across artifacts and process-wide signals reach every one', async () => {
  const f = await fixture();
  f.gpu.surface(2, 'night', []); f.gpu.surface(4, 'arena', []);
  await f.paint();
  assert.deepEqual(await f.gpu.settled(), ['gpu pending', 'gpu/world pending']);
  assert.deepEqual(f.gpu.decorate({ op: 'state' }, {}).world, ['gpu', 'gpu/world']);
  assert.deepEqual(f.gpu.clock(true), { pending: true, settleAt: 9, reply: { world: [{ stem: 'gpu' }, { stem: 'gpu/world' }] } });
  f.gpu.deviceLost();
  assert.deepEqual(f.calls.filter(c => c[1] === 'deviceLost').map(c => c[0]), ['gpu', 'gpu/world']);
  f.gpu.swap(3, 'world'); f.gpu.swap(4);
  assert.deepEqual(f.calls.filter(c => c[1] === 'swap'), [['gpu/world', 'swap', 3], ['gpu', 'swap', 4]]);
  assert.deepEqual(f.gpu.handle({ id: 4 }), { stem: 'gpu/world', id: 4 });
  assert.equal(f.gpu.placementHidden({}), true);
  assert.equal(await f.gpu.surfaceWork('world', 'capture'), 'gpu/world:world');
  f.gpu.destroy(4);
  await assert.rejects(f.gpu.surfaceWork('world', 'capture'), /expected one live surface, found 0/);
  f.gpu.reset(true);
  assert.deepEqual(f.gpu.handle({ id: 2 }), { error: 'view 2 has no world' });
});
