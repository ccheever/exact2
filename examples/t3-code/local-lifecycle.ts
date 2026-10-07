// The primary environment's lifecycle stream (20261005-local-primary-environment items 6 and 9),
// after T3 Code (MIT, see LICENSE-T3; reference 1e2ecbd975): packages/client-runtime/src/state/
// server.ts (`welcome`, `legacyThreadMigration` over `subscribeServerLifecycle`), apps/web/src/
// state/server.ts primaryServerWelcomeAtom / primaryServerLegacyThreadMigrationAtom and
// components/LegacyThreadMigrationToast.tsx. The server replays the last event of each type on
// subscribe, then streams new ones.
//  - `welcome` feeds the first-run gate (pages-welcome.ts: the bootstrap project and thread, and
//    whether the workspace bootstrap is still pending).
//  - `legacyThreadMigration` {status: running|complete, totalThreadCount}, primary only: while it
//    runs, a loading toast with no timeout, "Restoring your threads…"; it closes when the migration
//    leaves `running`.
// The primary streams over T3Client's transport while it is the focus (one dispatch line in
// client.ts drain, through keep-alive.ts) and over its fleet transport otherwise.
import { obj, str, num, type Obj } from './domain';
import type { FleetEntry } from './settings-b-fleet';
import { focusedOnPrimary, primary } from './local-primary';
import { dismissToast, pushToast } from './toast';
import { subscriptionSerial } from './shell-vcs';
import type { T3Client } from './client';
import { letGo } from './let-go';

export const LIFECYCLE_KEY = 'server-lifecycle';
type Stream = { generation: number; id: string; tried: boolean };
const streams = new WeakMap<object, Stream>();
const streamOf = (owner: object, generation: number): Stream => {
  let value = streams.get(owner);
  if (!value || value.generation !== generation) streams.set(owner, value = { generation, id: '', tried: false });
  return value;
};

/** What the primary last said: the welcome payload and the legacy thread migration. */
export const primaryLifecycle: { environmentId: string; welcome: Obj | null; migration: { status: string; totalThreadCount: number } | null } = {
  environmentId: '', welcome: null, migration: null,
};
/** The primary changed (another T3 home): forget what the last one said. */
function adoptEnvironment(environmentId: string): void {
  if (primaryLifecycle.environmentId === environmentId) return;
  primaryLifecycle.environmentId = environmentId; primaryLifecycle.welcome = null; primaryLifecycle.migration = null;
}

/** One lifecycle event (ServerLifecycleStreamEvent). */
export function applyLifecycle(environmentId: string, value: unknown): void {
  const event = obj(value), payload = obj(event.payload);
  adoptEnvironment(environmentId);
  if (event.type === 'welcome') primaryLifecycle.welcome = payload;
  else if (event.type === 'legacyThreadMigration' && (payload.status === 'running' || payload.status === 'complete')) {
    primaryLifecycle.migration = { status: str(payload.status), totalThreadCount: Math.max(0, Math.trunc(num(payload.totalThreadCount))) };
  }
}

function handle(stream: Stream | undefined, environmentId: string, entry: Obj): boolean {
  if (str(entry.key) !== LIFECYCLE_KEY) return false;
  if (!stream || num(entry.generation, -1) !== stream.generation) return true;
  const id = str(entry.subscriptionId);
  if (stream.id && subscriptionSerial(id) < subscriptionSerial(stream.id)) return true;
  const item = obj(entry.value);
  if (item._retryDue || item._streamEnded || item._transportError) { stream.id = ''; stream.tried = false; return true; }
  applyLifecycle(environmentId, entry.value);
  return true;
}

type Focused = { environmentId: string; origin: string; generation: number };
/** client.ts drain (through keep-alive.ts): the focused primary's lifecycle entries. */
export function lifecycleEvent(client: Focused, entry: Obj): boolean { return handle(streams.get(client), client.environmentId, entry); }
/** settings-b-fleet.ts drain (through keep-alive.ts): the background primary's. */
export function lifecycleFleetEvent(entry: FleetEntry, event: Obj): boolean { return handle(streams.get(entry), entry.environmentId, event); }

async function subscribe(stream: Stream, call: (request: Obj) => Promise<Obj>): Promise<void> {
  if (stream.id || stream.tried) return;
  stream.tried = true;
  try { stream.id = str((await call({ op: 'subscribe', key: LIFECYCLE_KEY, method: 'subscribeServerLifecycle', payload: {} })).id); }
  catch (error) { stream.tried = false; if (letGo(error)) throw error; }
}
/** keep-alive.ts keepAlivePrepare: subscribe the focused connection when it is the primary. */
export async function watchLifecycle(client: Focused, call: (request: Obj) => Promise<Obj>): Promise<void> {
  if (focusedOnPrimary(client)) await subscribe(streamOf(client, client.generation), call);
}
/** keep-alive.ts keepAliveFleetPass: subscribe the background primary. */
export async function lifecycleFleetPass(call: (request: Obj) => Promise<Obj>, entry: FleetEntry): Promise<void> {
  if (entry.primary && entry.environmentId === primary.target?.environmentId) await subscribe(streamOf(entry, entry.generation), call);
}

/** The welcome payload the first-run gate reads (null until the primary sent one). */
export const primaryWelcome = (): Obj | null => primary.target?.environmentId && primaryLifecycle.environmentId === primary.target.environmentId ? primaryLifecycle.welcome : null;

// ── LegacyThreadMigrationToast ──────────────────────────────────────────────
const toastIds = new WeakMap<object, number>();
/** The toast's description: the count with the user's grouping (toLocaleString). */
export function migrationDescription(totalThreadCount: number): string {
  return `Migrating ${totalThreadCount.toLocaleString()} ${totalThreadCount === 1 ? 'thread' : 'threads'} from the previous version. You can keep working while this finishes.`;
}
/** After each refresh: open the loading toast while the primary's migration runs; close it once it does not. */
export function migrationToast(client: T3Client): void {
  const migration = primary.target && primaryLifecycle.environmentId === primary.target.environmentId ? primaryLifecycle.migration : null;
  const open = toastIds.get(client);
  if (migration?.status === 'running') {
    if (open !== undefined) return;
    toastIds.set(client, pushToast(client, { kind: 'loading', title: 'Restoring your threads…', description: migrationDescription(migration.totalThreadCount), timeoutMs: 0, hideCopy: true }));
    return;
  }
  if (open !== undefined) { dismissToast(client, open); toastIds.delete(client); }
}
