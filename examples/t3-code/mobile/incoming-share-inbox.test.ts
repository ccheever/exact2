// @ref llp/1109.005-composer-and-transcript.decision.md#incoming-share-inbox
import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import type { Native } from './shared/protocol';
import { incomingShareInboxDecode as decode, mobileIncomingShareRead as read, mobileIncomingShare as item,
  mobileIncomingShareReservation as reservation, mobileIncomingShareInboxState as snapshot,
  incomingSharePresentation as transition, mobileIncomingSharePresentation as presentation,
  incomingShareSubtitle, type IncomingShareReservation } from './incoming-share-inbox';
import type { IncomingShareEntry } from './incoming-share-model';
const A = `share-${'a'.repeat(64)}`, B = `share-${'b'.repeat(64)}`;
const file = '11111111-1111-4111-8111-111111111111', adoption = '22222222-2222-4222-8222-222222222222';
const empty = () => ({ presentedShareId: null, dismissedShareId: null });
function entry(id = A, createdAt = '2026-10-08T01:00:00.000Z'): IncomingShareEntry {
  return { schemaVersion: 1, id, instanceId: '33333333-3333-4333-8333-333333333333', createdAt, text: 'shared text',
    attachments: [{ id: file, kind: 'image', name: 'shared.png', mimeType: 'image/png', sizeBytes: 10 }], warnings: ['preserve this warning'] };
}
function reserved(phase: IncomingShareReservation['phase'] = 'reserved'): IncomingShareReservation {
  return { shareId: A, adoptionId: adoption, phase, attachmentIds: [file],
    destination: { draftKey: 'new-task:share-editor', environmentId: 'environment', projectId: 'project', origin: 'https://share.test' } };
}
function contents(entries: IncomingShareEntry[] = [entry()], reservations: IncomingShareReservation[] = [], available = true) { return { available, entries, reservations }; }
function fixture() {
  const client = new T3Client(), watches: string[] = [];
  const calls: { input: Obj; resolve(value: unknown): void; reject(error: unknown): void }[] = [];
  const native: Native = { available: true, watch(topic) { watches.push(topic); }, later(input) {
    return new Promise((resolve, reject) => { calls.push({ input: obj(input), resolve, reject }); });
  } };
  const answer = (index: number, value: unknown) => calls[index].resolve({ ok: true, generation: 1, value });
  return { client, native, calls, watches, answer, async load(value = contents()) { const promise = read(client, native); answer(calls.length - 1, value); return promise; } };
}

test('inbox decoder retains source newest-first order, warning/attachment descriptors and unavailable producer saved data', async () => {
  const f = fixture(); const older = entry(B, '2026-10-08T00:00:00.000Z');
  const state = await f.load(contents([older, entry()], [], false));
  expect(state).toMatchObject({ ready: true, available: false, error: '' });
  expect(f.watches).toEqual(['t3.incoming-shares']); expect(f.calls[0].input).toEqual({ op: 'mobileIncomingShares', action: 'read' });
  expect(item(f.client, A)).toEqual(entry()); expect(presentation(f.client, '', false, 'home').location).toBe(`/new?incomingShareId=${A}`);
});

test('native read reservation parser preserves staged and cancelling ownership with defensive copies', async () => {
  for (const phase of ['reserved', 'staged', 'cancelling'] as const) {
    const f = fixture(), raw = contents([entry()], [reserved(phase)]); await f.load(raw);
    expect(reservation(f.client, A)).toEqual(reserved(phase));
    raw.entries[0].warnings[0] = 'mutated input'; raw.reservations[0].attachmentIds.length = 0;
    const saved = reservation(f.client, A), share = item(f.client, A); if (!saved || !share) throw Error('Expected saved entry');
    saved.destination.projectId = 'mutated output'; saved.attachmentIds.length = 0; share.text = 'mutated'; share.attachments[0].name = 'mutated';
    expect(item(f.client, A)).toEqual(entry()); expect(reservation(f.client, A)).toEqual(reserved(phase));
  }
});

