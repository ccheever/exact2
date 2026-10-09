// Lane settings-a: Diagnostics' resource telemetry, process signals and tables.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { Obj } from './domain';
import type { Native } from './protocol';
import { formatBytes, formatCpuTime, formatInterval, killConfirmation, syncTelemetry, telemetryCommand, telemetryEvent, telemetryLocal, telemetryPage, visibleProcesses } from './settings-a-telemetry';
import { diagnosticsPage, visibleLive } from './diagnostics-view';
import { toasts } from './toast';

const aggregate = (extra: Obj = {}) => ({ processCount: 0, currentCpuPercent: 0, cpuTimeMs: 0, currentRssBytes: 0, peakRssBytes: 0, ioReadBytes: 0, ioWriteBytes: 0, ioReadBytesPerSecond: 0, ioWriteBytesPerSecond: 0, processStarts: 0, processExits: 0, ...extra });
const proc = (pid: number, ppid: number, depth: number, category: string, childPids: number[] = []) => ({ identity: { pid, startTimeMs: pid * 10 }, ppid, childPids, depth, name: `p${pid}`, command: `/bin/p${pid} --flag`,
  status: 'running', category, cpuPercent: 1.25, cpuTimeMs: 1500, residentBytes: 2048, peakResidentBytes: 4096, virtualBytes: 0, ioReadBytes: 0, ioWriteBytes: 512, ioReadBytesPerSecond: 0, ioWriteBytesPerSecond: 0, ioSemantics: 'storage', runTimeMs: 0, firstSeenAt: '', lastSeenAt: '' });
const snapshot = (extra: Obj = {}) => ({
  readAt: '2026-10-04T00:00:00.000Z', sampleIntervalMs: 5000, speedLimitPercent: { _tag: 'None' },
  processes: [proc(10, 1, 0, 'server', [11]), proc(11, 10, 1, 'provider-root', [12]), proc(12, 11, 2, 'server-child')],
  groups: { backend: aggregate({ processCount: 1 }), electron: aggregate(), monitor: aggregate(), allT3: aggregate({ processCount: 3, currentCpuPercent: 3.75, cpuTimeMs: 4500, currentRssBytes: 6144, ioWriteBytesPerSecond: 2 * 1024 * 1024 }) },
  power: { onBattery: 'unknown', lowPowerMode: 'unknown', idle: 'unknown', idleSeconds: null, locked: 'unknown', suspended: false, thermalState: 'unknown', stale: false, updatedAt: '' },
  attribution: { readAt: '', entries: [{ component: 'sqlite', operation: 'write', logicalReadBytes: 0, logicalWriteBytes: 2048, count: 3, durationMs: 1500 }] },
  health: { native: { status: 'unavailable', lastSampleAt: { _tag: 'None' }, lastError: { _tag: 'Some', value: 'Resource monitor binary was not found for darwin/arm64.' } },
    desktop: { status: 'unavailable', lastSampleAt: { _tag: 'None' }, lastError: { _tag: 'Some', value: "Desktop telemetry is unavailable in 'web' mode." } },
    sidecarVersion: { _tag: 'None' }, sidecarPid: { _tag: 'None' }, restartCount: 5, collectionDurationMicros: 0, scannedProcessCount: 0, retainedProcessCount: 0, inaccessibleProcessCount: 0 },
  ...extra,
});

type Fake = { ready: boolean; environmentId: string; generation: number; config: Obj; calls: Obj[]; requests: [string, Obj][]; replies: Record<string, Obj | Error> };
function fake(): Fake {
  return { ready: true, environmentId: 'env1', generation: 1, config: { observability: { logsDirectoryPath: '/logs' } }, calls: [], requests: [], replies: {} };
}
const as = (client: Fake) => {
  const target = client as unknown as T3Client & Fake;
  Object.assign(target, { restAccess: () => ({
    call: async (request: Obj) => { client.calls.push(request); return request.op === 'subscribe' ? { id: '7' } : {}; },
    request: async (method: string, payload: Obj) => { client.requests.push([method, payload]); const reply = client.replies[method]; if (reply instanceof Error) throw reply; return reply ?? {}; },
    read: async (method: string, payload: Obj) => { client.requests.push([method, payload]); const reply = client.replies[method]; if (reply instanceof Error) throw reply; return reply ?? {}; },
  }) });
  return target as T3Client;
};
const native = { available: true } as unknown as Native;
const NOW = Date.parse('2026-10-04T00:48:00.000Z');

