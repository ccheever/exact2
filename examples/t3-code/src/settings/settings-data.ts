import { T3Client } from '../client/client';
import { effectiveWorktreeRules, arr, obj, str, num, applyShell, initialShell, type Obj } from '../client/domain';
import type { Native } from '../client/protocol';
import { projectIdentity } from '../client/presentation';
import { liveEnvironments, watchStorageReports, watchDatabaseStorage } from '../client/live-streams';
import { cleanupEntries, cleanupHasRules, cleanupPermitted, databaseCompactionNotices, formatBytes, type CleanupEntry, type DatabaseNotice } from './storage-maintenance';
import { shellPrefs } from '../shell/shell-prefs';
import { diagnosticsRow, dividedSection, generalSections, inheritanceGroups, memberFiles, resolveScope, scopeRepresentative, scopeTarget, serverContext, type CoreRow } from './settings-core';
import { branchNamingRows, DEFAULTS as SOURCE_CONTROL_DEFAULTS, type ScopedRow } from './source-control-view';
import { can } from '../connections/command-permissions';
import { environmentClient, groupOfMember } from './projects-fleet';
import { settingsScopeEnvironmentLabel } from './settings-scope';
import { letGo } from '../client/let-go';
import { sleep } from '../client/command-waits';

const errorMessage = (error: unknown) => error instanceof Error ? error.message : 'Could not load this section.';
type NoticeRow = { id: string; name: string; version: string; license: string; bundles: string; sourceUrl: string; expanded: boolean; noticeText: string };
type ArchiveGroup = { id: string; title: string; mark: string; ink: string; surface: string; first: boolean; threads: { id: string; scope: string; title: string; description: string; first: boolean }[] };
const notices = new WeakMap<T3Client, { origin: string; generation: number; retry: number; entries: Obj[] }>();

/**
 * StorageCleanupSections' artifact and log status, `useDebouncedValue(\`${targetIds}:${mixed}\`, 300)`: a row reads
 * "Mixed across selected machines" only once more than one machine has disagreed for 300 ms, so a save that reaches
 * the machines one after another does not flash it. The first answer after the page opens is settled at once, as the
 * hook's first value is; a later change settles after 300 ms on the app's clock and asks again (`t3.notify`).
 */
const artifactStatuses = new WeakMap<T3Client, Map<string, { pending: string; settled: string; seq: number }>>();
export function settledStatus(client: T3Client, native: Native, key: string, value: string): string {
  let statuses = artifactStatuses.get(client);
  if (!statuses) artifactStatuses.set(client, statuses = new Map());
  const entry = statuses.get(key);
  if (!entry) { statuses.set(key, { pending: value, settled: value, seq: 0 }); return value; }
  if (entry.pending !== value) {
    const seq = ++entry.seq, owned = statuses;
    entry.pending = value;
    void sleep(300).then(async () => {
      if (entry.seq !== seq || owned.get(key) !== entry) return;
      entry.settled = value;
      await native.later({ op: 'wake', topic: 't3.notify' });
    }).catch(() => {});
  }
  if (entry.settled !== entry.pending) native.watch('t3.notify');
  return entry.settled;
}

// The connected server owns this manifest. Do not substitute a fabricated list
// of this native client's dependencies for the T3 bundle's actual notices.
export function decodeNotices(value: Obj): Obj[] {
  if (value.schemaVersion !== 1 || !Array.isArray(value.entries)) throw new Error('The open-source license manifest has an unsupported format.');
  const keys = new Set<string>();
  return value.entries.map((entry: unknown, index: number) => {
    const item = obj(entry);
    if (!['package', 'custom'].includes(str(item.kind)) || !Array.isArray(item.bundles) || !item.bundles.length || item.bundles.some(bundle => typeof bundle !== 'string') || ['license', 'name', 'noticeText'].some(key => typeof item[key] !== 'string') || !(item.version === null || typeof item.version === 'string')) throw new Error(`License entry ${index + 1} has an invalid shape.`);
    let sourceUrl = '';
    if (item.sourceUrl !== null) {
      try { const url = new URL(str(item.sourceUrl)); if (!['http:', 'https:'].includes(url.protocol)) throw new Error(); sourceUrl = url.href; }
      catch { throw new Error(`License entry ${index + 1} has an invalid source URL.`); }
    }
    const id = encodeURIComponent(JSON.stringify([item.kind, item.name, item.version]));
    if (keys.has(id)) throw new Error('The open-source license manifest contains a duplicate entry.');
    keys.add(id);
    return { ...item, id, sourceUrl };
  });
}

