import type { T3Client } from './shared/client';
import { ClientError, type Native } from './shared/protocol';
import { arr, obj } from './shared/domain';
import { letGo } from './shared/let-go';
import { mobileThreadHistoryCommit, mobileThreadHistoryLimits, mobileThreadHistoryTicket, type MobileThreadHistoryItem } from './mobile-thread-history-store';

interface History {
  owner: string; route: string; error: string; pending: object | null; cursors: string[]; noProgress: number; attempt: number; acceptedSerial: number;
}
const histories = new WeakMap<T3Client, History>();
const owner = (client: T3Client) => JSON.stringify([client.origin, client.environmentId, client.generation,
  client.projectId, client.threadId, client.threadEpoch, client.historyEpoch]);
const requestKey = (client: T3Client, state: History) => JSON.stringify([state.owner, state.route, client.thread?.historyCursor, state.attempt]);
const rowKey = (row: Record<string, unknown>) => JSON.stringify([row.sourceThreadId, row.sourceItemId]);
function compareText(left: string, right: string): number {
  let a = 0, b = 0;
  while (a < left.length && b < right.length) {
    const x = left.codePointAt(a)!, y = right.codePointAt(b)!;
    if (x !== y) return x - y;
    a += x > 0xffff ? 2 : 1; b += y > 0xffff ? 2 : 1;
  }
  return (left.length - a) - (right.length - b);
}
function historyItems(client: T3Client, added: string[]): MobileThreadHistoryItem[] {
  const wanted = new Set(added), rows = arr(client.thread?.projection.visibleTurnItems).filter(row => wanted.has(rowKey(row)));
  const limits = mobileThreadHistoryLimits, result: MobileThreadHistoryItem[] = [];
  for (const row of rows) {
    const key = rowKey(row), ordinal = obj(row.item).ordinal, payload = JSON.stringify(row);
    if (new TextEncoder().encode(key).length > limits.maxKeyUnits || !Number.isSafeInteger(ordinal) || Number(ordinal) < 0
      || new TextEncoder().encode(payload).length > limits.maxPayloadBytes) continue;
    result.push({ key, ordinal: Number(ordinal), payload });
  }
  return result.sort((left, right) => left.ordinal - right.ordinal || compareText(left.key, right.key));
}
function persistHistoryPage(client: T3Client, native: Native, items: MobileThreadHistoryItem[], revision: number,
  beforeCursor: string | null, hasMore: boolean, current: () => boolean): void {
  if (!client.historyStorageEnabled || !native.available || !client.environmentId || !client.threadId || !current()) return;
  const identity = { environmentId: client.environmentId, threadId: client.threadId };
  void (async () => {
    try {
      const token = await mobileThreadHistoryTicket(native, identity);
      if (!current()) return;
      await mobileThreadHistoryCommit(native, identity, token, revision, beforeCursor, null, hasMore, items);
    } catch {
      // The display path owns the server result; a cache failure must not hold
      // the history control or interfere with Stop, Retry, or live events.
    }
  })();
}

export function mobileThreadHistory(client: T3Client, route: string, active: boolean) {
  const identity = owner(client), visit = active ? route : '';
  let state = histories.get(client);
  if (!state || state.owner !== identity || state.route !== visit) {
    state = { owner: identity, route: visit, error: '', pending: null, cursors: [], noProgress: 0, attempt: (state?.attempt ?? -1) + 1, acceptedSerial: 0 };
    histories.set(client, state);
  }
  return { historyRequest: visit && client.ready && client.thread?.hasMore && client.thread.historyCursor ? requestKey(client, state) : '',
    historyLoading: state.pending !== null, historyError: state.error };
}

export function mobileThreadHistoryError(client: T3Client): string {
  const state = histories.get(client);
  return state?.owner === owner(client) ? state.error : '';
}

export async function mobileLoadThreadHistory(client: T3Client, native: Native, key: string, retry: boolean,
  displaysOlder: (keys: ReadonlySet<string>) => boolean) {
  const state = histories.get(client), cursor = client.thread?.historyCursor;
  if (!state?.route || state.owner !== owner(client) || key !== requestKey(client, state)
    || !client.ready || !client.thread?.hasMore || !cursor || state.pending || client.historyTargetedLoading || state.error && !retry) return;
  if (retry) { state.error = ''; state.cursors = []; state.noProgress = 0; }
  const token = {}, current = () => histories.get(client) === state && state.pending === token && state.owner === owner(client);
  state.pending = token;
  try {
    const added = await client.history(native, state.route, current);
    if (!current()) throw new ClientError('This history read was superseded.', 'superseded');
    if (added === null) return;
    const next = client.thread?.historyCursor;
    const pageRevision = client.historyRevision, pageItems = historyItems(client, added);
    const acceptedSerial = ++state.acceptedSerial;
    state.cursors.push(cursor);
    if (state.cursors.length > 64) state.cursors.shift();
    state.noProgress = added.length > 0 && displaysOlder(new Set(added)) ? 0 : state.noProgress + 1;
    if (client.thread?.hasMore && (!next || state.cursors.includes(next)))
      state.error = 'Earlier activity did not advance. Retry to load it again.';
    else if (client.thread?.hasMore && state.noProgress >= 8)
      state.error = 'Earlier activity could not be displayed after several pages. Retry to continue.';
    persistHistoryPage(client, native, pageItems, pageRevision, next ?? null, client.thread?.hasMore === true,
      () => histories.get(client) === state && state.acceptedSerial === acceptedSerial && state.owner === owner(client) && client.historyRevision === pageRevision
        && client.thread?.historyCursor === next);
  } catch (error) {
    if (letGo(error)) throw error;
    if (!current() || client.thread?.historyCursor !== cursor)
      throw new ClientError('This history read was superseded.', 'superseded');
    state.error = error instanceof Error && error.message ? error.message : 'Could not load earlier activity.';
  } finally {
    if (current()) { state.pending = null; state.attempt++; }
  }
}
