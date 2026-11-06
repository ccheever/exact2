import type { Pending, T3Client } from './shared/client';
import { arr, obj, str, type Obj } from './shared/domain';
import { bridgeReply, ClientError, type Files, type Native } from './shared/protocol';
import { letGo } from './shared/let-go';

export interface QuestionRecord { origin: string; environmentId: string; threadId: string; requestId: string; signature: string;
  pending: Pending; drafts: Record<string, { selected: string[]; custom: string }>; phase: 'preflight' | 'issued' | 'uncertain' | 'unsent' | 'settled'; retiring?: boolean }
type Store = Record<string, QuestionRecord>;
interface Removal { marker: Obj; mutationId: string; releaseToken: string; process: string; pinId: unknown; pinDigest: unknown }
// Invocation abandonment can leave a completed native removal whose reply was never adopted.
const removals = new WeakMap<QuestionRecord, Removal>();
const cleanupFailures = new WeakSet<QuestionRecord>();
const field = 'mobileQuestionCommands', metadataField = 'mobileQuestionMetadata';
const store = (local: T3Client['local']): Store => {
  const value = obj(local)[field];
  if (value && typeof value === 'object' && !Array.isArray(value)) return value as unknown as Store;
  const next: Store = {}; Object.assign(local, { [field]: next }); return next;
};
const metadata = (client: T3Client) => obj(obj(client.local)[metadataField]);
const id = (record: QuestionRecord) => str(record.pending.payload.commandId);
const canonicalOrigin = (client: T3Client) => client.origin.trim().replace(/\/+$/, '');
const homes = new WeakMap<T3Client, { home: string; endpoint: string; environmentId: string; generation: number }>();
/** Only a native transport observation may establish logical home. Active endpoint guards remain separate. */
const blocked = new WeakMap<T3Client, string>();
const wake = new WeakMap<T3Client, { native: number; read?: number }>();
/** This entrypoint is called only by adoption of an actual Native status response. */
export function questionJournalStatus(client: T3Client, raw: Obj, observedGeneration: number): void {
  const home = str(raw.homeOrigin), endpoint = str(raw.origin), environmentId = str(raw.environmentId);
  if (observedGeneration !== client.generation) return;
  homes.delete(client);
  if (home && endpoint === canonicalOrigin(client) && environmentId === client.environmentId)
    homes.set(client, { home, endpoint, environmentId, generation: observedGeneration });
  if (Number.isSafeInteger(raw.questionWake) && Number(raw.questionWake) >= 0)
    wake.set(client, { native: Number(raw.questionWake), read: wake.get(client)?.read });
}
function verifiedReply(client: T3Client, raw: Obj): void {
  const proof = obj(raw.verifiedHome);
  if (typeof proof.generation === 'number' && Number.isSafeInteger(proof.generation) && proof.generation === client.generation
    && proof.endpoint === canonicalOrigin(client) && proof.environmentId === client.environmentId && str(proof.home))
    homes.set(client, { home: str(proof.home), endpoint: str(proof.endpoint), environmentId: str(proof.environmentId), generation: proof.generation });
  if (Number.isSafeInteger(raw.journalWake) && Number(raw.journalWake) >= 0)
    wake.set(client, { native: Number(raw.journalWake), read: Number(raw.journalWake) });
}
export function questionJournalError(client: T3Client): string {
  const unresolved = Object.values(store(client.local)).some(record => record.environmentId === client.environmentId
    && (!questionJournalHome(client) || !str(obj(obj(metadata(client).commands)[id(record)]).homeOrigin)));
  return blocked.get(client) ?? (unresolved ? 'Connect to the verified saved home to resolve this retained answer.' : '');
}
export const questionJournalNeedsRead = (client: T3Client) => !!blocked.get(client) || Object.keys(store(client.local)).length > 0
  || Object.keys(obj(metadata(client).prepared)).length > 0 || Object.keys(obj(metadata(client).tombstones)).length > 0
  || !!wake.get(client) && wake.get(client)!.native !== wake.get(client)!.read;
export function questionJournalHome(client: T3Client): string {
  const proof = homes.get(client);
  return proof && proof.endpoint === canonicalOrigin(client) && proof.environmentId === client.environmentId && proof.generation === client.generation ? proof.home : '';
}
const belongs = (client: T3Client, record: QuestionRecord) => {
  const home = str(obj(obj(metadata(client).commands)[id(record)]).homeOrigin), observed = questionJournalHome(client);
  return record.environmentId === client.environmentId && !!home && !!observed && home === observed;
};
export const questionRecordForPending = (client: T3Client) => Object.values(store(client.local)).find(record => record.pending === client.pending);
export const questionRecordForRequest = (client: T3Client, requestId: string, signature: string) => Object.values(store(client.local)).find(record =>
  belongs(client, record) && record.threadId === client.threadId && record.requestId === requestId && record.signature === signature);
