import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';
import { glueDeclarations } from './glue-fixture.mjs';

const source = readFileSync(new URL('../glue.js', import.meta.url), 'utf8');
const declarations = glueDeclarations(source, ['bootNow', 'mediaChanged', 'pageChanged']);

// Run the host through its ABI call. The stand-in accepts only a positive
// viewport, then stops before DOM adoption, which the built-app test covers.
function host(width, height) {
  const listeners = [], calls = [], accepted = new Error('accepted');
  const accept = (kind, w, h) => {
    calls.push([kind, w, h]);
    if (!(w > 0 && h > 0)) throw new Error('Runner(InvalidViewport)');
    throw accepted;
  };
  const state = {
    innerWidth: width, innerHeight: height, bootAttempt: 0, incarnation: 0,
    performance, TextEncoder, Uint8Array, encoder: new TextEncoder(), exact: {},
    memory: { buffer: new ArrayBuffer(100) }, devAssets: null, page: null, agentMode: true,
    document: { activeElement: null }, root: { childElementCount: 1 },
    addEventListener: (kind, fn) => { if (kind === 'resize') listeners.push(fn); },
    requestAnimationFrame: () => {}, positionContexts: () => {},
    readOut: value => value, prepareFonts: async () => [],
    launchLocation: () => '/', preferences: () => 0, foldBits: () => 0, now: () => 0,
    loadStage: async () => {}, focus: { keep: () => null }, ask: () => ({}),
    waitForInflight: async () => true, SETTLE_DEADLINE_MS: 1000,
    pageFacts: { bits: () => 0 }, applyBatch: () => {}, presence: { resize: x => x },
    wasm: {
      exact_plan: () => 1, exact_out: () => 0, exact_in: () => 0, exact_plan_fonts: () => '[]',
      exact_boot: (w, h) => accept('boot', w, h),
      exact_boot_plan: (_, w, h) => accept('plan', w, h),
      exact_boot_module: () => accept('module', 400, 300),
      exact_resize: (w, h) => { calls.push(['resize', w, h]); return '{}'; },
      exact_set_page: () => { calls.push(['page']); return '{}'; },
    },
  };
  vm.createContext(state);
  vm.runInContext(`${declarations}\nglobalThis.boot = bootNow; globalThis.media = mediaChanged; globalThis.pageChanged = pageChanged;`, state);
  return { state, calls, listeners,
    boot: (...args) => state.boot(...args).catch(error => { if (error !== accepted) throw error; return 'accepted'; }),
    resize(w, h) { state.innerWidth = w; state.innerHeight = h; for (const fn of listeners.splice(0)) fn(); },
  };
}

// Await the host's font preparation and the continuation that either boots
// or registers its resize wait. Neither uses a timer in this fixture.
const prepared = async () => { await Promise.resolve(); await Promise.resolve(); };

test('boot waits for both dimensions, rechecks each resize, and takes no wait at a positive size', async () => {
  for (const [width, height] of [[0, 0], [0, 300], [400, 0], [400, 300]]) {
    const h = host(width, height), boot = h.boot();
    await prepared();
    if (!width || !height) {
      expect(h.calls).toEqual([]);
      expect(h.listeners).toHaveLength(1);
      h.resize(400, 0);
      await prepared();
      expect(h.calls).toEqual([]);
      expect(h.listeners).toHaveLength(1);
      h.resize(400, 300);
    }
    expect(await boot).toBe('accepted');
    expect(h.calls).toEqual([['boot', 400, 300]]);
    expect(h.listeners).toHaveLength(0);
  }
});

test('a plan reload rechecks size and generation after awaiting inspection or resize', async () => {
  for (const stale of [false, 'current', 'attempt']) {
    const h = host(400, 300);
    let resume, current = true;
    h.state.loadStage = () => new Promise(resolve => { resume = resolve; });
    const boot = h.boot(new Uint8Array([1]), null, () => current);
    await prepared();
    h.resize(0, 0);
    resume();
    await prepared();
    expect(h.calls).toEqual([]);
    expect(h.listeners).toHaveLength(1);
    if (stale === 'current') current = false;
    if (stale === 'attempt') h.state.bootAttempt++;
    h.resize(420, 320);
    expect(await boot).toBe(stale ? null : 'accepted');
    expect(h.calls).toEqual(stale ? [] : [['plan', 420, 320]]);
    expect(h.listeners).toHaveLength(0);
  }
});

test('module replacement keeps the runner viewport and does not wait for the page', async () => {
  const h = host(0, 0);
  expect(await h.boot(new Uint8Array([1]), null, () => true,
    { rust: new Uint8Array([2]), receipt: new Uint8Array([3]) })).toBe('accepted');
  expect(h.calls).toEqual([['module', 400, 300]]);
  expect(h.listeners).toHaveLength(0);
});

test('a generation retired during inspection returns without waiting for a viewport', async () => {
  const h = host(0, 0);
  let resume, current = true;
  h.state.loadStage = () => new Promise(resolve => { resume = resolve; });
  const boot = h.boot(new Uint8Array([1]), null, () => current);
  await prepared();
  current = false; resume();
  expect(await boot).toBeNull();
  expect(h.calls).toEqual([]);
  expect(h.listeners).toHaveLength(0);
});

test('pre-rendered children do not authorize runner reports; a running runner receives only positive resizes', () => {
  const h = host(400, 300);
  h.state.media(); h.state.pageChanged();
  expect(h.calls).toEqual([]);
  h.state.incarnation = 1;
  for (const [width, height] of [[0, 0], [0, 300], [400, 0]]) {
    h.resize(width, height); h.state.media();
  }
  expect(h.calls).toEqual([]);
  h.resize(420, 320); h.state.media(); h.state.pageChanged();
  expect(h.calls).toEqual([['resize', 420, 320], ['page']]);
});
