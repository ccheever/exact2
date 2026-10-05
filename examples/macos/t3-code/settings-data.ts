import { T3Client } from './client';
import { effectiveWorktreeRules, arr, obj, str, num, applyShell, initialShell, type Obj } from './domain';
import type { Native } from './protocol';
import { projectIdentity } from './presentation';

const errorMessage = (error: unknown) => error instanceof Error ? error.message : 'Could not load this section.';
const windows = [{ id: '5m', windowMs: 300000, bucketMs: 15000 }, { id: '15m', windowMs: 900000, bucketMs: 30000 }, { id: '30m', windowMs: 1800000, bucketMs: 60000 }, { id: '1h', windowMs: 3600000, bucketMs: 120000 }];
type NoticeRow = { id: string; name: string; version: string; license: string; bundles: string; sourceUrl: string; expanded: boolean; noticeText: string };
type ArchiveGroup = { id: string; title: string; mark: string; ink: string; surface: string; threads: { id: string; scope: string; title: string; description: string; first: boolean }[] };
type DiagnosticSection = { id: string; title: string; error: string; readAt: string; rows: ReturnType<typeof row>[] };
const notices = new WeakMap<T3Client, { origin: string; generation: number; retry: number; entries: Obj[] }>();

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
export async function archivedSettings(client: T3Client, native: Native | null | undefined, environmentId: string, projectId: string, active: boolean, now = 0) {
  const empty = { available: false, loading: false, error: '', groups: [] as ArchiveGroup[] };
  if (!active) return empty;
  try {
    if (!native?.available || !client.ready || (environmentId !== "" && environmentId !== client.environmentId) || (projectId && !client.shell.projects.some(project => project.id === projectId))) throw new Error('This scope is unavailable. Choose a connected environment and an existing checkout.');
    const shell = applyShell(initialShell(), await client.readSettings(native, 'orchestration.getArchivedShellSnapshot'));
    // ArchivedThreadsPanel: archivedAt ?? createdAt, newest first, then id.
    const key = (thread: Obj) => str(thread.archivedAt || thread.createdAt);
    const groups = shell.projects.filter(project => !projectId || project.id === projectId).map(project => ({ id: str(project.id), title: str(project.title), mark: projectIdentity(str(project.title)).projectMark, ink: projectIdentity(str(project.title)).projectInk, surface: projectIdentity(str(project.title)).projectSurface, threads: shell.threads.filter(thread => thread.projectId === project.id).sort((a, b) => key(b).localeCompare(key(a)) || str(b.id).localeCompare(str(a.id))).map((thread, index) => ({ id: str(thread.id), scope: `${client.environmentId}:${project.id}:${thread.id}`, title: str(thread.title), first: index === 0,
      description: `Archived ${relativeTimeLabel(key(thread), now)} · Created ${relativeTimeLabel(str(thread.createdAt), now)}` })) })).filter(group => group.threads.length);
    return { ...empty, available: true, groups };
  } catch (error) { return { ...empty, error: errorMessage(error) }; }
}

const bytes = (value: unknown) => { const n = num(value); return n < 1024 ? `${n} B` : n < 1048576 ? `${(n / 1024).toFixed(1)} KB` : `${(n / 1048576).toFixed(1)} MB`; };
const duration = (value: unknown) => num(value) < 1000 ? `${Math.round(num(value))} ms` : `${(num(value) / 1000).toFixed(2)} s`;
const row = (id: string, label: string, value: string, detail = '', copy = '') => ({ id, label, value, detail, copy });