export const questionJournalBlocked = (client: T3Client) => !!blocked.get(client) || Object.values(store(client.local)).some(record =>
  record.environmentId === client.environmentId && (record.phase !== 'unsent' && record.phase !== 'settled' || client.local.pending[record.environmentId] === record.pending));
export function questionJournalObserve(client: T3Client): void {
  if (client.pending) return;
  const record = Object.values(store(client.local)).find(record => belongs(client, record) && record.threadId === client.threadId);
  if (record) { record.pending.uncertain = true; client.local.pending[record.environmentId] = record.pending; }
}
/** Pure presentation filtering. Native readPreferences performs this before shared loading. */
export function questionJournalDocument(document: Obj): Obj {
  const owned = new Set([...Object.keys(obj(document[field])), ...Object.keys(obj(obj(document[metadataField]).tombstones)), ...Object.keys(obj(obj(document[metadataField]).prepared))]);
  const pending = { ...obj(document.pending) };
  for (const [key, entry] of Object.entries(pending)) if (owned.has(str(obj(obj(entry).payload).commandId))) delete pending[key];
  return { ...document, pending };
}
export function questionJournalHydrate(client: T3Client, document: Obj, cleanupOwner?: QuestionRecord): void {
  const records = store(client.local), incoming = obj(document[field]);
  if (document[metadataField]) Object.assign(client.local, { [metadataField]: document[metadataField] });
  if (obj(document.mobileQuestionRecovery).blocked === true) { blocked.set(client, str(obj(document.mobileQuestionRecovery).message) || 'The saved question state needs recovery.'); return; }
  for (const [key, raw] of Object.entries(incoming)) {
    const value = obj(raw), pending = obj(value.pending), payload = obj(pending.payload);
    if (key !== payload.commandId || pending.method !== 'orchestration.dispatchCommand'
      || !['runtime-request.respond', 'thread.user-input.dismiss', 'run.interrupt'].includes(str(payload.type))
      || !['preflight', 'issued', 'uncertain', 'unsent', 'settled'].includes(str(value.phase))
      || !str(value.origin) || !str(value.environmentId) || !str(value.threadId) || !str(value.requestId) || !str(value.signature)) {
      blocked.set(client, 'The native question journal returned an invalid receipt.'); return;
    }
  }
  blocked.delete(client);
  for (const [key, prior] of Object.entries(records)) if (!(key in incoming)) {
    if (cleanupFailures.has(prior) && prior !== cleanupOwner) continue;
    if (client.local.pending[prior.environmentId] === prior.pending) delete client.local.pending[prior.environmentId];
    delete records[key];
  }
  for (const [key, raw] of Object.entries(incoming)) {
    const value = obj(raw), pending = obj(value.pending), payload = obj(pending.payload);
    const prior = records[key];
    if (prior && cleanupFailures.has(prior) && prior !== cleanupOwner) continue;
    const record = prior && JSON.stringify(prior.pending.payload) === JSON.stringify(payload) ? prior : JSON.parse(JSON.stringify(value)) as QuestionRecord;
    record.phase = value.phase as QuestionRecord['phase']; record.retiring = false; record.pending.uncertain = true; records[key] = record;
    if (!client.local.pending[record.environmentId] && belongs(client, record)) client.local.pending[record.environmentId] = record.pending;
  }
}
/** Only complete authoritative projection consumption permits save-only resolution. */
export function questionJournalReconcile(client: T3Client): boolean | null {
  const record = questionRecordForPending(client); if (!record) return null;
  if (!belongs(client, record) || record.threadId !== client.threadId || !client.threadLive || str(obj(client.projection.thread).id) !== record.threadId) return false;
  const resolved = questionJournalConsumed(client, record);
  if (resolved) record.phase = 'settled';
  // Shared refresh must not delete a native receipt through ordinary persistence.
  return false;
}
function consumption(client: T3Client, record: QuestionRecord): Obj | null {
  if (!belongs(client, record) || !client.threadLive || str(obj(client.projection.thread).id) !== record.threadId) return null;
  if (record.pending.payload.type === 'run.interrupt') return questionJournalConsumed(client, record)
    ? { complete: true, threadId: record.threadId, runs: arr(client.projection.runs).map(value => ({ id: value.id, status: value.status })) } : null;
  const requests = arr(client.projection.runtimeRequests), pending = requests.find(value => value.id === record.requestId && value.status === 'pending');
  const latest = arr(client.projection.turnItems).slice().reverse().find(value => value.requestId === record.requestId && value.type === 'user_input_request');
  let original = ''; try { original = str(JSON.parse(record.signature)?.[0]); } catch { /* Bare current text receipt may have no item identity. */ }
  if (pending && (!original || !str(latest?.id) || latest?.id === original)) return null;
  return { complete: true, threadId: record.threadId, requests: requests.map(value => ({ id: value.id, status: value.status })), latestItemId: str(latest?.id) };
}
export function questionJournalConsumed(client: T3Client, record: QuestionRecord): boolean {
  if (!belongs(client, record) || !client.threadLive || str(obj(client.projection.thread).id) !== record.threadId) return false;
  return record.pending.payload.type === 'run.interrupt'
    ? !arr(client.projection.runs).some(run => run.id === record.pending.payload.runId && ['preparing', 'starting', 'running', 'waiting'].includes(str(run.status)))
    : !!consumption(client, record);
}
const marker = (client: T3Client, record: QuestionRecord): Obj => {
  const journal = metadata(client), saved = obj(obj(journal.commands)[id(record)]);
  if (!str(journal.epoch) || !Number.isSafeInteger(journal.revision) || !Number.isSafeInteger(saved.recordRevision)
    || !str(saved.recordDigest) || !str(saved.payloadDigest)) throw new ClientError('Refresh this saved answer before retrying.', 'Persistence');
  return { epoch: journal.epoch, revision: journal.revision, commandId: id(record), recordRevision: saved.recordRevision,
    recordDigest: saved.recordDigest, payloadDigest: saved.payloadDigest };
};
/** Operations are invocation scoped. The native attempt persists its own outcome after let-go. */
export async function questionJournalWrite(client: T3Client, native: Native, _storage: Files, record: QuestionRecord,
  before: () => void, retry = false, catalog?: Record<string, string>): Promise<void> {
  const local = client.local, endpoint = canonicalOrigin(client), generation = client.generation, environmentId = record.environmentId, key = id(record);
  let owned = retry ? record : undefined, admitted = false, created = retry, cleanupStage = false, completedRemoval = false, resolved = record.phase === 'settled';
  const current = () => client.local === local && client.generation === generation && canonicalOrigin(client) === endpoint;
  const owns = () => current() && store(local)[key] === owned && local.pending[environmentId] === owned?.pending;
  const confirmsRemoval = (value: Obj): boolean => {
    const expected = removals.get(record); if (!expected || !owns()) return false;
    const journal = obj(value[metadataField]), tombstone = obj(obj(journal.tombstones)[key]), mutation = obj(obj(journal.mutations)[expected.mutationId]);
    return !obj(value[field])[key] && !obj(journal.commands)[key] && !obj(journal.prepared)[key]
      && tombstone.recordDigest === expected.marker.recordDigest && tombstone.payloadDigest === expected.marker.payloadDigest
      && tombstone.releaseToken === expected.releaseToken && tombstone.epoch === expected.marker.epoch && tombstone.process === expected.process
      && tombstone.pinId === expected.pinId && tombstone.pinDigest === expected.pinDigest
      && mutation.action === 'remove' && mutation.commandId === key && mutation.epoch === expected.marker.epoch
      && mutation.revision === Number(expected.marker.revision) + 1 && Number.isSafeInteger(journal.revision) && Number(journal.revision) >= Number(mutation.revision)
      && typeof mutation.digest === 'string' && /^[0-9a-f]{64}$/.test(mutation.digest);
  };
  const call = async (action: string, extra: Obj = {}, guard = true): Promise<Obj> => {
    if (guard) before();
    const mutationId = action === 'read' ? undefined : `${key}:${action}:${String(extra.epoch ?? '')}:${String(extra.revision ?? '')}`;
    if (action === 'remove' && owned && owns()) {
      const journal = metadata(client), saved = obj(obj(journal.commands)[key]);
      if (saved.releaseAck !== true || !str(saved.releaseToken) || !str(journal.process))
        throw new ClientError('Refresh this saved answer before removing it.', 'Persistence');
      removals.set(record, { marker: { ...extra }, mutationId: mutationId!, releaseToken: str(saved.releaseToken), process: str(journal.process), pinId: saved.pinId, pinDigest: saved.pinDigest });
    }
    const response = await bridgeReply(native, { op: 'mobileQuestionJournal', action, ...extra, generation, ...(mutationId ? { mutationId } : {}) });
    if (!response.ok) throw new ClientError(response.error!.message, response.error!.kind, response.error!.uncertain);
    const value = obj(response.value);
    if (guard) before();
    if (!current()) throw new ClientError('The question connection changed.', 'stale');
    // A successor installed while native work ran keeps both its pending slot and its local map.
    if (owned && store(local)[key] !== owned) throw new ClientError('This saved answer changed.', 'stale');
    if (action === 'create' && obj(value[field])[key] && !store(local)[key]) { store(local)[key] = record; owned = record; }
    if (value.stale !== true && confirmsRemoval(value)) { completedRemoval = true; removals.delete(record); cleanupFailures.delete(record); }
    verifiedReply(client, value); questionJournalHydrate(client, value, record); owned = store(local)[key];
    if (value.stale === true) throw new ClientError('Refresh this saved answer before retrying its mutation.', 'stale');
    return value;
  };
  const exact = () => marker(client, owned ?? record);
  const cleanup = async () => {
    cleanupStage = true;
    if (!owned || !owns()) return;
    if (owned.phase !== 'settled' && owned.phase !== 'unsent') return;
    try {
      if (owned.phase === 'settled' && owned.pending.payload.type !== 'run.interrupt') {
        const proof = consumption(client, owned);
        if (!proof) return;
        await call('terminal', { ...exact(), phase: 'settled', consumption: proof });
        if (!owned || !owns()) return;
      }
      await call('release', exact());
      if (!owned || !owns()) return;
      const token = str(obj(obj(metadata(client).commands)[key]).releaseToken);
      await call('ack', { ...exact(), releaseToken: token });
      if (!owned || !owns()) return;
      await call('remove', exact());
    } catch (error) {
      cleanupFailures.add(record);
      if (letGo(error)) throw error;
      if (owned) owned.pending.uncertain = true;
      throw new ClientError('Could not save the answer outcome. Use Retry to finish saving it; your answers are retained.', 'Persistence');
    }
  };
  try {
    before();
    await call('read');
    if (retry) {
      if (completedRemoval) return;
      if (!owned || !owns()) throw new ClientError('This saved answer changed.');
      if (resolved && owned.phase !== 'unsent') { cleanupStage = true; await call('terminal', { ...exact(), phase: 'settled', ...(consumption(client, owned) ? { consumption: consumption(client, owned)! } : {}) }); }
      if (owned?.phase === 'unsent' || owned?.phase === 'settled') return cleanup();
    } else {
      if (local.pending[environmentId] || store(local)[key]) throw new ClientError('Resolve the previous operation first.');
      await call('prepare', { record: record as unknown as Obj, catalog: catalog ?? {}, expectedOrigin: endpoint, expectedEnvironmentId: environmentId });
      const journal = metadata(client);
      await call('create', { epoch: journal.epoch, revision: journal.revision, commandId: key });
      created = true;
      if (!owned) throw new ClientError('The native question receipt was not created.', 'protocol');
    }
    if (!owns()) throw new ClientError('This saved answer changed.');
    await call('bind', exact());
    if (!owns()) throw new ClientError('This saved answer changed.');
    await call('phase', { ...exact(), phase: 'issued' });
    before(); if (!owns()) throw new ClientError('This saved answer changed.');
    admitted = true;
    const value = await client.call(native, { op: 'request', method: 'orchestration.dispatchCommand', question: exact(),
      expectedOrigin: endpoint, expectedEnvironmentId: environmentId }, generation, true);
    before();
    if (owns()) { verifiedReply(client, value); questionJournalHydrate(client, value, record); owned = store(local)[key]; }
    await cleanup();
  } catch (error) {
    if (letGo(error)) throw error;
    if (cleanupStage) { cleanupFailures.add(record); throw error; }
    if (created && owns()) {
      if (!admitted && owned?.phase === 'preflight') {
        await call('terminal', { ...exact(), phase: 'unsent' }, false); await cleanup();
      } else if (admitted) {
        // Native may still be live, uncertain, or retaining a failed outcome save. Read is save-only.
        await call('read', {}, false);
        if (completedRemoval) return;
        if (owned?.phase === 'settled' || owned?.phase === 'unsent') return cleanup();
      }
    }
    throw error;
  }
}

/** A refresh reads committed native outcomes before projecting or reconciling receipts. */
export async function questionJournalRefresh(client: T3Client, native: Native, storage: Files): Promise<void> {
  try {
  const local = client.local, generation = client.generation, endpoint = canonicalOrigin(client);
  const before = () => { if (client.local !== local || client.generation !== generation || canonicalOrigin(client) !== endpoint)
    throw new ClientError('The question refresh changed.', 'superseded'); };
  before(); const response = await bridgeReply(native, { op: 'mobileQuestionJournal', action: 'read' }); before();
  if (!response.ok) throw new ClientError(response.error!.message, response.error!.kind, response.error!.uncertain);
  const value = obj(response.value); verifiedReply(client, value); questionJournalHydrate(client, value); questionJournalReconcile(client);
  const record = questionRecordForPending(client);
  // Unconfirmed cleanup keeps this receipt's manual Retry; other environments still refresh normally.
  if (record && !cleanupFailures.has(record) && (record.phase === 'settled' || record.phase === 'unsent')) await questionJournalWrite(client, native, storage, record, before, true);
  } catch (error) {
    if (letGo(error)) throw error;
    blocked.set(client, error instanceof Error ? error.message : 'The saved question state needs recovery.');
  }
}
