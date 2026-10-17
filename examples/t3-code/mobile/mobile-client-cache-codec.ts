// @ref llp/1109.009-mobile-settings.decision.md#offline-cache-storage
import { applyShell, initialShell, obj, threadSnapshot, type Obj, type Shell, type ThreadState } from './shared/domain';
import { applyConfig } from './shared/protocol';

/** App-owned display cache, not the upstream schema-3 orchestration envelope.
 * Shared Shell retains active rows only and does not retain the wire schemaVersion
 * or archivedThreads. These codecs preserve exactly that adopted read projection.
 * They return data only: session grants, synchronization and command ownership are
 * never restored here. Callers must separately check the current cache/read owner.
 */
export const MOBILE_CLIENT_CACHE_SCHEMA_VERSION = 1;
export const MOBILE_CLIENT_CACHE_FORMAT = 't3-code-mobile-read-cache';
type Kind = 'shell' | 'thread' | 'server-config';

function identity(value: string): void {
  if (!value || value.trim() !== value) throw new Error('Invalid mobile cache identity.');
}
function record(value: unknown): Obj {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) throw new Error('Invalid mobile cache object.');
  return value as Obj;
}
function envelope(environmentId: string, kind: Kind): Obj {
  identity(environmentId);
  return { format: MOBILE_CLIENT_CACHE_FORMAT, schemaVersion: MOBILE_CLIENT_CACHE_SCHEMA_VERSION, environmentId, kind };
}
function decode(text: string, environmentId: string, kind: Kind): Obj {
  identity(environmentId);
  const saved = record(JSON.parse(text));
  if (saved.format !== MOBILE_CLIENT_CACHE_FORMAT || saved.schemaVersion !== MOBILE_CLIENT_CACHE_SCHEMA_VERSION
    || saved.environmentId !== environmentId || saved.kind !== kind) throw new Error('Mobile cache owner or format changed.');
  return saved;
}
function discardInvalid<T>(read: () => T): T | null {
  try { return read(); } catch { return null; }
}

/** Explicit wire reconstruction makes the existing adopted-state validator reusable. */
export function encodeMobileShellCache(environmentId: string, shell: Shell): string {
  const snapshot = { snapshotSequence: shell.sequence, projects: shell.projects, threads: shell.threads };
  applyShell(initialShell(), snapshot);
  return JSON.stringify({ ...envelope(environmentId, 'shell'), snapshot });
}
export function decodeMobileShellCache(text: string, environmentId: string): Shell | null {
  return discardInvalid(() => {
    const snapshot = record(decode(text, environmentId, 'shell').snapshot);
    return applyShell(initialShell(), { snapshotSequence: snapshot.snapshotSequence, projects: snapshot.projects, threads: snapshot.threads });
  });
}

function boundedSnapshot(snapshot: unknown, threadId: string): ThreadState {
  identity(threadId);
  const wire = record(snapshot);
  // Our format always writes all three history fields. A missing field is corrupt,
  // not an older full-history record that the shared wire parser may default.
  if (!Object.hasOwn(wire, 'historyCursor') || !Object.hasOwn(wire, 'hasMoreHistory')
    || !Object.hasOwn(wire, 'latestLocalTurnOrdinal') || wire.historyCursor === undefined
    || wire.hasMoreHistory === undefined || wire.latestLocalTurnOrdinal === undefined) throw new Error('Missing mobile cache history boundary.');
  const state = threadSnapshot(wire);
  if (obj(state.projection.thread).id !== threadId) throw new Error('Mobile cache thread changed.');
  if (obj(state.projection.thread).deletedAt != null) throw new Error('Deleted thread is not a cache snapshot.');
  return state;
}
export function encodeMobileThreadCache(environmentId: string, threadId: string, thread: ThreadState): string {
  const snapshot = { projection: thread.projection, snapshotSequence: thread.sequence, historyCursor: thread.historyCursor,
    hasMoreHistory: thread.hasMore, latestLocalTurnOrdinal: thread.latestLocalTurnOrdinal };
  boundedSnapshot(snapshot, threadId);
  return JSON.stringify({ ...envelope(environmentId, 'thread'), threadId, snapshot });
}
export function decodeMobileThreadCache(text: string, environmentId: string, threadId: string): ThreadState | null {
  return discardInvalid(() => {
    const saved = decode(text, environmentId, 'thread');
    if (saved.threadId !== threadId) throw new Error('Mobile cache thread changed.');
    return boundedSnapshot(saved.snapshot, threadId);
  });
}

function cachedConfig(config: unknown, environmentId: string): Obj {
  const input = record(config);
  if (record(input.environment).environmentId !== environmentId) throw new Error('Mobile cache environment changed.');
  if (!Array.isArray(input.providers)) throw new Error('Invalid mobile cache provider catalog.');
  input.providers.forEach(record);
  // Pinned serverConfigProjection.withoutEnvironmentThemes drops both published
  // themes AND usage-limit sources. Shared snapshot adoption does the same from {}.
  return applyConfig({}, { type: 'snapshot', config: input });
}
export function encodeMobileConfigCache(environmentId: string, config: Obj): string {
  return JSON.stringify({ ...envelope(environmentId, 'server-config'), config: cachedConfig(config, environmentId) });
}
export function decodeMobileConfigCache(text: string, environmentId: string): Obj | null {
  return discardInvalid(() => cachedConfig(decode(text, environmentId, 'server-config').config, environmentId));
}