export async function diagnosticsSettings(client: T3Client, native: Native | null | undefined, environmentId: string, period: string, active: boolean) {
  const empty = { available: false, error: '', sections: [] as DiagnosticSection[] };
  if (!active) return empty;
  if (!native?.available || !client.ready || (environmentId !== "" && environmentId !== client.environmentId)) return { ...empty, error: 'Choose one connected environment to view diagnostics.' };
  const window = windows.find(window => window.id === period);
  if (!window) return { ...empty, error: 'Unsupported resource history period.' };
  const section = async (id: string, title: string, method: string, payload: Obj, project: (value: Obj) => ReturnType<typeof row>[]) => {
    try {
      const value = await client.readSettings(native, method, payload);
      const error = obj(value.error);
      return { id, title, error: str(error.message || obj(error.value).message), readAt: str(value.readAt), rows: project(value) };
    } catch (error) { return { id, title, error: errorMessage(error), readAt: '', rows: [] }; }
  };
  // Independent read-only diagnostics; partial errors stay with their section.
  const sections = await Promise.all([
    section('processes', 'Live Processes', 'server.getProcessDiagnostics', {}, value => [row('count', 'Child Processes', `${num(value.processCount)}`), row('cpu', 'CPU', `${num(value.totalCpuPercent).toFixed(1)}%`, 'Total CPU across live child processes of the current server; parent/desktop processes are excluded.'), row('memory', 'Memory', bytes(value.totalRssBytes), 'Total resident memory across live child processes of the current server.'), ...arr(value.processes).map(process => row(`process-${process.pid}`, `${process.pid} · ${process.command}`, `${num(process.cpuPercent).toFixed(1)}% · ${bytes(process.rssBytes)} · ${process.elapsed}`, `Parent ${process.ppid} · ${process.status}`))]),
    section('history', 'Resource History', 'server.getProcessResourceHistory', { windowMs: window.windowMs, bucketMs: window.bucketMs }, value => [row('samples', 'Retained samples', `${num(value.retainedSampleCount)}`), row('cpu-seconds', 'Observed CPU time', `${num(value.totalCpuSecondsApprox).toFixed(2)} s`), ...arr(value.buckets).map((bucket, index) => row(`bucket-${index}`, str(bucket.startedAt), `CPU average ${num(bucket.avgCpuPercent).toFixed(1)}% · peak ${num(bucket.maxCpuPercent).toFixed(1)}% · memory ${bytes(bucket.maxRssBytes)}`)), ...arr(value.topProcesses).map(process => row(str(process.processKey), `${process.pid} · ${process.command}`, `${num(process.avgCpuPercent).toFixed(1)}% average · ${bytes(process.maxRssBytes)} peak memory`, `Samples ${process.sampleCount} · ${process.firstSeenAt} — ${process.lastSeenAt}`))]),
    section('traces', 'Trace Diagnostics', 'server.getTraceDiagnostics', {}, value => [row('records', 'Trace records', `${num(value.recordCount)}`), row('failures', 'Failures', `${num(value.failureCount)}`), row('parse', 'Parse errors', `${num(value.parseErrorCount)}`), row('interruptions', 'Interruptions', `${num(value.interruptionCount)}`), row('slow', 'Slow spans', `${num(value.slowSpanCount)}`, `Threshold ${duration(value.slowSpanThresholdMs)}`), row('path', 'Trace file', str(value.traceFilePath)), ...arr(value.latestFailures).map((span, index) => row(`failure-${index}`, `Latest Failures · ${span.name}`, duration(span.durationMs), str(span.cause), str(span.traceId))), ...arr(value.commonFailures).map((span, index) => row(`common-${index}`, `Most Common Failures · ${span.name}`, `${span.count}`, str(span.cause), str(span.traceId))), ...arr(value.slowestSpans).map((span, index) => row(`slow-${index}`, `Slowest Spans · ${span.name}`, duration(span.durationMs), str(span.endedAt), str(span.traceId))), ...arr(value.latestWarningAndErrorLogs).map((span, index) => row(`log-${index}`, `Span Logs · ${span.spanName}`, str(span.level), str(span.message), str(span.traceId))), ...arr(value.topSpansByCount).map((span, index) => row(`top-${index}`, `Top Span Names · ${span.name}`, `${span.count} calls · ${span.failureCount} failures · ${duration(span.averageDurationMs)} avg · ${duration(span.maxDurationMs)} max`))]),
  ]);
  return { ...empty, available: true, sections };
}

