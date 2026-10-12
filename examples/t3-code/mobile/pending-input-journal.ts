import type { Pending, T3Client } from './shared/client';
import { arr, obj, str, type Obj } from './shared/domain';
import { ClientError, type Files, type Native } from './shared/protocol';
import { letGo } from './shared/let-go';

export interface QuestionRecord { origin: string; environmentId: string; threadId: string; requestId: string; signature: string;
  pending: Pending; drafts: Record<string, { selected: string[]; custom: string }>; phase: 'preflight' | 'issued' | 'uncertain' | 'unsent' | 'settled'; retiring?: boolean }
type Store = Record<string, QuestionRecord>;
const field = 'mobileQuestionCommands';
const store = (local: T3Client['local']): Store => {
  const value = obj(local)[field];
  if (value && typeof value === 'object' && !Array.isArray(value)) return value as unknown as Store;
  const next: Store = {}; Object.assign(local, { [field]: next }); return next;
};
const id = (record: QuestionRecord) => str(record.pending.payload.commandId);
const canonicalOrigin = (client: T3Client) => client.origin.trim().replace(/\/+$/, '');
export const questionRecordForPending = (client: T3Client) => Object.values(store(client.local)).find(record => record.pending === client.pending);
export const questionRecordForRequest = (client: T3Client, requestId: string, signature: string) => Object.values(store(client.local)).find(record =>
  record.origin === canonicalOrigin(client) && record.environmentId === client.environmentId && record.threadId === client.threadId
  && record.requestId === requestId && record.signature === signature);
export const questionJournalBlocked = (client: T3Client) => Object.values(store(client.local)).some(record => record.environmentId === client.environmentId && record.origin === canonicalOrigin(client)
  && (record.phase !== 'unsent' && record.phase !== 'settled' || client.local.pending[record.environmentId] === record.pending));
/** A fresh owner observation may surface a retained receipt after an unrelated successor finishes. */
export function questionJournalObserve(client: T3Client): void {
  if (client.pending) return;
  const record = Object.values(store(client.local)).find(record => record.environmentId === client.environmentId && record.origin === canonicalOrigin(client)
    && record.threadId === client.threadId && record.phase !== 'unsent' && record.phase !== 'settled');
  if (record) { record.pending.uncertain = true; client.local.pending[record.environmentId] = record.pending; }
}

/** Shared load treats every generic pending as replayable. Questionnaire receipts have
 * their own decoder and guarded Retry; never place them in that serialized map. */
export function questionJournalDocument(document: Obj): Obj {
  const records = obj(document[field]), pending = { ...obj(document.pending) }, saved: Obj = {};
  for (const [key, raw] of Object.entries(records)) {
    const record = raw as unknown as QuestionRecord;
    if (str(obj(obj(pending[record.environmentId]).payload).commandId) === id(record)) delete pending[record.environmentId];
    if (!record.retiring && record.phase !== 'unsent' && record.phase !== 'settled') saved[key] = raw;
  }
  return { ...document, pending, [field]: saved };
}
export function questionJournalHydrate(client: T3Client, document: Obj): void {
  const records = store(client.local);
  for (const [key, raw] of Object.entries(obj(document[field]))) {
    const value = obj(raw), pending = obj(value.pending), payload = obj(pending.payload);
    if (key !== payload.commandId || pending.method !== 'orchestration.dispatchCommand'
      || !['runtime-request.respond', 'thread.user-input.dismiss', 'run.interrupt'].includes(str(payload.type))
      || !['preflight', 'issued', 'uncertain', 'unsent', 'settled'].includes(str(value.phase))
      || !str(value.origin) || !str(value.environmentId) || !str(value.threadId) || !str(value.requestId) || !str(value.signature)) continue;
    const record = JSON.parse(JSON.stringify(value)) as QuestionRecord; record.retiring = false; record.pending.uncertain = true;
    records[key] = record;
    if (!client.local.pending[record.environmentId]) client.local.pending[record.environmentId] = record.pending;
  }
}
/** Refresh can settle a receipt even after the question card has disappeared. */
export function questionJournalReconcile(client: T3Client): boolean | null {
  const record = questionRecordForPending(client); if (!record) return null;
  if (record.origin !== canonicalOrigin(client) || record.environmentId !== client.environmentId || record.threadId !== client.threadId
    || !client.threadLive || str(obj(client.projection.thread).id) !== record.threadId) return false;
  const resolved = record.pending.payload.type === 'run.interrupt' ? !arr(client.projection.runs).some(run => run.id === record.pending.payload.runId && ['preparing', 'starting', 'running', 'waiting'].includes(str(run.status)))
    : !arr(client.projection.runtimeRequests).some(request => request.id === record.requestId && request.status === 'pending');
  if (resolved) record.phase = 'settled';
  return record.phase === 'settled' || record.phase === 'unsent';
}
/** Apply only the exact terminal records included in this successful local save. */
export function questionJournalPersistence(client: T3Client): () => void {
  const local = client.local, records = store(local);
  const terminal = Object.entries(records).filter(([, record]) => record.phase === 'settled' || record.phase === 'unsent');
  return () => {
    if (client.local !== local || (obj(local)[field] as unknown) !== records) return;
    for (const [key, record] of terminal) if (records[key] === record && (record.phase === 'settled' || record.phase === 'unsent')) {
      if (local.pending[record.environmentId] === record.pending) delete local.pending[record.environmentId];
      delete records[key];
    }
  };
}
const filteredStorage = (storage: Files): Files => ({ fs: { ...storage.fs, async atomicWriteFile(path, bytes) {
  if (path !== 'app:/data/t3-code.json') return storage.fs.atomicWriteFile(path, bytes);
  const document = obj(JSON.parse(new TextDecoder().decode(bytes)));
  return storage.fs.atomicWriteFile(path, new TextEncoder().encode(JSON.stringify(questionJournalDocument(document))));
} } });

