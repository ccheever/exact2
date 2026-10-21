// StorageSettings CleanupSection, CleanupResults and DatabaseStorageNotice (MIT T3 Code
// 74541bab17, LICENSE-T3). Commands execute on each owning environment.
import { arr, effectiveWorktreeRules, num, obj, str, type Obj } from '../client/domain';
import type { T3Client } from '../client/client';
import { bridgeReply, ClientError, type Files, type Native } from '../client/protocol';
import { liveEnvironments, type LiveEnvironment } from '../client/live-streams';
import { fleet } from '../connections/settings-b-fleet';
import { shellPrefs } from '../shell/shell-prefs';
import { pushToast } from '../shell/toast';
import { letGo } from '../client/let-go';
const GIB = 1024 ** 3;

/** lib/formatBytes.ts. */
export function formatBytes(value: number): string {
  if (value < 1_024) return `${Math.round(value)} B`;
  const units = ['KB', 'MB', 'GB', 'TB'] as const;
  let next = value, unitIndex = -1;
  do { next /= 1_024; unitIndex += 1; } while (next >= 1_024 && unitIndex < units.length - 1);
  return `${next.toFixed(next >= 100 ? 0 : next >= 10 ? 1 : 2)} ${units[unitIndex]}`;
}

export type DatabaseNotice = { environmentId: string; title: string; command: string };
/**
 * databaseCompactionRecommendations (client-runtime state/server.ts) and DatabaseStorageNotice: every machine whose
 * database still holds at least 1 GiB of freed pages without incremental vacuum (shouldRecommendDatabaseCompaction),
 * the most reclaimable first, less the dismissed ones. One Database section lists them all.
 */
export function databaseCompactionNotices(environments: readonly { environmentId: string; label: string; status: Obj | undefined }[], dismissed: readonly string[]): DatabaseNotice[] {
  return environments.flatMap(({ environmentId, label, status }) => status && status.incrementalVacuumEnabled !== true && num(status.reclaimableBytes) >= GIB
    ? [{ environmentId, label, reclaimable: num(status.reclaimableBytes), command: str(status.compactCommand) }] : [])
    .sort((left, right) => right.reclaimable - left.reclaimable)
    .filter(({ environmentId }) => !dismissed.includes(environmentId))
    .map(({ environmentId, label, reclaimable, command }) => ({ environmentId, title: `${(reclaimable / GIB).toFixed(1)} GiB on ${label}`, command }));
}

export type CleanupDetail = { key: string; label: string; value: string; code: boolean };
export type CleanupEntry = { id: string; name: string; reason: string; failed: boolean; details: CleanupDetail[]; size: string; first: boolean };
/**
 * CleanupResults: each entry named by its folder (a thread's worktree by its last path segment), its reason, then the
 * machine, project, folder and thread it concerns; the files and bytes it freed or would free on the right. `leading`:
 * this report opens the fold, so its first line draws no divider of its own.
 */
