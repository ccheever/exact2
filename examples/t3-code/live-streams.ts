// Live lists for every connected environment (MIT reference, see LICENSE-T3; T3 Code
// 1e2ecbd975: apps/web/src/state/server.ts scheduledTasksLive, state/projectClones.ts
// environmentProjectClonesAtom, packages/contracts/src/rpc.ts scheduledTasks.subscribe and
// subscribeProjectClones). Each stream sends a full list on subscribe and after every
// change, so the newest value replaces the last. The focused environment streams over
// T3Client's transport (one dispatch line in client.ts drain, as r4-surfaces-device.ts);
// every other switched-on environment streams over its fleet transport (two lines in
// settings-b-fleet.ts). A subscription lasts for its connection's generation; a failed
// stream keeps its last list and error until the next pass subscribes again.
import type { T3Client } from './client';
import { arr, obj, str, type Obj, type Shell } from './domain';
import { bridgeReply, ClientError, type Native } from './protocol';
import { subscriptionSerial } from './shell-vcs';
import { EnvironmentFleet, fleet, type FleetEntry } from './settings-b-fleet';

export const SCHEDULED_TASKS_KEY = 'scheduled-tasks';
export const PROJECT_CLONES_KEY = 'project-clones';
export const LIVE_KEYS = [SCHEDULED_TASKS_KEY, PROJECT_CLONES_KEY];
const METHODS: Record<string, string> = { [SCHEDULED_TASKS_KEY]: 'scheduledTasks.subscribe', [PROJECT_CLONES_KEY]: 'subscribeProjectClones' };

type Stream = { id: string; floor: number; maxSeen: number; value: Obj[] | null; error: string; tried: boolean; subscribes: number };
type Streams = { generation: number; streams: Record<string, Stream> };
const owners = new WeakMap<object, Streams>();
function streamsOf(owner: object, generation: number): Streams {
  let value = owners.get(owner);
  if (!value || value.generation !== generation) {
    const previous = value?.streams ?? {};
    value = { generation, streams: {} };
    // Serials only grow on a transport, so a new generation keeps the high-water mark.
    for (const key of LIVE_KEYS) value.streams[key] = { id: '', floor: previous[key]?.maxSeen ?? 0, maxSeen: previous[key]?.maxSeen ?? 0, value: null, error: '', tried: false, subscribes: previous[key]?.subscribes ?? 0 };
    owners.set(owner, value);
  }
  return value;
}

/** One inbox entry for `key`: the newest subscription's list wins; a failure keeps the last list. */
function apply(stream: Stream, key: string, entry: Obj): void {
  const id = str(entry.subscriptionId), serial = subscriptionSerial(id);
  stream.maxSeen = Math.max(stream.maxSeen, serial);
  if (serial <= stream.floor || (stream.id && serial < subscriptionSerial(stream.id))) return;
  stream.id = id;
  const raw = entry.value, item = obj(raw);
  if (item._retryDue || item._streamEnded) { stream.id = ''; stream.tried = false; return; }
  if (item._transportError) { stream.error = str(obj(item._transportError).message, 'The live stream ended.'); return; }
  const list = key === PROJECT_CLONES_KEY ? (Array.isArray(raw) ? raw : null) : (Array.isArray(item.tasks) ? item.tasks : null);
  if (list) { stream.value = list.map(obj); stream.error = ''; }
}

/** client.ts drain: `scheduled-tasks` and `project-clones` entries of the focused connection. */
export function liveEvent(client: T3Client, entry: Obj): void {
  const key = str(entry.key);
  apply(streamsOf(client, client.generation).streams[key]!, key, entry);
}

export const cloneTracking = (config: Obj) => obj(obj(config.environment).capabilities).projectCloneTracking === true;

/** Subscribe the focused connection's streams once per generation (clones only where the server tracks them). */
export async function watchLive(client: T3Client, native: Native | null | undefined): Promise<void> {
  if (!native?.available || !client.ready) return;
  const owned = streamsOf(client, client.generation);
  for (const key of LIVE_KEYS) {
    const stream = owned.streams[key]!;
    if (stream.id || stream.tried || (key === PROJECT_CLONES_KEY && !cloneTracking(client.config))) continue;
    stream.tried = true; stream.floor = stream.maxSeen;
    try {
      const reply = await client.restAccess(native).call({ op: 'subscribe', key, method: METHODS[key], payload: {} });
      stream.subscribes++;
      const serial = subscriptionSerial(str(reply.id));
      stream.maxSeen = Math.max(stream.maxSeen, serial);
      if (serial > stream.floor && (!stream.id || serial > subscriptionSerial(stream.id))) stream.id = str(reply.id);
    } catch (error) { stream.tried = false; stream.error = error instanceof Error ? error.message : 'Could not subscribe.'; }
  }
}