export async function licenseSettings(client: T3Client, native: Native | null | undefined, query: string, expanded: string, retry: number, active: boolean) {
  const empty = { ready: false, error: '', total: 0, matched: 0, entries: [] as NoticeRow[] };
  if (!active) return empty;
  try {
    let cached = notices.get(client);
    if (!cached || cached.origin !== client.origin || cached.generation !== client.generation || cached.retry !== retry) {
      if (!native?.available) throw new Error('Open on macOS to load the server notices.');
      cached = { origin: client.origin, generation: client.generation, retry, entries: decodeNotices(await client.readNotices(native)) };
      notices.set(client, cached);
    }
    const terms = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
    const entries = cached.entries.filter(entry => {
      const searchable = [entry.name, entry.version, entry.license, ...entry.bundles as string[]].join(' ').toLowerCase();
      return terms.every(term => searchable.includes(term));
    }).map(entry => ({ id: str(entry.id), name: str(entry.name), version: str(entry.version), license: str(entry.license), bundles: (entry.bundles as string[]).map(bundle => ({ web: 'Web', server: 'Server', desktop: 'Desktop', assets: 'Assets', mobile: 'Mobile', ios: 'iOS', android: 'Android', 'device-tools': 'Device tools' }[bundle] || bundle)).join(', '), sourceUrl: str(entry.sourceUrl), expanded: entry.id === expanded, noticeText: entry.id === expanded ? str(entry.noticeText) : '' }));
    return { ...empty, ready: true, entries, total: cached.entries.length, matched: entries.length };
  } catch (error) { return { ...empty, error: errorMessage(error) }; }
}