export function cleanupEntries(report: Obj, machine: string, leading = false): { entries: CleanupEntry[]; omitted: string; omittedFirst: boolean; empty: string; emptyFirst: boolean } {
  const entries = arr(report.entries).map((entry, index): CleanupEntry => {
    const path = typeof entry.path === 'string' ? entry.path : null, projectPath = typeof entry.projectPath === 'string' ? entry.projectPath : null;
    const threadTitle = typeof entry.threadTitle === 'string' ? entry.threadTitle : '';
    const filesCount = typeof entry.files === 'number' ? entry.files : null, freed = typeof entry.bytes === 'number' ? entry.bytes : null;
    const name = entry.kind === 'worktree' ? ((entry.threadId == null ? path : path?.split(/[\\/]/).filter(Boolean).pop()) ?? 'Worktrees')
      : entry.kind === 'logs' ? 'Rotated logs' : 'Browser artifacts';
    const details: CleanupDetail[] = [{ key: 'machine', label: 'Machine', value: machine, code: false },
      ...(projectPath ? [{ key: 'project', label: 'Project', value: projectPath, code: true }] : []),
      ...(path ? [{ key: 'folder', label: 'Folder', value: path, code: true }] : []),
      ...(threadTitle ? [{ key: 'thread', label: 'Thread', value: threadTitle, code: false }] : [])];
    const sized = entry.outcome === 'removed' || (entry.outcome === 'kept' && freed !== null);
    const size = !sized ? '' : [entry.kind !== 'worktree' && filesCount !== null ? `${filesCount.toLocaleString()} ${filesCount === 1 ? 'file' : 'files'}` : null,
      freed !== null ? formatBytes(freed) : null].filter(Boolean).join(' · ');
    return { id: `${path ?? str(entry.kind)}:${str(entry.threadId) || index}`, name, reason: str(entry.reason), failed: entry.outcome === 'failed', details, size, first: leading && index === 0 };
  });
  const omitted = num(report.omittedCount);
  return { entries, omitted: omitted > 0 ? `${machine}: ${omitted.toLocaleString()} more results.` : '', omittedFirst: leading && entries.length === 0,
    empty: entries.length === 0 && omitted === 0 ? `${machine}: Nothing needed cleanup.` : '', emptyFirst: leading && entries.length === 0 };
}
export function cleanupHasRules(settings: Obj): boolean {
  const enabled = (rules: Obj) => typeof rules.worktreeAfterDays === 'number' || typeof rules.worktreeSettledAfterDays === 'number' || rules.worktreeOnDelete === true || rules.worktreeOnMerge === true || rules.worktreeUnchanged === true;
  const storage = obj(settings.storageCleanup);
  return typeof storage.browserArtifactsAfterDays === 'number' || typeof storage.logsAfterDays === 'number' || enabled(effectiveWorktreeRules(settings, {}))
    || Object.values(obj(settings.projectSettingsOverrides)).some(raw => { const policy = obj(obj(raw).worktreeCleanup); return policy.mode === 'custom' && enabled(obj(policy.rules)); });
}
export function cleanupPermitted(client: Pick<T3Client, 'scopes'>, environment: Pick<LiveEnvironment, 'focused' | 'key'>): boolean {
  const scopes = environment.focused ? client.scopes : fleet.entries.get(environment.key)?.scopes ?? [];
  return scopes.includes('settings:write');
}
export async function runCleanup(client: T3Client, native: Native, environmentIds: readonly string[]): Promise<void> {
  const environments = liveEnvironments(client, native).filter(environment => environmentIds.includes(environment.environmentId) && environment.connected
    && obj(obj(environment.config.environment).capabilities).storageCleanupRun === true && cleanupPermitted(client, environment));
  if (!environments.length) throw new ClientError('Connect an environment with permission to run cleanup.');
  await Promise.all(environments.map(async environment => {
    try {
      const report = await environment.request('server.runStorageCleanup', {}, true);
      const failure = arr(report.entries).find(entry => entry.outcome === 'failed');
      if (failure) pushToast(client, { kind: 'error', title: `Cleanup failed on ${environment.label}`, description: str(failure.reason) });
    } catch (error) {
      if (letGo(error)) throw error;
      pushToast(client, { kind: 'error', title: `Could not run cleanup on ${environment.label}`, description: error instanceof Error ? error.message : String(error) });
    }
  }));
}
export async function databaseCommand(client: T3Client, native: Native, storage: Files, op: string, environmentId: string, command: string): Promise<string> {
  if (op === 'storage-dismiss-database') {
    const prefs = shellPrefs(client);
    prefs.databaseStorageDismissals = [...new Set([...prefs.databaseStorageDismissals, environmentId])].slice(-200);
    await client.savePreferences(storage);
  } else if (op === 'storage-copy-database') {
    const reply = await bridgeReply(native, { op: 'copyText', text: command });
    if (!reply.ok) throw new ClientError(reply.error?.message || 'Could not copy command.');
  } else throw new ClientError('Unsupported database notice action.');
  return '';
}
