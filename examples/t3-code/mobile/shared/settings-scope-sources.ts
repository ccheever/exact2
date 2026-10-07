// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/settings-scope-sources.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// The settings scope's environments, project groups and per-environment writes
// (reference: SettingsScopeContext.tsx, useSettingsProjectGroups.ts, useScopedSettings.ts
// useRunScopedPlan at T3 Code 1e2ecbd975, MIT, LICENSE-T3). The focused environment is
// T3Client's own connection; every other switched-on saved environment is a fleet entry
// (settings-b-fleet.ts) whose server config subscribeServerConfig keeps live. Each write
// goes through its environment's own transport and all of them are awaited together.
import { obj, str, type Obj } from './domain';
import { bridgeReply, type Native } from './protocol';
import type { T3Client } from './client';
import { fleet as appFleet, EnvironmentFleet, environmentKey, type FleetEntry } from './settings-b-fleet';
import { machineKind } from './connections';
import { groupLabel } from './r6-polish-groups';
import type { ScopeGroup, ScopeMember } from './settings-scope';
import type { ScopedSettingsEnvironment } from './scoped-settings-plan';
import { pushToast } from './toast';

/** One environment the scope can name: its connection, config and the transport its writes take. */
export interface ScopeEnvironment extends ScopedSettingsEnvironment {
  readonly displayUrl: string | null;
  readonly kind: string;
  /** '' for the focused environment, else the fleet key (`origin\nenvironmentId`). */
  readonly fleetKey: string;
}
type Focused = Pick<T3Client, 'environmentId' | 'config' | 'ready'> & Partial<Pick<T3Client, 'origin' | 'local'>> & { projectGroups(): { key: string; name: string; members: Obj[] }[] };

const hostOf = (origin: string) => { try { return new URL(origin).host || origin; } catch { return origin; } };
const configOf = (config: Obj, connected: boolean) => connected && config.settings !== undefined
  ? { settings: obj(config.settings), environment: { capabilities: { projectSettingsOverrides: obj(obj(config.environment).capabilities).projectSettingsOverrides === true } } } : null;

/** Every environment the scope menu lists: the focused one first, then each switched-on background one. */
export function scopeEnvironments(client: Focused, source: EnvironmentFleet = appFleet): ScopeEnvironment[] {
  const list: ScopeEnvironment[] = [];
  const focusedKey = client.environmentId && client.origin ? environmentKey(client.origin, client.environmentId) : '';
  if (client.environmentId) {
    const environment = obj(client.config.environment);
    list.push({ environmentId: client.environmentId, label: str(environment.label, 'This environment'), displayUrl: client.origin ?? null, kind: machineKind(client.config),
      connection: { phase: client.ready ? 'connected' : 'disconnected' }, serverConfig: configOf(client.config, client.ready), fleetKey: '' });
  }
  for (const entry of source.entries.values()) {
    if (!entry.environmentId || entry.key === focusedKey || list.some(known => known.environmentId === entry.environmentId)) continue;
    const saved = source.saved.find(item => environmentKey(str(item.origin), str(item.environmentId)) === entry.key);
    const synchronized = entry.phase === 'connected' && entry.synchronized === entry.generation;
    list.push({ environmentId: entry.environmentId, label: str(obj(entry.config.environment).label) || str(saved?.label) || hostOf(entry.origin), displayUrl: entry.origin,
      kind: machineKind(entry.config, str(saved?.machine)), connection: { phase: synchronized ? 'connected' : entry.phase === 'connected' ? 'connecting' : entry.phase },
      serverConfig: configOf(entry.config, synchronized), fleetKey: entry.key });
  }
  return list;
}

const projectPath = (value: unknown) => str(value).trim().replace(/\\/g, '/').replace(/\/+$/, '');
/** The clone's checkout id is the project id: the scope state's `settingsCheckout` carries it. */
const memberOf = (project: Obj, environmentId: string): ScopeMember => ({ ...project, id: str(project.id), environmentId, workspaceRoot: str(project.workspaceRoot), physicalProjectKey: str(project.id) });

/**
 * Project groups across environments (useSettingsProjectGroups): the focused shell's groups,
 * joined by each synchronized background environment's projects under the same grouping key.
 */