/** settings-b-fleet.ts drain: true when the entry was one of these streams'. */
export function liveFleetEvent(entry: FleetEntry, event: Obj): boolean {
  const key = str(event.key);
  if (!LIVE_KEYS.includes(key)) return false;
  if (Number(event.generation) === entry.generation) apply(streamsOf(entry, entry.generation).streams[key]!, key, event);
  return true;
}
/** settings-b-fleet.ts syncEntry, after its drain: subscribe what this generation still lacks. */
export async function liveFleetPass(call: (request: Obj) => Promise<Obj>, entry: FleetEntry): Promise<void> {
  const owned = streamsOf(entry, entry.generation);
  for (const key of LIVE_KEYS) {
    const stream = owned.streams[key]!;
    if (stream.id || stream.tried || (key === PROJECT_CLONES_KEY && !cloneTracking(entry.config))) continue;
    stream.tried = true; stream.floor = stream.maxSeen;
    try {
      const reply = await call({ op: 'subscribe', key, method: METHODS[key], payload: {} });
      stream.subscribes++;
      const serial = subscriptionSerial(str(reply.id));
      stream.maxSeen = Math.max(stream.maxSeen, serial);
      if (serial > stream.floor && (!stream.id || serial > subscriptionSerial(stream.id))) stream.id = str(reply.id);
    } catch (error) { stream.tried = false; stream.error = error instanceof Error ? error.message : 'Could not subscribe.'; }
  }
}

export type LiveList = { value: Obj[] | null; error: string; subscribes: number };
const listOf = (owner: object, generation: number, key: string): LiveList => {
  const stream = streamsOf(owner, generation).streams[key]!;
  return { value: stream.value, error: stream.error, subscribes: stream.subscribes };
};

/**
 * One environment as the Scheduled tasks page, the details panel and the clone toasts see
 * it: its facts, its live lists and a request addressed to its own transport.
 */
export type LiveEnvironment = {
  environmentId: string; label: string; focused: boolean; connected: boolean; config: Obj; shell: Shell; key: string;
  tasks: LiveList; clones: LiveList; request: (method: string, payload?: Obj, write?: boolean) => Promise<Obj>; ids: (count: number) => Promise<string[]>;
};
const environmentLabel = (config: Obj, fallback: string) => str(obj(config.environment).label, fallback);

/** The focused environment first, then each background one in catalog order. */
export function liveEnvironments(client: T3Client, native: Native | null | undefined, source: EnvironmentFleet = fleet): LiveEnvironment[] {
  const out: LiveEnvironment[] = [];
  if (client.environmentId) {
    out.push({ environmentId: client.environmentId, label: environmentLabel(client.config, 'This environment'), focused: true, connected: client.ready,
      config: client.config, shell: client.shell, key: '', tasks: listOf(client, client.generation, SCHEDULED_TASKS_KEY), clones: listOf(client, client.generation, PROJECT_CLONES_KEY),
      request: (method, payload = {}, write = false) => {
        if (!native?.available) throw new ClientError('Open this app on macOS to connect to T3 Code.');
        return client.restAccess(native).request(method, payload, write);
      },
      ids: count => { if (!native?.available) throw new ClientError('Open this app on macOS to connect to T3 Code.'); return client.restAccess(native).ids(count); } });
  }
  for (const entry of source.entries.values()) {
    if (out.some(environment => environment.environmentId === entry.environmentId)) continue;
    const saved = source.saved.find(candidate => str(candidate.environmentId) === entry.environmentId);
    out.push({ environmentId: entry.environmentId, label: environmentLabel(entry.config, str(saved?.label, entry.environmentId)), focused: false,
      connected: entry.phase === 'connected' && entry.synchronized === entry.generation, config: entry.config, shell: entry.shell, key: entry.key,
      tasks: listOf(entry, entry.generation, SCHEDULED_TASKS_KEY), clones: listOf(entry, entry.generation, PROJECT_CLONES_KEY),
      request: async (method, payload = {}) => {
        if (!native?.available) throw new ClientError('Open this app on macOS to connect to T3 Code.');
        const reply = await bridgeReply(EnvironmentFleet.native(native, entry.key), { op: 'request', method, payload, generation: entry.generation });
        if (!reply.ok) throw new ClientError(reply.error!.message, reply.error!.kind);
        return obj(reply.value);
      },
      ids: async count => {
        if (!native?.available) throw new ClientError('Open this app on macOS to connect to T3 Code.');
        const reply = await bridgeReply(EnvironmentFleet.native(native, entry.key), { op: 'ids', count, generation: entry.generation });
        const ids = (Array.isArray(reply.value) ? reply.value : []).filter(value => typeof value === 'string' && value).map(String);
        if (!reply.ok || ids.length !== count) throw new ClientError('Could not allocate request identifiers.');
        return ids;
      } });
  }
  return out;
}
export function liveEnvironment(client: T3Client, native: Native | null | undefined, environmentId: string): LiveEnvironment | undefined {
  return liveEnvironments(client, native).find(environment => environment.environmentId === (environmentId || client.environmentId));
}

/**
 * The tasks a write checks against: the live list, else (before the stream's first value
 * reached this client) one `scheduledTasks.list`, so a write never acts on a guess.
 */
export async function currentTasks(environment: LiveEnvironment): Promise<Obj[]> {
  if (environment.tasks.value) return environment.tasks.value;
  return arr((await environment.request('scheduledTasks.list', {})).tasks);
}
