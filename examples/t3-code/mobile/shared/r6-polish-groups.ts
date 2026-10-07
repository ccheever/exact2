// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/r6-polish-groups.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// r6-polish: logical project names across the focused environment and the
// fleet's background environments, adapted from T3 Code (MIT; see LICENSE-T3):
// packages/client-runtime/src/state/projectGrouping.ts (deriveLogicalProjectKey,
// deriveProjectGroupLabel, buildProjectGroups) and apps/web/src/components/Sidebar.tsx
// (projectDisplayNameByKey: every member project reads its group's displayName).
// Two checkouts of `acme/shared` named "shared" read "acme/shared", in the sidebar
// rows' project label and the draft hero alike.
import { obj, str, type Obj } from './domain';
import { fleet, type EnvironmentFleet } from './settings-b-fleet';

type Grouping = { environmentId: string; shell: { projects: Obj[] }; local?: { groupingMode?: string; groupingOverrides?: Record<string, string> } };

const projectPath = (value: unknown) => str(value).trim().replace(/\\/g, '/').replace(/\/+$/, '');
const unique = (values: string[]) => [...new Set(values.map(value => value.trim()).filter(Boolean))];

/** deriveProjectGroupLabel: a shared title unless it is only the repository's name, then the display name, then the repository name. */
export function groupLabel(members: Obj[]): string {
  const representative = str(members[0]?.title);
  if (members.length < 2) return representative;
  const titles = unique(members.map(member => str(member.title)));
  const displayNames = unique(members.map(member => str(obj(member.repositoryIdentity).displayName)));
  const names = unique(members.map(member => str(obj(member.repositoryIdentity).name)));
  if (titles.length === 1 && !displayNames.includes(titles[0]!) && !names.includes(titles[0]!)) return titles[0]!;
  if (displayNames.length === 1) return displayNames[0]!;
  if (names.length === 1) return names[0]!;
  return representative;
}

/** deriveLogicalProjectKey under the client's grouping mode and per-checkout overrides. */
export function logicalKey(client: Grouping, environmentId: string, project: Obj): string {
  const physical = `${environmentId}:${projectPath(project.workspaceRoot)}`;
  const mode = client.local?.groupingOverrides?.[physical] || client.local?.groupingMode || 'repository';
  const identity = obj(project.repositoryIdentity), canonical = str(identity.canonicalKey);
  const root = projectPath(identity.rootPath), path = projectPath(project.workspaceRoot);
  const relative = root && path.startsWith(`${root}/`) ? path.slice(root.length + 1) : '';
  return mode === 'separate' || !canonical ? physical : canonical + (mode === 'repository_path' && relative ? `::${relative}` : '');
}

/**
 * Each project's group name, keyed as sidebar rows carry project ids: the
 * focused environment's own ids, and `fleet:<environment>:<id>` for a
 * background environment's (settings-b-fleet.ts fleetThreads).
 */
export function projectDisplayNames(client: Grouping, source: EnvironmentFleet = fleet): Map<string, string> {
  const members: { id: string; key: string; project: Obj }[] = (client.shell?.projects ?? []).map(project =>
    ({ id: str(project.id), key: logicalKey(client, client.environmentId, project), project }));
  for (const entry of source.entries.values()) {
    if (entry.phase !== 'connected' || entry.synchronized !== entry.generation || entry.environmentId === client.environmentId) continue;
    for (const project of entry.shell.projects)
      members.push({ id: `fleet:${entry.environmentId}:${str(project.id)}`, key: logicalKey(client, entry.environmentId, project), project });
  }
  const groups = new Map<string, Obj[]>();
  for (const member of members) groups.set(member.key, [...(groups.get(member.key) ?? []), member.project]);
  const labels = new Map([...groups].map(([key, projects]) => [key, groupLabel(projects)]));
  return new Map(members.map(member => [member.id, labels.get(member.key) || str(member.project.title)]));
}

/** The group key each project belongs to (the hero's menu lists a group once). */
export function projectGroupKeys(client: Grouping): Map<string, string> {
  return new Map((client.shell?.projects ?? []).map(project => [str(project.id), logicalKey(client, client.environmentId, project)]));
}

let cache: { client: unknown; revision: number; fleet: number; names: Map<string, string> } | null = null;
/** projectDisplayNames, kept while neither the client nor the fleet changed. */
export function cachedDisplayNames(client: Grouping & { revision: number }): Map<string, string> {
  if (cache && cache.client === client && cache.revision === client.revision && cache.fleet === fleet.revision) return cache.names;
  const names = projectDisplayNames(client);
  cache = { client, revision: client.revision, fleet: fleet.revision, names };
  return names;
}

/** buildSidebarProjectPickerEntries for the hero's menu: each group once (its first project in order), under its group's name. */
export function heroGroups<T extends { id: string; name: string }>(client: Grouping & { revision: number }, projects: T[]): T[] {
  const names = cachedDisplayNames(client), keys = projectGroupKeys(client), seen = new Set<string>();
  return projects.filter(project => { const key = keys.get(project.id) ?? project.id; return !seen.has(key) && (seen.add(key), true); })
    .map(project => ({ ...project, name: names.get(project.id) || project.name }));
}