describe('resource telemetry', () => {
  test('formatting follows ResourceTelemetryDiagnostics', () => {
    expect([formatBytes(0), formatBytes(2048), formatBytes(150 * 1024 * 1024)]).toEqual(['0 B', '2.00 KB', '150 MB']);
    expect([formatCpuTime(0), formatCpuTime(15000), formatCpuTime(120000)]).toEqual(['0.00s', '15.0s', '2.00m']);
    expect([formatInterval(5000), formatInterval(1000), formatInterval(500)]).toEqual(['5 seconds', '1 second', '500 ms']);
  });
  test('one subscription while the page is open; its snapshot projects every section', async () => {
    const client = fake(), t3 = as(client);
    client.replies['server.getResourceTelemetryHistory'] = { buckets: [{ startedAt: 'a', avgCpuPercent: 2, maxCpuPercent: 4, ioReadBytes: 0, ioWriteBytes: 100 }], topProcesses: [] };
    let view = await telemetryPage(t3, native, true, NOW, 0);
    expect(client.calls).toEqual([{ op: 'subscribe', key: 'resource-telemetry', method: 'subscribeResourceTelemetry', payload: {} }]);
    expect(view).toMatchObject({ ready: false, updated: 'Waiting for sample', sampling: 'Sampling every ...', retry: false, groups: [], sources: [] });
    expect(view.stats.map(stat => [stat.label, stat.value])).toEqual([['Current CPU', '...'], ['Resident memory', '...'], ['Process count', '...'], ['Read throughput', '...'], ['Write throughput', '...'], ['CPU speed limit', '...']]);
    telemetryEvent(t3, { subscriptionId: '6', value: snapshot() });
    expect((await telemetryPage(t3, native, true, NOW, 0)).ready).toBe(false);
    telemetryEvent(t3, { subscriptionId: '7', value: snapshot() });
    view = await telemetryPage(t3, native, true, NOW, 0);
    expect(view).toMatchObject({ ready: true, badge: 'Native unavailable', badgeTone: 'danger', updated: 'Updated', updatedValue: '48m ago', sampling: 'Sampling every 5 seconds', retry: true, hostSignals: false });
    expect(view.stats.map(stat => [stat.value, stat.detail, stat.tone])).toEqual([['3.8%', '4.50s observed CPU time', ''], ['6.00 KB', '0 B combined process peaks', ''], ['3', '0 starts · 0 exits', ''],
      ['0 B/s', '0 B observed', ''], ['2.00 MB/s', '0 B observed', 'warning'], ['Unknown', 'unknown thermal state', '']]);
    expect(view.groups.map(group => [group.label, group.count])).toEqual([['Backend + agents', '1 process'], ['Desktop', '0 processes'], ['Monitor overhead', '0 processes']]);
    expect(view.sources).toEqual([{ id: 'native', label: 'Native process monitor', detail: 'Resource monitor binary was not found for darwin/arm64.', badge: 'unavailable', tone: 'danger' },
      { id: 'desktop', label: 'Electron main process', detail: 'Available when this page runs inside the desktop app.', badge: 'Desktop only', tone: 'neutral' }]);
    expect(view.health.map(row => `${row.label}=${row.value}`)).toEqual(['Collection time=0 µs', 'Process scan=0/0 retained', 'Inaccessible=0', 'Sidecar=Unavailable', 'Restarts=5']);
    expect(view.buckets).toEqual([{ id: 'a', cpu: 100, read: 0, write: 100, tip: 'CPU avg 2.0%\nCPU peak 4.0%\nRead 0 B\nWrite 100 B' }]);
    expect(view.tree.rows.map(row => [row.cells[0]!.text, row.toggle, row.dot, row.signal, row.depth])).toEqual([['p10', 'collapse', 'violet', false, 0], ['p11', 'collapse', 'success', true, 1], ['p12', '', 'success', true, 2]]);
    expect(view.tree.headers.map(head => head.text).at(-1)).toBe('Kill');
    expect(view.attribution.rows[0]!.cells.map(cell => cell.text)).toEqual(['sqlite', 'write', '0 B', '2.00 KB', '3', '1.50s']);
    // Closing the page drops the stream.
    await syncTelemetry(t3, native, false);
    expect(client.calls.at(-1)).toEqual({ op: 'unsubscribe', key: 'resource-telemetry' });
  });
  test('collapsing hides descendants; the timeline window and history are cached for five seconds', async () => {
    const processes = snapshot().processes as Obj[];
    expect(visibleProcesses(processes, new Set(['11:110'])).map(entry => (entry.identity as Obj).pid)).toEqual([10, 11]);
    expect(visibleProcesses(processes, new Set(['10:100'])).map(entry => (entry.identity as Obj).pid)).toEqual([10]);
    const client = fake(), t3 = as(client);
    await telemetryPage(t3, native, true, NOW, 0);
    await telemetryPage(t3, native, true, NOW + 1000, 0);
    expect(client.requests.filter(([method]) => method === 'server.getResourceTelemetryHistory').length).toBe(1);
    telemetryLocal(t3, 'diag-window', '1h');
    await telemetryPage(t3, native, true, NOW + 2000, 0);
    expect(client.requests.at(-1)).toEqual(['server.getResourceTelemetryHistory', { windowMs: 3_600_000, bucketMs: 120_000 }]);
    expect(() => telemetryLocal(t3, 'diag-window', '2h')).toThrow('Unsupported resource history period.');
  });
  test('SIGINT goes straight to the server; SIGKILL asks first; a refused signal is a toast', async () => {
    const client = fake(), t3 = as(client);
    client.replies['server.signalProcess'] = { signaled: true };
    await telemetryCommand(t3, native, 'diag-signal', { pid: '11', start: '110', signal: 'SIGINT', source: 'tree' });
    expect(client.requests.at(-1)).toEqual(['server.signalProcess', { pid: 11, startTimeMs: 110, signal: 'SIGINT' }]);
    await telemetryCommand(t3, native, 'diag-signal', { pid: '12', start: '120', signal: 'SIGKILL', source: 'tree' });
    expect(killConfirmation(t3)).toEqual({ pid: '12', message: 'Send SIGKILL to process 12? This cannot be handled by the process.' });
    expect(client.requests.length).toBe(1);
    await telemetryCommand(t3, native, 'diag-kill-cancel', {});
    expect(killConfirmation(t3).pid).toBe('');
    await telemetryCommand(t3, native, 'diag-signal', { pid: '12', start: '120', signal: 'SIGKILL', source: 'live' });
    client.replies['server.signalProcess'] = { signaled: false, message: { _tag: 'Some', value: 'Process 12 is not a live descendant.' } };
    await telemetryCommand(t3, native, 'diag-kill', {});
    expect(client.requests.at(-1)).toEqual(['server.signalProcess', { pid: 12, startTimeMs: 120, signal: 'SIGKILL' }]);
    expect(toasts(t3).at(-1)).toMatchObject({ kind: 'error', title: 'Could not send SIGKILL', description: 'Process 12 is not a live descendant.' });
    await expect(telemetryCommand(t3, native, 'diag-signal', { pid: '0', start: '1', signal: 'SIGINT' })).rejects.toThrow('That process is no longer available.');
  });
  test('Retry monitor asks the server to restart the collector and keeps its snapshot', async () => {
    const client = fake(), t3 = as(client);
    client.replies['server.retryResourceTelemetry'] = { accepted: true, snapshot: snapshot({ sampleIntervalMs: 1000 }) };
    await telemetryPage(t3, native, true, NOW, 0);
    await telemetryCommand(t3, native, 'diag-retry', {});
    expect((await telemetryPage(t3, native, true, NOW, 0)).sampling).toBe('Sampling every 1 second');
    client.replies['server.retryResourceTelemetry'] = new Error('monitor offline');
    await telemetryCommand(t3, native, 'diag-retry', {});
    expect(toasts(t3).at(-1)).toMatchObject({ title: 'Could not restart resource monitor', description: 'monitor offline' });
  });
});