test('decoder refuses malformed entries, repeated identities and unowned or overlapping reservations atomically', async () => {
  const invalid: unknown[] = [null, {}, { ...contents(), available: 'yes' },
    { ...contents(), entries: [entry(), entry()] },
    { ...contents(), entries: [{ ...entry(), schemaVersion: 2 }] },
    { ...contents(), entries: [{ ...entry(), attachments: [entry().attachments[0], entry().attachments[0]] }] },
    { ...contents(), reservations: [{ ...reserved(), shareId: B }] },
    { ...contents(), reservations: [{ ...reserved(), attachmentIds: [adoption] }] },
    { ...contents(), reservations: [{ ...reserved(), attachmentIds: [file, file] }] },
    { ...contents(), reservations: [{ ...reserved(), phase: 'consumed' }] },
    { ...contents(), reservations: [{ ...reserved(), destination: { ...reserved().destination, draftKey: 'new-task:pending-message' } }] },
    { ...contents(), reservations: [reserved(), reserved()] },
    { ...contents([entry(), entry(B)]), reservations: [reserved(), { ...reserved(), shareId: B }] },
  ];
  const f = fixture(); await f.load(contents([entry()], [reserved()]));
  for (const value of invalid) {
    expect(() => decode(value)).toThrow(); const reading = read(f.client, f.native); f.answer(f.calls.length - 1, value); const state = await reading;
    expect(state.error).not.toBe(''); expect(item(f.client, A)).toEqual(entry()); expect(reservation(f.client, A)).toEqual(reserved());
  }
  await f.load(contents([entry(B)])); expect(snapshot(f.client).error).toBe(''); expect(item(f.client, A)).toBeNull(); expect(item(f.client, B)).not.toBeNull();
});

test('late old read or failure cannot restore consumed content or replace a newer successful read', async () => {
  const f = fixture(); await f.load();
  const old = read(f.client, f.native), fresh = read(f.client, f.native); f.answer(2, contents([])); await fresh;
  const revision = snapshot(f.client).revision; f.answer(1, contents()); await old;
  expect(item(f.client, A)).toBeNull(); expect(snapshot(f.client).revision).toBe(revision);
  const failing = read(f.client, f.native), next = read(f.client, f.native); f.answer(4, contents([entry(B)])); await next;
  f.calls[3].reject(Error('late error')); await failing;
  expect(item(f.client, B)).not.toBeNull(); expect(snapshot(f.client).error).toBe('');
});

test('producer handle unavailable preserves known inbox and invalidates an older pending answer', async () => {
  const f = fixture(); await f.load(); const stale = read(f.client, f.native);
  const state = await read(f.client, { available: false, watch() { throw Error('must not watch'); }, async later() { throw Error('must not read'); } });
  expect(state.available).toBe(false); expect(item(f.client, A)).toEqual(entry());
  f.answer(1, contents([])); await stale; expect(item(f.client, A)).toEqual(entry());
  expect(presentation(f.client, '', false, 'home').location).not.toBe('');
});

test('expired answer propagates cancellation and never replaces readable inbox; watch failure becomes retained read error', async () => {
  const f = fixture(); await f.load(); const cancelled = read(f.client, f.native);
  f.calls[1].reject({ name: 'FetchError', kind: 'Aborted' }); await expect(cancelled).rejects.toMatchObject({ kind: 'superseded' });
  expect(item(f.client, A)).toEqual(entry());
  const state = await read(f.client, { available: true, watch() { throw Error('watch failed'); }, async later() { throw Error('must not read'); } });
  expect(state.error).toBe('watch failed'); expect(item(f.client, A)).toEqual(entry());
});

test('durable ID dismissal survives refreshed object identity and only replacement or consumption resets it', async () => {
  const f = fixture(); await f.load(); const first = presentation(f.client, '', false, 'home');
  expect(first.previousState).toBe(''); expect(first.requestRoute).toBe('home'); expect(first.location).toBe(`/new?incomingShareId=${A}`);
  const mounted = presentation(f.client, first.state, true, 'new-task'); expect(mounted.location).toBe(''); expect(mounted.state).toBe(first.state);
  const closed = presentation(f.client, mounted.state, false, 'home'); expect(JSON.parse(closed.state)).toEqual({ presentedShareId: null, dismissedShareId: A });
  await f.load(); expect(presentation(f.client, closed.state, false, 'home').location).toBe('');
  await f.load(contents([entry(B)])); expect(presentation(f.client, closed.state, false, 'home').location).toBe(`/new?incomingShareId=${B}`);
  await f.load(contents([])); const consumed = presentation(f.client, closed.state, false, 'home'); expect(JSON.parse(consumed.state)).toEqual(empty());
  await f.load(); expect(presentation(f.client, consumed.state, false, 'home').location).toBe(`/new?incomingShareId=${A}`);
});

