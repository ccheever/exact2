// Pinned365aa87982 QuestionAnswerHistory.tsx AnswerFile and state/assets.ts.
// @ref llp/1109.005-composer-and-transcript.decision.md#work-log-detail-rows
import type { T3Client } from './shared/client';
import { arr, obj, str, type Obj } from './shared/domain';
import { ClientError, type Native } from './shared/protocol';
import { letGo } from './shared/let-go';

const STALE_MS = 5 * 60_000, IDLE_MS = 60 * 60_000, MAX_IDLE_URLS = 256;
interface Entry { serial: number; url: string; refreshAt: number; expiresAt: number; seenAt: number; pending: boolean }
interface State { owner: string; serial: number; entries: Map<string, Entry> }
const states = new WeakMap<T3Client, State>();
/** Selection is part of ownership even when the displayed item was inherited. */
export function mobileAnswerFilesOwner(client: T3Client) {
  return JSON.stringify([client.origin, client.environmentId, client.generation, client.threadId, client.threadEpoch]);
}
const fingerprint = (item: Obj, file: Obj) => JSON.stringify([item.updatedAt ?? null, obj(item.questionAnswer).requestId ?? null, file]);
function identity(client: T3Client, row: Obj, questionId: string, file: Obj) {
  return JSON.stringify([mobileAnswerFilesOwner(client), str(row.sourceThreadId), str(row.sourceItemId), questionId, str(file.id), fingerprint(obj(row.item), file)]);
}
function find(client: T3Client, id: string): Obj | undefined {
  let parts: unknown;
  try { parts = JSON.parse(id); } catch { return; }
  if (!Array.isArray(parts) || parts.length !== 6 || parts.some(value => typeof value !== 'string') || parts[0] !== mobileAnswerFilesOwner(client)) return;
  const row = arr(client.projection.visibleTurnItems).find(row => row.sourceThreadId === parts[1] && row.sourceItemId === parts[2]);
  const item = obj(row?.item);
  if (item.type !== 'user_input_request') return;
  return arr(obj(obj(item.questionAnswer).attachmentsByQuestionId)[parts[3] as string])
    .find(file => str(file.id) === parts[4] && fingerprint(item, file) === parts[5]);
}
function assertOwned(client: T3Client, id: string) {
  if (!client.ready || !find(client, id)) throw new ClientError('That answer attachment is no longer available.', 'superseded');
}
function stateOf(client: T3Client): State {
  const owner = mobileAnswerFilesOwner(client); let state = states.get(client);
  if (!state || state.owner !== owner) { state = { owner, serial: 0, entries: new Map() }; states.set(client, state); }
  return state;
}
function entryFor(client: T3Client, id: string): Entry | undefined {
  const state = states.get(client);
  return state?.owner === mobileAnswerFilesOwner(client) ? state.entries.get(id) : undefined;
}
function cached(client: T3Client, id: string, now: number): Entry | undefined {
  const entry = entryFor(client, id);
  return entry && !entry.pending && entry.refreshAt > now ? entry : undefined;
}
function prune(client: T3Client, state: State, now: number, active: ReadonlySet<string>) {
  for (const [id, entry] of state.entries) if (!find(client, id) || !active.has(id) && now - entry.seenAt >= IDLE_MS) state.entries.delete(id);
  const idle = [...state.entries.keys()].filter(id => !active.has(id));
  for (const id of idle.slice(0, Math.max(0, idle.length - MAX_IDLE_URLS))) state.entries.delete(id);
}
/** Pure scalar snapshot: no reads, URL minting or cache writes. */
export function mobileAnswerFile(client: T3Client, row: Obj, questionId: string, file: Obj, now: number) {
  const id = identity(client, row, questionId, file);
  const entry = entryFor(client, id), connected = client.connection === 'connected' || client.connection === 'connecting';
  return { id, name: str(file.name), image: file.type === 'image',
    url: connected && find(client, id) && entry && entry.expiresAt > now ? entry.url : '' };
}
function guarded(client: T3Client, id: string, native: Native): Native {
  return { available: native.available, watch: topic => native.watch(topic), later: async request => {
    assertOwned(client, id); const result = await native.later(request); assertOwned(client, id); return result;
  } };
}
function actualURL(origin: string, relative: unknown): string {
  if (!str(relative)) throw new ClientError('The answer attachment URL is unavailable.');
  const url = new URL(str(relative), `${origin.replace(/\/+$/, '')}/`);
  if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password) throw new ClientError('The answer attachment URL cannot be opened.');
  return url.href;
}
async function sign(client: T3Client, id: string, native: Native, now: number, active = new Set([id])): Promise<string> {
  if (!Number.isFinite(now) || now < 0) throw new ClientError('The attachment clock is unavailable.');
  assertOwned(client, id);
  const pending = entryFor(client, id);
  if (pending?.pending) return pending.expiresAt > now ? pending.url : '';
  const hit = cached(client, id, now); if (hit) { hit.seenAt = now; return hit.url; }
  const state = stateOf(client), prior = entryFor(client, id);
  const entry: Entry = { serial: ++state.serial, url: prior?.url ?? '', expiresAt: prior?.expiresAt ?? 0, refreshAt: 0, seenAt: now, pending: true };
  state.entries.delete(id); state.entries.set(id, entry);
  active.add(id); prune(client, state, now, active);
  const current = () => states.get(client) === state && state.entries.get(id) === entry;
  try {
    const file = find(client, id)!;
    // Source AnswerFile passes only this attachment resource: no guessed MIME,
    // disposition, filesystem path, or Quick Look ownership is introduced.
    const result = await client.rpc(guarded(client, id, native), 'assets.createUrl', { resource: { _tag: 'attachment', attachmentId: str(file.id) } });
    assertOwned(client, id);
    if (!current()) throw new ClientError('A newer attachment read replaced this one.', 'superseded');
    const expiresAt = Number(result.expiresAt);
    if (!Number.isFinite(expiresAt) || expiresAt <= now) throw new ClientError('The answer attachment URL has expired.');
    entry.url = actualURL(client.origin, result.relativeUrl); entry.expiresAt = expiresAt;
    entry.refreshAt = Math.min(now + STALE_MS, expiresAt);
    return entry.url;
  } catch (error) {
    if (letGo(error)) { if (current()) state.entries.delete(id); throw error; }
    if (current()) { entry.url = ''; entry.refreshAt = now + STALE_MS; }
    return '';
  } finally { if (current()) entry.pending = false; }
}
/** Root reads only this descriptor, then owns the explicit awaited mutation.
 * Visible IDs come from the actual rendered transcript, including parent folds. */
