// Settings → Diagnostics, the server's resource telemetry (lane settings-a).
// Reference ResourceTelemetryDiagnostics.tsx (+ .logic.ts), resourceTelemetryState.ts
// and DiagnosticsSettings.tsx's process signals. The snapshot is the
// `subscribeResourceTelemetry` stream (one subscription while the page is open);
// the timeline is `server.getResourceTelemetryHistory` (stale after 5 s); signals
// go through `server.signalProcess`, SIGKILL only after a confirmation.
import type { T3Client } from './client';
import { arr, num, obj, str, type Obj } from './domain';
import { ClientError, type Native } from './protocol';
import { relativeTimeLabel } from './settings-data';
import { pushToast } from './toast';
import { letGo } from './let-go';

// ── Formatting (ResourceTelemetryDiagnostics.tsx) ───────────────────────────
export function formatBytes(value: number): string {
  if (value < 1024) return `${Math.round(value)} B`;
  const units = ['KB', 'MB', 'GB', 'TB'];
  let next = value, index = -1;
  do { next /= 1024; index++; } while (next >= 1024 && index < units.length - 1);
  return `${next.toFixed(next >= 100 ? 0 : next >= 10 ? 1 : 2)} ${units[index]}`;
}
export const formatRate = (value: number) => `${formatBytes(value)}/s`;
export function formatCpuTime(ms: number): string {
  const seconds = ms / 1000;
  if (seconds < 60) return `${seconds.toFixed(seconds >= 10 ? 1 : 2)}s`;
  const minutes = seconds / 60;
  if (minutes < 60) return `${minutes.toFixed(minutes >= 10 ? 1 : 2)}m`;
  return `${(minutes / 60).toFixed(2)}h`;
}
export function formatMicros(value: number): string {
  if (value < 1000) return `${Math.round(value)} µs`;
  if (value < 1_000_000) return `${(value / 1000).toFixed(2)} ms`;
  return `${(value / 1_000_000).toFixed(2)} s`;
}
export function formatInterval(ms: number): string {
  if (ms < 1000) return `${Math.max(0, Math.round(ms))} ms`;
  const seconds = ms / 1000, text = Number.isInteger(seconds) ? String(seconds) : seconds.toFixed(1).replace(/\.0$/, '');
  return `${text} ${seconds === 1 ? 'second' : 'seconds'}`;
}
const CATEGORY: Record<string, string> = { server: 'Server', 'server-child': 'Backend child', 'provider-root': 'Provider', 'terminal-root': 'Terminal', 'electron-main': 'Electron main',
  'electron-renderer': 'Renderer', 'electron-gpu': 'GPU', 'electron-utility': 'Electron utility', 'resource-monitor': 'Monitor', 'unknown-t3': 'T3 process' };
