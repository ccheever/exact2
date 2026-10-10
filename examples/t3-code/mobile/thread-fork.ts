// Pinned365aa87982 ThreadFeed.AssistantForkButton and threadForkNavigation.
// @ref llp/1109.005-composer-and-transcript.decision.md#fork-from-response
import type { T3Client } from './shared/client';
import { arr, obj, str, type Obj } from './shared/domain';
import { bridgeReply, ClientError, type Files, type Native } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { mobileSessionGrants } from './mobile-grants';

interface State { connection: string; visit: string; active: boolean; epoch: number; busy: string; capabilities: Map<string, boolean> }
const states = new WeakMap<T3Client, State>();
const connection = (client: T3Client) => JSON.stringify([client.origin, client.environmentId, client.generation]);
const owner = (client: T3Client) => JSON.stringify([connection(client), client.projectId, client.threadId, client.threadEpoch]);
const stale = () => new ClientError('The response or thread screen changed.', 'superseded');
function state(client: T3Client): State {
  let value = states.get(client);
  if (!value || value.connection !== connection(client)) {
    value = { connection: connection(client), visit: '', active: false, epoch: 0, busy: '', capabilities: new Map() };
    states.set(client, value);
  }
  return value;
}

/** Missing source-session evidence keeps the source's portable fallback. Known
 * sessions require native turn-fork support or full-thread handoff. */
export function mobileCanForkResponse(row: Obj, capabilities?: Obj): boolean {
  const item = obj(row.item);
  if (item.type !== 'assistant_message' || !str(item.runId) || item.status !== 'completed') return false;
  if (capabilities === undefined) return true;
  return obj(capabilities.threads).canForkThread === true && obj(capabilities.threads).canForkFromTurn === true
    && obj(capabilities.identity).nativeThreadIds === 'strong' || obj(capabilities.context).supportsFullThreadHandoff === true;
}
const supportKey = (row: Obj) => JSON.stringify([row.sourceThreadId, row.sourceItemId, obj(row.item).providerThreadId ?? null]);
function sourceCapabilities(client: T3Client, row: Obj): Obj | undefined {
  const projection = client.projection;
  const item = arr(projection.turnItems).find(item => item.id === row.sourceItemId) ?? obj(row.item);
  const providerThread = arr(projection.providerThreads).find(thread => thread.id === item.providerThreadId);
  const session = providerThread && arr(projection.providerSessions).find(session => session.id === providerThread.providerSessionId);
  return session?.capabilities === undefined ? undefined : obj(session.capabilities);
}
/** Remember only derived source-item capability evidence from genuinely loaded
 * projections. Inherited items never borrow the selected thread's sessions. No
 * copied projection, transport reducer, native handle or promise is retained. */
export function mobileThreadForkObserveProjection(client: T3Client): void {
  const projection = client.projection, thread = obj(projection.thread), values = state(client).capabilities;
  if (!client.threadLive || thread.id !== client.threadId) return;
  for (const key of values.keys()) if (JSON.parse(key)[0] === client.threadId) values.delete(key);
  for (const row of arr(projection.visibleTurnItems)) {
    if (row.sourceThreadId !== client.threadId) continue;
    if (obj(row.item).type !== 'assistant_message') continue;
    const capabilities = sourceCapabilities(client, row);
    if (capabilities !== undefined) values.set(supportKey(row), mobileCanForkResponse(row, capabilities));
  }
}
/** Called synchronously by the existing root resource before its awaited reads. */
export function mobileThreadForkObserveRoute(client: T3Client, visit: string, active: boolean): void {
  const value = state(client);
  if (value.visit !== visit || value.active !== active) value.epoch++;
  value.visit = visit; value.active = active;
  mobileThreadForkObserveProjection(client);
}
function response(client: T3Client, rowId: string): Obj | undefined {
  return arr(client.projection.visibleTurnItems).find(row => JSON.stringify([row.sourceThreadId, row.sourceItemId]) === rowId);
}
function eligible(client: T3Client, row: Obj): boolean {
  if (row.sourceThreadId === client.threadId) return mobileCanForkResponse(row, sourceCapabilities(client, row));
  return mobileCanForkResponse(row) && state(client).capabilities.get(supportKey(row)) !== false;
}
function key(client: T3Client, row: Obj): string {
  const item = obj(row.item), value = state(client);
  return JSON.stringify([owner(client), value.visit, value.epoch, row.sourceThreadId, row.sourceItemId, item.runId,
    item.providerThreadId ?? null, item.updatedAt ?? null, str(obj(client.projection.thread).title), eligible(client, row)]);
}
export function mobileThreadForkPresentation(client: T3Client, rowId: string) {
  const row = response(client, rowId), value = state(client);
  const canFork = !!row && eligible(client, row), token = row && canFork ? key(client, row) : '';
  return { canFork, forkKey: token, forkBusy: !!token && value.busy === token };
}