type StorageRuleView = { key: string; label: string; description: string; enabled: boolean; days: string; retention: boolean; first: boolean; decrease: string; increase: string };
// StorageSettings.tsx: copy, order and scope rules of the reference route.
const storageDefinitions = [
  ['worktreeOnDelete', 'Delete worktrees with deleted threads', 'Remove unused worktrees when active or archived threads are deleted. Worktrees with local changes are kept.', false],
  ['worktreeAfterDays', 'Delete inactive worktrees', 'Remove worktrees after their threads have been inactive for this many days. Branches and thread history are kept.', true],
  ['worktreeOnMerge', 'Delete merged worktrees', 'Remove worktrees whose pull request is merged and whose commits are included in the default branch.', false],
  ['worktreeUnchanged', 'Delete unchanged worktrees', 'Remove worktrees with no commits beyond the default branch.', false],
  ['browserArtifactsAfterDays', 'Delete old browser artifacts', 'Delete saved browser captures after this many days. Older capture links will no longer open.', true],
  ['logsAfterDays', 'Delete old rotated logs', 'Delete inactive rotated log files after this many days. Current logs are kept.', true],
] as const;
export async function storageSettings(client: T3Client, native: Native | null | undefined, environmentId: string, projectId: string, active: boolean) {
  const modeLabels: Record<string, string> = { inherit: 'Inherit', off: 'Off', custom: 'Custom' };
  const empty = { available: false, project: projectId !== '', error: '', notice: '', mode: 'inherit', modeLabel: 'Inherit', modeDescription: '', scope: `${environmentId}:${projectId}`, modes: [] as { value: string; label: string; selected: boolean }[], worktrees: [] as StorageRuleView[], artifacts: [] as StorageRuleView[] };
  if (!active) return empty;
  try {
    if (!native?.available || !client.ready || (environmentId !== "" && environmentId !== client.environmentId) || (projectId && !client.shell.projects.some(project => project.id === projectId))) throw new Error('This scope is unavailable. Choose a connected environment and existing checkout.');
    const config = await client.readSettings(native, 'server.getConfig');
    const capabilities = obj(obj(config.environment).capabilities);
    if (projectId && capabilities.projectWorktreeCleanup !== true) return { ...empty, notice: 'Update the selected machines to configure project worktree cleanup.' };
    if (capabilities.storageCleanup !== true) return { ...empty, notice: 'Update the selected environments to use storage cleanup, or choose a machine that supports it.' };
    const settings = await client.readSettings(native, 'server.getSettings');
    const override = obj(obj(settings.projectSettingsOverrides)[projectId]);
    const mode = projectId ? str(obj(override.worktreeCleanup).mode, 'inherit') : 'inherit';
    const rules = { ...obj(settings.storageCleanup), ...effectiveWorktreeRules(settings, override) };
    const view = (definitions: (typeof storageDefinitions)[number][], first: boolean) => definitions.map(([key, label, description, retention], index) => ({ key, label, description, retention, first: first && index === 0, enabled: retention ? typeof rules[key] === 'number' : rules[key] === true, days: typeof rules[key] === 'number' ? String(rules[key]) : '8',
      decrease: String(Math.max(1, num(rules[key], 8) - 1)), increase: String(Math.min(3650, num(rules[key], 8) + 1)) }));
    const showRules = !projectId || mode === 'custom';
    return { ...empty, available: true, mode, modeLabel: modeLabels[mode] || 'Inherit',
      modeDescription: mode === 'off' ? "Keep this project's worktrees until you delete them manually." : mode === 'custom' ? 'Use these rules for this project.' : "Use each machine's worktree cleanup settings.",
      modes: ['inherit', 'off', 'custom'].map(value => ({ value, label: modeLabels[value], selected: value === mode })),
      worktrees: showRules ? view(storageDefinitions.slice(0, 4), !projectId) : [],
      artifacts: projectId ? [] : view(storageDefinitions.slice(4), true) };
  } catch(error) { return { ...empty, error: errorMessage(error) }; }
}
