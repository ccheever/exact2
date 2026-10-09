import { expect, test } from 'bun:test';
import { MobileClientStorageState, mobileStorageBytes, mobileStorageAggregate, mobileInformationSnapshot, MOBILE_STORAGE_ERROR } from './settings-information';
import { obj, type Obj } from './shared/domain';
import type { Native } from './shared/protocol';
import type { MobileCacheSummaryRow } from './mobile-client-cache';

function fixture() {
  const state = new MobileClientStorageState(), calls: Obj[] = [];
  let rows: MobileCacheSummaryRow[] = [
    { environmentId: 'alpha', kind: 'shell', recordCount: 1, payloadBytes: 1000 },
    { environmentId: 'alpha', kind: 'thread', recordCount: 2, payloadBytes: 24 },
    { environmentId: 'beta', kind: 'project-favicon', recordCount: 1, payloadBytes: 20 },
  ];
  let choice = 'clear', fail = '';
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    const good = (value: unknown) => ({ ok: true, generation: 0, value });
    if (request.op === 'mobileAlert') return good({ choice });
    if (request.op !== 'mobileClientCache') throw new Error('Unexpected protected-store or server request');
    if (request.action === fail) throw new Error('Private file path must not enter UI');
    if (request.action === 'inspect') return good({ rows: rows.map(row => ({ ...row })),
      recordCount: rows.reduce((total, row) => total + row.recordCount, 0),
      payloadBytes: rows.reduce((total, row) => total + row.payloadBytes, 0) });
    if (request.action === 'clear') {
      const before = rows.reduce((total, row) => total + row.recordCount, 0);
      rows = request.environmentId === undefined ? [] : rows.filter(row => row.environmentId !== request.environmentId);
      return good({ removed: before - rows.reduce((total, row) => total + row.recordCount, 0) });
    }
    throw new Error('Unexpected cache operation');
  } };
  const environments = [{ environmentId: 'alpha', label: 'Zebra Mac', machine: 'laptop' },
    { environmentId: 'beta', label: 'Alpha server', machine: 'linux' }];
  state.enter('settingsClientStorage', 'visit-1');
  return { state, native, calls, environments,
    choice(value: string) { choice = value; }, fail(value: string) { fail = value; }, empty() { rows = []; },
    prepare: () => state.prepare(native), snapshot: () => state.snapshot(environments),
    command: (op: string, value = '') => state.command(op, value, native, { routeKey: 'visit-1', environments }) };
}
function hold(native: Native, predicate: (request: Obj) => boolean) {
  const entered = Promise.withResolvers<void>(), finish = Promise.withResolvers<void>(), original = native.later;
  native.later = async input => {
    const result = await original(input);
    if (predicate(obj(input))) { entered.resolve(); await finish.promise; }
    return result;
  };
  return { entered: entered.promise, release: () => finish.resolve() };
}

