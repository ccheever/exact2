// settings-diagnostics-and-scope. S2-2: Settings › Diagnostics fills again. The page stayed empty: its answer was
// asked on data.revision, every drained event bumped that revision, and the resource telemetry stream the page opens
// sends a sample at least once a second, so Exact replaced the answer (LLP 1016 D5) before its reads came back, and the
// next answer sent them again. The reference re-renders only ResourceTelemetryDiagnostics on a sample and reads Live
// Processes, Resource History and Trace Diagnostics once per visit and on Refresh (DiagnosticsSettings.tsx:708-760;
// client-runtime state/server.ts: no refreshIntervalMs). These tests go through the app's own answer(), with a small
// runner of the two resources: `data` is asked when a topic it watches changed, `diagnostics` when its arguments change,
// and a replaced answer's pending native calls reject as Exact's do (let-go.ts).
import { afterEach, describe, expect, test } from 'bun:test';
import { answer } from './app';
import { Backend, storage } from './client-fixture';
import { obj, str, type Obj } from './domain';
import type { Native } from './protocol';
import { resetPrimary } from './local-primary-fixture';
import { resetHighlightSlicing } from './r12-render-highlight';
import { TELEMETRY_KEY } from './settings-a-telemetry';

const READ_AT = '2026-10-09T08:00:00.000Z';
const REPLIES: Record<string, Obj> = {
  'server.getProcessDiagnostics': { readAt: READ_AT, processCount: 1, totalCpuPercent: 0.5, totalRssBytes: 2048, serverPid: 42,
    processes: [{ pid: 43, ppid: 42, depth: 0, command: '/usr/bin/codex app-server', childPids: [], cpuPercent: 0.5, rssBytes: 2048, startTimeMs: 5, elapsed: '00:10', status: 'S' }] },
  'server.getProcessResourceHistory': { readAt: READ_AT, totalCpuSecondsApprox: 1, retainedSampleCount: 3, sampleIntervalMs: 1000, buckets: [], topProcesses: [] },
  'server.getTraceDiagnostics': { readAt: READ_AT, recordCount: 1787, failureCount: 207, slowSpanCount: 0, parseErrorCount: 0, slowSpanThresholdMs: 1000,
    latestFailures: [], commonFailures: [], slowestSpans: [], latestWarningAndErrorLogs: [], topSpansByCount: [{ name: 'ws.rpc.server.getConfig', count: 2, failureCount: 0, averageDurationMs: 1, maxDurationMs: 2 }] },
  'server.getResourceTelemetryHistory': { buckets: [], topProcesses: [] },
};
const READS = Object.keys(REPLIES);
const sample = (n: number): Obj => ({ readAt: READ_AT, sampleIntervalMs: 1000, processes: [], attribution: { entries: [] }, power: {},
  groups: { allT3: { processCount: n, currentCpuPercent: n }, backend: {}, electron: {}, monitor: {} }, health: { native: { status: 'healthy' }, desktop: { status: 'healthy' } } });

/**
 * The lane server behind the fake transport: subscribeResourceTelemetry sends the current sample first, as the
 * server's stream does; a diagnostics read replies `latency` turns after the transport sent it; a shared read
 * (T3Transport `share`) joins an identical one still pending and sends nothing.
 */
