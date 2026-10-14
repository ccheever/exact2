// legacy-sidebar-full (U22): the legacy sidebar's remaining reference behaviour (T3 Code 1e2ecbd975, MIT, see
// LICENSE-T3): LS-1 a row's pull request link (handlePrClick, useOpenPrLink), LS-2 the discovered-port button
// (useThreadDiscoveredPorts, openDiscoveredPort), LS-3 files dropped on a row (sidebarPendingFileDropStore and
// ChatView's consumePendingFileDrop), LS-4 Archive and New thread in the tab order (group-focus-within), LS-5
// "New thread on <branch>" (handleNewThread with the branch), LS-6 a background environment's project and thread
// actions and environmentGrouping.test.ts's physical dedupe cases.
import { afterEach, describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { T3Client } from './client';
import type { Obj } from './domain';
import type { Files, Native } from './protocol';
import { toasts } from './toast';
import { fleet, type FleetEntry } from './settings-b-fleet';
import { sidebarPrefs, sidebarSession } from './sidebar-state';
import { sidebarCommand, sidebarLocal } from './sidebar-commands';
import { surfaceStore } from './r4-surfaces-panel';
import { SIDEBAR_DISCOVERED_SERVERS_KEY, discoveredServersEvent } from './browser-targets';
import { draftContext } from './composer-controls-branch';
import { draftFiles } from './composer-editor-files';
import { browserHistory } from './browser-history';
import { legacyGroups, legacySession, legacySidebarSnapshot } from './legacy-sidebar-view';
import { fleetPortsEvent, fleetPortsPass, openPendingPort, portLabels, rowPorts, watchRowPorts } from './legacy-sidebar-ports';
import { takeRowDrops } from './legacy-sidebar-drop';
import { threadOps } from './client-ops-threads';
import type { OpOut } from './client-ops';
import { terminalUiStore } from './terminal-ui-state';
import { contractComponent, type Env } from './contract-action-fixture';

const at = (minute: number) => `2026-03-09T10:${String(minute).padStart(2, '0')}:00.000Z`;
const files: Files = { fs: { async mkdir() {}, async atomicWriteFile() {}, async readFile() { return new ArrayBuffer(0); } } };
const identity = (name: string) => ({ projectMark: name.slice(0, 2), projectInk: 'ink', projectSurface: 'surface' });
const obj = (value: unknown): Obj => (value && typeof value === 'object' ? value as Obj : {});
const thread = (id: string, extra: Obj = {}): Obj => ({ id, projectId: 'p1', title: `Thread ${id}`, status: 'idle', latestRunId: null, activeProviderThreadId: null,
  pendingRuntimeRequest: null, createdAt: at(0), updatedAt: at(0), archivedAt: null, settledOverride: null, lineage: { relationshipToParent: null }, ...extra });
const repo = { provider: 'github', canonicalKey: 'github.com/acme/app', displayName: 'acme/app' };
const PR = 'https://github.com/acme/app/pull/7';
const linked = (url = PR) => [{ host: 'github.com', repository: 'acme/app', number: 7, url, source: 'manual', linkedAt: at(1), stack: null,
  snapshot: { state: 'open', title: 'Ship it', headBranch: 'feat', baseBranch: 'main', isDraft: false, updatedAt: at(1), syncedAt: at(1) } }];

// A legacy component run by contract-action-fixture.ts as the host mounts it: its state, its props (a test changes them as
// a batch would) and the host's functions; `view` is its state and derives, `drawn` whether a line of its view is drawn.
interface Mounted { props: Obj; act(action: string, ...args: unknown[]): void; readonly view: Env; drawn(marker: string): boolean }
const mount = (name: string, props: Obj, host: Obj): Mounted => {
  const component = contractComponent('legacy-sidebar.contract', name), state = component.state();
  const scope = () => ({ ...host, ...props });
  return {
    props,
    act: (action, ...args) => component.call(action, state, scope(), args),
    get view() { return { ...state, ...component.derives(state, scope()) }; },
    drawn: marker => component.drawn(marker, state, scope()),
  };
};

interface Rig { client: T3Client; native: Native; ops: Obj[]; rpcs: { method: string; payload: Obj }[]; opened: string[]; drafts: string[] }
/** A real client on `local` (thread-1 open unless said), its server answering preview and dispatch calls, its module recording ops. */
function rig(options: { threads?: Obj[]; projects?: Obj[]; threadId?: string; modifiers?: Obj; picks?: string[]; settings?: Obj; failOpen?: boolean; caps?: Obj; presentation?: Obj; editorApplies?: boolean } = {}): Rig {
  const client = new T3Client(), ops: Obj[] = [], rpcs: { method: string; payload: Obj }[] = [], opened: string[] = [], drafts: string[] = [], picks = [...(options.picks ?? [])];
  let ids = 0;
  Object.assign(client, { loaded: true, connection: 'connected', origin: 'http://127.0.0.1:16562', configLive: true, shellLive: true, threadLive: true, scopes: ['orchestration:operate'], presentation: options.presentation ?? {} });
  client.environmentId = 'local'; client.threadId = options.threadId ?? 'thread-1'; client.projectId = 'p1';
  client.shell = { projects: options.projects ?? [{ id: 'p1', title: 'App', workspaceRoot: '/work/app', repositoryIdentity: repo, createdAt: at(0), updatedAt: at(0) }], threads: options.threads ?? [thread('thread-1'), thread('thread-2', { pullRequests: linked() })], sequence: 1 };
  client.config = { environment: { label: 'This Mac', capabilities: { pullRequests: true, threadPullRequests: true, threadVisitedTracking: true, serverResolvedCommandContext: true, ...(options.caps ?? {}) } }, settings: {}, providers: [], keybindings: [] };
  client.local.clientSettings = { ...client.local.clientSettings, legacySidebarEnabled: true, browserLinkTarget: 'system', confirmThreadDelete: true, ...(options.settings ?? {}) } as typeof client.local.clientSettings;
  client.request = async (_native, method, payload) => {
    rpcs.push({ method, payload });
    if (method === 'preview.list') return { sessions: [], serverEpoch: 'epoch-1', revision: 1 };
    if (method === 'preview.open') {
      if (options.failOpen) throw new Error('The preview server refused.');
      return { threadId: String(payload.threadId), tabId: 'tab-1', navStatus: { _tag: 'Loading', url: String(payload.url), title: '' }, canGoBack: false, canGoForward: false, updatedAt: at(2) };
    }
    return {};
  };
  Object.assign(client, {
    async openSelected(_native: Native, id: string) { opened.push(id); client.threadId = id; },
    async openDraft(_native: Native, projectId: string) { drafts.push(projectId); client.threadId = ''; client.projectId = projectId; },
    adoptStatus(value: Obj) { ops.push({ op: 'adopted', value }); },
  });
  const native: Native = { available: true, watch() {}, async later(request) {
    const op = obj(request); ops.push(op);
    if (op.op === 'sidebarModifiers') return { ok: true, generation: Number(op.generation ?? 0), value: options.modifiers ?? {} };
    if (op.op === 'sidebarMenu') return { ok: true, generation: Number(op.generation ?? 0), value: { id: picks.shift() ?? null } };
    if (op.op === 'ids') return { ok: true, generation: 0, value: Array.from({ length: Number(op.count) }, () => `00000000-0000-4000-8000-${String(ids++).padStart(12, '0')}`) };
    if (op.op === 'editorInsert') return { ok: true, generation: 0, value: { applied: options.editorApplies === true } };
    if (op.op === 'subscribe') return { ok: true, generation: Number(op.generation ?? 0), value: { id: `sub-${ops.length}` } };
    const generation = op.fleet ? 3 : Number(op.generation ?? 0);
    return { ok: true, generation, value: op.op === 'connect' ? { state: 'connecting' } : {} };
  } };
  return { client, native, ops, rpcs, opened, drafts };
}
const opsOf = (ops: Obj[], name: string) => ops.filter(op => op.op === name);
const fleetRequests = (ops: Obj[]) => ops.filter(op => op.fleet === 'remote' && op.op === 'request').map(op => ({ method: op.method, payload: obj(op.payload) }));

/** A background environment the fleet keeps connected: Studio, with the shared repository and a solo project. */
function background(threads: Obj[] = [], projects?: Obj[]): FleetEntry {
  const entry = { key: 'remote', origin: 'http://studio.local:3773', environmentId: 'env-remote', phase: 'connected', message: '', traceId: '', generation: 3, synchronized: 3, lastEvent: 0,
    subscriptions: {}, config: { environment: { label: 'Studio', capabilities: { threadVisitedTracking: true } }, settings: {} },
    shell: { projects: projects ?? [{ id: 'r1', title: 'solo', workspaceRoot: '/srv/solo', createdAt: at(0), updatedAt: at(0) }], threads, sequence: 1 }, scopes: [], error: '', requested: false } as unknown as FleetEntry;
  fleet.entries.set('remote', entry);
  return entry;
}
afterEach(() => { fleet.entries.clear(); });

describe('LS-1: the pull request glyph (handlePrClick)', () => {
  test('beside an open thread, a pull request its environment reads opens in that thread\'s right panel, and the thread opens', async () => {
    const { client, native, opened } = rig();
    expect(await sidebarCommand(client, native, files, 'legacy-pr', 'thread-2', '')).toBe('sidebar:opened:focus');
    expect(opened).toEqual(['thread-2']);
    const panel = surfaceStore(client).panels.get('local:thread-2')!;
    expect(panel).toMatchObject({ visible: true, active: 'pull-request:p1:github.com:acme%2Fapp:7' });
    expect(panel.surfaces.map(surface => [surface.kind, surface.pr?.number])).toEqual([['pull-request', 7]]);
    expect(sidebarSession(client)).toMatchObject({ selection: [], anchor: 'thread-2' });
  });
  test('the open thread\'s own glyph opens the panel in place', async () => {
    const { client, native, opened } = rig({ threadId: 'thread-2' });
    client.diffOpen = true;
    expect(await sidebarCommand(client, native, files, 'legacy-pr', 'thread-2', '')).toBe('');
    expect(opened).toEqual([]);
    expect(client.diffOpen).toBe(false);
    expect(surfaceStore(client).panels.get('local:thread-2')?.visible).toBe(true);
  });
  test('with no thread route (the Pull Requests page, an unsent draft), it opens on the Pull Requests page', async () => {
    const { client, native, opened } = rig();
    const result = await sidebarCommand(client, native, files, 'legacy-pr', 'thread-2', 'pull-requests');
    expect(result.startsWith('sidebar:pr-page:')).toBe(true);
    expect(JSON.parse(result.slice(16))).toEqual({ projectId: 'p1', host: 'github.com', repository: 'acme/app', number: 7 });
    client.threadId = ''; // resolveActiveThreadRouteRef: a draft route has no thread until it is sent
    expect((await sidebarCommand(client, native, files, 'legacy-pr', 'thread-2', '')).startsWith('sidebar:pr-page:')).toBe(true);
    expect(opened).toEqual([]);
    expect(surfaceStore(client).panels.get('local:thread-2')).toBeUndefined();
  });
  test('⌘ or Ctrl leaves it to the system browser; a link no project reads follows "Open links in"', async () => {
    const held = rig({ modifiers: { command: true } });
    expect(await sidebarCommand(held.client, held.native, files, 'legacy-pr', 'thread-2', '')).toBe('');
    expect(opsOf(held.ops, 'remoteEditorsOpen').map(op => op.url)).toEqual([PR]);
    expect(surfaceStore(held.client).panels.get('local:thread-2')).toBeUndefined();
    const elsewhere = 'https://github.com/other/repo/pull/3';
    const system = rig({ threads: [thread('thread-1'), thread('thread-2', { pullRequests: linked(elsewhere) })], caps: { threadPullRequests: false } });
    expect(await sidebarCommand(system.client, system.native, files, 'legacy-pr', 'thread-2', '')).toBe('');
    expect(opsOf(system.ops, 'remoteEditorsOpen').map(op => op.url)).toEqual([elsewhere]);
    expect(system.opened).toEqual([]);
    const app = rig({ threads: [thread('thread-1'), thread('thread-2', { pullRequests: linked(elsewhere) })], caps: { threadPullRequests: false }, settings: { browserLinkTarget: 'app' } });
    expect(await sidebarCommand(app.client, app.native, files, 'legacy-pr', 'thread-2', '')).toBe('');
    expect(app.rpcs.find(call => call.method === 'preview.open')?.payload).toMatchObject({ threadId: 'thread-2', url: elsewhere });
    expect(surfaceStore(app.client).panels.get('local:thread-2')?.visible).toBe(true);
    expect(app.opened).toEqual([], 'an ordinary link does not navigate (openedInRightPanel is false)');
  });
  test('the row is a button labelled with the status tooltip; a pending link keeps its "status pending" label', () => {
    const { client } = rig({ threads: [thread('thread-1'), thread('thread-2', { pullRequests: linked() }), thread('thread-3', { pullRequests: [{ ...linked()[0], snapshot: null }] })] });
    const rows = legacySidebarSnapshot(client, Date.parse(at(30)), { projectIdentity: identity }).projects[0]!.threads;
    expect(rows.find(row => row.id === 'thread-2')).toMatchObject({ prTooltip: 'PR #7 - Open: Ship it', prPending: false });
    expect(rows.find(row => row.id === 'thread-3')).toMatchObject({ prTooltip: 'PR #7, status pending', prPending: true });
    const view = readFileSync(new URL('./legacy-sidebar.contract', import.meta.url), 'utf8');
    expect(view).toContain('button cursor="pointer" press=run("legacy-pr", t.id, page) dblclick=nested focus=within("pr", true)');
  });
});

describe('LS-2: the discovered-port button (useThreadDiscoveredPorts, openDiscoveredPort)', () => {
  const server = (port: number, threadId: string | null) => ({ host: '127.0.0.1', port, url: `http://localhost:${port}/`, processName: 'vite', pid: 42, terminal: threadId ? { threadId, terminalId: 'term-1' } : null });
  test('the sidebar keeps its own stream (no configured URLs) while the legacy sidebar shows', async () => {
    const { client, native, ops } = rig();
    await watchRowPorts(client, native);
    expect(opsOf(ops, 'subscribe').map(op => [op.key, op.method, op.payload])).toEqual([[SIDEBAR_DISCOVERED_SERVERS_KEY, 'subscribeDiscoveredLocalServers', {}]]);
    client.local.clientSettings = { ...client.local.clientSettings, legacySidebarEnabled: false } as typeof client.local.clientSettings;
    await watchRowPorts(client, native);
    expect(opsOf(ops, 'unsubscribe').map(op => op.key)).toEqual([SIDEBAR_DISCOVERED_SERVERS_KEY]);
  });
  test('a thread\'s ports are the servers its terminals opened; the label names the first and the tooltip counts the rest', async () => {
    const { client, native, ops } = rig();
    await watchRowPorts(client, native);
    const subscriptionId = String(opsOf(ops, 'subscribe')[0]!.id ?? '');
    void subscriptionId;
    discoveredServersEvent(client, { key: SIDEBAR_DISCOVERED_SERVERS_KEY, subscriptionId: 'sub-1', value: { servers: [server(5173, 'thread-2'), server(8080, 'thread-2'), server(3000, null), server(4000, 'thread-1')] } });
    expect(rowPorts(client, 'thread-2').map(port => port.port)).toEqual([5173, 8080]);
    expect(portLabels(rowPorts(client, 'thread-2'))).toEqual({ portLabel: 'Open localhost:5173', portTip: 'Open localhost:5173 (+1)' });
    expect(portLabels(rowPorts(client, 'thread-1'))).toEqual({ portLabel: 'Open localhost:4000', portTip: 'Open localhost:4000' });
    expect(portLabels([])).toEqual({ portLabel: '', portTip: '' });
    const rows = legacySidebarSnapshot(client, Date.parse(at(30)), { projectIdentity: identity }).projects[0]!.threads;
    expect(rows.find(row => row.id === 'thread-2')).toMatchObject({ portLabel: 'Open localhost:5173', portTip: 'Open localhost:5173 (+1)' });
  });
  test('the button goes to the thread and opens its first port beside it, recorded in its history', async () => {
    const { client, native, ops, rpcs, opened } = rig();
    await watchRowPorts(client, native);
    discoveredServersEvent(client, { key: SIDEBAR_DISCOVERED_SERVERS_KEY, subscriptionId: 'sub-1', value: { servers: [server(5173, 'thread-2')] } });
    void ops;
    expect(await sidebarCommand(client, native, files, 'legacy-port', 'thread-2', '')).toBe('sidebar:opened:focus');
    expect(opened).toEqual(['thread-2']);
    expect(rpcs.find(call => call.method === 'preview.open')?.payload).toMatchObject({ threadId: 'thread-2', url: 'http://localhost:5173/' });
    expect(surfaceStore(client).panels.get('local:thread-2')).toMatchObject({ visible: true, active: 'browser:tab-1' });
    // recordVisitForThread: a thread with no registered project keeps the visit pending (browser-history.ts), as the reference does.
    const pending = (browserHistory(client.local) as unknown as { state: { pendingVisitsByThreadKey: Record<string, { url: string }[]> } }).state.pendingVisitsByThreadKey['local:thread-2'];
    expect(pending?.map(visit => visit.url)).toEqual(['http://localhost:5173/']);
  });
  test('a preview that cannot open says so', async () => {
    const { client, native } = rig({ failOpen: true, threadId: 'thread-2' });
    await watchRowPorts(client, native);
    discoveredServersEvent(client, { key: SIDEBAR_DISCOVERED_SERVERS_KEY, subscriptionId: 'sub-1', value: { servers: [server(5173, 'thread-2')] } });
    expect(await sidebarCommand(client, native, files, 'legacy-port', 'thread-2', '')).toBe('');
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'error', title: 'Unable to open preview', description: 'The preview server refused.' });
  });
});