test('storage distinguishes loading, error and empty from actual inspected zero', async () => {
  const f = fixture(); expect(f.snapshot().loading).toBe(true); expect(f.snapshot().disabled).toBe(true);
  expect(f.snapshot().detail).toBe('Inspecting cached data…');
  f.fail('inspect'); await f.prepare();
  expect(f.snapshot().available).toBe(false); expect(f.snapshot().loading).toBe(false);
  expect(f.snapshot().title).toBe('Storage unavailable'); expect(f.snapshot().detail).toBe('Restart the app and try again.');
  expect(f.snapshot().error).toBe(MOBILE_STORAGE_ERROR);
  f.fail(''); f.empty(); await f.prepare();
  expect(f.snapshot().available).toBe(true); expect(f.snapshot().title).toBe('No cached data');
  expect(f.snapshot().action).toBe('Clear 0 B'); expect(f.snapshot().disabled).toBe(true); expect(f.snapshot().error).toBe('');
});
test('default storage actions use the source danger foreground, not its pale background', () => {
  const f = fixture();
  expect(f.snapshot().danger).toBe('#c10007');
  expect(mobileInformationSnapshot().storage.danger).toBe('#c10007');
});
test('source aggregation retains exact counts per kind and environment', () => {
  expect(mobileStorageAggregate({ recordCount: 3, payloadBytes: 1024, rows: [
    { environmentId: 'a', kind: 'shell', recordCount: 1, payloadBytes: 1000 },
    { environmentId: 'a', kind: 'thread', recordCount: 2, payloadBytes: 24 },
  ] })).toEqual({ recordCount: 3, payloadBytes: 1024, environments: [
    { environmentId: 'a', recordCount: 3, payloadBytes: 1024, kinds: { shell: 1, thread: 2 } },
  ] });
});
test('summary sorts by saved labels and falls back to environment identity and server icon', async () => {
  const f = fixture(); await f.prepare(); const view = f.snapshot();
  expect(view.recordCount).toBe(4); expect(view.payloadBytes).toBe(1044);
  expect(view.environments.map(row => [row.id, row.label, row.action, row.machine, row.first])).toEqual([
    ['beta', 'Alpha server', 'Clear 20 B', 'terminal', true], ['alpha', 'Zebra Mac', 'Clear 1.0 KB', 'laptopcomputer', false],
  ]);
  const unknown = f.state.snapshot([]).environments[0]!;
  expect(unknown.label).toBe('alpha'); expect(unknown.machine).toBe('server.rack');
  expect(f.state.snapshot(f.environments, true, '#123456').clearing).toBe(true);
  expect(f.state.snapshot(f.environments, true, '#123456').disabled).toBe(true);
  expect(f.state.snapshot(f.environments, false, '#123456').danger).toBe('#123456');
});
test.each([[0, '0 B'], [1023, '1023 B'], [1024, '1.0 KB'], [10239, '10.0 KB'], [10240, '10 KB'],
  [1048575, '1024 KB'], [1048576, '1.0 MB'], [10485760, '10 MB']] as const)('source bytes %s format as %s', (bytes, expected) => {
  expect(mobileStorageBytes(bytes)).toBe(expected);
});
test('environment confirmation clears only captured scope then re-inspects', async () => {
  const f = fixture(); await f.prepare(); f.calls.length = 0;
  await f.command('clear-cache-environment', 'alpha');
  expect(f.calls).toEqual([
    { op: 'mobileAlert', kind: 'clear-client-cache', title: 'Clear cache for Zebra Mac?',
      message: 'This removes offline threads, server metadata, and cached branches for this environment. The saved connection and credentials stay intact.' },
    { op: 'mobileClientCache', action: 'clear', environmentId: 'alpha' },
    { op: 'mobileClientCache', action: 'inspect' },
  ]);
  expect(f.snapshot().environments.map(row => row.id)).toEqual(['beta']); expect(f.snapshot().payloadBytes).toBe(20);
});
test('global confirmation uses source copy and leaves actual empty summary', async () => {
  const f = fixture(); await f.prepare(); f.calls.length = 0; await f.command('clear-cache-all');
  expect(f.calls[0]).toEqual({ op: 'mobileAlert', kind: 'clear-client-caches', title: 'Clear all client caches?',
    message: 'This removes offline data for every environment. Connections, credentials, account data, and app preferences stay intact.' });
  expect(f.calls[1]).toEqual({ op: 'mobileClientCache', action: 'clear' });
  expect(f.snapshot().environments).toEqual([]); expect(f.snapshot().recordCount).toBe(0);
});
test.each(['cancel', 'remove', ''])('confirmation choice %s does not clear', async choice => {
  const f = fixture(); await f.prepare(); f.calls.length = 0; f.choice(choice); await f.command('clear-cache-all');
  expect(f.calls).toHaveLength(1); expect(f.snapshot().recordCount).toBe(4); expect(f.snapshot().error).toBe('');
});
test('unknown environment and stale route never reach native confirmation', async () => {
  const f = fixture(); await f.prepare(); f.calls.length = 0;
  await f.command('clear-cache-environment', 'forged');
  await f.state.command('clear-cache-all', '', f.native, { routeKey: 'stale' });
  f.state.enter('settingsAbout', 'visit-2'); await f.command('clear-cache-all'); expect(f.calls).toEqual([]);
});
test('route departure and reopen refuse a late destructive confirmation', async () => {
  const f = fixture(); await f.prepare(); f.calls.length = 0;
  const held = hold(f.native, request => request.op === 'mobileAlert'), pending = f.command('clear-cache-all');
  await held.entered; f.state.enter('home', 'other'); f.state.enter('settingsClientStorage', 'visit-2'); held.release(); await pending;
  expect(f.calls).toHaveLength(1); expect(f.snapshot().recordCount).toBe(4);
});
test('duplicate clears are suppressed while confirmation is pending', async () => {
  const f = fixture(); await f.prepare(); f.calls.length = 0;
  const held = hold(f.native, request => request.op === 'mobileAlert'), pending = f.command('clear-cache-all');
  await held.entered; await f.command('clear-cache-environment', 'alpha'); held.release(); await pending;
  expect(f.calls.filter(row => row.op === 'mobileAlert')).toHaveLength(1);
  expect(f.calls.filter(row => row.action === 'clear')).toHaveLength(1);
});
test('runtime clear callback is awaited once before inspecting and retains inline failure', async () => {
  const f = fixture(); await f.prepare(); f.calls.length = 0;
  const entered = Promise.withResolvers<void>(), release = Promise.withResolvers<void>();
  const pending = f.state.command('clear-cache-environment', 'alpha', f.native, { routeKey: 'visit-1',
    clearCache: async (native, environmentId) => { expect(native).toBe(f.native); expect(environmentId).toBe('alpha');
      entered.resolve(); await release.promise; throw new Error('Sensitive disk details'); } });
  await entered.promise; expect(f.snapshot().clearing).toBe(true); expect(f.snapshot().disabled).toBe(true);
  expect(f.calls).toHaveLength(1); release.resolve(); await pending;
  expect(f.snapshot().clearing).toBe(false); expect(f.snapshot().recordCount).toBe(4);
  expect(f.snapshot().error).toBe(MOBILE_STORAGE_ERROR); expect(f.calls).toHaveLength(1);
});
test('abandoned clear propagates and cannot continue to another native operation', async () => {
  const f = fixture(); await f.prepare(); f.calls.length = 0;
  await expect(f.state.command('clear-cache-all', '', f.native, { clearCache: async () => {
    throw { name: 'FetchError', kind: 'Aborted' };
  } })).rejects.toMatchObject({ name: 'FetchError', kind: 'Aborted' });
  expect(f.calls).toHaveLength(1); expect(f.snapshot().clearing).toBe(false); expect(f.snapshot().error).toBe('');
});
test('late inspection cannot replace a newer route visit result', async () => {
  const f = fixture(), held = hold(f.native, request => request.action === 'inspect'), pending = f.prepare();
  await held.entered; f.state.enter('home', 'home'); held.release(); await pending;
  expect(f.snapshot().available).toBe(false);
});
