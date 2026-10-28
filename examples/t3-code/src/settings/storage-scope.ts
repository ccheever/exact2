// Storage uses the shared settings planner so each selected checkout keeps its
// own inherited rules and every write uses the owning environment's transport.
import type { T3Client } from '../client/client';
import { obj, str, type Obj, type Json } from '../client/domain';
import { ClientError, type Native } from '../client/protocol';
import { resolveScope, settingPlan } from './settings-core';
import { fleet } from '../connections/settings-b-fleet';
import { persistScopedSettingsPatch, scopedPlanNotice } from './scoped-settings-plan';
import { postScopedNotice, scopedWriter } from './settings-scope-sources';
import { runCleanup } from './storage-maintenance';

export function storageScope(client: T3Client, key: string) {
  const parts = key.split('|');
  if (parts.length !== 3) throw new ClientError('That storage scope is no longer available.');
  return resolveScope(client, parts[0], parts[1], parts[2]);
}
export async function scopedStorageCommand(client: T3Client, native: Native, op: string, input: Record<string, string>, validate: (settings: Obj, projectId: string, key: string, raw: string) => Obj): Promise<string> {
  const scope = storageScope(client, input.scope);
  if (scope.kind === 'unavailable' || !scope.connected) throw new ClientError(scope.message || 'Choose a connected environment.');
  if (op === 'storage-run') {
    await runCleanup(client, native, scope.selected.map(environment => environment.environmentId));
    return '';
  }
  const project = scope.kind === 'project' || scope.kind === 'checkout';
  const selected = scope.selected.filter(environment => environment.connection.phase === 'connected');
  const capability = (key: string) => selected.every(environment => obj(obj((environment.fleetKey ? fleet.entries.get(environment.fleetKey)?.config : client.config)?.environment).capabilities)[key] === true);
  const key = str(input.key), raw = str(input.value);
  // Worktree location is the environment's own setting (WorktreesDirectoryRow): it asks only for worktreesDirectory.
  if (key !== 'worktreesDirectory' && (!capability('storageCleanup') || (project && !capability('projectWorktreeCleanup')))) throw new ClientError('Update the selected environments to configure storage cleanup.');
  if ((key === 'worktreeSettledAfterDays' && !capability('settledWorktreeCleanup')) || (key === 'worktreesDirectory' && !capability('worktreesDirectory'))) throw new ClientError('Update the selected environments to change this storage setting.');
  const settings = selected.some(environment => environment.environmentId === client.environmentId && !environment.fleetKey) ? await client.settingsCoreRequest(native, 'server.getSettings', {}) : undefined;
  let name = 'storageCleanup', value: Json, clear = false;
  if (project) {
    name = 'worktreeCleanup';
    if (key === 'mode') {
      if (!['inherit', 'off', 'custom'].includes(raw)) throw new ClientError('Unsupported worktree cleanup mode.');
      clear = raw === 'inherit'; value = raw === 'off' ? { mode: 'off' } : { mode: 'custom', rules: {} };
    } else {
      const checked = validate({}, '', key, raw);
      if (key === 'worktreesDirectory' || !key.startsWith('worktree')) throw new ClientError('This storage setting belongs to the environment.');
      value = { mode: 'custom', rules: obj(checked.storageCleanup) };
    }
  } else {
    const checked = validate({}, '', key, raw);
    name = key === 'worktreesDirectory' ? key : 'storageCleanup'; value = checked[name] ?? null;
  }
  const plan = settingPlan(client, scope, name, value, clear, settings);
  const refused = scopedPlanNotice(plan);
  if (refused) { postScopedNotice(client, refused); return ''; }
  const result = await persistScopedSettingsPatch(plan, scopedWriter(client, native, scope.environments), () => {});
  postScopedNotice(client, scopedPlanNotice(plan, result));
  return '';
}
