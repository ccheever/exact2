// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/settings-scope.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// MIT T3 Code 1e2ecbd975 (LICENSE-T3): apps/web/src/components/settings/settingsScope.ts
// and settingsScopeAxis.ts. Changes: ids are plain strings (no branded EnvironmentId);
// a member's `physicalProjectKey` is the clone's checkout id (the project id the scope
// state carries in `settingsCheckout`), see settings-scope-sources.ts.

/**
 * Two axes. `machine` narrows the environment axis (absent = all environments);
 * `project` and `checkout` narrow the project axis (absent = environment defaults).
 * Device-local preferences are not a scope: they render regardless of the selection.
 */
export interface SettingsScopeSearch { project?: string; machine?: string; checkout?: string }

export interface ScopeMember { id: string; environmentId: string; workspaceRoot: string; physicalProjectKey: string; [key: string]: unknown }
export interface ScopeGroup { projectKey: string; displayName: string; memberProjects: readonly ScopeMember[] }
export interface ScopeEnvironmentName { environmentId: string; label: string }

type ScopeTargets = { label: string; members: readonly ScopeMember[]; environmentIds: readonly string[] };
export type UnavailableReason = 'project-required' | 'project-missing' | 'environment-missing' | 'checkout-missing';
export type ResolvedSettingsScope = ScopeTargets & (
  | { kind: 'all' }
  | { kind: 'environment'; environmentId: string }
  | { kind: 'project'; group: ScopeGroup; environmentId: string | null }
  | { kind: 'checkout'; group: ScopeGroup; checkout: ScopeMember; environmentId: string }
  | { kind: 'unavailable'; reason: UnavailableReason; message: string }
);

/** Stale IDs remain visible to the resolver so a removed target reads as unavailable, not as "all". */
export function validateSettingsScopeSearch(raw: Record<string, unknown>): SettingsScopeSearch {
  const stringValue = (value: unknown) => typeof value === 'string' && value.trim().length > 0 ? value : undefined;
  const project = stringValue(raw.project), machine = stringValue(raw.machine), checkout = stringValue(raw.checkout);
  return { ...(project === undefined ? {} : { project }), ...(machine === undefined ? {} : { machine }), ...(checkout === undefined ? {} : { checkout }) };
}

/** Resolves only existing targets. An unavailable selection never broadens a subsequent write. */
export function resolveSettingsScope(search: SettingsScopeSearch, groups: readonly ScopeGroup[], environments: readonly ScopeEnvironmentName[]): ResolvedSettingsScope {
  const unavailable = (reason: UnavailableReason, message: string): ResolvedSettingsScope =>
    ({ kind: 'unavailable', reason, label: 'Unavailable selection', message, members: [], environmentIds: [] });
  if (search.checkout && !search.project) return unavailable('project-required', 'Select a project to choose one of its checkouts.');
  const environment = environments.find(candidate => candidate.environmentId === search.machine);
  if (search.machine && !environment) return unavailable('environment-missing', 'This environment is no longer available.');
  if (search.project) {
    const group = groups.find(candidate => candidate.projectKey === search.project);
    if (!group) return unavailable('project-missing', 'This project is no longer available.');
    const members = group.memberProjects.filter(member => (search.machine === undefined || member.environmentId === search.machine)
      && (search.checkout === undefined || member.physicalProjectKey === search.checkout));
    if (members.length === 0) {
      return unavailable('checkout-missing', search.checkout ? 'This checkout is no longer available in the selected project and environment.' : 'This project has no checkout on this environment.');
    }
    if (search.checkout) {
      const checkout = members[0]!;
      const checkoutEnvironment = environments.find(candidate => candidate.environmentId === checkout.environmentId);
      if (!checkoutEnvironment) return unavailable('environment-missing', "This checkout's environment is no longer available.");
      const sharesEnvironment = group.memberProjects.some(member => member.environmentId === checkout.environmentId && member.physicalProjectKey !== checkout.physicalProjectKey);
      return { kind: 'checkout', group, checkout, environmentId: checkout.environmentId,
        label: `${group.displayName} / ${checkoutEnvironment.label}${sharesEnvironment ? ` · ${checkout.workspaceRoot}` : ''}`, members, environmentIds: [checkout.environmentId] };
    }
    return { kind: 'project', group, environmentId: environment?.environmentId ?? null, label: `${group.displayName} / ${environment?.label ?? 'All checkouts'}`,
      members, environmentIds: [...new Set(members.map(member => member.environmentId))] };
  }
  if (environment) return { kind: 'environment', environmentId: environment.environmentId, label: environment.label, members: [], environmentIds: [environment.environmentId] };
  return { kind: 'all', label: 'All environments', members: [], environmentIds: environments.map(candidate => candidate.environmentId) };
}

// ── settingsScopeAxis.ts ──────────────────────────────────────────────────
type AxisEnvironment = { environmentId: string; label: string; displayUrl?: string | null };

export function settingsScopeEnvironmentLabel(environment: AxisEnvironment, environments: readonly AxisEnvironment[]): string {
  const duplicate = environments.some(other => other.environmentId !== environment.environmentId && other.label === environment.label);
  return duplicate ? `${environment.label} · ${environment.displayUrl ?? environment.environmentId}` : environment.label;
}

export const ALL_ENVIRONMENTS_VALUE = 'all';
export const ALL_PROJECTS_VALUE = 'all';

/** The environment axis: `all` or an environment id. A legacy checkout link without `machine` still names one environment. */
export function environmentAxisValue(search: SettingsScopeSearch, resolvedEnvironmentId?: string | null): string {
  return search.machine ?? resolvedEnvironmentId ?? ALL_ENVIRONMENTS_VALUE;
}
/** The project axis: `all` or a project key. */
export function projectAxisValue(search: SettingsScopeSearch): string { return search.project ?? ALL_PROJECTS_VALUE; }

/** Choosing an environment keeps the project; a pre-existing checkout narrowing is dropped. */
export function selectEnvironmentAxis(search: SettingsScopeSearch, value: string): SettingsScopeSearch {
  const next: SettingsScopeSearch = {};
  if (search.project) next.project = search.project;
  if (value !== ALL_ENVIRONMENTS_VALUE) next.machine = value;
  return next;
}
/** Choosing a project keeps the environment axis. */
export function selectProjectAxis(search: SettingsScopeSearch, value: string): SettingsScopeSearch {
  const next: SettingsScopeSearch = {};
  if (value !== ALL_PROJECTS_VALUE) next.project = value;
  if (search.machine) next.machine = search.machine;
  return next;
}

/** Provider configuration always belongs to one environment, including project scopes. */
export function selectSingleEnvironmentScope(search: SettingsScopeSearch, scope: ResolvedSettingsScope,
  environments: readonly { environmentId: string; connection: { phase: string } }[], primaryEnvironmentId: string | null): SettingsScopeSearch {
  if (search.machine || scope.kind === 'unavailable') return search;
  const selectedIds = new Set(scope.environmentIds);
  const candidates = environments.filter(environment => selectedIds.has(environment.environmentId));
  const selected = candidates.find(environment => environment.environmentId === primaryEnvironmentId)
    ?? candidates.find(environment => environment.connection.phase === 'connected') ?? candidates[0];
  return selected ? { ...search, machine: selected.environmentId } : search;
}

/** The clone's scope state (`settingsMachine`, `settingsProjectKey`, `settingsCheckout`, empty = absent) as a search. */
export const scopeSearch = (machine: string, project: string, checkout: string): SettingsScopeSearch => validateSettingsScopeSearch({ machine, project, checkout });