export function scopeProjectGroups(client: Focused, source: EnvironmentFleet = appFleet): ScopeGroup[] {
  const groups = new Map<string, { name: string; members: ScopeMember[]; projects: Obj[] }>();
  for (const group of client.projectGroups()) groups.set(group.key, { name: group.name, members: group.members.map(project => memberOf(project, client.environmentId)), projects: [...group.members] });
  const local = obj(client.local as unknown), overrides = obj(local.groupingOverrides), mode = str(local.groupingMode, 'repository');
  for (const entry of source.entries.values()) {
    if (entry.phase !== 'connected' || entry.synchronized !== entry.generation || entry.environmentId === client.environmentId) continue;
    for (const project of entry.shell.projects) {
      const physical = `${entry.environmentId}:${projectPath(project.workspaceRoot)}`, grouping = str(overrides[physical], mode);
      const identity = obj(project.repositoryIdentity), canonical = str(identity.canonicalKey), root = projectPath(identity.rootPath), path = projectPath(project.workspaceRoot);
      const relative = root && path.startsWith(root + '/') ? path.slice(root.length + 1) : '';
      const key = grouping === 'separate' || !canonical ? physical : canonical + (grouping === 'repository_path' && relative ? `::${relative}` : '');
      const group = groups.get(key) ?? { name: '', members: [], projects: [] };
      group.members.push(memberOf(project, entry.environmentId)); group.projects.push(project);
      groups.set(key, group);
    }
  }
  return [...groups].map(([projectKey, group]) => ({ projectKey, displayName: group.name || groupLabel(group.projects), memberProjects: group.members }));
}

const entryOf = (source: EnvironmentFleet, key: string): FleetEntry | undefined => source.entries.get(key);
type Writer = Pick<T3Client, 'config'> & { settingsCoreRequest(native: Native, method: string, payload: Obj, write?: boolean): Promise<Obj> };

/** One request to the environment's own transport: T3Client for the focused one, the fleet for the rest. */
export async function environmentRequest(client: Writer, native: Native, environment: Pick<ScopeEnvironment, 'fleetKey'>, method: string, payload: Obj, write = false, source: EnvironmentFleet = appFleet): Promise<Obj> {
  if (!environment.fleetKey) return client.settingsCoreRequest(native, method, payload, write);
  const entry = entryOf(source, environment.fleetKey);
  if (!entry || entry.phase !== 'connected') throw new Error('The environment is not connected.');
  const reply = await bridgeReply(EnvironmentFleet.native(native, entry.key), { op: 'request', method, payload, generation: entry.generation });
  if (!reply.ok) throw new Error(reply.error!.message);
  if (reply.generation !== entry.generation) throw new Error('The connection changed.');
  return obj(reply.value);
}

/** persistScopedSettingsPatch's persistServer: one server.updateSettings per environment; the answer replaces that environment's settings. */
export function scopedWriter(client: Writer, native: Native, environments: readonly ScopeEnvironment[], source: EnvironmentFleet = appFleet) {
  return async ({ environmentId, input }: { environmentId: string; input: { patch: Obj } }): Promise<{ _tag: 'Success' | 'Failure' }> => {
    const environment = environments.find(candidate => candidate.environmentId === environmentId);
    if (!environment) return { _tag: 'Failure' };
    const updated = await environmentRequest(client, native, environment, 'server.updateSettings', input, true, source);
    if (!environment.fleetKey) client.config = { ...client.config, settings: updated };
    else { const entry = entryOf(source, environment.fleetKey); if (entry) { entry.config = { ...entry.config, settings: updated }; source.revision++; } }
    return { _tag: 'Success' };
  };
}

/** t3.json per member through its environment's transport; absent or unreadable files resolve to null. */
export async function scopeMemberFiles(client: Writer, native: Native | null | undefined, members: readonly ScopeMember[], environments: readonly ScopeEnvironment[],
  parse: (contents: string) => Obj | null, source: EnvironmentFleet = appFleet): Promise<Map<string, Obj | null>> {
  const files = new Map<string, Obj | null>();
  if (!native?.available) return files;
  await Promise.all(members.map(async member => {
    const environment = environments.find(candidate => candidate.environmentId === member.environmentId);
    if (!environment || environment.connection.phase !== 'connected') return;
    try {
      const result = await environmentRequest(client, native, environment, 'projects.readFile', { cwd: member.workspaceRoot, relativePath: 't3.json' }, false, source);
      files.set(member.physicalProjectKey, result.truncated === true ? null : parse(str(result.contents)));
    } catch { files.set(member.physicalProjectKey, null); }
  }));
  return files;
}

/** useRunScopedPlan's toast, posted to the app's queue. */
export function postScopedNotice(client: T3Client, notice: { kind: 'warning' | 'error'; title: string; description: string } | null): void {
  if (notice) pushToast(client, { kind: notice.kind, title: notice.title, description: notice.description });
}
