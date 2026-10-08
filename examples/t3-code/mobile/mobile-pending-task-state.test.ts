// @ref llp/1109.005-composer-and-transcript.decision.md#local-outbox-storage
import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import type { Files } from './shared/protocol';
import { mobilePendingTaskEditorKey as key, mobilePendingTaskEditorsHydrate as hydrate,
  mobilePendingTaskEditorsPersisted as persisted, mobilePendingTaskEditorsSnapshot as snapshot,
  mobilePendingTaskEditorsReady as ready, mobilePendingTaskEditorsCreate as create,
  mobilePendingTaskEditorsReplace as replace, mobilePendingTaskEditorsRemove as remove,
  type MobilePendingTaskMarker } from './mobile-pending-task-state';

const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value));
function marker(messageId = 'message'): MobilePendingTaskMarker {
  const owner = { origin: 'https://home.test', environmentId: 'env', threadId: 'thread', messageId, commandId: `command-${messageId}` };
  return { version: 1, owner, session: 'route-session-1', revision: 1, draftKey: `new-task:pending-${messageId}`, contentRevision: 0,
    baseline: { token: 'epoch:1', revision: 1, record: { schemaVersion: 1, ...owner, text: 'Original', attachments: [],
      createdAt: '2026-10-08T12:00:00.000Z', creation: { projectId: 'project', workspaceMode: 'local', branch: null, worktreePath: null },
      context: { version: 1, records: [{ version: 1, contextId: 'ref', kind: 'terminal', label: 'Log', text: 'Saved output' }] } } }, pending: null };
}
function opened() { const client = new T3Client(); hydrate(client, {}); return client; }
const document = (value: unknown) => ({ version: 1, mobilePendingTaskEditors: value });
const envelope = (value = marker()) => ({ version: 1, markers: { [key(value.owner)]: value } });

