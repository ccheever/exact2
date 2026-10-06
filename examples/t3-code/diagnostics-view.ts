// Settings → Diagnostics view model (reference DiagnosticsSettings.tsx): the
// resource telemetry sections (settings-a-telemetry.ts), then the server's live
// processes, resource history and trace diagnostics as stat grids and tables.
// Each read is fetched once per visit and again on its Refresh, as the
// reference's queries are; signals and the logs folder are commands.
import { arr, obj, str, num, type Obj } from './domain';
import type { T3Client } from './client';
import { ClientError, type Native } from './protocol';
import { relativeTimeLabel } from './settings-data';
import { dxCell, dxFinish, dxHeaders, dxRow, liveCollapsed, option, telemetryPage, type DxTable } from './settings-a-telemetry';
import { preferredEditor } from './shell-details';
import { letGo } from './let-go';

// RESOURCE_HISTORY_WINDOWS: the process history buckets per period.
const WINDOWS: Record<string, [number, number]> = { '5m': [300000, 30000], '15m': [900000, 60000], '30m': [1800000, 120000], '1h': [3600000, 300000] };
type Stat = { id: string; label: string; value: string; tone: string; tip: string };

export const count = (value: unknown) => Math.round(num(value)).toLocaleString('en-US');
export function duration(value: unknown): string { const v = num(value); return v < 1000 ? `${Math.round(v)} ms` : `${(v / 1000).toFixed(v >= 10000 ? 1 : 2)} s`; }
export function bytes(value: unknown): string {
  let next = num(value);
  if (next < 1024) return `${next} B`;
  const units = ['KB', 'MB', 'GB']; let index = -1;
  do { next /= 1024; index++; } while (next >= 1024 && index < units.length - 1);
  return `${next.toFixed(next >= 10 ? 1 : 2)} ${units[index]}`;
}
/** DiagnosticsSettings formatCpuTime (seconds). */
export function cpuTime(value: unknown): string {
  const seconds = num(value);
  if (seconds < 60) return `${seconds.toFixed(seconds >= 10 ? 1 : 2)}s`;
  const minutes = seconds / 60;
  if (minutes < 60) return `${minutes.toFixed(minutes >= 10 ? 1 : 2)}m`;
  return `${(minutes / 60).toFixed(2)}h`;
}
export function shortTrace(id: string): string { return id.length <= 32 ? id : `${id.slice(0, 18)}...${id.slice(-10)}`; }
export function processName(command: string): string {
  const first = command.trim().split(/\s+/)[0]; if (!first) return command;
  const segments = first.replace(/^['"]|['"]$/g, '').split(/[\\/]/).filter(Boolean);
  return segments[segments.length - 1] || first;
}
const shortName = (command: string) => { const name = processName(command); return name.length > 42 ? `${name.slice(0, 39)}...` : name; };
function processType(entry: Obj): string {
  if (num(entry.depth) > 0) return 'Subprocess';
  if (/\b(codex|claude|opencode|cursor)\b/i.test(str(entry.command))) return 'Agent';
  return 'Process';
}
/** ProcessDiagnosticsTable: a collapsed row hides the deeper rows that follow it. */
export function visibleLive(processes: Obj[], collapsed: Set<string>): Obj[] {
  const out: Obj[] = []; let hidden: number | null = null;
  for (const entry of processes) {
    if (hidden !== null) { if (num(entry.depth) > hidden) continue; hidden = null; }
    out.push(entry);
    if (collapsed.has(String(num(entry.pid)))) hidden = num(entry.depth);
  }
  return out;
}
/** ExpandableText: clamp text longer than 180 characters or with a line break. */
const expandable = (text: string) => text.length > 180 || text.includes('\n');

// One fetch per visit and Refresh: the key is the environment, period and refresh count.
type Reads = { key: string; processes: { value: Obj; error: string }; history: { value: Obj; error: string }; traces: { value: Obj; error: string } };
const cache = new WeakMap<T3Client, Reads>();

export async function diagnosticsPage(client: T3Client, native: Native | null | undefined, environmentId: string, period: string, active: boolean, now = 0, refresh = 0) {
  const blankTable: DxTable = { id: '', minWidth: 0, headers: [], rows: [], empty: '' };
  const telemetry = await telemetryPage(client, native, active && !!native?.available && client.ready && (environmentId === '' || environmentId === client.environmentId), now, refresh);
  const empty = { available: false, error: '', period, checkedProcesses: '', checkedHistory: '', checkedTraces: '', checkedProcessesValue: '', checkedHistoryValue: '', checkedTracesValue: '',
    canOpenLogs: false, processStats: [] as Stat[], processErrors: [] as string[], processes: blankTable,
    historyStats: [] as Stat[], historyErrors: [] as string[], history: blankTable, buckets: [] as { id: string; avg: number; peak: number; label: string }[],
    traceStats: [] as Stat[], traceErrors: [] as string[], latestFailures: blankTable, commonFailures: blankTable, slowest: blankTable, logs: blankTable, topSpans: blankTable, telemetry };
  if (!active) { cache.delete(client); return empty; }
  if (!native?.available || !client.ready || (environmentId !== "" && environmentId !== client.environmentId)) return { ...empty, error: 'Choose one connected environment to view diagnostics.' };
  const window = WINDOWS[period];
  if (!window) return { ...empty, error: 'Unsupported resource history period.' };
  const access = client.restAccess(native);
  const read = async (method: string, payload: Obj) => { try { return { value: await access.request(method, payload), error: '' }; } catch (error) { if (letGo(error)) throw error; return { value: {} as Obj, error: error instanceof Error ? error.message : 'Could not load diagnostics.' }; } };
  const key = `${client.environmentId}|${period}|${refresh}`;
  let reads = cache.get(client);
  if (!reads || reads.key !== key) {
    const [processes, history, traces] = await Promise.all([read('server.getProcessDiagnostics', {}), read('server.getProcessResourceHistory', { windowMs: window[0], bucketMs: window[1] }), read('server.getTraceDiagnostics', {})]);
    reads = { key, processes, history, traces };
    cache.set(client, reads);
  }
  const { processes, history, traces } = reads;
  const checked = (value: Obj) => str(value.readAt) ? relativeTimeLabel(str(value.readAt), now) : '';
  const checkedLabel = (value: Obj) => str(value.readAt) ? (/ago$/.test(checked(value)) ? 'Checked' : `Checked ${checked(value)}`) : 'Checking';
  const checkedValue = (value: Obj) => /ago$/.test(checked(value)) ? checked(value).replace(/ ago$/, '') : '';
  const p = processes.value, h = history.value, t = traces.value;
  const errorOf = (value: Obj) => str(obj(option(value.error)).message);
  const ready = (value: Obj, fallback: string) => Object.keys(value).length ? fallback : '...';
  const traceError = errorOf(t), partial = option(t.partialFailure) === true;
  const collapsed = liveCollapsed(client);
  const live = visibleLive(arr(p.processes), collapsed);
  // ProcessResourceHistoryTable: children indent from the shallowest child depth.
  const top = arr(h.topProcesses), shallowest = top.filter(entry => entry.isServerRoot !== true).reduce<number | null>((min, entry) => min === null ? num(entry.depth) : Math.min(min, num(entry.depth)), null);
  const observability = obj(client.config.observability);
  return { ...empty, available: true,
    checkedProcesses: checkedLabel(p), checkedHistory: checkedLabel(h), checkedTraces: checkedLabel(t), checkedProcessesValue: checkedValue(p), checkedHistoryValue: checkedValue(h), checkedTracesValue: checkedValue(t),
    canOpenLogs: str(observability.logsDirectoryPath) !== '',
    processStats: [
      { id: 'count', label: 'Child Processes', value: ready(p, count(p.processCount)), tone: '', tip: '' },
      { id: 'cpu', label: 'CPU', value: ready(p, `${num(p.totalCpuPercent).toFixed(1)}%`), tone: '', tip: 'Total CPU across live child processes of the current server process. The desktop shell and other parent processes are not included.' },
      { id: 'memory', label: 'Memory', value: ready(p, bytes(p.totalRssBytes)), tone: '', tip: 'Total resident memory across live child processes of the current server process. The desktop shell and other parent processes are not included.' },
      { id: 'pid', label: 'Server PID', value: ready(p, String(num(p.serverPid))), tone: '', tip: '' }],
    processErrors: [errorOf(p), processes.error].filter(Boolean),
    processes: dxFinish({ id: 'processes', minWidth: 1040, empty: Object.keys(p).length ? 'No live descendant processes found.' : 'Loading live processes...',
      headers: dxHeaders([['Name', 24], ['CPU', 8, 'right'], ['Memory', 10, 'right'], ['Command', 33], ['PID', 8, 'right'], ['Type', 11], ['Kill', 6, 'right']]),
      rows: live.map(entry => {
        const pid = String(num(entry.pid)), children = Array.isArray(entry.childPids) && entry.childPids.length > 0;
        return dxRow(pid, [dxCell('name', processName(str(entry.command)), 24, { tip: str(entry.command) }), dxCell('cpu', `${num(entry.cpuPercent).toFixed(1)}%`, 8, { mono: true, align: 'right' }),
          dxCell('memory', bytes(entry.rssBytes), 10, { mono: true, align: 'right' }), dxCell('command', str(entry.command), 33, { tone: 'muted', tip: str(entry.command) }),
          dxCell('pid', pid, 8, { mono: true, align: 'right', tone: 'muted' }), dxCell('type', processType(entry), 11, { tone: 'muted' }), dxCell('kill', '', 6, { align: 'right' })],
        { depth: Math.min(num(entry.depth), 6), toggle: children ? (collapsed.has(pid) ? 'expand' : 'collapse') : '', dot: 'success', name: processName(str(entry.command)), signal: true,
          target: `source=live&pid=${pid}&start=${num(entry.startTimeMs)}` });
      }) }),
    historyStats: [
      { id: 'cpu-time', label: 'CPU Time', value: ready(h, cpuTime(h.totalCpuSecondsApprox)), tone: '', tip: 'Approximate active CPU time for the T3 server root process and its descendants during the selected window. It grows only while sampled processes use CPU and older samples leave as the window moves.' },
      { id: 'samples', label: 'Samples', value: ready(h, count(h.retainedSampleCount)), tone: '', tip: 'In-memory process samples retained by the server. This resets when the server restarts.' },
      { id: 'interval', label: 'Interval', value: ready(h, duration(h.sampleIntervalMs)), tone: '', tip: '' },
      { id: 'processes', label: 'Processes', value: ready(h, count(top.length)), tone: '', tip: '' }],
    historyErrors: [errorOf(h), history.error].filter(Boolean),
    buckets: arr(h.buckets).map((bucket, index) => {
      const max = Math.max(1, ...arr(h.buckets).map(entry => num(entry.maxCpuPercent)));
      return { id: `${index}`, avg: Math.max(2, (num(bucket.avgCpuPercent) / max) * 100), peak: Math.max(2, (num(bucket.maxCpuPercent) / max) * 100),
        label: `Avg ${num(bucket.avgCpuPercent).toFixed(1)}%, peak ${num(bucket.maxCpuPercent).toFixed(1)}%` };
    }),
    history: dxFinish({ id: 'history', minWidth: 980, empty: Object.keys(h).length ? 'No process resource samples found for this window.' : 'Collecting process resource samples...',
      headers: dxHeaders([['Process', 24], ['CPU Time', 10, 'right'], ['Current', 10, 'right'], ['Average', 10, 'right'], ['Peak', 10, 'right'], ['Max Mem', 10, 'right'], ['Command', 16], ['PID', 10, 'right']]),
      rows: top.map(entry => dxRow(str(entry.processKey, `${num(entry.pid)}`), [
        dxCell('process', shortName(str(entry.command)), 24, { tip: str(entry.command) }), dxCell('cpu-time', cpuTime(entry.cpuSecondsApprox ?? entry.totalCpuSeconds), 10, { mono: true, align: 'right' }),
        dxCell('current', `${num(entry.currentCpuPercent).toFixed(1)}%`, 10, { mono: true, align: 'right' }), dxCell('average', `${num(entry.avgCpuPercent).toFixed(1)}%`, 10, { mono: true, align: 'right' }),
        dxCell('peak', `${num(entry.maxCpuPercent).toFixed(1)}%`, 10, { mono: true, align: 'right' }), dxCell('max-mem', bytes(entry.maxRssBytes), 10, { mono: true, align: 'right' }),
        dxCell('command', str(entry.command), 16, { tone: 'muted', tip: str(entry.command) }), dxCell('pid', String(num(entry.pid)), 10, { mono: true, align: 'right', tone: 'muted' })],
        { depth: entry.isServerRoot === true || shallowest === null ? 0 : Math.min(6, Math.max(1, num(entry.depth) - shallowest + 1)), toggle: 'none', dot: entry.isServerRoot === true ? 'warning-root' : 'success',
          name: `${entry.isServerRoot === true ? 'Root' : 'Child'} process ${shortName(str(entry.command))}` })) }),
    traceStats: [
      { id: 'spans', label: 'Spans', value: ready(t, count(t.recordCount)), tone: '', tip: '' },
      { id: 'failures', label: 'Failures', value: ready(t, count(t.failureCount)), tone: num(t.failureCount) > 0 ? 'danger' : '', tip: '' },
      { id: 'slow', label: 'Slow Spans', value: ready(t, count(t.slowSpanCount)), tone: num(t.slowSpanCount) > 0 ? 'warning' : '', tip: Object.keys(t).length ? `Spans with a duration of ${duration(t.slowSpanThresholdMs)} or longer.` : 'Spans at or above the configured slow-span threshold.' },
      { id: 'parse', label: 'Parse Errors', value: ready(t, count(t.parseErrorCount)), tone: num(t.parseErrorCount) > 0 ? 'warning' : '', tip: '' }],
    traceErrors: [traceError ? (partial ? `Some trace files could not be read, so diagnostics may be incomplete. ${traceError}` : traceError) : '', traces.error].filter(Boolean),
    latestFailures: dxFinish({ id: 'latest-failures', minWidth: 640, empty: Object.keys(t).length ? 'No failed spans found.' : 'Loading failures...',
      headers: dxHeaders([['Span', 33], ['Cause', 45], ['Duration', 12], ['Ended', 10]]),
      rows: arr(t.latestFailures).map((entry, index) => dxRow(`${index}`, [dxCell('span', str(entry.name), 33, { tone: 'strong' }), dxCell('cause', str(entry.cause), 45, { tone: 'muted' }),
        dxCell('duration', duration(entry.durationMs), 12, { mono: true }), dxCell('ended', relativeTimeLabel(str(entry.endedAt), now) || 'No trace records', 10, { mono: true, tone: 'muted' })],
      { expand: expandable(str(entry.cause)), detail: 'Show full error' })) }),
    commonFailures: dxFinish({ id: 'common-failures', minWidth: 760, empty: Object.keys(t).length ? 'No repeated failures found.' : 'Loading failure groups...',
      headers: dxHeaders([['Span', 34], ['Count', 10], ['Cause', 45], ['Last Seen', 11]]),
      rows: arr(t.commonFailures).map((entry, index) => dxRow(`${index}`, [dxCell('span', str(entry.name), 34, { tone: 'strong' }), dxCell('count', count(entry.count), 10, { mono: true }),
        dxCell('cause', str(entry.cause), 45, { tone: 'muted' }), dxCell('last', relativeTimeLabel(str(entry.lastSeenAt), now) || 'No trace records', 11, { mono: true, tone: 'muted' })],
      { expand: expandable(str(entry.cause)), detail: 'Show full error' })) }),
    slowest: dxFinish({ id: 'slowest', minWidth: 900, empty: Object.keys(t).length ? 'No spans found.' : 'Loading slow spans...',
      headers: dxHeaders([['Span', 44], ['Duration', 14], ['Ended', 12], ['Trace', 30]]),
      rows: arr(t.slowestSpans).map((entry, index) => dxRow(`${index}`, [dxCell('span', str(entry.name), 44, { tone: 'strong' }), dxCell('duration', duration(entry.durationMs), 14, { mono: true }),
        dxCell('ended', relativeTimeLabel(str(entry.endedAt), now) || 'No trace records', 12, { mono: true, tone: 'muted' }), dxCell('trace', '', 30)],
      { trace: str(entry.traceId), traceLabel: shortTrace(str(entry.traceId)) })) }),
    logs: dxFinish({ id: 'logs', minWidth: 920, empty: Object.keys(t).length ? 'No warnings or errors found.' : 'Loading recent logs...',
      headers: dxHeaders([['Time', 11], ['Level', 9], ['Span', 24], ['Message', 26], ['Trace', 30]]),
      rows: arr(t.latestWarningAndErrorLogs).map((entry, index) => dxRow(`${index}`, [dxCell('time', relativeTimeLabel(str(entry.seenAt ?? entry.timestamp ?? entry.at), now) || 'No trace records', 11, { mono: true, tone: 'muted' }),
        dxCell('level', str(entry.level), 9), dxCell('span', str(entry.spanName), 24, { tone: 'strong' }), dxCell('message', str(entry.message), 26, { tone: 'muted' }), dxCell('trace', '', 30)],
      { trace: str(entry.traceId), traceLabel: shortTrace(str(entry.traceId)), level: str(entry.level), expand: expandable(str(entry.message)), detail: 'Show full message' })) }),
    topSpans: dxFinish({ id: 'top-spans', minWidth: 760, empty: Object.keys(t).length ? 'No spans found.' : 'Loading span names...',
      headers: dxHeaders([['Span', 48], ['Count', 13], ['Failures', 13], ['Average', 13], ['Max', 13]]),
      rows: arr(t.topSpansByCount).map((entry, index) => dxRow(`${index}`, [dxCell('span', str(entry.name), 48, { tone: 'strong' }), dxCell('count', count(entry.count), 13, { mono: true }),
        dxCell('failures', count(entry.failureCount), 13, { mono: true }), dxCell('average', duration(entry.averageDurationMs), 13, { mono: true }), dxCell('max', duration(entry.maxDurationMs), 13, { mono: true })])) }),
  };
}

/** rest:diag-open-logs — the server opens its logs folder in the preferred editor (openLogsDirectory). */
export async function openLogsFolder(client: T3Client, native: Native): Promise<string> {
  const access = client.restAccess(native);
  const config = await access.request('server.getConfig');
  const path = str(obj(config.observability).logsDirectoryPath);
  if (!path) throw new ClientError('The logs folder is not available for this environment.');
  const available = (Array.isArray(config.availableEditors) ? config.availableEditors : []).filter((editor): editor is string => typeof editor === 'string');
  const editor = preferredEditor(available, '');
  if (!editor) throw new ClientError('No available editors found.');
  await access.request('shell.openInEditor', { cwd: path, editor }, true);
  return '';
}