// timestampFormat.ts formatRelativeTimeLabel: "just now", "5m ago", "3h ago", "2d ago".
export function relativeTimeLabel(iso: string, now: number): string {
  const at = Date.parse(iso);
  if (!Number.isFinite(at)) return '';
  const diff = now - at;
  if (diff < 60000) return 'just now';
  const minutes = Math.floor(diff / 60000);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  return hours < 24 ? `${hours}h ago` : `${Math.floor(hours / 24)}d ago`;
}
// useArchivedThreadSnapshots (archivedThreadsState.ts): one archivedShellSnapshot query per visit (Atom.swr revalidates on
// mount), again on a new connection and after this client's own unarchive or delete (refreshArchivedThreadsForEnvironment;
// here settingsRefresh, which app.contract bumps when a rest command succeeds), never on a wake or the minute tick.
const archives = new WeakMap<T3Client, Map<string, { key: string; value: Obj; error: string }>>();
export async function archivedSettings(client: T3Client, native: Native | null | undefined, environmentId: string, projectId: string, active: boolean, now = 0, refresh = 0, machine = '', projectKey = '', checkout = '') {
  const empty = { available: false, loading: false, error: '', groups: [] as ArchiveGroup[] };
  if (!active) { archives.delete(client); return empty; }
  try {
    if (!native?.available) throw new Error('Open on macOS to load archived threads.');
    const legacy = !projectKey && projectId ? groupOfMember(client, projectId) : undefined;
    if (!projectKey && projectId && !legacy) throw new Error('This scope is unavailable. Choose an existing checkout.');
    const scope = resolveScope(client, machine || environmentId, legacy?.key ?? projectKey, checkout);
    if (!scope.connected || scope.kind === 'unavailable') throw new Error(scope.message || 'Choose a connected environment.');
    let cache = archives.get(client);
    if (!cache) archives.set(client, cache = new Map());
    const groups = new Map<string, ArchiveGroup & { dated: { at: string; row: ArchiveGroup['threads'][number] }[] }>();
    const failures: string[] = [], multiple = scope.selected.length > 1;
    for (const environment of scope.selected) {
      if (environment.connection.phase !== 'connected') continue;
      const label = settingsScopeEnvironmentLabel(environment, scope.environments);
      const remote = environmentClient(client, environment.environmentId, native);
      const visit = `${remote.origin}|${remote.generation}|${refresh}`;
      let kept = cache.get(environment.environmentId);
      if (!kept || kept.key !== visit) {
        try { kept = { key: visit, value: await remote.restAccess(native).read('orchestration.getArchivedShellSnapshot'), error: '' }; }
        catch (failure) { if (letGo(failure)) throw failure; kept = { key: visit, value: {}, error: errorMessage(failure) }; }
        cache.set(environment.environmentId, kept);
      }
      if (kept.error) { failures.push(`${label}: ${kept.error}`); continue; }
      const shell = applyShell(initialShell(), kept.value);
      const selectedIds = scope.members.filter(member => member.environmentId === environment.environmentId).map(member => member.id);
      for (const project of shell.projects.filter(project => scope.kind === 'all' || scope.kind === 'environment' || selectedIds.includes(project.id))) {
        const logical = scope.groups.find(group => group.memberProjects.some(member => member.environmentId === environment.environmentId && member.id === project.id));
        const id = logical?.projectKey ?? `${environment.environmentId}:${project.id}`;
        let group = groups.get(id);
        if (!group) {
          const title = logical?.displayName ?? str(project.title), identity = projectIdentity(title);
          groups.set(id, group = { id, title, mark: identity.projectMark, ink: identity.projectInk, surface: identity.projectSurface, first: false, threads: [], dated: [] });
        }
        for (const thread of shell.threads.filter(thread => thread.projectId === project.id)) {
          const at = str(thread.archivedAt || thread.createdAt), threadId = str(thread.id);
          group.dated.push({ at, row: { id: multiple ? `${environment.environmentId}:${threadId}` : threadId, scope: `${environment.environmentId}:${project.id}:${threadId}`, title: str(thread.title), first: false,
            description: `${multiple ? label + ' · ' : ''}Archived ${relativeTimeLabel(at, now)} · Created ${relativeTimeLabel(str(thread.createdAt), now)}` } });
        }
      }
    }
    return { ...empty, available: true, error: failures.join(' · '), groups: [...groups.values()].filter(group => group.dated.length).map(({ dated, ...group }, index) => ({ ...group, first: index === 0,
      threads: dated.sort((a, b) => b.at.localeCompare(a.at) || b.row.id.localeCompare(a.row.id)).map((entry, index) => ({ ...entry.row, first: index === 0 })) })) };
  } catch (error) { return { ...empty, error: errorMessage(error) }; }
}

const bytes = (value: unknown) => formatBytes(num(value));

type StorageRuleView = { mixed: boolean; key: string; label: string; description: string; enabled: boolean; days: string; retention: boolean; first: boolean; decrease: string; increase: string };
// StorageSettings.tsx: copy, order and scope rules of the reference route. Cleanup's rules, then the two under its
// Advanced fold, then Artifacts and logs.
const storageDefinitions = [
  ['worktreeOnDelete', 'Delete worktrees with deleted threads', 'Remove unused worktrees when active or archived threads are deleted. Worktrees with local changes are kept.', false],
  ['worktreeOnMerge', 'Delete merged worktrees', 'Remove worktrees whose pull request is merged and whose commits are included in the default branch.', false],
  ['worktreeAfterDays', 'Delete inactive worktrees', 'Remove worktrees after their threads have been inactive for this many days. Branches and thread history are kept.', true],
  ['worktreeSettledAfterDays', 'Delete settled worktrees', 'Remove worktrees this many days after their thread settles. Set 0 days to remove them at the next cleanup. Branches and thread history are kept.', true],
  ['worktreeUnchanged', 'Delete unchanged worktrees', 'Remove worktrees with no commits beyond the default branch.', false],
  ['browserArtifactsAfterDays', 'Delete old browser artifacts', 'Delete saved browser captures after this many days. Older capture links will no longer open.', true],
  ['logsAfterDays', 'Delete old rotated logs', 'Delete inactive rotated log files after this many days. Current logs are kept.', true],
] as const;
// CoreRowView's code sample, which no storage row draws.
const NO_PREVIEW = { path: '', status: '', additions: 0, deletions: 0, scheme: '', lines: [] };
export function lastRunSummary(entries: readonly { label: string; counts: string }[], machineCount: number): string {
  const first = entries[0];
  if (!first) return '';
  if (machineCount <= 1) return first.counts;
  if (entries.length === machineCount && entries.every(entry => entry.counts === first.counts)) return `${first.counts} on every machine`;
  return entries.map(entry => `${entry.label}: ${entry.counts}`).join(' · ');
}
/**
 * WorktreesSettingsPanel's Creation rows (Start from remote, and Submodules under Advanced) and its Branches section
 * (BranchNamingSettings), which the storage panel draws between its Worktree location and Cleanup.
 */