test('explicit hydration distinguishes unread ownership from known-empty preferences', () => {
  const client = new T3Client();
  expect(snapshot(client)).toMatchObject({ hydrated: false, ready: false, blocked: true, markers: [] });
  expect(create(client, marker())).toBeNull(); expect(ready(client)).toBe(false);
  hydrate(client, {});
  expect(snapshot(client)).toEqual({ hydrated: true, ready: true, blocked: false, markers: [], errors: [] });
  expect(create(client, marker())?.revision).toBe(1);
  expect(snapshot(client)).toMatchObject({ hydrated: true, ready: true, blocked: true });
});
test('actual preference persistence JSON replays marker and pending mutation while readiness stays cold', async () => {
  const client = opened(), original = marker(); create(client, original);
  const pending = { ...original, revision: 2, contentRevision: 2, pending: { mutationId: 'epoch:2', contentRevision: 2 } };
  expect(replace(client, original, pending)).toEqual(pending);
  let bytes = new Uint8Array();
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return bytes; }, async atomicWriteFile(_path, value) { bytes = value.slice(); } } };
  await client.persist(storage);
  const saved = JSON.parse(new TextDecoder().decode(bytes)), cold = new T3Client();
  expect(saved.mobilePendingTaskEditors).toEqual(envelope(pending)); expect(ready(cold)).toBe(false);
  hydrate(cold, saved);
  expect(snapshot(cold).markers).toEqual([pending]); expect(snapshot(cold).blocked).toBe(true);
  expect(remove(cold, pending)).toBe(false);
});
test('malformed envelope and entries survive serialization and refuse all mutations', () => {
  const original = marker(), wrongOwner = clone(original); wrongOwner.owner.commandId = 'other';
  const cases: unknown[] = [null, [], 'invalid', { version: 2, markers: {} }, { version: 1, markers: [] },
    { version: 1, markers: { bad: original } }, envelope(wrongOwner), envelope({ ...original, revision: 0 }),
    envelope({ ...original, contentRevision: -1 }), envelope({ ...original, revision: Number.MAX_SAFE_INTEGER + 1 }),
    envelope({ ...original, pending: { mutationId: 'epoch:2', contentRevision: 1 } }),
    envelope({ ...original, draftKey: 'new-task:unrelated' }), { ...envelope(), future: true }];
  for (const raw of cases) {
    const client = new T3Client(); hydrate(client, document(raw));
    expect(snapshot(client)).toMatchObject({ hydrated: true, ready: false, blocked: true });
    expect(persisted(client)).toEqual(raw); expect(create(client, marker('other'))).toBeNull();
    expect(replace(client, original, { ...original, revision: 2 })).toBeNull(); expect(remove(client, original)).toBe(false);
    expect(JSON.parse(JSON.stringify(client.local)).mobilePendingTaskEditors).toEqual(raw);
  }
});
test('readable siblings remain visible but invalid sibling prevents partial writes', () => {
  const original = marker(), raw = { version: 1, markers: { [key(original.owner)]: original, broken: { version: 9 } } };
  const client = opened(); client.local = new T3Client().local; hydrate(client, document(raw));
  expect(snapshot(client).markers).toEqual([original]); expect(snapshot(client).ready).toBe(false);
  expect(remove(client, original)).toBe(false); expect(persisted(client)).toEqual(raw);
});
test('create, hydration, snapshots and returned replacements never alias caller content', () => {
  const client = opened(), original = marker(), returned = create(client, original)!;
  original.baseline.record.text = 'caller changed'; returned.baseline.record.context!.records = [];
  const read = snapshot(client); read.markers[0]!.owner.origin = 'https://different.test';
  const saved = persisted(client) as ReturnType<typeof envelope>; saved.markers[key(marker().owner)]!.session = 'changed';
  expect(snapshot(client).markers).toEqual([marker()]);
  const raw = envelope(), cold = new T3Client(); hydrate(cold, document(raw)); raw.markers[key(marker().owner)]!.baseline.record.text = 'changed';
  expect(snapshot(cold).markers).toEqual([marker()]);
});
test('exact owner/session/revision CAS rejects old session and old completion after reopen', () => {
  const client = opened(), first = marker(); create(client, first);
  const successor = { ...first, revision: 2, session: 'route-session-2', contentRevision: 1 };
  expect(replace(client, first, successor)).toEqual(successor);
  expect(replace(client, first, { ...first, revision: 2, contentRevision: 1 })).toBeNull();
  expect(remove(client, first)).toBe(false);
  expect(remove(client, { ...successor, session: first.session })).toBe(false);
  expect(remove(client, { ...successor, owner: { ...successor.owner, commandId: 'forged' } })).toBe(false);
  expect(remove(client, successor)).toBe(true); expect(snapshot(client).blocked).toBe(false);
});
test('baseline only advances for exact original creation owner and revision never regresses or overflows', () => {
  const client = opened(), original = marker(); create(client, original);
  const changed = clone(original); changed.revision++; changed.baseline.record.text = 'Edited';
  expect(replace(client, original, changed)).toBeNull();
  changed.baseline.revision++; changed.baseline.token = 'epoch:2'; changed.contentRevision = 1;
  expect(replace(client, original, changed)).toEqual(changed);
  const moved = clone(changed); moved.revision++; moved.baseline.revision++; moved.baseline.record.commandId = 'different';
  expect(replace(client, changed, moved)).toBeNull();
  expect(replace(client, changed, { ...changed, revision: 3, contentRevision: 0 })).toBeNull();
  const max = { ...changed, revision: Number.MAX_SAFE_INTEGER };
  const cold = new T3Client(); hydrate(cold, document(envelope(max)));
  expect(replace(cold, max, { ...max, revision: max.revision + 1 })).toBeNull();
});
test('same native message cannot gain a second marker through another environment', () => {
  const client = opened(), first = marker(); create(client, first);
  const second = clone(first); second.owner.environmentId = 'other'; second.baseline.record.environmentId = 'other';
  expect(create(client, second)).toBeNull();
  const cold = new T3Client(); hydrate(cold, document({ version: 1, markers: { [key(first.owner)]: first, [key(second.owner)]: second } }));
  expect(snapshot(cold)).toMatchObject({ hydrated: true, ready: false, blocked: true });
});
test('late hydration does not erase live edits and replacement local requires a fresh read', () => {
  const client = opened(); create(client, marker()); hydrate(client, {});
  expect(snapshot(client).markers).toEqual([marker()]);
  client.local = new T3Client().local;
  expect(snapshot(client)).toMatchObject({ hydrated: false, ready: false, blocked: true });
  hydrate(client, {}); expect(snapshot(client).blocked).toBe(false);
});
test('a saved native completion can advance baseline while preserving newer editor content', () => {
  const client = opened(), first = marker(); create(client, first);
  const pending = { ...first, revision: 2, contentRevision: 1, pending: { mutationId: 'epoch:2', contentRevision: 1 } };
  expect(replace(client, first, pending)).toEqual(pending);
  const typed = { ...pending, revision: 3, contentRevision: 2 };
  expect(replace(client, pending, typed)).toEqual(typed);
  expect(remove(client, typed)).toBe(false);
  const completed = { ...typed, revision: 4, pending: null, baseline: { ...typed.baseline, token: 'epoch:2', revision: 2,
    record: { ...typed.baseline.record, text: 'Content revision one' } } };
  expect(replace(client, typed, completed)).toEqual(completed);
  expect(snapshot(client).markers[0]).toMatchObject({ contentRevision: 2, pending: null, baseline: { record: { text: 'Content revision one' } } });
  expect(replace(client, pending, { ...pending, revision: 3, pending: null })).toBeNull();
  expect(snapshot(client).blocked).toBe(true);
});