export const categoryLabel = (category: string) => CATEGORY[category] ?? 'T3 process';
/** categoryDotClass: monitor warning, electron info, server violet, the rest success. */
export function categoryDot(category: string): string {
  if (category === 'resource-monitor') return 'warning';
  if (category.startsWith('electron-')) return 'info';
  return category === 'server' ? 'violet' : 'success';
}
const IO: Record<string, string> = { storage: 'Storage bytes', logical: 'Logical bytes', 'all-io': 'All I/O bytes', unavailable: 'Unavailable' };
/** Effect's Option over the wire: a bare value, null, or { _tag: 'Some' | 'None' }. */
export function option(value: unknown): unknown {
  const v = obj(value);
  if (v._tag === 'Some') return v.value;
  if (v._tag === 'None' || value === null || value === undefined) return null;
  return value;
}
export function processName(entry: Obj): string {
  if (str(entry.name).trim()) return str(entry.name);
  const first = (str(entry.command).trim().split(/\s+/)[0] ?? '').replace(/^['"]|['"]$/g, '');
  return first.split(/[\\/]/).filter(Boolean).pop() ?? first;
}

// ── The generic table the Diagnostics page draws (colgroup percentages over a minimum width) ──
export type DxCell = { id: string; text: string; tone: string; align: string; width: number; mono: boolean; tip: string; lead: boolean };
export type DxRow = { id: string; first: boolean; cells: DxCell[]; depth: number; toggle: string; dot: string; signal: boolean; busy: boolean; target: string; name: string; expand: boolean; detail: string; trace: string; traceLabel: string; level: string };
export type DxTable = { id: string; minWidth: number; headers: DxCell[]; rows: DxRow[]; empty: string };
export const dxCell = (id: string, text: string, width: number, extra: Partial<DxCell> = {}): DxCell => ({ id, text, tone: '', align: 'left', width, mono: false, tip: '', lead: false, ...extra });
export const dxRow = (id: string, cells: DxCell[], extra: Partial<DxRow> = {}): DxRow =>
  ({ id, first: false, cells, depth: 0, toggle: '', dot: '', signal: false, busy: false, target: '', name: '', expand: false, detail: '', trace: '', traceLabel: '', level: '', ...extra });
export const dxHeaders = (columns: [string, number, string?][]) => columns.map(([label, width, align]) => dxCell(label, label, width, { align: align ?? 'left' }));
/** Mark the leading column (sm:pl-5) and the first row (no divider above it). */
export function dxFinish(table: DxTable): DxTable {
  const lead = (cells: DxCell[]) => cells.map((cell, index) => index === 0 ? { ...cell, lead: true } : cell);
  return { ...table, headers: lead(table.headers), rows: table.rows.map((row, index) => ({ ...row, first: index === 0, cells: lead(row.cells) })) };
}

// ── State: the subscription, the last snapshot, the timeline window, collapsed rows, signals ──
type Telemetry = { id: string; generation: number; snapshot: Obj | null; error: string; window: string; collapsed: Set<string>; liveCollapsed: Set<string>;
  history: { key: string; at: number; value: Obj | null; error: string } | null; signaling: Set<string>; kill: { pid: number; start: number; target: string } | null };
const states = new WeakMap<T3Client, Telemetry>();
const stateOf = (client: T3Client): Telemetry => {
  let state = states.get(client);
  if (!state) states.set(client, state = { id: '', generation: -1, snapshot: null, error: '', window: '15m', collapsed: new Set(), liveCollapsed: new Set(), history: null, signaling: new Set(), kill: null });
  return state;
};
const generationOf = (client: T3Client) => num((client as unknown as { generation?: number }).generation, 0);
export const TELEMETRY_KEY = 'resource-telemetry';
/** The client's event drain hands every `resource-telemetry` entry here (client.ts). */
export function telemetryEvent(client: T3Client, entry: Obj): void {
  const state = states.get(client);
  if (!state || !state.id || str(entry.subscriptionId) !== state.id) return;
  const item = obj(entry.value);
  if (item._transportError || item._streamEnded) {
    state.error = str(obj(item._transportError).message, 'The resource monitor stream ended.');
    state.id = '';
    return;
  }
  state.snapshot = item; state.error = '';
}
/** Subscribe while the page is open; drop the stream when it closes or the connection turns over. */
export async function syncTelemetry(client: T3Client, native: Native | null | undefined, active: boolean): Promise<void> {
  const state = stateOf(client), generation = generationOf(client);
  if (!active || !native?.available || !client.ready) {
    if (state.id && native?.available) await client.restAccess(native).call({ op: 'unsubscribe', key: TELEMETRY_KEY }).catch(() => undefined);
    state.id = ''; state.history = null; state.kill = null;
    if (!active) { state.collapsed.clear(); state.liveCollapsed.clear(); }
    return;
  }
  if (state.id && state.generation === generation) return;
  if (state.generation !== generation) state.snapshot = null;
  state.generation = generation;
  try { state.id = str((await client.restAccess(native).call({ op: 'subscribe', key: TELEMETRY_KEY, method: 'subscribeResourceTelemetry', payload: {} })).id); state.error = ''; }
  catch (error) { state.id = ''; if (!letGo(error)) state.error = error instanceof Error ? error.message : 'Could not start resource telemetry.'; }
}

const WINDOWS: Record<string, [number, number]> = { '5m': [300_000, 15_000], '15m': [900_000, 30_000], '30m': [1_800_000, 60_000], '1h': [3_600_000, 120_000] };
async function history(client: T3Client, native: Native, now: number, refresh: number): Promise<Telemetry['history']> {
  const state = stateOf(client), window = WINDOWS[state.window] ?? WINDOWS['15m']!, key = `${state.window}|${refresh}`;
  // resourceTelemetryHistory: staleTimeMs 5_000.
  if (state.history && state.history.key === key && now - state.history.at < 5000) return state.history;
  try { state.history = { key, at: now, value: await client.restAccess(native).request('server.getResourceTelemetryHistory', { windowMs: window[0], bucketMs: window[1] }), error: '' }; }
  catch (error) { if (letGo(error)) throw error; state.history = { key, at: now, value: state.history?.value ?? null, error: error instanceof Error ? error.message : 'Could not load resource history.' }; }
  return state.history;
}

/** visibleResourceTelemetryProcesses: collapsing a row hides every descendant by parent PID. */
export function visibleProcesses(processes: Obj[], collapsed: Set<string>): Obj[] {
  const children = new Map<number, Obj[]>();
  for (const entry of processes) children.set(num(entry.ppid), [...(children.get(num(entry.ppid)) ?? []), entry]);
  const hidden = new Set<string>();
  const hide = (pid: number): void => { for (const child of children.get(pid) ?? []) { const key = identity(child); if (hidden.has(key)) continue; hidden.add(key); hide(num(obj(child.identity).pid)); } };
  for (const entry of processes) if (collapsed.has(identity(entry))) hide(num(obj(entry.identity).pid));
  return processes.filter(entry => !hidden.has(identity(entry)));
}
const identity = (entry: Obj) => `${num(obj(entry.identity).pid)}:${num(obj(entry.identity).startTimeMs)}`;
const canSignal = (category: string) => category === 'server-child' || category === 'provider-root' || category === 'terminal-root';
/** resourceHistoryBarHeight. */
export const barHeight = (value: number, max: number, minimum: number) => value <= 0 ? 0 : Math.max(minimum, (value / Math.max(1, max)) * 100);
const STATUS_TONE: Record<string, string> = { healthy: 'default', starting: 'warning', degraded: 'warning' };
const statusTone = (status: string) => STATUS_TONE[status] ?? 'danger';
const bool = (value: unknown, yes: string, no: string) => value === 'true' ? yes : value === 'false' ? no : 'Unknown';

export type TelemetryView = ReturnType<typeof telemetryView>;
export function telemetryView(client: T3Client, now: number, timeline: Telemetry['history']) {
  const state = stateOf(client), snapshot = state.snapshot, groups = obj(snapshot?.groups), all = obj(groups.allT3), ready = snapshot !== null;
  const health = obj(snapshot?.health), native = obj(health.native), desktop = obj(health.desktop), power = obj(snapshot?.power);
  const speed = ready ? option(snapshot!.speedLimitPercent) : null;
  const readAt = ready ? relativeTimeLabel(str(snapshot!.readAt), now) : '';
  const nativeStatus = ready ? str(native.status) : '';
  const writeRate = num(all.ioWriteBytesPerSecond);
  const stat = (id: string, icon: string, label: string, value: string, detail: string, tone = '') => ({ id, icon, label, value: ready ? value : '...', detail: ready ? detail : '', tone });
  const aggregate = (id: string, label: string, accent: string, value: Obj) => ({ id, label, accent, count: `${num(value.processCount)} ${num(value.processCount) === 1 ? 'process' : 'processes'}`,
    cpu: `${num(value.currentCpuPercent).toFixed(1)}%`, memory: formatBytes(num(value.currentRssBytes)), read: formatRate(num(value.ioReadBytesPerSecond)), write: formatRate(num(value.ioWriteBytesPerSecond)) });
  const source = (id: string, label: string, value: Obj) => {
    const error = option(value.lastError), browser = str(value.status) === 'unavailable' && typeof error === 'string' && error.includes("'web' mode");
    return { id, label, detail: browser ? 'Available when this page runs inside the desktop app.' : typeof error === 'string' && error ? error : 'No reported errors',
      badge: browser ? 'Desktop only' : str(value.status), tone: browser ? 'neutral' : statusTone(str(value.status)) };
  };
  const signals = ready && ['onBattery', 'lowPowerMode', 'idle', 'locked', 'thermalState'].some(key => str(power[key]) !== 'unknown');
  const sidecar = option(health.sidecarVersion), sidecarPid = option(health.sidecarPid);
  const value = timeline?.value ?? null, buckets = arr(value?.buckets);
  const maxCpu = Math.max(1, ...buckets.map(bucket => num(bucket.avgCpuPercent))), maxIo = Math.max(1, ...buckets.map(bucket => num(bucket.ioReadBytes) + num(bucket.ioWriteBytes)));
  const processes = arr(snapshot?.processes), visible = visibleProcesses(processes, state.collapsed);
  return {
    ready, badge: ready ? `Native ${nativeStatus}` : '', badgeTone: statusTone(nativeStatus), updated: readAt ? 'Updated' : 'Waiting for sample', updatedValue: readAt,
    sampling: `Sampling every ${ready ? formatInterval(num(snapshot!.sampleIntervalMs)) : '...'}`, error: state.error,
    stats: [
      stat('cpu', 'cpu', 'Current CPU', `${num(all.currentCpuPercent).toFixed(1)}%`, `${formatCpuTime(num(all.cpuTimeMs))} observed CPU time`),
      stat('memory', 'memory-stick', 'Resident memory', formatBytes(num(all.currentRssBytes)), `${formatBytes(num(all.peakRssBytes))} combined process peaks`),
      stat('processes', 'activity', 'Process count', String(num(all.processCount)), `${num(all.processStarts)} starts · ${num(all.processExits)} exits`),
      stat('read', 'hard-drive', 'Read throughput', formatRate(num(all.ioReadBytesPerSecond)), `${formatBytes(num(all.ioReadBytes))} observed`),
      stat('write', 'database', 'Write throughput', formatRate(writeRate), `${formatBytes(num(all.ioWriteBytes))} observed`, writeRate >= 10 * 1024 * 1024 ? 'danger' : writeRate >= 1024 * 1024 ? 'warning' : ''),
      stat('speed', 'gauge', 'CPU speed limit', typeof speed === 'number' ? `${speed.toFixed(0)}%` : 'Unknown', `${str(power.thermalState, 'unknown')} thermal state`, typeof speed === 'number' && speed < 80 ? 'warning' : ''),
    ],
    groups: ready ? [aggregate('backend', 'Backend + agents', 'success', obj(groups.backend)), aggregate('desktop', 'Desktop', 'info', obj(groups.electron)), aggregate('monitor', 'Monitor overhead', 'warning', obj(groups.monitor))] : [],
    // shouldShowResourceMonitorRetry.
    retry: (!ready && state.error !== '') || ['degraded', 'unavailable', 'stopped'].includes(nativeStatus),
    hostSignals: signals,
    host: signals ? [
      { id: 'power', label: 'Power source', value: bool(power.onBattery, 'Battery', 'External power'), tone: '' },
      { id: 'low-power', label: 'Low power mode', value: bool(power.lowPowerMode, 'Enabled', 'Disabled'), tone: '' },
      { id: 'idle', label: 'Idle', value: `${bool(power.idle, 'Idle', 'Active')}${typeof power.idleSeconds === 'number' ? ` · ${Math.round(power.idleSeconds)}s` : ''}`, tone: '' },
      { id: 'session', label: 'Session', value: power.suspended === true ? 'Suspended' : bool(power.locked, 'Locked', 'Unlocked'), tone: '' },
      { id: 'thermal', label: 'Thermal', value: str(power.thermalState, 'unknown'), tone: ['serious', 'critical'].includes(str(power.thermalState)) ? 'danger' : '' },
    ] : [],
    sources: ready ? [source('native', 'Native process monitor', native), source('desktop', 'Electron main process', desktop)] : [],
    health: ready ? [
      { id: 'collection', label: 'Collection time', value: formatMicros(num(health.collectionDurationMicros)), tone: '' },
      { id: 'scan', label: 'Process scan', value: `${num(health.retainedProcessCount)}/${num(health.scannedProcessCount)} retained`, tone: '' },
      { id: 'inaccessible', label: 'Inaccessible', value: String(num(health.inaccessibleProcessCount)), tone: num(health.inaccessibleProcessCount) > 0 ? 'warning' : '' },
      { id: 'sidecar', label: 'Sidecar', value: typeof sidecar === 'string' && sidecar ? `${sidecar}${typeof sidecarPid === 'number' ? ` · PID ${sidecarPid}` : ''}` : 'Unavailable', tone: '' },
      { id: 'restarts', label: 'Restarts', value: String(num(health.restartCount)), tone: '' },
    ] : [],
    window: state.window, historyError: timeline?.error ?? '',
    buckets: buckets.map((bucket, index) => ({ id: str(bucket.startedAt, String(index)), cpu: barHeight(num(bucket.avgCpuPercent), maxCpu, 2), read: barHeight(num(bucket.ioReadBytes), maxIo, 1),
      write: barHeight(num(bucket.ioWriteBytes), maxIo, 1),
      tip: `CPU avg ${num(bucket.avgCpuPercent).toFixed(1)}%\nCPU peak ${num(bucket.maxCpuPercent).toFixed(1)}%\nRead ${formatBytes(num(bucket.ioReadBytes))}\nWrite ${formatBytes(num(bucket.ioWriteBytes))}` })),
    history: dxFinish({
      id: 'timeline', minWidth: 1020, empty: 'No retained process samples in this window.',
      headers: dxHeaders([['Process', 24], ['Category', 11], ['CPU Time', 10, 'right'], ['Peak CPU', 10, 'right'], ['Peak Mem', 11, 'right'], ['Read', 11, 'right'], ['Write', 11, 'right'], ['Samples', 7, 'right'], ['PID', 5, 'right']]),
      rows: arr(value?.topProcesses).map(entry => dxRow(identity(entry), [
        dxCell('process', str(entry.name) || str(entry.command), 24, { tip: str(entry.command) || str(entry.name) }), dxCell('category', categoryLabel(str(entry.category)), 11, { tone: 'muted' }),
        dxCell('cpu-time', formatCpuTime(num(entry.cpuTimeMs)), 10, { mono: true, align: 'right' }), dxCell('peak-cpu', `${num(entry.maxCpuPercent).toFixed(1)}%`, 10, { mono: true, align: 'right' }),
        dxCell('peak-mem', formatBytes(num(entry.peakRssBytes)), 11, { mono: true, align: 'right' }), dxCell('read', formatBytes(num(entry.ioReadBytes)), 11, { mono: true, align: 'right', tone: 'info' }),
        dxCell('write', formatBytes(num(entry.ioWriteBytes)), 11, { mono: true, align: 'right', tone: 'warning' }), dxCell('samples', String(num(entry.sampleCount)), 7, { mono: true, align: 'right', tone: 'muted' }),
        dxCell('pid', String(num(obj(entry.identity).pid)), 5, { mono: true, align: 'right', tone: 'muted' })])),
    }),
    tree: dxFinish({
      id: 'tree', minWidth: 1320, empty: 'Waiting for the native process monitor.',
      headers: dxHeaders([['Process', 20], ['Category', 10], ['CPU', 7, 'right'], ['CPU Time', 8, 'right'], ['Memory', 9, 'right'], ['Read/s', 9, 'right'], ['Write/s', 9, 'right'], ['Read Total', 10, 'right'], ['Write Total', 8, 'right'], ['PID', 6, 'right'], ['Kill', 4, 'right']]),
      rows: visible.map(entry => {
        const key = identity(entry), name = processName(entry), category = str(entry.category), children = Array.isArray(entry.childPids) && entry.childPids.length > 0;
        return dxRow(key, [
          dxCell('process', name, 20, { tip: str(entry.command) || str(entry.name) }), dxCell('category', categoryLabel(category), 10, { tone: 'muted' }),
          dxCell('cpu', `${num(entry.cpuPercent).toFixed(1)}%`, 7, { mono: true, align: 'right' }), dxCell('cpu-time', formatCpuTime(num(entry.cpuTimeMs)), 8, { mono: true, align: 'right' }),
          dxCell('memory', formatBytes(num(entry.residentBytes)), 9, { mono: true, align: 'right' }), dxCell('read-rate', formatRate(num(entry.ioReadBytesPerSecond)), 9, { mono: true, align: 'right', tone: 'info' }),
          dxCell('write-rate', formatRate(num(entry.ioWriteBytesPerSecond)), 9, { mono: true, align: 'right', tone: 'warning' }), dxCell('read-total', formatBytes(num(entry.ioReadBytes)), 10, { mono: true, align: 'right', tone: 'muted' }),
          dxCell('write-total', formatBytes(num(entry.ioWriteBytes)), 8, { mono: true, align: 'right', tone: 'muted', tip: IO[str(entry.ioSemantics)] ?? 'Unavailable' }),
          dxCell('pid', String(num(obj(entry.identity).pid)), 6, { mono: true, align: 'right', tone: 'muted' }), dxCell('kill', '', 4, { align: 'right' })],
        { depth: Math.min(num(entry.depth), 7), toggle: children ? (state.collapsed.has(key) ? 'expand' : 'collapse') : '', dot: categoryDot(category), name,
          signal: canSignal(category), busy: state.signaling.has(key), target: `source=tree&pid=${num(obj(entry.identity).pid)}&start=${num(obj(entry.identity).startTimeMs)}` });
      }),
    }),
    attribution: dxFinish({
      id: 'attribution', minWidth: 720, empty: 'No instrumented application I/O has been recorded yet.',
      headers: dxHeaders([['Component', 22], ['Operation', 28], ['Logical Read', 14, 'right'], ['Logical Write', 14, 'right'], ['Count', 10, 'right'], ['Time', 12, 'right']]),
      rows: arr(obj(snapshot?.attribution).entries).map(entry => dxRow(`${str(entry.component)}:${str(entry.operation)}`, [
        dxCell('component', str(entry.component), 22, { tone: 'strong' }), dxCell('operation', str(entry.operation), 28, { tone: 'muted' }),
        dxCell('read', formatBytes(num(entry.logicalReadBytes)), 14, { mono: true, align: 'right', tone: 'info' }), dxCell('write', formatBytes(num(entry.logicalWriteBytes)), 14, { mono: true, align: 'right', tone: 'warning' }),
        dxCell('count', String(num(entry.count)), 10, { mono: true, align: 'right' }), dxCell('time', `${(num(entry.durationMs) / 1000).toFixed(2)}s`, 12, { mono: true, align: 'right', tone: 'muted' })])),
    }),
  };
}

/** The Diagnostics page's telemetry: subscribe, read the timeline, project the view. */
export async function telemetryPage(client: T3Client, native: Native | null | undefined, active: boolean, now: number, refresh: number) {
  await syncTelemetry(client, native, active);
  const timeline = active && native?.available && client.ready ? await history(client, native, now, refresh) : null;
  return telemetryView(client, now, timeline);
}

// ── Commands ───────────────────────────────────────────────────────────────
/** The SIGKILL confirmation the root dialog shows (ensureLocalApi().dialogs.confirm). */
export function killConfirmation(client: T3Client): { pid: string; message: string } {
  const kill = states.get(client)?.kill;
  return kill ? { pid: String(kill.pid), message: `Send SIGKILL to process ${kill.pid}? This cannot be handled by the process.` } : { pid: '', message: '' };
}
export function isStaleSignal(message: string): boolean { return message.includes('not a live descendant'); }

/** restlocal:diag-window | diag-tree | diag-live-tree — view state only. */
export function telemetryLocal(client: T3Client, op: string, value: string): string {
  const state = stateOf(client);
  if (op === 'diag-window') { if (!WINDOWS[value]) throw new ClientError('Unsupported resource history period.'); state.window = value; return ''; }
  const set = op === 'diag-tree' ? state.collapsed : op === 'diag-live-tree' ? state.liveCollapsed : null;
  if (!set) throw new ClientError(`Unknown settings action: ${op}`);
  if (set.has(value)) set.delete(value); else set.add(value);
  return '';
}
export const liveCollapsed = (client: T3Client) => stateOf(client).liveCollapsed;

/** rest:diag-signal (pid, start, signal, source), rest:diag-kill / diag-kill-cancel, rest:diag-retry. */
export async function telemetryCommand(client: T3Client, native: Native, op: string, input: Record<string, string>): Promise<string> {
  const state = stateOf(client), access = client.restAccess(native);
  if (op === 'diag-retry') {
    try { const result = await access.request('server.retryResourceTelemetry', {}, true); if (result.snapshot) state.snapshot = obj(result.snapshot); state.error = ''; }
    catch (error) { if (letGo(error)) throw error; pushToast(client, { kind: 'error', title: 'Could not restart resource monitor', description: error instanceof Error ? error.message : 'The resource monitor retry failed.' }); }
    return '';
  }
  if (op === 'diag-kill-cancel') { state.kill = null; return ''; }
  const kill = op === 'diag-kill';
  const pid = kill ? state.kill?.pid ?? 0 : Number(input.pid), start = kill ? state.kill?.start ?? 0 : Number(input.start);
  const signal = kill ? 'SIGKILL' : input.signal === 'SIGKILL' ? 'SIGKILL' : input.signal === 'SIGINT' ? 'SIGINT' : '';
  if (op !== 'diag-signal' && !kill) throw new ClientError(`Unknown settings action: ${op}`);
  if (!signal || !Number.isInteger(pid) || pid <= 0 || !Number.isFinite(start) || start < 0) throw new ClientError('That process is no longer available.');
  if (!kill && signal === 'SIGKILL') { state.kill = { pid, start, target: str(input.source) }; return ''; }
  state.kill = null;
  const key = `${pid}:${start}`;
  if (state.signaling.has(key)) return '';
  state.signaling.add(key);
  try {
    const result = await access.request('server.signalProcess', { pid, startTimeMs: start, signal }, true);
    if (result.signaled !== true) {
      const message = option(result.message);
      pushToast(client, { kind: 'error', title: `Could not send ${signal}`, description: typeof message === 'string' && message ? message : `Failed to send ${signal} to process ${pid}.` });
    }
  } catch (error) {
    if (letGo(error)) throw error;
    pushToast(client, { kind: 'error', title: `Could not send ${signal}`, description: error instanceof Error ? error.message : `Failed to send ${signal}.` });
  } finally { state.signaling.delete(key); }
  return '';
}
