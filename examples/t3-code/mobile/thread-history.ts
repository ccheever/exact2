import type { T3Client } from './shared/client';
import { ClientError, type Native } from './shared/protocol';
import { letGo } from './shared/let-go';

interface History {
  owner: string; route: string; error: string; pending: object | null; cursors: string[]; noProgress: number; attempt: number;
}
const histories = new WeakMap<T3Client, History>();
const owner = (client: T3Client) => JSON.stringify([client.origin, client.environmentId, client.generation,
  client.projectId, client.threadId, client.threadEpoch, client.historyEpoch]);
const requestKey = (client: T3Client, state: History) => JSON.stringify([state.owner, state.route, client.thread?.historyCursor, state.attempt]);

export function mobileThreadHistory(client: T3Client, route: string, active: boolean) {
  const identity = owner(client), visit = active ? route : '';
  let state = histories.get(client);
  if (!state || state.owner !== identity || state.route !== visit) {
    state = { owner: identity, route: visit, error: '', pending: null, cursors: [], noProgress: 0, attempt: (state?.attempt ?? -1) + 1 };
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
    || !client.ready || !client.thread?.hasMore || !cursor || state.pending || state.error && !retry) return;
  if (retry) { state.error = ''; state.cursors = []; state.noProgress = 0; }
  const token = {}, current = () => histories.get(client) === state && state.pending === token && state.owner === owner(client);
  state.pending = token;
  try {
    const added = await client.history(native, state.route, current);
    if (!current()) throw new ClientError('This history read was superseded.', 'superseded');
    if (added === null) return;
    const next = client.thread?.historyCursor;
    state.cursors.push(cursor);
    if (state.cursors.length > 64) state.cursors.shift();
    state.noProgress = added.length > 0 && displaysOlder(new Set(added)) ? 0 : state.noProgress + 1;
    if (client.thread?.hasMore && (!next || state.cursors.includes(next)))
      state.error = 'Earlier activity did not advance. Retry to load it again.';
    else if (client.thread?.hasMore && state.noProgress >= 8)
      state.error = 'Earlier activity could not be displayed after several pages. Retry to continue.';
  } catch (error) {
    if (letGo(error)) throw error;
    if (!current() || client.thread?.historyCursor !== cursor)
      throw new ClientError('This history read was superseded.', 'superseded');
    state.error = error instanceof Error && error.message ? error.message : 'Could not load earlier activity.';
  } finally {
    if (current()) { state.pending = null; state.attempt++; }
  }
}