describe('LS-2 on a background environment: its own stream, and its thread opens before the tab', () => {
  const server = (port: number, threadId: string) => ({ host: '127.0.0.1', port, url: `http://localhost:${port}/`, processName: 'node', pid: 7, terminal: { threadId, terminalId: 't' } });
  test('the fleet subscribes while the legacy sidebar shows rows; a background row shows its own ports', async () => {
    const { client, native } = rig();
    const entry = background([thread('t9', { projectId: 'r1' })]);
    await watchRowPorts(client, native);
    const calls: Obj[] = [];
    await fleetPortsPass(async request => { calls.push(request); return { id: 'sub-7' }; }, entry);
    expect(calls).toEqual([{ op: 'subscribe', key: SIDEBAR_DISCOVERED_SERVERS_KEY, method: 'subscribeDiscoveredLocalServers', payload: {} }]);
    expect(fleetPortsEvent(entry, { key: SIDEBAR_DISCOVERED_SERVERS_KEY, generation: 3, subscriptionId: 'sub-7', value: { servers: [server(3000, 't9'), server(3001, 'other')] } })).toBe(true);
    expect(fleetPortsEvent(entry, { key: 'shell', generation: 3 })).toBe(false);
    expect(portLabels(rowPorts(client, 'fleet:env-remote:t9'))).toEqual({ portLabel: 'Open localhost:3000', portTip: 'Open localhost:3000' });
    expect(legacySidebarSnapshot(client, Date.parse(at(30)), { projectIdentity: identity }).projects.flatMap(project => project.threads).find(row => row.id === 'fleet:env-remote:t9'))
      .toMatchObject({ portLabel: 'Open localhost:3000', remoteLabel: 'Studio' });
    client.local.sidebarOpen = false;
    await watchRowPorts(client, native);
    await fleetPortsPass(async request => { calls.push(request); return {}; }, entry);
    expect(calls.at(-1)).toEqual({ op: 'unsubscribe', key: SIDEBAR_DISCOVERED_SERVERS_KEY });
    expect(rowPorts(client, 'fleet:env-remote:t9')).toEqual([]);
  });
  test('the button focuses the background environment, and the port opens once its thread is open there', async () => {
    const { client, native, ops, rpcs } = rig();
    const entry = background([thread('t9', { projectId: 'r1' })]);
    await watchRowPorts(client, native);
    await fleetPortsPass(async () => ({ id: 'sub-7' }), entry);
    fleetPortsEvent(entry, { key: SIDEBAR_DISCOVERED_SERVERS_KEY, generation: 3, subscriptionId: 'sub-7', value: { servers: [server(3000, 't9')] } });
    expect(await sidebarCommand(client, native, files, 'legacy-port', 'fleet:env-remote:t9', '')).toBe('sidebar:opened:focus');
    expect(opsOf(ops, 'connect').at(-1)).toMatchObject({ origin: 'http://studio.local:3773' });
    expect(rpcs.some(call => call.method === 'preview.open')).toBe(false);
    client.environmentId = 'env-remote'; client.threadId = 'other';
    await openPendingPort(client, native);
    expect(rpcs.some(call => call.method === 'preview.open')).toBe(false); // the environment holds; another thread is open: dropped
    client.environmentId = 'local'; client.threadId = 'thread-1'; // back where the window was, Studio in the background again
    const again = background([thread('t9', { projectId: 'r1' })]);
    await fleetPortsPass(async () => ({ id: 'sub-8' }), again);
    fleetPortsEvent(again, { key: SIDEBAR_DISCOVERED_SERVERS_KEY, generation: 3, subscriptionId: 'sub-8', value: { servers: [server(3000, 't9')] } });
    expect(await sidebarCommand(client, native, files, 'legacy-port', 'fleet:env-remote:t9', '')).toBe('sidebar:opened:focus');
    client.environmentId = 'env-remote'; client.threadId = 't9';
    await openPendingPort(client, native);
    expect(rpcs.find(call => call.method === 'preview.open')?.payload).toMatchObject({ threadId: 't9' });
  });
});