/** The root selected mutation owns navigation. Dispatching a fork must not
 * change shared selection before that route commits: selectRouteThread would
 * otherwise restore the original conversation while this response is pending. */
export async function mobileThreadForkAction(client: T3Client, rowId: string, token: string, visit: string,
  nativeInput: Native | null | undefined, storage: Files) {
  const value = state(client), environmentId = client.environmentId;
  const result = (message = '', threadId = '') => ({ revision: client.revision, requestRoute: visit, environmentId, threadId, message });
  const current = () => states.get(client) === value && value.connection === connection(client) && value.active && value.visit === visit
    && !!token && mobileThreadForkPresentation(client, rowId).forkKey === token;
  const check = () => { if (!current()) throw stale(); };
  check();
  if (!nativeInput?.available) return result('Open T3 Code on your iPhone or iPad to fork a response.');
  if (value.busy) return result();
  if (!client.writable) return result('Reconnect and check your connection permissions before forking.');
  if (client.busy || client.pending) return result('Resolve the current operation before forking a response.');
  value.busy = token; client.revision++;
  const live = letGoAware(nativeInput);
  const native: Native = { available: live.available, watch: topic => { check(); live.watch(topic); }, later: async input => {
    check(); const reply = await live.later(input); check(); return reply;
  } };
  // Persistence yields too: a departing route cannot issue a write afterwards.
  const files: Files = { fs: { mkdir: async path => { check(); const reply = await storage.fs.mkdir(path); check(); return reply; },
    readFile: async path => { check(); const reply = await storage.fs.readFile(path); check(); return reply; },
    atomicWriteFile: async (path, bytes) => { check(); const reply = await storage.fs.atomicWriteFile(path, bytes); check(); return reply; } } };
  try {
    await client.call(native, { op: 'r10Wake', topic: 't3.notify' });
    const session = await client.http(native, '/api/auth/session'); check();
    if (!mobileSessionGrants(session, 'orchestration:operate')) throw new ClientError('This connection cannot fork conversations.');
    const [commandId, targetThreadId] = await client.ids(native, 2); check();
    if (!client.writable || client.busy || client.pending) throw new ClientError('Resolve the current operation before forking a response.');
    const row = response(client, rowId)!, source = obj(row.item);
    await client.dispatch(native, files, { type: 'thread.fork', commandId: commandId!, createdBy: 'user', creationSource: 'mobile',
      sourceThreadId: str(row.sourceThreadId), targetThreadId: targetThreadId!, sourcePoint: { type: 'run', runId: str(source.runId) },
      title: `${str(obj(client.projection.thread).title)} fork` }, 'Fork response', check);
    check();
    const ready = () => client.shellLive && client.shell.threads.some(thread => thread.id === targetThreadId && thread.projectId === client.projectId);
    if (!ready()) {
      const sleep = async (ms: number) => {
        const reply = await bridgeReply(native, { op: 'timelineSleep', ms, generation: client.generation });
        if (!reply.ok) throw new ClientError(reply.error!.message, reply.error!.kind);
        const time = obj(reply.value).monotonicMs;
        if (typeof time !== 'number' || !Number.isFinite(time) || time < 0) throw new ClientError('The thread wait returned an invalid clock.');
        return time;
      };
      let time = await sleep(0); const deadline = time + 2000;
      await client.call(native, { op: 'r10Wake', topic: 't3.notify' });
      // Native owns the clock; data modules cannot read Date.now(). Bounded even
      // if a broken clock fails to progress, with source40ms/2000ms cadence.
      for (let polls = 0; !ready() && time < deadline && polls < 50; polls++) time = await sleep(Math.min(40, deadline - time));
    }
    check();
    if (ready()) return result('', targetThreadId!);
    value.busy = ''; client.revision++;
    await client.call(native, { op: 'mobileAlert', kind: 'info', title: 'Fork created',
      message: 'Its thread data did not reach this client. Reconnect and try opening it from the thread list.' });
    return result();
  } catch (error) {
    if (letGo(error)) throw error;
    check();
    return result(error instanceof Error ? error.message : 'Could not fork this response.');
  } finally {
    if (value.busy === token) { value.busy = ''; client.revision++; }
  }
}