/** One record and one local owner throughout. No old continuation clears a successor.
 * A failed cleanup retains the receipt and exposes existing Retry as a save-only retry. */
export async function questionJournalWrite(client: T3Client, native: Native, storage: Files, record: QuestionRecord,
  before: () => void, retry = false): Promise<void> {
  const local = client.local, environmentId = record.environmentId, records = store(local), key = id(record), generation = client.generation;
  const ownsRecord = () => client.local === local && (obj(local)[field] as unknown) === records && records[key] === record;
  const ownsPending = () => ownsRecord() && local.pending[environmentId] === record.pending;
  const persist = async () => { if (!ownsRecord()) throw new ClientError('This saved answer changed.'); await client.persist(filteredStorage(storage)); };
  const finish = async (phase: 'unsent' | 'settled') => {
    if (!ownsRecord()) return;
    record.phase = phase;
    if (!ownsPending()) return;
    record.retiring = true;
    try {
      await persist();
      if (ownsPending()) { delete local.pending[environmentId]; delete records[key]; }
      else if (ownsRecord()) record.retiring = false;
    } catch (error) {
      if (ownsRecord()) { record.retiring = false; record.pending.uncertain = true; }
      if (letGo(error)) throw error;
      throw new ClientError('Could not save the answer outcome. Use Retry to finish saving it; your answers are retained.');
    }
  };
  if (retry) {
    if (!ownsPending()) throw new ClientError('This saved answer changed.');
    before();
    if (record.phase === 'unsent' || record.phase === 'settled') return finish(record.phase);
  } else {
    before();
    if (local.pending[environmentId] || records[key]) throw new ClientError('Resolve the previous operation first.');
    records[key] = record; local.pending[environmentId] = record.pending;
  }
  let dispatched = false;
  try {
    // Persist only the app receipt. A failed preflight cannot leave a generic replay.
    await persist(); before();
    if (!ownsPending()) throw new ClientError('This saved answer changed.');
    record.phase = 'issued';
    await persist(); before();
    if (!ownsPending()) throw new ClientError('This saved answer changed.');
    const guarded: Native = { available: native.available, watch: topic => native.watch(topic), later: async raw => {
      before(); if (!ownsPending()) throw new ClientError('This saved answer changed.');
      if (obj(raw).op === 'request') dispatched = true;
      return native.later(raw);
    } };
    await client.request(guarded, record.pending.method, record.pending.payload, generation, true);
  } catch (error) {
    const uncertain = dispatched && (letGo(error) || error instanceof ClientError && error.uncertain);
    if (uncertain) {
      if (ownsRecord()) {
        record.phase = 'uncertain'; record.pending.uncertain = true;
        try { await persist(); } catch (saveError) { if (letGo(saveError)) throw saveError; }
      }
    } else await finish('unsent');
    throw error;
  }
  await finish('settled');
}
