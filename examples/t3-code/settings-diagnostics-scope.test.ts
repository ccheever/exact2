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
import { arr, obj, str, type Obj } from './domain';
import { SCHEDULED_TASKS_KEY } from './live-streams';
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
  environmentId = 'env1';
  turn = 0;
  latency = 0;
  samples = 0;
  sent: string[] = [];
  private inFlight: { method: string; key: string; at: number; resolve: () => void; promise: Promise<unknown> }[] = [];
  override emit(key: string, value: Obj, subscriptionId = this.subscriptions[key]) { super.emit(key, value, subscriptionId); this.woke = true; }
  override status() { return { ...super.status(), environmentId: this.environmentId }; }
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

// The app's client adopts only a newer generation (client.ts adoptStatus), and the app tests share it in file order:
// above providers-scope.test.ts (41), which runs before this file, and below settings-integrations-reads.test.ts (60), after it.
let generation = 49;
async function launched(latency = 0, setup: (server: DiagnosticsServer) => void = () => {}) {
  const server = new DiagnosticsServer(), files = storage().files;
  server.generation = ++generation; server.serial = generation * 1000; server.latency = latency;
  setup(server);
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

  // A project with two checkouts (one repository, so one group in the default "repository" grouping): from a bare id the
  // pages read and write the whole project, as when "work" is chosen in the scope menu; a checkout chosen there stays one.
  // Its own environment: the app's client loads an environment's shell once (client.ts synchronize), so the next file's
  // server (env1) loads its own shell again.
  const KEY = 'github.com/lane/work', ENV = 'env-work';
  const identity = { canonicalKey: KEY, rootPath: '/work', displayName: 'lane/work' };
  const twoCheckouts = (server: DiagnosticsServer) => {
    server.environmentId = ENV;
    server.shell = { ...server.shell, projects: [{ id: 'p1', title: 'work', workspaceRoot: '/work', repositoryIdentity: identity },
      { id: 'p2', title: 'work', workspaceRoot: '/work-review', repositoryIdentity: identity }], threads: [...arr(server.shell.threads), { ...arr(server.shell.threads)[0], id: 't3', projectId: 'p2' }] };
    server.details.t3 = server.details.t1!;
    server.config = { ...server.config, environment: { ...obj(server.config.environment), environmentId: ENV, label: 'Lane', capabilities: { ...obj(obj(server.config.environment).capabilities), projectSettingsOverrides: true } } };
  };

  test("Scheduled Tasks from a bare project id lists every checkout's tasks (taskScope)", async () => {
    const { server, files } = await launched(0, twoCheckouts);
    // scheduledSettings: environmentId, projectId, editor, editingId, active, now, …, machine, projectKey, checkout (app.ts).
    const tasks = async (projectId: string, projectKey: string, checkout: string) => {
      const page = obj(await answer('scheduledSettings', [ENV, projectId, '', '', true, Date.parse(READ_AT), 0, 0, '', projectKey, checkout], null, files, server));
      return arr(page.sections).flatMap(section => arr(section.tasks)).map(task => str(task.id));
    };
    await tasks('p1', '', ''); // opens the scheduled-tasks stream
    const task = (id: string, projectId: string) => ({ id, title: id, prompt: 'Triage', enabled: true, schedule: { type: 'interval', everyMs: 60_000 }, projectId,
      nextRunAt: READ_AT, lastRunAt: null, lastRunStatus: 'never', lastRunError: null, runCount: 0 });
    server.emit(SCHEDULED_TASKS_KEY, { tasks: [task('in-p1', 'p1'), task('in-p2', 'p2'), task('elsewhere', 'p9')] });
    await answer('snapshot', [], null, files, server);
    expect(await tasks('p1', '', '')).toEqual(['in-p1', 'in-p2']);
    expect(await tasks('', KEY, '')).toEqual(['in-p1', 'in-p2']); // the project chosen in the scope menu
    expect(await tasks('', KEY, 'p1')).toEqual(['in-p1']); // a checkout stays one
  });

  test("Integrations from a bare project id scopes the device switches to the project and writes every checkout's override", async () => {
    const { server, files } = await launched(0, twoCheckouts);
    // integrationsPage: environmentId, projectId, active, refresh, revision, machine, projectKey, checkout (app.contract).
    const page = (projectId: string, projectKey: string, checkout: string) => answer('integrationsPage', [ENV, projectId, true, 0, 0, '', projectKey, checkout], null, files, server).then(obj);
    const fromLink = await page('p1', '', '');
    expect([fromLink.error, fromLink.deviceScope]).toEqual(['', `|${KEY}|`]);
    expect((await page('', KEY, '')).deviceScope).toBe(fromLink.deviceScope); // the project chosen in the scope menu
    expect((await page('', KEY, 'p1')).deviceScope).toBe(`|${KEY}|p1`); // a checkout stays one
    // Agent device access, pressed on that page (DeviceRow's rest("device", …&scope=<deviceScope>)): one write with both overrides.
    const from = server.calls.length;
    await answer('command', ['rest:device', '', `key=enableAgentDeviceAccess&value=true&scope=${str(fromLink.deviceScope)}`, 0], null, files, server);
    const writes = server.calls.slice(from).filter(call => call.method === 'server.updateSettings')
      .map(call => obj(obj(obj(call.payload).patch).projectSettingsOverrides));
    expect(writes).toEqual([{ p1: { enableAgentDeviceAccess: true }, p2: { enableAgentDeviceAccess: true } }]);
  });

  // Last: it removes the project.
  test("a two-checkout project's page lists both checkouts and its Danger row removes both (ProjectSettingsPanel.tsx:497-523)", async () => {
    // The reference's project scope (no checkout): every member, "Remove this project everywhere" / "Remove all entries",
    // and removeMembers over all of them; a checkout chosen in the scope menu: that one, "Remove checkout".
    const { server, files } = await launched(0, twoCheckouts);
    const view = (args: unknown[]) => answer('projectsView', args, null, files, server).then(obj);
    const danger = (value: Obj) => ({ members: arr(value.members).map(member => str(member.id)), hasOther: value.hasOther, removeTitle: value.removeTitle,
      removeLabel: value.removeLabel, removeTarget: value.removeTarget, confirmTitle: value.confirmTitle });
    const fromLink = await view(['', '', 'p1', true]);
    expect(danger(fromLink)).toEqual({ members: ['p1', 'p2'], hasOther: false, removeTitle: 'Remove this project everywhere', removeLabel: 'Remove all entries',
      removeTarget: '', confirmTitle: 'Remove project "work" and delete its 3 threads?' });
    expect(str(fromLink.confirmDescription)).toContain('This removes 2 grouped project entries.');
    expect(danger(await view([KEY, '', '', true]))).toEqual(danger(fromLink)); // the project chosen in the scope menu
    expect(danger(await view([KEY, 'p1', '', true]))).toEqual({ members: ['p1'], hasOther: true, removeTitle: 'Remove checkout', removeLabel: 'Remove checkout',
      removeTarget: 'p1', confirmTitle: 'Remove checkout "work" and delete its 2 threads?' });
    const core = await answer('settingsCore', ['', '', '', 'p1', 'projects', '', true], null, files, server).then(obj);
    expect([core.kind, core.connective, core.environmentLabel, core.scopeKey]).toEqual(['project', 'across', 'All environments', `|${KEY}|`]);
    // Confirm (settings-rest-dialogs.contract: an empty removeTarget confirms raw("remove-group", key, "")): both entries go.
    await answer('command', ['remove-group', str(fromLink.key), '', 0], null, files, server);
    expect(server.committed.filter(payload => payload.type === 'project.delete').map(payload => payload.projectId)).toEqual(['p1', 'p2']);
  });
});