class DiagnosticsServer extends Backend {
  woke = false;
  turn = 0;
  latency = 0;
  samples = 0;
  sent: string[] = [];
  private inFlight: { method: string; key: string; at: number; resolve: () => void; promise: Promise<unknown> }[] = [];
  override emit(key: string, value: Obj, subscriptionId = this.subscriptions[key]) { super.emit(key, value, subscriptionId); this.woke = true; }
  override async later(input: unknown): Promise<unknown> {
    const request = obj(input), method = str(request.method);
    if (request.op === 'subscribe' && request.key === TELEMETRY_KEY) {
      this.calls.push(request);
      const id = `sub-${++this.serial}`;
      this.subscriptions[TELEMETRY_KEY] = id;
      this.emit(TELEMETRY_KEY, sample(++this.samples));
      return this.good({ id });
    }
    if (request.op === 'request' && READS.includes(method)) {
      this.calls.push(request);
      const key = request.share === true ? `${method}\n${JSON.stringify(request.payload ?? {})}` : '';
      const joined = key ? this.inFlight.find(entry => entry.key === key) : undefined;
      if (joined) return joined.promise;
      this.sent.push(method);
      let resolve = () => {};
      const promise = new Promise<void>(done => { resolve = done; }).then(() => this.good(REPLIES[method]));
      this.inFlight.push({ method, key, at: this.turn, resolve, promise });
      this.reply();
      return promise;
    }
    return super.later(input);
  }
  /** Replies every read sent `latency` turns ago or earlier. */
  reply() {
    const due = this.inFlight.filter(entry => this.turn - entry.at >= this.latency);
    this.inFlight = this.inFlight.filter(entry => !due.includes(entry));
    for (const entry of due) entry.resolve();
  }
  tick() { this.turn++; this.reply(); }
  /** The resource monitor's next sample on the open stream. */
  next() { this.emit(TELEMETRY_KEY, sample(++this.samples)); }
}

/** One answer's native: once Exact lets the answer go, its pending calls reject as the runtime's do (FetchError Aborted). */
function owned(native: Native) {
  let gone = false;
  const waiting = new Set<(error: unknown) => void>();
  const aborted = () => Object.assign(new Error('the answer was let go before this reply'), { name: 'FetchError', kind: 'Aborted' });
  return {
    native: { available: true, watch: (topic: string) => native.watch(topic), later: (request: unknown) => gone ? Promise.reject(aborted()) : new Promise((resolve, reject) => {
      waiting.add(reject);
      native.later(request).then(value => { waiting.delete(reject); if (!gone) resolve(value); }, error => { waiting.delete(reject); if (!gone) reject(error); });
    }) } as Native,
    letGo() { gone = true; for (const reject of waiting) reject(aborted()); waiting.clear(); },
  };
}

const settle = () => new Promise(resolve => setTimeout(resolve, 0));
type Page = { available: boolean; processStats: { value: string }[]; traceStats: { value: string }[]; telemetry: { stats: { value: string }[] } };

/**
 * app.contract's two resources: `data` (snapshot) is asked when a topic it watches changed (the server woke the
 * transport); `diagnostics` is asked when its arguments change (`data.revision`, `data.telemetry`), and a new
 * answer replaces the one in flight. Each turn: the server may send a sample, then replies what is due.
 */
async function run(server: DiagnosticsServer, files: ReturnType<typeof storage>['files'], options: { turns: number; sampleEachTurn: boolean }) {
  const state = { asks: 0, letGo: 0, page: null as Page | null, start: 0, revisions: [] as number[], telemetry: 0 };
  let data = obj(await answer('snapshot', [], null, files, server));
  state.start = Number(data.revision); state.telemetry = Number(data.telemetry);
  let asked = '', flight: ReturnType<typeof owned> | null = null;
  const ask = () => {
    const args = ['env1', '15m', true, 0, 0, data.revision, data.telemetry], key = JSON.stringify(args);
    if (key === asked) return;
    if (flight) { flight.letGo(); state.letGo++; }
    asked = key; state.asks++;
    const mine = flight = owned(server);
    answer('diagnosticsSettings', args, null, files, mine.native).then(page => { if (flight === mine) { state.page = page as Page; flight = null; } }, () => {});
  };
  for (let turn = 0; turn < options.turns; turn++) {
    server.woke = false;
    ask();
    await settle();
    if (options.sampleEachTurn) server.next();
    server.tick();
    await settle();
    if (server.woke) { data = obj(await answer('snapshot', [], null, files, server)); state.revisions.push(Number(data.revision)); state.telemetry = Number(data.telemetry); }
    else if (!flight) return { ...state, idle: true };
  }
  return { ...state, idle: false };
}

let generation = 71; // above the other app tests' servers (settings-integrations-reads.test.ts: 59): the app's client bootstraps from each new one
async function launched(latency = 0) {
  const server = new DiagnosticsServer(), files = storage().files;
  server.generation = ++generation; server.serial = generation * 1000; server.latency = latency;
  for (let turn = 0; turn < 8; turn++) { server.woke = false; await answer('snapshot', [], null, files, server); if (!server.woke) break; }
  return { server, files };
}

