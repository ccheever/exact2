import { expect, test } from 'bun:test';
import { MobileDraftClient, mobileDraftRecoveryHandles } from './mobile-draft-recovery';
import { mobileNewTaskDraftCreate as create, mobileNewTaskDraftBind as bind, mobileNewTaskDraftStore as store,
  mobileNewTaskDraftPresentation as presentation, mobileNewTaskDraftRetarget as retarget,
  mobileNewTaskDraftDiscard as discard } from './mobile-new-task-drafts';
import { mobileNewTaskContextGuard as guard, mobileNewTaskContextWrite as write, mobileNewTaskContextRead as read,
  mobileNewTaskContextProject as project, mobileCreateContextHistory, mobileReferencedComposerContext } from './mobile-new-task-context';
import { obj, type Obj } from './shared/domain';
import { contextLink } from './shared/composer-editor-menu';
import type { Files, Native } from './shared/protocol';
const A = 'new-task:A', B = 'new-task:B', origin = 'https://context.test';
const terminal = (id = 'terminal_a', text = 'captured original'): Obj => ({ version: 1, kind: 'terminal', contextId: id,
  label: 'Terminal', terminalId: 'pane-a', terminalLabel: 'Terminal', lineStart: 1, lineEnd: 2, text });
const link = (record: Obj) => contextLink(String(record.kind), String(record.contextId), String(record.label));
function fixture(document: Obj = { version: 1 }) {
  const client = new MobileDraftClient(); let disk = JSON.stringify(document), fail = false;
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode(disk).buffer; },
    async atomicWriteFile(_path, bytes) { if (fail) throw new Error('disk failure'); disk = new TextDecoder().decode(bytes); } } };
  const native: Native = { available: true, watch() {}, async later(input) {
    return { ok: true, generation: 1, value: obj(input).op === 'status' ? { phase: 'disconnected' } : {} };
  } };
  const load = async () => {
    const handles = mobileDraftRecoveryHandles(client, native, storage);
    await client.refresh(handles.native, handles.storage);
    Object.assign(client, { environmentId: 'env', origin, projectId: 'project', threadId: '', generation: 1 });
  };
  return { client, storage, native, load, disk: () => obj(JSON.parse(disk)), fail(value = true) { fail = value; } };
}
async function setup() {
  const f = fixture(); await f.load();
  for (const id of ['A', 'B']) create(f.client, { id, origin, environmentId: 'env', projectId: 'project', createdAt: '2026-10-08T00:00:00.000Z' });
  bind(f.client, A, 'flow-A'); return f;
}
function edit(client: MobileDraftClient, key: string, text: string, record?: Obj, removeId = '') {
  return write(client, guard(client, key)!, text, record, removeId);
}
test('actual client persist/load owns same context ID independently and presentation cannot mutate it', async () => {
  const f = await setup(), a = terminal(), b = terminal('terminal_a', 'other draft');
  expect(edit(f.client, A, link(a), a)).toBe(true); expect(edit(f.client, B, link(b), b)).toBe(true);
  await f.client.persist(f.storage);
  const r = fixture(f.disk()); await r.load();
  expect(read(r.client, A)).toEqual({ ok: true, context: { version: 1, records: [a] } });
  expect(read(r.client, B)).toEqual({ ok: true, context: { version: 1, records: [b] } });
  const shown = presentation(r.client, A)!; obj((obj(shown.context).records as unknown[])[0]).text = 'mutated';
  expect(read(r.client, A)).toEqual({ ok: true, context: { version: 1, records: [a] } });
  expect(shown.text).toBe(link(a));
});
test('guard rejects old revision, ABA and new incarnation without changing text or context', async () => {
  const f = await setup(), original = guard(f.client, A)!, a = terminal();
  expect(edit(f.client, A, 'one')).toBe(true); expect(edit(f.client, A, '')).toBe(true);
  expect(write(f.client, original, link(a), a)).toBe(false); expect(f.client.local.drafts[A]).toBe('');
  const captured = guard(f.client, A)!; expect(discard(f.client, A)).toBe(true);
  create(f.client, { id: 'A', origin, environmentId: 'env', projectId: 'project', createdAt: '2026-10-08T00:00:01.000Z' });
  store(f.client).records[A].revision = captured.revision;
  expect(write(f.client, captured, link(a), a)).toBe(false); expect(read(f.client, A)).toEqual({ ok: true, context: undefined });
});
test('retarget keeps captured attribution and advances the insertion guard', async () => {
  const f = await setup(), record = { version: 1, kind: 'thread', contextId: 'thread_same', label: 'Thread', title: 'Thread', environmentId: 'original', threadId: 'same' };
  edit(f.client, A, link(record), record); const captured = guard(f.client, A)!;
  expect(retarget(f.client, A, { environmentId: 'other', projectId: 'other', origin: 'https://other.test' })).toBe(true);
  expect(read(f.client, A)).toEqual({ ok: true, context: { version: 1, records: [record] } });
  expect(write(f.client, captured, 'stale')).toBe(false);
});
test('failed preference save retains atomic content; successful restart restores exact payload', async () => {
  const f = await setup(), a = terminal(); edit(f.client, A, link(a), a); f.fail();
  await expect(f.client.persist(f.storage)).rejects.toThrow('disk failure');
  expect(presentation(f.client, A)?.text).toBe(link(a)); expect(read(f.client, A)).toEqual({ ok: true, context: { version: 1, records: [a] } });
  f.fail(false); await f.client.persist(f.storage); const r = fixture(f.disk()); await r.load();
  expect(read(r.client, A)).toEqual(read(f.client, A)); expect(guard(r.client, A)?.revision).toBe(1);
});
test('malformed saved context survives actual hydration and typing; producers cannot erase it', async () => {
  for (const malformed of [null, 'raw', [], { version: 2, records: [] }, { version: 1, records: [terminal(), terminal()] },
    { version: 1, records: [{ version: 1, kind: 'future', contextId: 'unknown', label: 'Future' }] }]) {
    const f = await setup(); f.client.local.drafts[A] = 'keep'; store(f.client).records[A].context = malformed;
    await f.client.persist(f.storage); const r = fixture(f.disk()); await r.load();
    expect(presentation(r.client, A)?.context).toEqual(malformed); expect(read(r.client, A).ok).toBe(false);
    expect(edit(r.client, A, 'typed')).toBe(true); expect(edit(r.client, A, link(terminal()), terminal())).toBe(false);
    expect(edit(r.client, A, '', undefined, 'unknown')).toBe(false); await r.client.persist(r.storage);
    expect(obj(obj(obj(r.disk().mobileNewTaskDrafts).records)[A]).context).toEqual(malformed);
    expect(r.client.local.drafts[A]).toBe('typed');
  }
});
test('remove and undo are per record, ephemeral, and updates replace the same payload ID', async () => {
  const f = await setup(), a = terminal(), replacement = terminal('terminal_a', 'edited');
  edit(f.client, A, link(a), a); edit(f.client, A, link(a), replacement);
  expect(read(f.client, A)).toEqual({ ok: true, context: { version: 1, records: [replacement] } });
  edit(f.client, A, 'removed', undefined, 'terminal_a');
  await f.client.persist(f.storage);
  expect(obj(obj(obj(f.disk().mobileNewTaskDrafts).records)[A]).context).toEqual({ version: 1, records: [] });
  edit(f.client, A, link(a)); expect(read(f.client, A)).toEqual({ ok: true, context: { version: 1, records: [replacement] } });
  edit(f.client, B, link(a)); expect(read(f.client, B)).toEqual({ ok: true, context: { version: 1, records: [] } });
  const r = fixture(f.disk()); await r.load(); edit(r.client, A, link(a));
  expect(read(r.client, A)).toEqual({ ok: true, context: { version: 1, records: [] } });
});
test('discard/recreate does not revive undo payloads', async () => {
  const f = await setup(), a = terminal(); edit(f.client, A, link(a), a); edit(f.client, A, 'gone'); discard(f.client, A);
  create(f.client, { id: 'A', origin, environmentId: 'env', projectId: 'project', createdAt: '2026-10-08T00:00:00.000Z' });
  edit(f.client, A, link(a)); expect(read(f.client, A)).toEqual({ ok: true, context: { version: 1, records: [] } });
});
test('history caps undo-only records at 200 while preserving oversized live recovery', () => {
  const history = mobileCreateContextHistory();
  for (let i = 0; i < 201; i++) history('', { version: 1, records: [terminal(`t_${i}`)] });
  expect(history(link(terminal('t_0')))).toBeUndefined();
  expect(history(link(terminal('t_1')))?.records.map(record => record.contextId)).toEqual(['t_1']);
  const records = Array.from({ length: 201 }, (_, i) => terminal(`live_${i}`)), text = records.map(link).join(' ');
  expect(history(text, { version: 1, records })?.records).toHaveLength(201);
  expect(project(text, { version: 1, records }).ok).toBe(false);
});
test('pure reference projection preserves source ordering, empty shape and screenshot dependencies', () => {
  const image = { version: 1, kind: 'image', contextId: 'shot', label: 'Image' }, preview = { version: 1, kind: 'preview-annotation', contextId: 'note', label: 'Note', screenshotContextId: 'shot' };
  const context = { version: 1 as const, records: [image, preview] };
  expect(mobileReferencedComposerContext(link(preview), context)).toBe(context);
  expect(mobileReferencedComposerContext('', context)).toBeUndefined();
  const empty = { version: 1 as const, records: [] }; expect(mobileReferencedComposerContext('', empty)).toBe(empty);
  expect(project(link(preview), context).ok).toBe(false);
  expect(project('', null).ok).toBe(false); expect(project('', undefined)).toEqual({ ok: true, context: undefined });
});
test('rejected invalid insertion does not alter draft text, revision or valid payload', async () => {
  const f = await setup(), a = terminal(); edit(f.client, A, link(a), a);
  const before = presentation(f.client, A); expect(edit(f.client, A, 'wrong', { ...a, lineStart: -1 })).toBe(false);
  expect(presentation(f.client, A)).toEqual(before);
});
