import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

// Run the production lazy module and applyBatch with a deterministic GPU and
// presenter. The runner's returned batch addresses the *final* outer tree.
async function fixture() {
  const views = new Map(), records = [], diagnostics = [];
  let next = 0, hud = null, expectedView = null;
  const changed = new Map();
  const exact = { views, root: { dataset: {} }, now: () => 0, devAssets: [],
    writeIn: text => text, wasm: { exact_surface_record(text) {
      records.push(text);
      return { ops: [() => {
        if (expectedView !== null) assert.ok(views.has(expectedView), 'recommit addressed a view before create');
        hud = text;
      }] };
    } }, send: batch => applyBatch(batch),
  };
  const gpu = { gpu_load() {}, gpu_seekable() {}, gpu_shader_names: () => '[]', gpu_shaders_clear() {},
    gpu_create: () => ++next, gpu_bind_at(id) { changed.set(id, JSON.stringify({ value: id })); return true; },
    gpu_published(id) { const r = changed.get(id); changed.delete(id); return r; },
    gpu_messages: () => undefined, gpu_wants_input: () => false, gpu_destroy() {},
  };
  const glue = readFileSync(new URL('../glue.js', import.meta.url), 'utf8');
  const applySource = glue.slice(glue.indexOf('function applyBatch(batch)'), glue.indexOf('\nfunction send(', glue.indexOf('function applyBatch(batch)')));
  const applyBatch = new Function('globalThis', 'apply', `const agentMode = false; ${applySource}; return applyBatch;`)({ exact }, batch => { for (const op of batch.ops) op(); });
  const source = readFileSync(new URL('../gpu-glue.js', import.meta.url), 'utf8')
    .replace('import init, * as gpu from "./gpu.js";', '').replaceAll('import.meta.url', '"http://fixture/"');
  await new (Object.getPrototypeOf(async function() {}).constructor)(
    'globalThis', 'gpu', 'init', 'document', 'devicePixelRatio', 'ResizeObserver', 'requestAnimationFrame', 'location', 'console',
    source
  )({ exact }, gpu, async () => {}, { createElement: () => ({}), head: { append() {} } }, 1,
    class { observe() {} disconnect() {} }, () => 1, { search: '' }, { error: (...args) => diagnostics.push(args.join(' ')) });
  function create(id, name = 'world') {
    const el = { isConnected: true, matches: () => true, style: {}, getBoundingClientRect: () => ({ width: 10, height: 10 }) };
    views.set(id, el); exact.gpu.surface(id, name, []);
  }
  function destroy(id) { views.delete(id); exact.gpu.destroy(id); }
  return { exact, records, diagnostics, create, destroy, applyBatch,
    expectView: id => { expectedView = id; }, stale: () => { hud = 'stale'; }, hud: () => hud };
}

test('destroy/create publications wait for the outermost apply and drain before return', async () => {
  const f = await fixture();
  f.create(1);
  f.expectView(2);
  f.applyBatch({ ops: [() => f.destroy(1), () => f.applyBatch({ ops: [() => f.create(2)] }), f.stale] });
  assert.deepEqual(f.records, ['world\0{"value":1}', 'world', 'world\0{"value":2}']);
  assert.equal(f.hud(), 'world\0{"value":2}');
});

test('first live canvas alone publishes and clears, with one named duplicate diagnostic', async () => {
  const f = await fixture();
  f.create(1); f.create(2);
  f.exact.gpu.surface(2, 'world', []);
  assert.deepEqual(f.records, ['world\0{"value":1}']);
  assert.equal(f.diagnostics.length, 1);
  assert.match(f.diagnostics[0], /world.*duplicate/);
  f.destroy(2);
  assert.equal(f.records.length, 1);
  f.create(3); f.destroy(1);
  assert.equal(f.records.at(-1), 'world');
  f.exact.gpu.surface(3, 'world', []);
  assert.equal(f.records.at(-1), 'world', 'an ignored instance does not silently take ownership');
  f.destroy(3); f.create(4);
  assert.equal(f.records.at(-1), 'world\0{"value":4}');
});