test('any mounted new-task sheet defers unopened share without marking it dismissed; consumption while mounted permits same-ID next handoff', async () => {
  const f = fixture(); await f.load();
  const unrelatedSheet = presentation(f.client, JSON.stringify(empty()), true, 'ordinary-task-branch');
  expect(unrelatedSheet.location).toBe(''); expect(JSON.parse(unrelatedSheet.state)).toEqual(empty());
  const opened = presentation(f.client, unrelatedSheet.state, false, 'home'); expect(opened.location).not.toBe('');
  await f.load(contents([])); const consumedMounted = presentation(f.client, opened.state, true, 'share-draft');
  expect(JSON.parse(consumedMounted.state)).toEqual(empty());
  await f.load(); const replacementMounted = presentation(f.client, consumedMounted.state, true, 'share-settings'); expect(replacementMounted.location).toBe('');
  expect(presentation(f.client, replacementMounted.state, false, 'home').location).not.toBe('');
});

test('presentation preparation never commits: a stale route proposal leaves current-route share eligible', async () => {
  const f = fixture(); await f.load();
  const serialized = JSON.stringify(empty()), stale = presentation(f.client, serialized, false, 'old-route');
  const current = presentation(f.client, serialized, false, 'new-route');
  expect(stale.requestRoute).not.toBe(current.requestRoute); expect(stale.previousState).toBe(serialized);
  expect(current.location).toBe(stale.location); expect(current.state).toBe(stale.state);
  // Root commits current.state and pushes current.location in one route-guarded action.
  const mounted = presentation(f.client, current.state, true, 'new-task'); expect(mounted.location).toBe('');
  expect(stale.previousState).not.toBe(current.state);
});

test('malformed serialized presentation state cannot forge dismissal and unread inbox cannot present', async () => {
  const f = fixture(); expect(presentation(f.client, '', false, 'home').location).toBe(''); await f.load();
  for (const state of ['garbage', 'null', '{"dismissedShareId":"other"}', JSON.stringify({ presentedShareId: { id: A }, dismissedShareId: false })])
    expect(presentation(f.client, state, false, 'home').location).toBe(`/new?incomingShareId=${A}`);
  expect(transition({ presentedShareId: A, dismissedShareId: null }, false, B)).toEqual({ state: { presentedShareId: B, dismissedShareId: null }, shareIdToPresent: B });
});

test('subtitle describes saved files even while producer unavailable', () => {
  expect(incomingShareSubtitle(null)).toBe(''); expect(incomingShareSubtitle({ ...entry(), attachments: [] })).toBe('Choose a project for what you shared');
  expect(incomingShareSubtitle(entry())).toBe('Choose a project for the image you shared');
  const mixed = entry(); mixed.attachments.push({ id: adoption, kind: 'file', name: 'a.txt', mimeType: 'text/plain', sizeBytes: 5 });
  expect(incomingShareSubtitle(mixed)).toBe('Choose a project for the 2 files you shared');
});


test('inbox revision distinguishes a prepared proposal from subsequent consumption before root commit', async () => {
  const f = fixture(); await f.load(); const state = JSON.stringify(empty());
  const prepared = presentation(f.client, state, false, 'home'); expect(prepared.inboxRevision).toBe(snapshot(f.client).revision);
  await f.load(contents([])); expect(prepared.inboxRevision).not.toBe(snapshot(f.client).revision);
  const consumed = presentation(f.client, state, false, 'home'); expect(consumed.location).toBe(''); expect(consumed.inboxRevision).toBe(snapshot(f.client).revision);
  await f.load(); const fresh = presentation(f.client, state, false, 'home');
  expect(fresh.location).toBe(prepared.location); expect(fresh.inboxRevision).not.toBe(prepared.inboxRevision);
});