export function mobileAnswerFilesRequest(client: T3Client, visibleIds: string[], now: number) {
  const owner = mobileAnswerFilesOwner(client), visible = [...new Set(visibleIds)].filter(id => !!find(client, id));
  if (!client.ready) return { owner, request: '' };
  const state = states.get(client), active = new Set(visible);
  // A pending read belongs to its existing mutation or Open command. Neither
  // needs an empty second mutation; its completion already refreshes the root.
  const read = visible.filter(id => !entryFor(client, id)?.pending && !cached(client, id, now));
  const idle = state ? [...state.entries].filter(([id]) => !active.has(id)) : [];
  const cleanup = !!state && (state.owner !== owner || idle.length > MAX_IDLE_URLS
    || idle.some(([id, entry]) => !find(client, id) || now - entry.seenAt >= IDLE_MS));
  return { owner, request: read.length || cleanup ? JSON.stringify({ visible, read }) : '' };
}
/** At most four reads run concurrently inside this answer. Only request metadata
 * lives in the cache; ordinary render revisions cannot replace its pending reads. */
export async function mobilePrepareAnswerFiles(client: T3Client, native: Native | null | undefined, now: number,
  expectedOwner: string, request: string, currentVisible: string[]) {
  if (mobileAnswerFilesOwner(client) !== expectedOwner) throw new ClientError('The conversation changed.', 'superseded');
  if (!native?.available || !client.ready || !client.threadId || !request) return;
  let parsed: Obj;
  try { parsed = obj(JSON.parse(request)); } catch { throw new ClientError('That attachment request is unavailable.', 'superseded'); }
  if (!Array.isArray(parsed.read) || parsed.read.some(id => typeof id !== 'string')) throw new ClientError('That attachment request is unavailable.', 'superseded');
  const active = new Set(currentVisible), wanted = [...new Set(parsed.read as string[])];
  if (wanted.some(id => !active.has(id) || !find(client, id))) throw new ClientError('The answer attachments changed.', 'superseded');
  const state = stateOf(client); prune(client, state, now, active);
  let cursor = 0;
  const workers = Array.from({ length: Math.min(4, wanted.length) }, async () => {
    while (cursor < wanted.length) { const id = wanted[cursor++]!; await sign(client, id, native, now, active); }
  });
  const settled = await Promise.allSettled(workers);
  const failed = settled.find((entry): entry is PromiseRejectedResult => entry.status === 'rejected');
  if (failed) throw failed.reason;
}
/** The signed link opens in the OS like source Linking.openURL; it is not a
 * transcript Quick Look attachment. Membership is checked at every await. */
export async function mobileOpenAnswerFile(client: T3Client, id: string, native: Native, now: number) {
  try {
    assertOwned(client, id);
    const url = await sign(client, id, native, now); assertOwned(client, id);
    if (!url) throw new ClientError('This answer attachment could not be opened.');
    const result = await client.restAccess(guarded(client, id, native)).call({ op: 'mobileOpenURL', url });
    assertOwned(client, id);
    if (result.opened !== true) throw new ClientError('This answer attachment could not be opened.');
    return { revision: client.revision, message: '' };
  } catch (error) { if (letGo(error)) throw error; return { revision: client.revision, message: error instanceof Error ? error.message : 'The attachment could not be opened.' }; }
}