export async function worktreeRows(client: T3Client, native: Native, scope: ReturnType<typeof resolveScope>, location: boolean) {
  const project = scope.kind === 'project' || scope.kind === 'checkout';
  const context = serverContext(client, scope, project ? await memberFiles(client, native, scope.members, scope) : new Map());
  const [creation] = generalSections(client, context, 'worktrees').map(dividedSection);
  const representative = scopeRepresentative(scope);
  const owner = representative ? environmentClient(client, representative.environmentId, native) : null;
  const settings = obj(owner?.config.settings);
  const branches = owner ? branchNamingRows(settings, str(scope.members.find(member => member.environmentId === owner.environmentId)?.id), str(obj(owner.config.environment).label, 'Environment'), can(owner, 'settings:write'))
    .map(row => ({ ...row, layerGroups: inheritanceGroups(context, row.key, SOURCE_CONTROL_DEFAULTS) })) : [];
  return { creation: (creation?.rows ?? []).map(row => ({ ...row, divider: row.divider || location })), creationAdvanced: creation?.advanced ?? [], branches };
}
export async function storageSettings(client: T3Client, native: Native | null | undefined, environmentId: string, projectId: string, active: boolean, machine: string | undefined = undefined, projectKey = '', checkout = '', now = 0) {
  const modeLabels: Record<string, string> = { inherit: 'Inherit', off: 'Off', custom: 'Custom' };
  const empty = { writeScope: `${machine ?? environmentId}|${projectKey}|${checkout}`, cleanupAvailable: false, cleanupDisabled: true, cleanupDescription: '', databaseNotices: [] as DatabaseNotice[], directory: '', directoryMixed: false, directorySupported: false, keepSupported: false, keepValue: '', keepDescription: '', keepOptions: [] as { value: string; label: string; selected: boolean }[],
    available: false, project: projectId !== '', error: '', notice: '', mode: 'inherit', modeLabel: 'Inherit', modeDescription: '', scope: `${environmentId}:${projectId}`, modes: [] as { value: string; label: string; selected: boolean }[], worktrees: [] as StorageRuleView[], worktreesAdvanced: [] as StorageRuleView[], artifacts: [] as StorageRuleView[], cleanupSummary: '', cleanupReports: [] as { id: string; label: string; counts: string; entries: CleanupEntry[]; omitted: string; omittedFirst: boolean; empty: string; emptyFirst: boolean }[],
    creation: [] as CoreRow[], creationAdvanced: [] as CoreRow[], branches: [] as ScopedRow[], diagnostics: [] as CoreRow[], codePreview: NO_PREVIEW };
  if (!active) { artifactStatuses.delete(client); return empty; }
  // Maintenance ends with Diagnostics whatever its storage sections show.
  const diagnostics = (selected: number) => [{ ...diagnosticsRow(selected === 1), divider: false }];
  try {
    if (!native?.available) throw new Error('Open on macOS to configure storage.');
    const legacy = !projectKey && projectId ? groupOfMember(client, projectId) : undefined;
    if (projectId && !projectKey && !legacy) throw new Error('This checkout is no longer available.');
    const scope = resolveScope(client, machine === undefined ? environmentId : machine, legacy?.key ?? projectKey, checkout);
    if (scope.kind === 'unavailable' || !scope.connected) throw new Error(scope.message || 'Choose a connected environment.');
    const project = scope.kind === 'project' || scope.kind === 'checkout';
    const context = serverContext(client, scope, new Map()), representative = scopeTarget(client, context);
    if (!representative) throw new Error('Choose a connected environment.');
    const selected = liveEnvironments(client, native).filter(environment => scope.selected.some(selected => selected.environmentId === environment.environmentId) && environment.connected);
    const supports = (key: string) => selected.length > 0 && selected.every(environment => obj(obj(environment.config.environment).capabilities)[key] === true);
    const directorySupported = !project && supports('worktreesDirectory');
    const configurable = supports('storageCleanup') && (!project || supports('projectWorktreeCleanup'));
    // Start from remote is divided from Worktree location, which the page draws wherever the directory is supported.
    const page = { ...empty, ...(await worktreeRows(client, native, scope, directorySupported)), diagnostics: diagnostics(scope.selected.length) };
    if (project && !configurable) return { ...page, project, notice: 'Update the selected environments to configure storage cleanup.' };
    const settings = representative.settings;
    const policy = (target: typeof representative) => obj(obj(obj(target.settings.projectSettingsOverrides)[target.projectId ?? '']).worktreeCleanup);
    const modes = context.targets.map(target => project ? str(policy(target).mode, 'inherit') : 'inherit');
    const mode = project ? str(policy(representative).mode, 'inherit') : 'inherit';
    const mixedModes = modes.some(value => value !== mode);
    const allRules = context.targets.map(target => ({ ...obj(target.settings.storageCleanup), ...effectiveWorktreeRules(target.settings, target.projectId ? obj(obj(target.settings.projectSettingsOverrides)[target.projectId]) : {}) }));
    const rules = allRules[context.targets.indexOf(representative)] ?? allRules[0] ?? {};
    const differs = (key: string) => allRules.some(value => JSON.stringify(value[key]) !== JSON.stringify(rules[key]));
    const targetIds = context.targets.map(target => target.environmentId).sort().join(',');
    const mixed = (key: string) => key !== 'browserArtifactsAfterDays' && key !== 'logsAfterDays' ? differs(key)
      : context.targets.length > 1 && settledStatus(client, native as Native, key, `${targetIds}:${differs(key)}`) === `${targetIds}:true`;
    const view = (definitions: (typeof storageDefinitions)[number][], first: boolean) => definitions.map(([key, label, description, retention], index) => ({ key, label, description: mixed(key) ? `Mixed across selected machines. ${description}` : description, mixed: mixed(key), retention, first: first && index === 0, enabled: retention ? typeof rules[key] === 'number' : rules[key] === true, days: typeof rules[key] === 'number' ? String(rules[key]) : '8',
      decrease: String(Math.max(key === 'worktreeSettledAfterDays' ? 0 : 1, num(rules[key], 8) - 1)), increase: String(Math.min(3650, num(rules[key], 8) + 1)) }));
    const environments = liveEnvironments(client, native).filter(environment => scope.selected.some(selected => selected.environmentId === environment.environmentId)
      && environment.connected && obj(obj(environment.config.environment).capabilities).storageCleanupRun === true);
    if (!project) await Promise.all([watchStorageReports(client, native, environments.map(environment => environment.environmentId)), watchDatabaseStorage(client, native, selected.filter(environment => obj(obj(environment.config.environment).capabilities).databaseStorage === true).map(environment => environment.environmentId))]);
    const cleanupReports = project ? [] : environments.filter(environment => environment.cleanup.value?.[0]).map((environment, index) => {
      const report = environment.cleanup.value![0]!;
      const counts = obj(report.counts), selected = scope.environments.find(selected => selected.environmentId === environment.environmentId);
      const label = selected ? settingsScopeEnvironmentLabel(selected, scope.environments) : environment.label;
      return { id: environment.environmentId, label, counts: `${num(counts.removed)} removed, ${num(counts.kept)} kept${num(counts.failed) > 0 ? `, ${num(counts.failed)} failed` : ''}`,
        ...cleanupEntries(report, label, index === 0) };
    });
    const showRules = !project || mode === 'custom';
    const databaseNotices = project ? [] : databaseCompactionNotices(selected.filter(environment => obj(obj(environment.config.environment).capabilities).databaseStorage === true)
      .map(environment => ({ environmentId: environment.environmentId, label: environment.label, status: environment.database.value?.[0] })), shellPrefs(client).databaseStorageDismissals);
    const writable = environments.filter(environment => cleanupPermitted(client, environment));
    const hasRules = environments.some(environment => cleanupHasRules(obj(environment.config.settings)));
    const directoryMixed = context.targets.some(target => str(target.settings.worktreesDirectory) !== str(settings.worktreesDirectory));
    const keep = str(rules.worktreeKeepWhen, 'uncommitted-changes');
    return { ...page, available: configurable, project, notice: !configurable ? 'Update the selected environments to configure storage cleanup.' : scope.selected.some(environment => environment.connection.phase !== 'connected') ? 'Offline environments are skipped until they reconnect.' : '', writeScope: `${machine === undefined ? environmentId : machine}|${legacy?.key ?? projectKey}|${checkout}`, databaseNotices, cleanupAvailable: !project && environments.length > 0, cleanupDisabled: !hasRules || writable.length === 0, cleanupDescription: !hasRules ? 'Turn on a cleanup rule to use this.' : environments.map(environment => { const report = environment.cleanup.value?.[0]; return `${environments.length > 1 ? environment.label + ': ' : ''}${report ? `Last run ${relativeTimeLabel(str(report.finishedAt), now)}. ${bytes(report.bytesFreed)} recovered.` : environment.cleanup.error ? 'Could not load the last run.' : environment.cleanup.value === null ? 'Loading last run…' : "Cleanup hasn't run yet."}`; }).join('\n'), cleanupReports, cleanupSummary: lastRunSummary(cleanupReports, selected.length), directory: directoryMixed ? '' : str(settings.worktreesDirectory), directoryMixed, directorySupported, keepSupported: showRules && supports('storageCleanupRun'), keepValue: mixed('worktreeKeepWhen') ? '' : keep,
      keepDescription: mixed('worktreeKeepWhen') ? 'Mixed across selected machines.' : keep === 'any-local-files' ? 'Includes ignored files, except node_modules.' : keep === 'tracked-changes' ? 'Deletes untracked and ignored files, including .env.' : 'Keeps edits and untracked files. Deletes ignored files, including .env.',
      keepOptions: [['any-local-files', 'Any local files'], ['uncommitted-changes', 'Uncommitted changes'], ['tracked-changes', 'Edited tracked files']].map(([value, label]) => ({ value, label, selected: !mixed('worktreeKeepWhen') && value === keep })),
      mode, modeLabel: mixedModes ? 'Mixed' : modeLabels[mode] || 'Inherit',

      modeDescription: mode === 'off' ? "Keep this project's worktrees until you delete them manually." : mode === 'custom' ? 'Use these rules for this project.' : "Use each machine's worktree cleanup settings.",
      modes: ['inherit', 'off', 'custom'].map(value => ({ value, label: modeLabels[value], selected: value === mode })),
      // Cleanup's first rule leads its card unless the project's mode row does; the fold's first leads the fold.
      worktrees: showRules ? view(storageDefinitions.slice(0, 3), !project) : [],
      worktreesAdvanced: showRules ? view(storageDefinitions.slice(3, 5).filter(([key]) => key !== 'worktreeSettledAfterDays' || supports('settledWorktreeCleanup')), true) : [],
      artifacts: project ? [] : view(storageDefinitions.slice(5), true) };
  } catch(error) { return { ...empty, error: errorMessage(error), diagnostics: diagnostics(0) }; }
}
