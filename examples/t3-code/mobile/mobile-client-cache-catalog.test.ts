import { expect, test } from 'bun:test';
import { obj, type Obj } from './shared/domain';
import type { Native } from './shared/protocol';
import { encodeMobileShellCache } from './mobile-client-cache-codec';
import { encodeMobileCatalogPayload, decodeMobileCatalogPayload, mobileCacheCatalogCurrent, mobileCacheCatalogIdentity, mobileCacheCatalogPrepare } from './mobile-client-cache-catalog';

function fixture() {
  const owner = {}, calls: Obj[] = [];
  let rows: Obj[] = [{ environmentId: 'one', origin: 'https://old.invalid' }];
  const native: Native = { available: true, watch() {}, async later(request) {
    calls.push(obj(request)); return { ok: true, generation: 0, value: { removed: 1 } };
  } };
  return { owner, calls, native, saved: () => rows, replace() { rows = [{ environmentId: 'one', origin: 'https://new.invalid' }]; } };
}
test('focused and fleet consumers sharing the client acknowledge a replacement only once', async () => {
  const f = fixture();
  expect(mobileCacheCatalogCurrent(f.owner, f.saved(), 'one')).toBe(true); f.replace();
  expect(mobileCacheCatalogCurrent(f.owner, f.saved(), 'one')).toBe(false);
  await mobileCacheCatalogPrepare(f.owner, f.native, f.saved);
  await mobileCacheCatalogPrepare(f.owner, f.native, f.saved);
  expect(f.calls).toEqual([{ op: 'mobileClientCache', action: 'clear', environmentId: 'one' }]);
  expect(mobileCacheCatalogCurrent(f.owner, f.saved(), 'one')).toBe(true);
});
test('unavailable native retains replacement cleanup even without any retained display', async () => {
  const f = fixture(); await mobileCacheCatalogPrepare(f.owner, null, f.saved); f.replace();
  await mobileCacheCatalogPrepare(f.owner, null, f.saved);
  expect(mobileCacheCatalogCurrent(f.owner, f.saved(), 'one')).toBe(false);
  await mobileCacheCatalogPrepare(f.owner, f.native, f.saved);
  expect(f.calls).toHaveLength(1); expect(mobileCacheCatalogCurrent(f.owner, f.saved(), 'one')).toBe(true);
});
test('concurrent cleanup does not grant reads or issue a duplicate native clear', async () => {
  const f = fixture(), finish = Promise.withResolvers<void>(), original = f.native.later;
  await mobileCacheCatalogPrepare(f.owner, null, f.saved); f.replace();
  f.native.later = async request => { await finish.promise; return original(request); };
  const first = mobileCacheCatalogPrepare(f.owner, f.native, f.saved);
  await mobileCacheCatalogPrepare(f.owner, f.native, f.saved);
  expect(mobileCacheCatalogCurrent(f.owner, f.saved(), 'one')).toBe(false);
  finish.resolve(); await first;
  expect(f.calls).toHaveLength(1); expect(mobileCacheCatalogCurrent(f.owner, f.saved(), 'one')).toBe(true);
});
test('a second replacement during native clear needs its own cleanup acknowledgment', async () => {
  const f = fixture(), finish = Promise.withResolvers<void>(), original = f.native.later;
  await mobileCacheCatalogPrepare(f.owner, null, f.saved); f.replace();
  f.native.later = async request => { await finish.promise; return original(request); };
  const first = mobileCacheCatalogPrepare(f.owner, f.native, f.saved);
  f.saved()[0]!.origin = 'https://third.invalid'; finish.resolve(); await first;
  expect(mobileCacheCatalogCurrent(f.owner, f.saved(), 'one')).toBe(false);
  await mobileCacheCatalogPrepare(f.owner, f.native, f.saved);
  expect(f.calls).toHaveLength(2); expect(mobileCacheCatalogCurrent(f.owner, f.saved(), 'one')).toBe(true);
});
test('lost cleanup answer propagates and never starts cleanup for another environment', async () => {
  const f = fixture(); f.saved().push({ environmentId: 'two', origin: 'https://two.invalid' });
  await mobileCacheCatalogPrepare(f.owner, null, f.saved); f.replace();
  f.native.later = async request => { f.calls.push(obj(request)); throw { name: 'FetchError', kind: 'Aborted' }; };
  await expect(mobileCacheCatalogPrepare(f.owner, f.native, f.saved)).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.calls).toHaveLength(1); expect(mobileCacheCatalogCurrent(f.owner, f.saved(), 'one')).toBe(false);
});
test('normalizing saved route whitespace or disabling an environment preserves its identity', async () => {
  const f = fixture(); await mobileCacheCatalogPrepare(f.owner, null, f.saved);
  const before = mobileCacheCatalogIdentity(f.saved(), 'one');
  f.saved()[0]!.origin = ' https://old.invalid/// '; f.saved()[0]!.enabled = false;
  expect(mobileCacheCatalogIdentity(f.saved(), 'one')).toBe(before);
  await mobileCacheCatalogPrepare(f.owner, f.native, f.saved);
  expect(f.calls).toHaveLength(0); expect(mobileCacheCatalogCurrent(f.owner, f.saved(), 'one')).toBe(true);
});


test('catalog wrapper preserves native environment and rejects mismatched outer, catalog or nested identities', () => {
  const environmentId = 'roundtrip-🧭', catalog = mobileCacheCatalogIdentity([{ environmentId, origin: 'https://source.invalid/' }], environmentId);
  const inner = encodeMobileShellCache(environmentId, { sequence: 7, projects: [], threads: [] });
  const encoded = encodeMobileCatalogPayload(catalog, inner), record = JSON.parse(encoded);
  expect(record.environmentId).toBe(environmentId);
  expect(decodeMobileCatalogPayload(encoded, catalog)).toBe(inner);
  const missing = { ...record }; delete missing.environmentId;
  const other = encodeMobileShellCache('other', { sequence: 0, projects: [], threads: [] });
  for (const changed of [missing, { ...record, environmentId: 'other' }, { ...record, catalogIdentity: '["other","https://source.invalid"]' },
    { ...record, payload: other }, { ...record, payload: '{}' }, { ...record, payload: 'null' }, { ...record, payload: 'invalid-json' }])
    expect(decodeMobileCatalogPayload(JSON.stringify(changed), catalog)).toBeNull();
  expect(() => encodeMobileCatalogPayload(catalog, other)).toThrow('environment changed');
  expect(() => encodeMobileCatalogPayload(catalog, '{}')).toThrow('environment changed');
  for (const invalid of ['', '{}', '["roundtrip-🧭"]', '["roundtrip-🧭","https://source.invalid",0]',
    '["roundtrip-🧭","https://source.invalid/"]', '[" roundtrip-🧭","https://source.invalid"]']) {
    expect(() => encodeMobileCatalogPayload(invalid, inner)).toThrow();
    expect(decodeMobileCatalogPayload(encoded, invalid)).toBeNull();
  }
});