describe('LS-7 (found while building): the row\'s tooltips', () => {
  const view = readFileSync(new URL('./legacy-sidebar.contract', import.meta.url), 'utf8');
  test('the title, the worktree glyph, the collapsed project\'s status and the controls carry the reference\'s tooltips', () => {
    expect(view).toContain('LegacyTitleTip(label=t.title, tipId=`legacy-tip-title-${t.id}`, relay=innerHover)');
    expect(view).toContain('Tip(label=t.worktreeLabel, side="top"');
    expect(view).toContain('Tip(label=p.statusLabel, side="top"');
    expect(view).toContain('Tip(label=(t.prPending ? "" : t.prTooltip)');
    expect(view).toContain('Tip(label=t.portTip');
  });
  // The host hovers only the nearest node with a hover handler (MouseChainMac `hoverUnderPointer`), so a pointer entering
  // a row on its title would hover the title's tooltip and not the row: Archive stayed hidden and a press there opened the
  // thread (the first after drive). Every tooltip trigger inside a row tells the row, as group-hover holds for the whole row.
  test('every tooltip trigger inside a thread row or a project header tells the row the pointer is on it', () => {
    const thread = view.slice(view.indexOf('component LegacyThread'), view.indexOf('// A legacy row\'s or project header\'s tooltip trigger'));
    const project = view.slice(view.indexOf('component LegacyProject'), view.indexOf('// SidebarThreadRow'));
    for (const [row, triggers] of [[thread, 6], [project, 2]] as const) { // thread: title, pull request, port, worktree, terminal, Archive
      expect(row).toContain('action innerHover(value: bool)\n    inner = value');
      expect(row).toContain('derive pointed = over or inner');
      expect(row.match(/\bTip\(/g) ?? []).toHaveLength(0); // a plain Tip's hover would not reach the row
      expect(row.match(/Tip\([^\n]*relay=innerHover\)/g) ?? []).toHaveLength(triggers);
    }
    for (const tip of ['component LegacyTitleTip', 'component LegacyRowTip']) {
      const body = view.slice(view.indexOf(tip), view.indexOf('\n  view', view.indexOf(tip)));
      expect(body).toContain('relay: action');
      expect(body).toContain('action setOver(value: bool)\n    relay(value)');
    }
  });
  // Review round 2: the thread list clips its rows (overflow hidden, the reference's, for autoAnimate), which cut a bubble
  // drawn beside each project's first row. The reference portals every TooltipPopup; the window's hover layer is the clone's.
  test('review 2: the row\'s and the header\'s tooltips are drawn by the window\'s hover layer, so the list\'s clip does not cut them', () => {
    const rows = view.slice(view.indexOf('component LegacyProject'), view.indexOf('// The rename field reports its focus'));
    expect(rows).not.toContain('TipBubble(');
    expect(view).toContain('border-color="light-dark(#e4e4e7, #ffffff14)" overflow="hidden"'); // the list still clips
    expect(rows.match(/(?:LegacyRowTip|LegacyTitleTip)\([^\n]*tipId=`legacy-tip-(\w+)-\$\{[tp]\.(?:id|key)\}`/g) ?? []).toHaveLength(8);
    const tips: unknown[][] = [], relayed: boolean[] = [];
    const host = { rem: 16, hoverTipAt: (...args: unknown[]) => { tips.push(args); }, frame: (id: string) => ({ id, x: 40, y: 200, width: 12, height: 12 }),
      legacyTip: (text: string, side: string, align: string, at: Obj, rem: number, testId: string) => ({ key: 'legacy-tip', kind: 'tooltip', text, side, align, at, rem, testId }) };
    const tip = mount('LegacyRowTip', { label: 'Open localhost:5173', side: 'top', align: 'center', tipId: 'legacy-tip-port-t1', shown: true, relay: (value: boolean) => { relayed.push(value); } }, host);
    tip.act('setOver', true);
    tip.act('setOver', false);
    expect(relayed).toEqual([true, false]);
    expect(tips).toEqual([[{ key: 'legacy-tip', kind: 'tooltip', text: 'Open localhost:5173', side: 'top', align: 'center', at: { id: 'legacy-tip-port-t1', x: 40, y: 200, width: 12, height: 12 }, rem: 16, testId: 'legacy-tip-port-t1-bubble' }, 'trigger', true],
      [{ key: 'legacy-tip', kind: 'tooltip', text: 'Open localhost:5173', side: 'top', align: 'center', at: { id: 'legacy-tip-port-t1', x: 40, y: 200, width: 12, height: 12 }, rem: 16, testId: 'legacy-tip-port-t1-bubble' }, 'trigger', false]]);
    const quiet = mount('LegacyRowTip', { label: '', side: 'top', align: 'center', tipId: 'legacy-tip-archive-t1', shown: true, relay() {} }, host);
    quiet.act('setOver', true); // Archive with confirmation: no tooltip
    expect(tips).toHaveLength(2);
    expect(view).toContain('fn legacyTip(label: string, side: string, align: string, at: Geometry, rem: number, testId: string): HoverTip = hoverTipAtFrame("legacy-tip", "tooltip", label');
  });
  test('review 2: the row or header the pointer is on closes a legacy tooltip a gone trigger left; Archive\'s press and a right-click close their own', () => {
    const tips: unknown[][] = [], runs: unknown[][] = [];
    const host = { performanceNow: () => 0, run: (...args: unknown[]) => { runs.push(args); }, hoverTipAt: (...args: unknown[]) => { tips.push(args); }, legacyTipOff: () => ({ key: 'legacy-tip' }) };
    const row = mount('LegacyThread', { reduced: false, page: '', busy: false, legacy: { confirmArchive: false }, t: { id: 't1', running: false, renaming: false, prIcon: '', portLabel: '' } }, host);
    row.act('hover', true);
    row.act('archive');
    const header = mount('LegacyProject', { reduced: false, p: { key: 'k', expanded: true, statusLabel: '' }, legacy: {}, now: 0, page: '', busy: false }, host);
    header.act('hover', true);
    row.act('openMenu');
    header.act('openMenu');
    expect(tips).toEqual(Array.from({ length: 5 }, () => [{ key: 'legacy-tip' }, 'trigger', false]));
    expect(runs).toEqual([['legacy-archive', 't1', ''], ['legacy-thread-menu', 't1', ''], ['legacy-project-menu', 'k', '']]);
    const view = readFileSync(new URL('./legacy-sidebar.contract', import.meta.url), 'utf8');
    expect(view.match(/contextmenu=openMenu /g) ?? []).toHaveLength(2);
  });
  test('the worktree tooltip names the worktree by its last segment (formatWorktreePathForDisplay), then the branch', () => {
    const { client } = rig({ threads: [thread('thread-1'), thread('wt', { worktreePath: '/work/.t3/worktrees/app-feature', branch: 'feature/x' }), thread('plain', { worktreePath: '/work/.t3/worktrees/solo' })] });
    const rows = legacySidebarSnapshot(client, Date.parse(at(30)), { projectIdentity: identity }).projects[0]!.threads;
    expect(rows.find(row => row.id === 'wt')?.worktreeLabel).toBe('Worktree: app-feature (feature/x)');
    expect(rows.find(row => row.id === 'plain')?.worktreeLabel).toBe('Worktree: solo');
  });
});

describe('LS-8 (found while building): list motion and the destructive "Delete anyway"', () => {
  const view = readFileSync(new URL('./legacy-sidebar.contract', import.meta.url), 'utf8');
  test('autoAnimate (180 ms ease-out, none under reduced motion): project and thread rows slide and a leaving row fades', () => {
    expect(view).toContain('keyframes lsRowOut\n  from opacity=1 scale=1\n  to opacity=0 scale=0.98');
    const motion = '-exact-layout-transition=(reduced ? "none" : "180ms ease-out") -exact-exit-animation=(reduced ? "none" : "lsRowOut 180ms ease-out")';
    expect(view.split(motion).length - 1).toBe(3); // the project item, its thread list and each thread row
    expect(view).toContain('keyframes lsRowIn\n  from opacity=0 scale=0.98\n  to opacity=1 scale=1');
    expect(view).toContain('animation=(t.enter and not reduced ? "lsRowIn 180ms ease-out" : "none")');
    expect(view).toContain('animation=(p.enter and not reduced ? "lsRowIn 180ms ease-out" : "none")');
  });
  test('a row fades in when it arrives in a shown list, not on the first draw and not after its tick', () => {
    const { client } = rig({ threads: [thread('thread-1')] });
    const rows = (now: number) => legacySidebarSnapshot(client, now, { projectIdentity: identity }).projects.flatMap(project => [[project.name, project.enter], ...project.threads.map(row => [row.id, row.enter])]);
    expect(rows(1000)).toEqual([['App', false], ['thread-1', false]]); // the first draw
    client.shell.threads.push(thread('thread-2'));
    expect(rows(1000)).toEqual([['App', false], ['thread-2', true], ['thread-1', false]]);
    expect(rows(1000)).toEqual([['App', false], ['thread-2', true], ['thread-1', false]]); // asked again in the same tick
    expect(rows(2000)).toEqual([['App', false], ['thread-2', false], ['thread-1', false]]);
    client.local.clientSettings = { ...client.local.clientSettings, legacySidebarEnabled: false } as typeof client.local.clientSettings;
    rows(3000);
    client.local.clientSettings = { ...client.local.clientSettings, legacySidebarEnabled: true } as typeof client.local.clientSettings;
    expect(rows(4000).every(([, enter]) => enter === false)).toBe(true); // switched on again: a first draw
  });
  test('the "Project is not empty" toast\'s Delete anyway is the destructive button', () => {
    const toastView = readFileSync(new URL('./shell-toast.contract', import.meta.url), 'utf8');
    // bg-destructive: --error, red-500 (#fb2c36), lighter in dark mode, as the clone's other destructive buttons.
    expect(toastView).toContain('background-color=(toast.actionDestructive ? "light-dark(#fb2c36, #fb414a)" : "light-dark(#1b4ed8, #346bf1)")');
    expect(toastView).not.toContain('#dc2626');
  });
});

describe('LS-3: files dropped on a row (consumePendingFileDrop, addDroppedFiles)', () => {
  test('once the dropped-on thread is open, its queued files join its composer: images to the shelf, files as chips', async () => {
    const queued = { rowDrops: ['thread-9', 'fleet:local:thread-2'] };
    const { client, native, ops } = rig({ threadId: 'thread-2', presentation: queued });
    const staged = [{ kind: 'file', id: '11111111-1111-4111-8111-111111111111', name: 'notes.txt', sizeBytes: 5, mimeType: 'text/plain' },
      { kind: 'image', id: '22222222-2222-4222-8222-222222222222', name: 'shot.png', sizeBytes: 64 }, { kind: 'unsupported-image', name: 'raw.tiff', sizeBytes: 9 }];
    native.later = (original => async (request: unknown) => obj(request).op === 'rowDropTake' ? (ops.push(obj(request)), { ok: true, generation: Number(obj(request).generation ?? 0), value: { files: staged } }) : original(request))(native.later.bind(native));
    expect(await takeRowDrops(client, native)).toBe(true);
    expect(opsOf(ops, 'rowDropTake')[0]).toMatchObject({ keys: ['thread-2', 'fleet:local:thread-2'] });
    expect(draftFiles(client.local).map(file => [file.name, file.draftKey])).toEqual([['notes.txt', 'local:thread-2']]);
    expect(client.local.snapshotDrafts['local:thread-2']?.map(image => image.name)).toEqual(['shot.png']);
    expect(opsOf(ops, 'editorInsert')[0]).toMatchObject({ owner: client.snapshotOwner });
    expect(client.local.drafts['local:thread-2']).toContain('notes.txt', 'another thread\'s editor refused the chips: they join the stored draft');
    expect(client.error).toBe("'raw.tiff' is not a supported image type. Attach GIF, HEIC, HEIF, JPEG, PNG, or WebP images.");
  });
  test('nothing queued for the open thread asks the module nothing', async () => {
    const { client, native, ops } = rig({ threadId: 'thread-1', presentation: { rowDrops: ['thread-2'] } });
    expect(await takeRowDrops(client, native)).toBe(false);
    expect(opsOf(ops, 'rowDropTake')).toEqual([]);
  });
  test('each legacy row\'s button carries the drop target hatch and its thread key', () => {
    const view = readFileSync(new URL('./legacy-sidebar.contract', import.meta.url), 'utf8');
    expect(view).toContain('hatch="t3-row-drop" data-thread-key=t.id');
    const manifest = JSON.parse(readFileSync(new URL('./app.json', import.meta.url), 'utf8'));
    expect(manifest.hatches).toContain('t3-row-drop');
    expect(manifest.data).toContain('thread-key');
  });
});

describe('LS-4: Archive and New thread in the tab order (group-focus-within)', () => {
  const view = readFileSync(new URL('./legacy-sidebar.contract', import.meta.url), 'utf8');
  const thread = view.slice(view.indexOf('component LegacyThread'), view.indexOf('component LegacyRenameField'));
  const project = view.slice(view.indexOf('component LegacyProject'), view.indexOf('// SidebarThreadRow'));
  test('Archive stays mounted (opacity 0, no pointer) and shows on hover or while the focus is in the row', () => {
    expect(thread).toContain('derive revealed = (pointed or inside) and not t.running');
    expect(thread).toContain('        box position="absolute" right="0.125rem" top="0.25rem" opacity=(revealed ? 1 : 0) pointer-events=(revealed ? "auto" : "none")');
    expect(thread).toContain('shown=(not confirmShown and not t.running), relay=innerHover)\n            if not confirmShown and not t.running\n              button cursor="pointer" press=archive');
    expect(thread).not.toContain('if over and not t.running');
    for (const target of ['press=open(t.id, "click")', 'press=run("legacy-pr"', 'press=run("legacy-port"', 'press=archive']) {
      const at = thread.indexOf(target);
      expect(at).toBeGreaterThan(0);
      expect(thread.slice(at, thread.indexOf('\n', at))).toMatch(/focus=within\("(\w+)", true\) blur=within\("\1", false\)/);
    }
    expect(thread).toContain('opacity=(revealed or confirmShown ? 0 : 1)'); // the meta hides the same way
  });
  test('Confirm takes the focus; the tooltip shows only without confirmation', () => {
    expect(thread).toContain('press=confirmArchive focus=within("confirm", true) blur=within("confirm", false) autofocus=true');
    expect(thread).toContain('Tip(label=(legacy.confirmArchive ? "" : "Archive")');
  });
  test('the project\'s New thread stays mounted and crossfades with the environment badge', () => {
    expect(project).toContain('derive revealed = pointed or inside');
    expect(project).toContain('opacity=(revealed ? 1 : 0) pointer-events=(revealed ? "auto" : "none")');
    expect(project).toContain('opacity=(revealed ? 0 : 1) pointer-events="none" role="img" aria-label=p.remoteLabel');
    expect(project).not.toContain('if over\n          box position="absolute" right="0.125rem"');
  });
});

describe('LS-5: "New thread on <branch>" carries the branch (handleNewThread with branch, worktreePath, envMode)', () => {
  test('a worktree thread\'s draft reuses its worktree; a plain one takes its branch on the local checkout', async () => {
    const { client, native, drafts } = rig({ threads: [thread('thread-1'), thread('wt', { branch: 'feature/x', worktreePath: '/work/.t3/wt' }), thread('local', { branch: 'fix/y' })], picks: ['new-thread-on-branch', 'new-thread-on-branch'] });
    expect(await sidebarCommand(client, native, files, 'legacy-thread-menu', 'wt', '')).toBe('sidebar:new-thread');
    expect(drafts).toEqual(['p1']);
    expect(draftContext(client)).toEqual({ envMode: 'worktree', branch: 'feature/x', worktreePath: '/work/.t3/wt' });
    expect(await sidebarCommand(client, native, files, 'legacy-thread-menu', 'local', '')).toBe('sidebar:new-thread');
    expect(draftContext(client)).toEqual({ envMode: 'local', branch: 'fix/y', worktreePath: '' });
  });
});

describe('LS-6: a background environment\'s rows act on that environment', () => {
  test('environmentGrouping.test.ts: stale rows of one checkout keep the fresher, with its identity, and their threads join it', () => {
    const shared = { canonicalKey: 'github.com/example/shared-repo' };
    const dup = rig({ threads: [], projects: [
      { id: 'project-primary', title: 'shared-repo', workspaceRoot: '/tmp/shared-repo', repositoryIdentity: shared, updatedAt: '2026-01-02T00:00:00.000Z', createdAt: at(0) },
      { id: 'project-duplicate', title: 'shared-repo', workspaceRoot: '/tmp/shared-repo/', repositoryIdentity: shared, updatedAt: '2026-01-01T00:00:00.000Z', createdAt: at(0) }] });
    background([], [{ id: 'project-remote', title: 'shared-repo', workspaceRoot: '/tmp/shared-repo', repositoryIdentity: shared, createdAt: at(0), updatedAt: at(0) }]);
    const groups = legacyGroups(dup.client);
    expect(groups.map(group => [group.key, group.members.map(member => member.id)])).toEqual([['github.com/example/shared-repo', ['project-primary', 'fleet:env-remote:project-remote']]]);
    fleet.entries.clear();
    const stale = rig({ threads: [thread('old', { projectId: 'project-stale' })], projects: [
      { id: 'project-stale', title: 'shared-repo', workspaceRoot: '/tmp/shared-repo/', repositoryIdentity: null, updatedAt: '2026-01-01T00:00:00.000Z', createdAt: at(0) },
      { id: 'project-canonical', title: 'shared-repo', workspaceRoot: '/tmp/shared-repo', repositoryIdentity: shared, updatedAt: '2026-01-02T00:00:00.000Z', createdAt: at(0) }] });
    const [group] = legacyGroups(stale.client);
    expect(group).toMatchObject({ key: 'github.com/example/shared-repo' });
    expect(group!.members.map(member => [member.id, member.aliases])).toEqual([['project-canonical', ['project-stale', 'project-canonical']]]);
    expect(legacySidebarSnapshot(stale.client, Date.parse(at(30)), { projectIdentity: identity }).projects[0]!.threads.map(row => row.id)).toEqual(['old']);
    const fresher = rig({ threads: [], projects: [
      { id: 'project-canonical', title: 'shared-repo', workspaceRoot: '/tmp/shared-repo', repositoryIdentity: null, updatedAt: '2026-01-02T00:00:00.000Z', createdAt: at(0) },
      { id: 'project-stale', title: 'shared-repo', workspaceRoot: '/tmp/shared-repo/', repositoryIdentity: shared, updatedAt: '2026-01-01T00:00:00.000Z', createdAt: at(0) }] });
    expect(legacyGroups(fresher.client).map(entry => [entry.key, entry.members.map(member => member.id)])).toEqual([['github.com/example/shared-repo', ['project-canonical']]]);
  });
  test('Rename, Remove (with "Project is not empty") and New thread on a background project go to its environment', async () => {
    const { client, native, ops } = rig({ picks: ['rename:env-remote:/srv/solo', 'delete:env-remote:/srv/solo'] });
    background([thread('t9', { projectId: 'r1' })]);
    const key = 'env-remote:/srv/solo';
    await sidebarCommand(client, native, files, 'legacy-project-menu', key, '');
    expect(legacySidebarSnapshot(client, Date.parse(at(30)), { projectIdentity: identity })).toMatchObject({ dialog: 'rename-project', dialogTarget: 'fleet:env-remote:r1', dialogDetail: 'Environment: Studio' });
    await sidebarCommand(client, native, files, 'legacy-rename-project', 'fleet:env-remote:r1', 'Solo 2');
    expect(fleetRequests(ops).map(request => [request.method, request.payload.type, request.payload.projectId, request.payload.title])).toEqual([['projects.mutate', 'project.update', 'r1', 'Solo 2']]);
    expect(legacySession(client).dialog.kind).toBe('');
    await sidebarCommand(client, native, files, 'legacy-project-menu', key, '');
    expect(toasts(client).at(-1)).toMatchObject({ title: 'Project is not empty', actionVariant: 'destructive', action: { label: 'Delete anyway', op: 'sidebar:legacy-remove-anyway', id: 'fleet:env-remote:r1' } });
    await sidebarCommand(client, native, files, 'legacy-remove-anyway', 'fleet:env-remote:r1', '');
    expect(legacySidebarSnapshot(client, Date.parse(at(30)), { projectIdentity: identity }).dialogTitle).toBe('Remove project "solo" and delete its 1 thread?');
    client.local.drafts['env-remote:t9'] = 'kept text';
    await sidebarCommand(client, native, files, 'legacy-dialog-confirm', '', '');
    expect(fleetRequests(ops).at(-1)).toMatchObject({ method: 'projects.mutate', payload: { type: 'project.delete', projectId: 'r1', force: true } });
    expect(client.local.drafts['env-remote:t9']).toBeUndefined();
    background([]);
    expect(await sidebarCommand(client, native, files, 'legacy-new-thread', key, '')).toBe('sidebar:new-thread');
    expect(opsOf(ops, 'fleetStop').map(op => op.fleet)).toEqual(['remote']);
    expect(opsOf(ops, 'connect').at(-1)).toMatchObject({ origin: 'http://studio.local:3773', credential: '' });
    expect(client.local.selections['env-remote']).toEqual({ projectId: 'r1', threadId: '' });
    expect(opsOf(ops, 'adopted')).toHaveLength(1);
  });
  test('a background thread\'s menu: mark unread, copy path and id, rename, archive with undo, and delete with its worktree question', async () => {
    const { client, native, ops } = rig({ picks: ['mark-unread', 'copy-path', 'copy-thread-id', 'rename', 'delete'] });
    background([thread('t9', { projectId: 'r1', worktreePath: '/srv/.t3/wt-9', branch: 'feat' })]);
    const id = 'fleet:env-remote:t9';
    for (let round = 0; round < 3; round++) await sidebarCommand(client, native, files, 'legacy-thread-menu', id, '');
    expect(fleetRequests(ops).map(request => [request.payload.type, request.payload.threadId])).toEqual([['thread.mark-unread', 't9']]);
    expect(opsOf(ops, 'copyText').map(op => op.text)).toEqual(['/srv/.t3/wt-9', 't9']);
    await sidebarCommand(client, native, files, 'legacy-thread-menu', id, '');
    expect(sidebarSession(client)).toMatchObject({ renameId: id, renameTitle: 'Thread t9' });
    await sidebarCommand(client, native, files, 'legacy-rename', id, ' Renamed ');
    expect(fleetRequests(ops).at(-1)!.payload).toMatchObject({ type: 'thread.metadata.update', threadId: 't9', title: 'Renamed' });
    await sidebarCommand(client, native, files, 'legacy-archive', id, '');
    expect(fleetRequests(ops).at(-1)!.payload).toMatchObject({ type: 'thread.archive', threadId: 't9' });
    expect(sidebarSession(client).undo).toMatchObject({ action: 'Archived', threadIds: [id] });
    await sidebarCommand(client, native, files, 'undo', '', '', Date.parse(at(1)));
    await sidebarCommand(client, native, files, 'legacy-thread-menu', id, '');
    expect(sidebarSession(client).dialog).toEqual({ kind: 'delete', threadIds: [id], title: 'Thread t9' });
    await sidebarCommand(client, native, files, 'dialog-confirm', '', '');
    expect(sidebarSession(client).dialog).toMatchObject({ kind: 'delete-worktree', threadIds: [id], title: 'wt-9' });
    await sidebarCommand(client, native, files, 'dialog-confirm', '', '');
    expect(fleetRequests(ops).slice(-4).map(request => [request.method, request.payload.type ?? request.payload.path ?? ''])).toEqual([
      ['terminal.close', ''], ['orchestration.dispatchCommand', 'thread.delete'], ['vcs.removeWorktree', '/srv/.t3/wt-9'], ['vcs.refreshStatus', '']]);
  });
  test('dblclick renames a background row; Project settings on a background project focuses its environment first', async () => {
    const { client, native, ops } = rig({ picks: ['project-settings'] });
    background([thread('t9', { projectId: 'r1' })]);
    await sidebarLocal(client, native, 'legacy-rename-start', 'fleet:env-remote:t9', '');
    expect(sidebarSession(client)).toMatchObject({ renameId: 'fleet:env-remote:t9', renameTitle: 'Thread t9' });
    expect(await sidebarCommand(client, native, files, 'legacy-project-menu', 'env-remote:/srv/solo', '')).toBe('sidebar:navigate');
    expect(opsOf(ops, 'connect')).toHaveLength(1);
    expect(sidebarSession(client).navigate).toEqual({ kind: 'project-settings', projectId: 'r1' });
    void sidebarPrefs;
  });
});

// The review of PR #469: the row that opened its thread kept the focus (so Archive stayed shown), a tooltip trigger that
// went away under the pointer left its row hovered, Confirm ended when the focus or the pointer moved within the row,
// the toast's destructive tone, and the drop and port edge cases.
describe('review: a thread change focuses the composer (ChatView\'s effect on activeThread.id)', () => {
  const select = async (client: T3Client, native: Native, id: string) => {
    const out = { message: '', id: '', value: '' } as OpOut;
    await threadOps.call(client, 'select-thread', id, 'click', 0, native, files, out);
    return out.message;
  };
  test('a row press that opens another thread focuses the composer; the open thread\'s own row and a ⌘-click do not', async () => {
    const { client, native } = rig();
    expect(await select(client, native, 'thread-2')).toBe('focus:composer');
    expect(await select(client, native, 'thread-2')).toBe(''); // no thread change: the focus stays on the row
    const held = rig({ modifiers: { command: true } });
    expect(await select(held.client, held.native, 'thread-2')).toBe(''); // a selection toggle opens nothing
    expect(await select(client, native, 'gone')).toBe(''); // a thread the shell does not hold: the empty state, no composer
  });
  test('not while the opened thread\'s terminal drawer is open, which owns the keys', async () => {
    const { client, native } = rig();
    terminalUiStore(client).getState().setTerminalOpen({ environmentId: 'local', threadId: 'thread-2' }, true);
    expect(await select(client, native, 'thread-2')).toBe('');
  });
  test('the pull request glyph and the port button say so with `sidebar:opened:focus`', async () => {
    const { client, native } = rig();
    terminalUiStore(client).getState().setTerminalOpen({ environmentId: 'local', threadId: 'thread-2' }, true);
    expect(await sidebarCommand(client, native, files, 'legacy-pr', 'thread-2', '')).toBe('sidebar:opened');
    const app = readFileSync(new URL('./app.contract', import.meta.url), 'utf8');
    expect(app).toContain('if result.message.startsWith("sidebar:opened")');
    expect(app).toContain('if result.message == "sidebar:opened:focus"\n            focus("composer")');
  });
});

describe('review: a row\'s hover and focus as the host reports them (LegacyThread, LegacyProject run)', () => {
  const clock = { now: 1000 };
  const tips: unknown[][] = [];
  // The host's functions and the hover layer's (T3Window `hoverTipAt`; `legacyTip` and `legacyTipOff` are the file's fns).
  const host = (runs: unknown[][] = []): Obj => ({ performanceNow: () => clock.now, preventDefault() {}, stopPropagation() {}, run: (...args: unknown[]) => { runs.push(args); }, open() {}, local() {},
    hoverTipAt: (...args: unknown[]) => { tips.push(args); }, rem: 16, frame: (id: string) => ({ id, x: 0, y: 0, width: 12, height: 12 }),
    legacyTip: (text: string, side: string, align: string, at: Obj, rem: number, testId: string) => ({ key: 'legacy-tip', kind: 'tooltip', text, side, align, at, testId }),
    legacyTipOff: () => ({ key: 'legacy-tip' }) });
  const row = (t: Obj = {}, confirmArchive = true, runs: unknown[][] = []) => mount('LegacyThread', { reduced: false, page: '', busy: false, legacy: { confirmArchive },
    t: { id: 't1', title: 'Fix', active: false, selected: false, running: false, renaming: false, prIcon: 'git-pull-request', portLabel: '', worktree: false, terminalCount: 0, ...t } }, host(runs));
  // A tooltip trigger of `owner` (LegacyRowTip), its relay into the owner's `innerHover`.
  const trigger = (owner: Mounted, label: string) => mount('LegacyRowTip', { label, side: 'top', align: 'center', tipId: `tip-${label}`, shown: true, offset: 16,
    relay: (value: boolean) => owner.act('innerHover', value) }, host());
  // The host hovers one node at a time: moving onto a trigger leaves the row and enters the trigger at one instant.
  const ontoTrigger = (run: Mounted) => { run.act('hover', false); run.act('innerHover', true); };
  // After a batch the host hit-tests the resting pointer (MouseChainMac `hoverUnderPointer`): the node it had hovered hears
  // its leave only while that node still exists (Presenter `send` drops one for a destroyed node). `at` is the trigger's
  // line in the owner's view; its `when` arms say whether the batch destroyed it.
  const relayout = (owner: Mounted, at: string, tip: Mounted) => { if (owner.drawn(at)) tip.act('setOver', false); };
  const archiveAt = 'LegacyRowTip(label=(legacy.confirmArchive', statusAt = 'LegacyRowTip(label=p.statusLabel';

  test('the row that opened its thread shows its time again once the composer took the focus and the pointer left', () => {
    const run = row();
    run.act('hover', true);
    run.act('within', 'row', true); // a mouse press makes the row's button first responder
    expect(run.view.revealed).toBe(true);
    run.act('within', 'row', false); // the composer takes the focus
    clock.now += 100;
    run.act('hover', false);
    expect(run.view.revealed).toBe(false);
  });
  test('a collapsed project\'s status trigger that goes as a press expands it leaves the header unhovered after the pointer leaves', () => {
    const header = mount('LegacyProject', { reduced: false, p: { key: 'k', expanded: false, statusLabel: 'Working' }, legacy: {}, now: 0, page: '', busy: false }, host());
    const status = trigger(header, 'Working');
    header.act('hover', true);
    header.act('hover', false); status.act('setOver', true);
    expect(header.view.pointed).toBe(true);
    // The press expands the project; the host's hit-test after the layout hovers the header itself again.
    header.props.p = { ...(header.props.p as Obj), expanded: true };
    relayout(header, statusAt, status);
    header.act('hover', true);
    header.act('hover', false);
    expect(header.view.pointed).toBe(false);
    expect(header.view.revealed).toBe(false);
  });
  test('Archive\'s trigger replaced by Confirm, then the pointer leaving: no hover, no Confirm, the time shown', () => {
    const runs: unknown[][] = [], run = row({}, true, runs), archive = trigger(run, '');
    run.act('hover', true);
    run.act('hover', false); archive.act('setOver', true); // Archive's tooltip trigger
    run.act('within', 'archive', true);
    run.act('archive');
    run.act('within', 'confirm', true); // autofocus
    expect(run.view.confirmShown).toBe(true);
    relayout(run, archiveAt, archive);
    run.act('hover', true); // the host's hit-test after the layout: Confirm has no hover handler, the row has
    expect(run.view.inner).toBe(false);
    clock.now += 100;
    run.act('hover', false);
    expect(run.view.pointed).toBe(false);
    expect(run.view.confirmShown).toBe(false); // handleMouseLeave
    expect(run.view.revealed).toBe(false); // Confirm went with the focus: no control of the row holds it
    clock.now += 500;
    run.act('hover', true); // back later: the confirmation stays ended
    expect(run.view.confirming).toBe(false);
    expect(runs).toEqual([]);
  });
  test('the pointer moving from Confirm onto the title, a trigger, keeps the confirmation; leaving the row from there ends it', () => {
    const run = row();
    run.act('hover', true);
    run.act('archive');
    clock.now += 100;
    ontoTrigger(run); // the title's tooltip trigger, within the row
    expect(run.view.confirmShown).toBe(true);
    clock.now += 100;
    run.act('innerHover', false); // out of the row from the title
    expect(run.view.confirmShown).toBe(false);
  });
  test('handleBlurCapture: the focus moving to another control of the row keeps Confirm; leaving the row ends it', () => {
    const run = row();
    run.act('within', 'archive', true);
    run.act('archive');
    run.act('within', 'confirm', true);
    run.act('within', 'confirm', false); run.act('within', 'pr', true); // ⇧Tab to the pull request link, one instant
    expect(run.view.confirmShown).toBe(true);
    run.act('within', 'pr', false); // Tab on, out of the row
    expect(run.view.confirmShown).toBe(false);
    clock.now += 100;
    run.act('within', 'row', true); // the focus comes back later: the confirmation stays ended
    expect(run.view.confirming).toBe(false);
    expect(run.view.revealed).toBe(true);
  });
  test('a focused control that goes away (the port\'s server stops, the thread starts running, a rename) counts as left', () => {
    const port = row({ portLabel: 'Open localhost:5173' });
    port.act('within', 'port', true);
    expect(port.view.inside).toBe(true);
    port.props.t = { ...(port.props.t as Obj), portLabel: '' };
    expect(port.view.inside).toBe(false);
    const renamed = row();
    renamed.act('within', 'row', true);
    renamed.props.t = { ...(renamed.props.t as Obj), renaming: true };
    renamed.act('within', 'rename', true); // the field's autofocus
    renamed.props.t = { ...(renamed.props.t as Obj), renaming: false }; // Enter saved it; the field went with no blur
    expect(renamed.view.inside).toBe(false);
    expect(renamed.view.revealed).toBe(false);
  });
  test('without confirmation Archive archives at once', () => {
    const runs: unknown[][] = [], run = row({}, false, runs);
    run.act('archive');
    expect(runs).toEqual([['legacy-archive', 't1', '']]);
    expect(run.view.confirmShown).toBe(false);
  });

  // Review round 2 (B2): the row re-sorts in the batch that takes its trigger away, so the host hovers another row and
  // never this one again; only the trigger's own leave can let go of the pointer.
  test('review 2: the pointer on Archive as a new message runs the thread and re-sorts it to the top: the row lets go', () => {
    const run = row(), archive = trigger(run, 'Archive');
    run.act('hover', true);
    run.act('hover', false); archive.act('setOver', true);
    expect(run.view.pointed).toBe(true);
    run.props.t = { ...(run.props.t as Obj), running: true }; // and the row moved: another row is under the pointer now
    relayout(run, archiveAt, archive);
    expect(run.view.pointed).toBe(false); // not painted hovered while it runs
    run.props.t = { ...(run.props.t as Obj), running: false };
    expect(run.view.revealed).toBe(false); // the run over: its time, not Archive, until the pointer crosses it
  });
  test('review 2: a collapsed project\'s status trigger that goes as the projects re-sort lets go of the header', () => {
    const header = mount('LegacyProject', { reduced: false, p: { key: 'k', expanded: false, statusLabel: 'Working' }, legacy: {}, now: 0, page: '', busy: false }, host());
    const status = trigger(header, 'Working');
    header.act('hover', true);
    header.act('hover', false); status.act('setOver', true);
    header.props.p = { ...(header.props.p as Obj), statusLabel: '' }; // its run ended; the project moved down the list
    relayout(header, statusAt, status);
    expect(header.view.pointed).toBe(false);
    expect(header.view.revealed).toBe(false);
  });
  test('review 2: every trigger of a row and a header keeps its box (display none) while it is not drawn', () => {
    const view = readFileSync(new URL('./legacy-sidebar.contract', import.meta.url), 'utf8');
    const thread = view.slice(view.indexOf('component LegacyThread'), view.indexOf('// A legacy row\'s or project header\'s tooltip trigger'));
    const project = view.slice(view.indexOf('component LegacyProject'), view.indexOf('// SidebarThreadRow'));
    const shown = [...`${thread}${project}`.matchAll(/LegacyRowTip\([^\n]*shown=([^\n]*?), relay=innerHover\)\n( *)if (.+)/g)].map(m => [m[1], m[3]]);
    expect(shown).toEqual([['(t.prIcon != "")', 't.prIcon != ""'], ['(t.portLabel != "")', 't.portLabel != ""'], ['t.worktree', 't.worktree'],
      ['(t.terminalCount > 0)', 't.terminalCount > 0'], ['(not confirmShown and not t.running)', 'not confirmShown and not t.running'],
      ['(not p.expanded and p.statusLabel != "")', 'not p.expanded and p.statusLabel != ""']]);
    expect(view).toContain('box id=tipId testId=tipId position="relative" hover=setOver flex-shrink=0 display=(shown ? "flex" : "none")');
    // Each trigger's box is drawn whatever its trigger's state: only the row (a rename) or the row's going takes it.
    const run = row({ prIcon: '', running: true, terminalCount: 0 });
    run.act('archive');
    for (const at of ['tipId=`legacy-tip-pr-', 'tipId=`legacy-tip-port-', 'tipId=`legacy-tip-worktree-', 'tipId=`legacy-tip-terminal-', 'tipId=`legacy-tip-archive-']) expect(run.drawn(at)).toBe(true);
  });
  test('review 2: a double-click on the pull request link after its first click moved the focus to the composer: the row is not focused', () => {
    const run = row();
    run.act('hover', true);
    run.act('within', 'pr', true); // the first mousedown focuses the link
    run.act('within', 'pr', false); // its click opened the thread; `sidebar:opened:focus` gave the composer the focus
    run.act('nested', 'pr'); // the second click's dblclick, on the link
    clock.now += 100;
    run.act('hover', false);
    expect(run.view.inside).toBe(false);
    expect(run.view.revealed).toBe(false); // its time, not Archive
    const view = readFileSync(new URL('./legacy-sidebar.contract', import.meta.url), 'utf8');
    expect(view.match(/dblclick=nested /g) ?? []).toHaveLength(2); // the pull request and the port: no rename under them
  });
});

describe('review: LS-1 on a background environment\'s row', () => {
  const remoteProject = { id: 'r2', title: 'app', workspaceRoot: '/srv/app', repositoryIdentity: repo, createdAt: at(0), updatedAt: at(0) };
  const reading = (entry: FleetEntry) => { entry.config = { environment: { label: 'Studio', capabilities: { pullRequests: true, threadPullRequests: true } }, settings: {} }; return entry; };
  test('beside a thread, the pull request opens in that thread\'s panel by its key (prTarget over its environment\'s projects), then that environment becomes the focus', async () => {
    const { client, native, ops } = rig();
    reading(background([thread('t9', { projectId: 'r2', pullRequests: linked() })], [remoteProject]));
    expect(await sidebarCommand(client, native, files, 'legacy-pr', 'fleet:env-remote:t9', '')).toBe('sidebar:opened:focus');
    expect(surfaceStore(client).panels.get('env-remote:t9')).toMatchObject({ visible: true, active: 'pull-request:r2:github.com:acme%2Fapp:7' });
    expect(surfaceStore(client).panels.get('local:thread-1')).toBeUndefined();
    expect(opsOf(ops, 'connect').at(-1)).toMatchObject({ origin: 'http://studio.local:3773' });
    expect(client.local.selections['env-remote']).toEqual({ projectId: 'r2', threadId: 't9' });
  });
  test('on a page, a pull request only a background environment reads opens on the Pull Requests page named with that environment', async () => {
    const { client, native, ops } = rig({ caps: { pullRequests: false } });
    reading(background([thread('t9', { projectId: 'r2', pullRequests: linked() })], [remoteProject]));
    const result = await sidebarCommand(client, native, files, 'legacy-pr', 'fleet:env-remote:t9', 'pull-requests');
    expect(result.startsWith('sidebar:pr-page:')).toBe(true);
    expect(JSON.parse(result.slice(16))).toEqual({ projectId: 'r2', host: 'github.com', repository: 'acme/app', number: 7, environmentId: 'env-remote' });
    expect(opsOf(ops, 'connect')).toEqual([]); // the page itself picks the environment: no focus switch here
  });
});

describe('review: drops and port opens whose thread did not open (handleThreadFileDrop, handleOpenDiscoveredPort)', () => {
  test('a drop queued for a thread that is not open goes once another thread opens; one whose thread opens is taken', async () => {
    const { client, native, ops } = rig({ threadId: 'thread-1', presentation: { rowDrops: ['thread-3'] } });
    expect(await takeRowDrops(client, native)).toBe(false); // its press is still on the way
    expect(opsOf(ops, 'rowDropClear')).toEqual([]);
    client.threadId = 'thread-2'; // the press found the app busy, or another thread opened first
    expect(await takeRowDrops(client, native)).toBe(false);
    expect(opsOf(ops, 'rowDropClear').map(op => op.keys)).toEqual([['thread-3']]);
    const landed = rig({ threadId: 'thread-1', presentation: { rowDrops: ['thread-3'] } });
    await takeRowDrops(landed.client, landed.native);
    landed.client.threadId = 'thread-3';
    await takeRowDrops(landed.client, landed.native);
    expect(opsOf(landed.ops, 'rowDropClear')).toEqual([]);
    expect(opsOf(landed.ops, 'rowDropTake').map(op => op.keys)).toEqual([['thread-3', 'fleet:local:thread-3']]);
  });
  test('a press whose thread fails to open lets the row\'s drop go', async () => {
    const { client, native, ops } = rig({ presentation: { rowDrops: ['fleet:env-gone:t1'] } });
    const out = { message: '', id: '', value: '' } as OpOut;
    await expect(threadOps.call(client, 'select-thread', 'fleet:env-gone:t1', 'click', 0, native, files, out)).rejects.toThrow('That environment is no longer connected.');
    expect(opsOf(ops, 'rowDropClear').map(op => op.keys)).toEqual([['fleet:env-gone:t1']]);
  });
  test('the open thread\'s own row only queues (no press); its drop is taken at the next refresh', () => {
    const view = readFileSync(new URL('./legacy-sidebar.contract', import.meta.url), 'utf8');
    expect(view).toContain('data-thread-key=t.id data-thread-open=(isOpen ? "1" : "0")');
    expect(JSON.parse(readFileSync(new URL('./app.json', import.meta.url), 'utf8')).data).toContain('thread-open');
    const swift = readFileSync(new URL('./modules/apple/T3RowDrop.swift', import.meta.url), 'utf8');
    expect(swift).toContain('self?.drop(key, urls, open: element?.data[.threadOpen] == "1") { element?.click() }');
  });
  test('a port whose environment fails to become the focus is not opened later', async () => {
    const { client, native, rpcs } = rig();
    const entry = background([thread('t9', { projectId: 'r1' })]);
    await watchRowPorts(client, native);
    await fleetPortsPass(async () => ({ id: 'sub-7' }), entry);
    fleetPortsEvent(entry, { key: SIDEBAR_DISCOVERED_SERVERS_KEY, generation: 3, subscriptionId: 'sub-7', value: { servers: [{ host: '127.0.0.1', port: 3000, url: 'http://localhost:3000/', processName: 'node', pid: 7, terminal: { threadId: 't9', terminalId: 't' } }] } });
    native.later = (original => async (request: unknown) => obj(request).op === 'connect' ? { ok: false, generation: 0, error: { kind: 'Connect', message: 'Studio refused.', uncertain: false } } : original(request))(native.later.bind(native));
    await expect(sidebarCommand(client, native, files, 'legacy-port', 'fleet:env-remote:t9', '')).rejects.toThrow('Studio refused.');
    client.environmentId = 'env-remote'; client.threadId = 't9'; // that environment became the focus later, by other means
    await openPendingPort(client, native);
    expect(rpcs.some(call => call.method === 'preview.open')).toBe(false);
  });
});
