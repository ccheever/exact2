// StorageSettings CleanupSection and DatabaseStorageNotice (MIT T3 Code
// 9c161462c9, LICENSE-T3). Commands execute on each owning environment.
import { arr, effectiveWorktreeRules, num, obj, str, type Obj } from '../client/domain';
import type { T3Client } from '../client/client';
import { bridgeReply, ClientError, type Files, type Native } from '../client/protocol';
import { liveEnvironments, type LiveEnvironment } from '../client/live-streams';
import { fleet } from '../connections/settings-b-fleet';
import { shellPrefs } from '../shell/shell-prefs';
import { pushToast } from '../shell/toast';
import { letGo } from '../client/let-go';
const GIB = 1024 ** 3;
export const EMPTY_DATABASE_NOTICE = { show: false, title: '', description: '', command: '', environmentId: '' };
export function databaseNotice(status: Obj | undefined, environmentId: string, label: string, dismissed: readonly string[]) {
  if (!status || status.incrementalVacuumEnabled === true || num(status.reclaimableBytes) < GIB || dismissed.includes(environmentId)) return EMPTY_DATABASE_NOTICE;
  return { show: true, environmentId, title: `Reclaim ${(num(status.reclaimableBytes) / GIB).toFixed(1)} GiB on ${label}`,
    description: `Old thread history is cleaned up automatically, but the database still holds on to the freed space. Run this on ${label} to return it to disk. It pauses and restarts the service; quit the desktop app first if it hosts this environment.`, command: str(status.compactCommand) };
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