afterEach(async () => {
  resetPrimary(); resetHighlightSlicing();
  await answer('diagnosticsSettings', ['env1', '15m', false, 0, 0, 0, 0], null, storage().files, null); // the page closes: its reads and stream are dropped
});

describe('Settings › Diagnostics asks again only when the reference does', () => {
  test('a telemetry sample asks only Diagnostics again: data.revision stays, data.telemetry moves, and the page reads nothing', async () => {
    const { server, files } = await launched();
    const opened = await run(server, files, { turns: 8, sampleEachTurn: false });
    // Opening: one ask, then one more for the stream's first sample; each read once; then idle. The `data` answer
    // that drained the first sample kept data.revision, so no other data.revision reader was asked.
    expect({ asks: opened.asks, idle: opened.idle, sent: server.sent.sort(), revisions: opened.revisions, telemetry: opened.telemetry > 0 })
      .toEqual({ asks: 2, idle: true, sent: [...READS].sort(), revisions: [opened.start], telemetry: true });
    expect(opened.page).toMatchObject({ available: true });
    expect(opened.page!.traceStats[0]).toMatchObject({ value: '1,787' });
    // Three samples: Diagnostics alone is asked (the run's first ask, then one per sample drained before its last
    // turn), no read is sent, and data.revision never moves; the next ask shows the latest sample.
    const live = await run(server, files, { turns: 3, sampleEachTurn: true });
    expect({ asks: live.asks, sent: server.sent.length, revisions: live.revisions, telemetry: live.telemetry - opened.telemetry })
      .toEqual({ asks: 3, sent: READS.length, revisions: [live.start, live.start, live.start], telemetry: 3 });
    const end = await run(server, files, { turns: 2, sampleEachTurn: false });
    expect({ idle: end.idle, sent: server.sent.length, processes: end.page!.telemetry.stats[2]!.value }).toEqual({ idle: true, sent: READS.length, processes: String(server.samples) });
  });

  test('a read the next answer joins lands although every answer is replaced before its reply', async () => {
    // A sample every turn and a reply two turns after the read was sent: every answer is let go before its reply.
    const { server, files } = await launched(2);
    const result = await run(server, files, { turns: 12, sampleEachTurn: true });
    expect(result.letGo).toBeGreaterThanOrEqual(2);
    expect(result.page).toMatchObject({ available: true });
    expect(result.page!.processStats.map(stat => stat.value)).toEqual(['1', '0.5%', '2.00 KB', '42']);
    expect(result.page!.traceStats[0]).toMatchObject({ value: '1,787' });
    // One request per read: an answer asked again joined the pending read instead of sending it again.
    expect(server.sent.sort()).toEqual([...READS].sort());
  });
});

describe('Project settings opens on every environment (PG-8)', () => {
  test("a bare project id is the project's group with no machine or checkout, as /projects/$projectKey redirects", async () => {
    // The palette, the sidebar, the thread menu and the details card name the project by its id (settingsProjectId);
    // routes/projects.$projectKey.tsx redirects to /settings/projects?project=<key> with machine undefined.
    const { server, files } = await launched();
    const core = (args: unknown[]) => answer('settingsCore', args, null, files, server).then(obj);
    const scope = (value: Obj) => [value.kind, value.projectLabel, value.connective, value.environmentLabel, value.scopeKey];
    const fromLink = await core(['', '', '', 'p1', 'projects', '', true]);
    expect(scope(fromLink)).toEqual(['project', 'Example', 'across', 'All environments', '|env1:/repo|']);
    expect(scope(fromLink)).toEqual(scope(await core(['', 'env1:/repo', '', '', 'projects', '', true]))); // the project chosen in the scope menu
    expect(scope(await core(['', 'env1:/repo', 'p1', '', 'projects', '', true]))).toEqual(['checkout', 'Example', 'on', 'This environment', '|env1:/repo|p1']); // a checkout stays one
    expect(await answer('projectsView', ['', '', 'p1', true], null, files, server)).toMatchObject({ selected: true, key: 'env1:/repo', name: 'Example', hasOther: false });
  });
});