describe('diagnostics tables', () => {
  test('live processes carry a Kill column and fold under a collapsed parent', async () => {
    const live = [{ pid: 20, depth: 0, command: '/usr/bin/codex app-server', childPids: [21], cpuPercent: 0, rssBytes: 0, startTimeMs: 5 }, { pid: 21, depth: 1, command: 'node child.js', childPids: [], cpuPercent: 0, rssBytes: 0, startTimeMs: 6 }];
    expect(visibleLive(live, new Set(['20'])).map(entry => entry.pid)).toEqual([20]);
    const client = fake(), t3 = as(client);
    client.replies['server.getProcessDiagnostics'] = { readAt: '2026-10-04T00:47:00.000Z', processCount: 2, totalCpuPercent: 0, totalRssBytes: 0, serverPid: 1, processes: live };
    client.replies['server.getTraceDiagnostics'] = { readAt: '2026-10-04T00:48:00.000Z', recordCount: 3, failureCount: 1, slowSpanCount: 0, parseErrorCount: 0, slowSpanThresholdMs: 1000,
      latestFailures: [{ name: 'shell.resolve', cause: 'x'.repeat(200), durationMs: 0, endedAt: '2026-10-04T00:48:00.000Z', traceId: 't' }], commonFailures: [], slowestSpans: [], latestWarningAndErrorLogs: [], topSpansByCount: [] };
    const page = await diagnosticsPage(t3, native, '', '15m', true, NOW, 0);
    expect(page.processes.headers.map(head => head.text)).toEqual(['Name', 'CPU', 'Memory', 'Command', 'PID', 'Type', 'Kill']);
    expect(page.processes.rows.map(row => [row.cells[0]!.text, row.toggle, row.signal, row.target])).toEqual([['codex', 'collapse', true, 'source=live&pid=20&start=5'], ['node', '', true, 'source=live&pid=21&start=6']]);
    expect([page.checkedProcesses, page.checkedProcessesValue, page.checkedTraces, page.checkedTracesValue]).toEqual(['Checked', '1m', 'Checked just now', '']);
    expect(page.latestFailures.rows[0]).toMatchObject({ expand: true, detail: 'Show full error' });
    expect(page.canOpenLogs).toBe(true);
    // One read per visit and refresh.
    const reads = client.requests.length;
    await diagnosticsPage(t3, native, '', '15m', true, NOW + 1000, 0);
    expect(client.requests.filter(([method]) => method !== 'server.getResourceTelemetryHistory').length).toBe(reads - 1);
    await diagnosticsPage(t3, native, '', '15m', true, NOW + 1000, 1);
    expect(client.requests.filter(([method]) => method === 'server.getTraceDiagnostics').length).toBe(2);
    expect(client.requests.find(([method]) => method === 'server.getProcessResourceHistory')![1]).toEqual({ windowMs: 900000, bucketMs: 60000 });
  });
});
