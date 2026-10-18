// The legacy sidebar's remaining reference behaviour (T3 Code 1e2ecbd975, MIT, see
// LICENSE-T3): a row's pull request link (handlePrClick, useOpenPrLink), the discovered-port button
// (useThreadDiscoveredPorts, openDiscoveredPort), files dropped on a row (sidebarPendingFileDropStore and
// ChatView's consumePendingFileDrop), Archive and New thread in the tab order (group-focus-within),
// "New thread on <branch>" (handleNewThread with the branch), a background environment's project and thread
// actions and environmentGrouping.test.ts's physical dedupe cases.
import { afterEach, describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import { ClientError, type Files, type Native } from '../../src/client/protocol';
import { toasts } from '../../src/shell/toast';
import { fleet, type FleetEntry } from '../../src/connections/settings-b-fleet';
import { sidebarPrefs, sidebarSession } from '../../src/sidebar/sidebar-state';
import { sidebarCommand, sidebarLocal, sidebarSelecting } from '../../src/sidebar/sidebar-commands';
import { surfaceStore } from '../../src/panel/panel-surfaces';
import { SIDEBAR_DISCOVERED_SERVERS_KEY, discoveredServersEvent } from '../../src/browser/browser-targets';
import { draftContext } from '../../src/composer/composer-controls-branch';
import { draftFiles } from '../../src/composer/composer-editor-files';
import { browserHistory } from '../../src/browser/browser-history';
import { legacyGroups, legacySession, legacySidebarSnapshot } from '../../src/sidebar/legacy-sidebar-view';
import { fleetPortsEvent, fleetPortsPass, openPendingPort, portLabels, rowPorts, watchRowPorts } from '../../src/sidebar/legacy-sidebar-ports';
import { clearRowDrops, takeRowDrops } from '../../src/sidebar/legacy-sidebar-drop';
import { threadOps } from '../../src/client/client-ops-threads';
import type { OpOut } from '../../src/client/client-ops';
import { terminalUiStore } from '../../src/terminal/terminal-ui-state';
import { contractComponent, value, type Env } from '../support/contract-action-fixture';
import { appPath } from '../support/app-files';

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
function rig(options: { threads?: Obj[]; projects?: Obj[]; threadId?: string; settings?: Obj; failOpen?: boolean; caps?: Obj; presentation?: Obj; editorApplies?: boolean } = {}): Rig {
  const client = new T3Client(), ops: Obj[] = [], rpcs: { method: string; payload: Obj }[] = [], opened: string[] = [], drafts: string[] = [];
  let ids = 0;
  Object.assign(client, { loaded: true, connection: 'connected', origin: 'http://127.0.0.1:16562', configLive: true, shellLive: true, threadLive: true, scopes: ['orchestration:read', 'orchestration:operate'], presentation: options.presentation ?? {} });
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

describe('the pull request glyph (handlePrClick)', () => {
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
    // `mod:`: the click's own MouseEvent held ⌘ or Ctrl (legacy-sidebar.contract prPress).
    const held = rig();
    expect(await sidebarCommand(held.client, held.native, files, 'legacy-pr', 'thread-2', 'mod:')).toBe('');
    expect(opsOf(held.ops, 'sidebarModifiers')).toEqual([]);
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
    const view = readFileSync(appPath('legacy-sidebar.contract'), 'utf8');
    expect(view).toContain('button cursor="pointer" press=prPress dblclick=nested focus=within("pr", true)');
    expect(view).toContain('  action prPress(e: MouseEvent)\n    run("legacy-pr", t.id, (e.metaKey or e.ctrlKey) ? `mod:${page}` : page)');
  });
});

describe('the discovered-port button (useThreadDiscoveredPorts, openDiscoveredPort)', () => {
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

describe('the discovered-port button on a background environment: its own stream, and its thread opens before the tab', () => {
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
    // The row is a native button, a leaf for VoiceOver: the machine glyph's label is part of its name, as the
    // reference's row (named from its content) reads it.
    const row = legacySidebarSnapshot(client, Date.parse(at(30)), { projectIdentity: identity }).projects.flatMap(project => project.threads).find(row => row.id === 'fleet:env-remote:t9')!;
    expect(row.label).toBe(`${row.title}, Studio`);
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

describe('the row\'s tooltips', () => {
  const view = readFileSync(appPath('legacy-sidebar.contract'), 'utf8');

  // The host hovers only the nearest node with a hover handler (MouseChainMac `hoverUnderPointer`), so a pointer entering
  // a row on its title would hover the title's tooltip and not the row: Archive stayed hidden and a press there opened the
  // thread. Every tooltip trigger inside a row tells the row, as group-hover holds for the whole row.

  // The thread list clips its rows (overflow hidden, the reference's, for autoAnimate), which cut a bubble
  // drawn beside each project's first row. The reference portals every TooltipPopup; the window's hover layer is the clone's.
  test('the row\'s and the header\'s tooltips are drawn by the window\'s hover layer, so the list\'s clip does not cut them', () => {
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
  test('the row or header the pointer is on closes a legacy tooltip a gone trigger left; Archive\'s press and a right-click close their own', () => {
    const tips: unknown[][] = [], runs: unknown[][] = [], focused: string[] = [], menus: unknown[][] = [];
    const host = { performanceNow: () => 0, run: (...args: unknown[]) => { runs.push(args); }, hoverTipAt: (...args: unknown[]) => { tips.push(args); }, legacyTipOff: () => ({ key: 'legacy-tip' }),
      focus: (id: string) => { focused.push(id); }, menu: (...args: unknown[]) => { menus.push(args); } };
    const row = mount('LegacyThread', { reduced: false, page: '', busy: false, legacy: { confirmArchive: false }, t: { id: 't1', running: false, renaming: false, prIcon: '', portLabel: '' } }, host);
    row.act('hover', true);
    row.act('archive');
    const header = mount('LegacyProject', { reduced: false, p: { key: 'k', expanded: true, statusLabel: '' }, legacy: {}, now: 0, page: '', busy: false }, host);
    header.act('hover', true);
    row.act('openMenu');
    header.act('openMenu');
    expect(tips).toEqual(Array.from({ length: 5 }, () => [{ key: 'legacy-tip' }, 'trigger', false]));
    // The right press names the row or header for its context popover; a row's also clears a selection it is not in.
    expect(menus).toEqual([['t1'], ['k']]);
    expect(runs).toEqual([['legacy-archive', 't1', ''], ['legacy-thread-menu', 't1', '']]);
    // A right press focuses its row or header, as Chromium's does in the reference.
    expect(focused).toEqual(['thread-t1', 'legacy-project-toggle-k']);
    const view = readFileSync(appPath('legacy-sidebar.contract'), 'utf8');
    expect(view.match(/contextmenu=openMenu contextPopover="legacy-(thread|project)-menu" /g) ?? []).toHaveLength(2);
  });
  test('the worktree tooltip names the worktree by its last segment (formatWorktreePathForDisplay), then the branch', () => {
    const { client } = rig({ threads: [thread('thread-1'), thread('wt', { worktreePath: '/work/.t3/worktrees/app-feature', branch: 'feature/x' }), thread('plain', { worktreePath: '/work/.t3/worktrees/solo' })] });
    const rows = legacySidebarSnapshot(client, Date.parse(at(30)), { projectIdentity: identity }).projects[0]!.threads;
    expect(rows.find(row => row.id === 'wt')?.worktreeLabel).toBe('Worktree: app-feature (feature/x)');
    expect(rows.find(row => row.id === 'plain')?.worktreeLabel).toBe('Worktree: solo');
  });
});

describe('list motion and the destructive "Delete anyway"', () => {
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
});

describe('files dropped on a row (consumePendingFileDrop, addDroppedFiles)', () => {
  test('once the dropped-on thread is open, its queued files join its composer: images to the shelf, files as chips', async () => {
    const queued = { rowDrops: ['fleet:local:thread-9', 'fleet:local:thread-2'] };
    const { client, native, ops } = rig({ threadId: 'thread-2', presentation: queued });
    const staged = [{ kind: 'file', id: '11111111-1111-4111-8111-111111111111', name: 'notes.txt', sizeBytes: 5, mimeType: 'text/plain' },
      { kind: 'image', id: '22222222-2222-4222-8222-222222222222', name: 'shot.png', sizeBytes: 64 }, { kind: 'unsupported-image', name: 'raw.tiff', sizeBytes: 9 }];
    native.later = (original => async (request: unknown) => obj(request).op === 'rowDropTake' ? (ops.push(obj(request)), { ok: true, generation: Number(obj(request).generation ?? 0), value: { files: staged } }) : original(request))(native.later.bind(native));
    expect(await takeRowDrops(client, native)).toBe(true);
    expect(opsOf(ops, 'rowDropTake')[0]).toMatchObject({ keys: ['fleet:local:thread-2'], imageMaxEdge: 2000 });
    expect(draftFiles(client.local).map(file => [file.name, file.draftKey])).toEqual([['notes.txt', 'local:thread-2']]);
    expect(client.local.snapshotDrafts['local:thread-2']?.map(image => image.name)).toEqual(['shot.png']);
    expect(opsOf(ops, 'editorInsert')[0]).toMatchObject({ owner: client.snapshotOwner });
    expect(client.local.drafts['local:thread-2']).toContain('notes.txt', 'another thread\'s editor refused the chips: they join the stored draft');
    expect(client.error).toBe("'raw.tiff' is not a supported image type. Attach GIF, HEIC, HEIF, JPEG, PNG, or WebP images.");
  });
  test('nothing queued for the open thread asks the module nothing', async () => {
    const { client, native, ops } = rig({ threadId: 'thread-1', presentation: { rowDrops: ['fleet:local:thread-2'] } });
    expect(await takeRowDrops(client, native)).toBe(false);
    expect(opsOf(ops, 'rowDropTake')).toEqual([]);
  });
});

describe('"New thread on <branch>" carries the branch (handleNewThread with branch, worktreePath, envMode)', () => {
  test('a worktree thread\'s draft reuses its worktree; a plain one takes its branch on the local checkout', async () => {
    const { client, native, drafts } = rig({ threads: [thread('thread-1'), thread('wt', { branch: 'feature/x', worktreePath: '/work/.t3/wt' }), thread('local', { branch: 'fix/y' })] });
    expect(await sidebarCommand(client, native, files, 'legacy-thread-choice', 'wt', 'new-thread-on-branch')).toBe('sidebar:new-thread');
    expect(drafts).toEqual(['p1']);
    expect(draftContext(client)).toEqual({ envMode: 'worktree', branch: 'feature/x', worktreePath: '/work/.t3/wt' });
    expect(await sidebarCommand(client, native, files, 'legacy-thread-choice', 'local', 'new-thread-on-branch')).toBe('sidebar:new-thread');
    expect(draftContext(client)).toEqual({ envMode: 'local', branch: 'fix/y', worktreePath: '' });
  });
});

describe('a background environment\'s rows act on that environment', () => {
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
    const { client, native, ops } = rig();
    background([thread('t9', { projectId: 'r1' })]);
    const key = 'env-remote:/srv/solo';
    await sidebarCommand(client, native, files, 'legacy-project-choice', key, 'rename:env-remote:/srv/solo');
    expect(legacySidebarSnapshot(client, Date.parse(at(30)), { projectIdentity: identity })).toMatchObject({ dialog: 'rename-project', dialogTarget: 'fleet:env-remote:r1', dialogDetail: 'Environment: Studio' });
    await sidebarCommand(client, native, files, 'legacy-rename-project', 'fleet:env-remote:r1', 'Solo 2');
    expect(fleetRequests(ops).map(request => [request.method, request.payload.type, request.payload.projectId, request.payload.title])).toEqual([['projects.mutate', 'project.update', 'r1', 'Solo 2']]);
    expect(legacySession(client).dialog.kind).toBe('');
    await sidebarCommand(client, native, files, 'legacy-project-choice', key, 'delete:env-remote:/srv/solo');
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
    const { client, native, ops } = rig();
    background([thread('t9', { projectId: 'r1', worktreePath: '/srv/.t3/wt-9', branch: 'feat' })]);
    const id = 'fleet:env-remote:t9';
    for (const choice of ['mark-unread', 'copy-path', 'copy-thread-id']) await sidebarCommand(client, native, files, 'legacy-thread-choice', id, choice);
    expect(fleetRequests(ops).map(request => [request.payload.type, request.payload.threadId])).toEqual([['thread.mark-unread', 't9']]);
    expect(opsOf(ops, 'copyText').map(op => op.text)).toEqual(['/srv/.t3/wt-9', 't9']);
    await sidebarCommand(client, native, files, 'legacy-thread-choice', id, 'rename');
    expect(sidebarSession(client)).toMatchObject({ renameId: id, renameTitle: 'Thread t9' });
    await sidebarCommand(client, native, files, 'legacy-rename', id, ' Renamed ');
    expect(fleetRequests(ops).at(-1)!.payload).toMatchObject({ type: 'thread.metadata.update', threadId: 't9', title: 'Renamed' });
    await sidebarCommand(client, native, files, 'legacy-archive', id, '');
    expect(fleetRequests(ops).at(-1)!.payload).toMatchObject({ type: 'thread.archive', threadId: 't9' });
    expect(sidebarSession(client).undo).toMatchObject({ action: 'Archived', threadIds: [id] });
    await sidebarCommand(client, native, files, 'undo', '', '', Date.parse(at(1)));
    await sidebarCommand(client, native, files, 'legacy-thread-choice', id, 'delete');
    expect(sidebarSession(client).dialog).toEqual({ kind: 'delete', threadIds: [id], title: 'Thread t9' });
    await sidebarCommand(client, native, files, 'dialog-confirm', '', '');
    expect(sidebarSession(client).dialog).toMatchObject({ kind: 'delete-worktree', threadIds: [id], title: 'wt-9' });
    await sidebarCommand(client, native, files, 'dialog-confirm', '', '');
    expect(fleetRequests(ops).slice(-4).map(request => [request.method, request.payload.type ?? request.payload.path ?? ''])).toEqual([
      ['terminal.close', ''], ['orchestration.dispatchCommand', 'thread.delete'], ['vcs.removeWorktree', '/srv/.t3/wt-9'], ['vcs.refreshStatus', '']]);
  });
  test('dblclick renames a background row; Project settings on a background project keeps the focused environment', async () => {
    const { client, native, ops } = rig();
    background([thread('t9', { projectId: 'r1' })]);
    await sidebarLocal(client, native, 'legacy-rename-start', 'fleet:env-remote:t9', '');
    expect(sidebarSession(client)).toMatchObject({ renameId: 'fleet:env-remote:t9', renameTitle: 'Thread t9' });
    // The reference navigates to /projects/$projectKey: Settings › Projects finds the member among every environment's groups.
    expect(await sidebarCommand(client, native, files, 'legacy-project-choice', 'env-remote:/srv/solo', 'project-settings')).toBe('sidebar:project-settings:fleet:env-remote:r1');
    expect(opsOf(ops, 'connect')).toHaveLength(0);
    expect(opsOf(ops, 'fleetStop')).toHaveLength(0);
    void sidebarPrefs;
  });
});

// The review of PR #469: the row that opened its thread kept the focus (so Archive stayed shown), a tooltip trigger that
// went away under the pointer left its row hovered, Confirm ended when the focus or the pointer moved within the row,
// the toast's destructive tone, and the drop and port edge cases.
describe('a thread change focuses the composer (ChatView\'s effect on activeThread.id)', () => {
  const select = async (client: T3Client, native: Native, id: string, how = 'click') => {
    const out = { message: '', id: '', value: '' } as OpOut;
    await threadOps.call(client, 'select-thread', id, how, 0, native, files, out);
    return out.message;
  };
  test('a row press that opens another thread focuses the composer; the open thread\'s own row and a ⌘-click do not', async () => {
    const { client, native } = rig();
    expect(await select(client, native, 'thread-2')).toBe('focus:composer');
    expect(await select(client, native, 'thread-2')).toBe(''); // no thread change: the focus stays on the row
    const held = rig();
    expect(await select(held.client, held.native, 'thread-2', 'click:meta')).toBe(''); // a selection toggle opens nothing
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
    const app = readFileSync(appPath('app.contract'), 'utf8');
    expect(app).toContain('if result.message.startsWith("sidebar:opened")');
    expect(app).toContain('if result.message == "sidebar:opened:focus"\n            focus("composer")');
  });
});

describe('a row\'s hover and focus as the host reports them (LegacyThread, LegacyProject run)', () => {
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

  // The row re-sorts in the batch that takes its trigger away, so the host hovers another row and
  // never this one again; only the trigger's own leave can let go of the pointer.
  test('the pointer on Archive as a new message runs the thread and re-sorts it to the top: the row lets go', () => {
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
  test('a collapsed project\'s status trigger that goes as the projects re-sort lets go of the header', () => {
    const header = mount('LegacyProject', { reduced: false, p: { key: 'k', expanded: false, statusLabel: 'Working' }, legacy: {}, now: 0, page: '', busy: false }, host());
    const status = trigger(header, 'Working');
    header.act('hover', true);
    header.act('hover', false); status.act('setOver', true);
    header.props.p = { ...(header.props.p as Obj), statusLabel: '' }; // its run ended; the project moved down the list
    relayout(header, statusAt, status);
    expect(header.view.pointed).toBe(false);
    expect(header.view.revealed).toBe(false);
  });
  test('every trigger of a row and a header keeps its box (display none) while it is not drawn', () => {
    const view = readFileSync(appPath('legacy-sidebar.contract'), 'utf8');
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
  test('a double-click on the pull request link after its first click moved the focus to the composer: the row is not focused', () => {
    const run = row();
    run.act('hover', true);
    run.act('within', 'pr', true); // the first mousedown focuses the link
    run.act('within', 'pr', false); // its click opened the thread; `sidebar:opened:focus` gave the composer the focus
    run.act('nested', 'pr'); // the second click's dblclick, on the link
    clock.now += 100;
    run.act('hover', false);
    expect(run.view.inside).toBe(false);
    expect(run.view.revealed).toBe(false); // its time, not Archive
    const view = readFileSync(appPath('legacy-sidebar.contract'), 'utf8');
    expect(view.match(/dblclick=nested /g) ?? []).toHaveLength(2); // the pull request and the port: no rename under them
  });
});

describe('the pull request glyph on a background environment\'s row', () => {
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

describe('drops and port opens whose thread did not open (handleThreadFileDrop, handleOpenDiscoveredPort)', () => {
  test('a drop queued for a thread that is not open goes once another thread opens; one whose thread opens is taken', async () => {
    const { client, native, ops } = rig({ threadId: 'thread-1', presentation: { rowDrops: ['fleet:local:thread-3'] } });
    expect(await takeRowDrops(client, native)).toBe(false); // its press is still on the way
    expect(opsOf(ops, 'rowDropClear')).toEqual([]);
    client.threadId = 'thread-2'; // the press found the app busy, or another thread opened first
    expect(await takeRowDrops(client, native)).toBe(false);
    expect(opsOf(ops, 'rowDropClear').map(op => op.keys)).toEqual([['fleet:local:thread-3']]);
    const landed = rig({ threadId: 'thread-1', presentation: { rowDrops: ['fleet:local:thread-3'] } });
    await takeRowDrops(landed.client, landed.native);
    landed.client.threadId = 'thread-3';
    await takeRowDrops(landed.client, landed.native);
    expect(opsOf(landed.ops, 'rowDropClear')).toEqual([]);
    expect(opsOf(landed.ops, 'rowDropTake').map(op => op.keys)).toEqual([['fleet:local:thread-3']]);
  });
  test('a press whose thread fails to open lets the row\'s drop go', async () => {
    const { client, native, ops } = rig({ presentation: { rowDrops: ['fleet:env-gone:t1'] } });
    const out = { message: '', id: '', value: '' } as OpOut;
    await expect(threadOps.call(client, 'select-thread', 'fleet:env-gone:t1', 'click', 0, native, files, out)).rejects.toThrow('That environment is no longer connected.');
    expect(opsOf(ops, 'rowDropClear').map(op => op.keys)).toEqual([['fleet:env-gone:t1']]);
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

// The IDs below come from a later real-input session and differ from the older LS numbering above.
describe('additive ranges and stable anchors', () => {
  for (const legacy of [true, false]) test(`Command A, Command C, Shift B retains A/B/C (${legacy ? 'legacy' : 'default'})`, async () => {
    // The press's own modifiers (sidebar-row.contract and legacy-sidebar.contract `openPress`): "click:meta", "click:shift".
    const { client, native } = rig({ threads: [thread('a'), thread('b'), thread('c')], settings: { legacySidebarEnabled: legacy } });
    const session = sidebarSession(client);
    await sidebarSelecting(client, native, 'a', 'click:meta');
    await sidebarSelecting(client, native, 'c', 'click:meta');
    await sidebarSelecting(client, native, 'b', 'click:shift');
    expect(new Set(session.selection)).toEqual(new Set(['a', 'b', 'c']));
    expect(session.anchor).toBe('c');
    await sidebarSelecting(client, native, 'b', 'click:shift');
    expect(session.selection).toHaveLength(3);
    await sidebarSelecting(client, native, 'a', 'click:meta');
    expect(session.selection).not.toContain('a');
    expect(session.anchor).toBe('c');
    await sidebarSelecting(client, native, 'a', 'click:shift');
    expect(new Set(session.selection)).toEqual(new Set(['a', 'b', 'c']));
    expect(await sidebarSelecting(client, native, 'b', 'click')).toBe(false);
    expect(session.selection).toEqual([]);
  });
  test('a missing anchor or a target in another project adds only the target and resets the anchor', async () => {
    const { client, native } = rig({ threadId: 'a', threads: [thread('a'), thread('b'), thread('c'), thread('other', { projectId: 'p2' })] });
    const session = sidebarSession(client);
    session.selection = ['other'];
    await sidebarSelecting(client, native, 'b', 'click:shift');
    expect(session.selection).toEqual(['other', 'b']); // the open thread is not an implicit anchor
    expect(session.anchor).toBe('b');
    session.anchor = 'gone';
    await sidebarSelecting(client, native, 'a', 'click:shift');
    expect(session.selection).toEqual(['other', 'b', 'a']);
    expect(session.anchor).toBe('a');
    await sidebarSelecting(client, native, 'other', 'click:shift');
    expect(session.selection).toHaveLength(3);
    expect(session.anchor).toBe('other');
  });
  test('a background modifier click selects without switching environments or requesting composer focus', async () => {
    const { client, native, ops } = rig();
    background([thread('t9', { projectId: 'r1' })]);
    const out = { message: '', id: '', value: '' } as OpOut;
    await threadOps.call(client, 'select-thread', 'fleet:env-remote:t9', 'click:meta', 0, native, files, out);
    expect(sidebarSession(client).selection).toEqual(['fleet:env-remote:t9']);
    expect(client.environmentId).toBe('local');
    expect(client.threadId).toBe('thread-1');
    expect(opsOf(ops, 'connect')).toEqual([]);
    expect(out.message).toBe('');
  });
});

describe('Mark unread uses the reference silent failure policy', () => {
  test('a server refusal for a local thread with no completed run stays silent, while archive failures still toast', async () => {
    const { client, native } = rig();
    client.request = async () => { throw new Error('No completed run'); };
    await sidebarCommand(client, native, files, 'legacy-thread-choice', 'thread-1', 'mark-unread');
    expect(toasts(client)).toEqual([]);
    expect(sidebarPrefs(client).visited['thread-1']).toBeUndefined();
    await sidebarCommand(client, native, files, 'legacy-archive', 'thread-1', '');
    expect(toasts(client).at(-1)?.title).toBe('Failed to archive thread');
  });
  test('a background server refusal stays silent; a superseded mutation still propagates', async () => {
    const { client, native } = rig();
    background([thread('t9', { projectId: 'r1' })]);
    let failure: Error = new Error('No completed run');
    const original = native.later.bind(native);
    native.later = async request => obj(request).op === 'request' ? Promise.reject(failure) : original(request);
    await sidebarCommand(client, native, files, 'legacy-thread-choice', 'fleet:env-remote:t9', 'mark-unread');
    expect(toasts(client)).toEqual([]);
    expect(sidebarPrefs(client).visited['fleet:env-remote:t9']).toBeUndefined();
    failure = new ClientError('The answer was let go.', 'superseded');
    await expect(sidebarCommand(client, native, files, 'legacy-thread-choice', 'fleet:env-remote:t9', 'mark-unread')).rejects.toBe(failure);
  });
  test('the untracked fallback makes only completed local/background threads unread', async () => {
    const { client, native, rpcs } = rig({ caps: { threadVisitedTracking: false },
      threads: [thread('thread-1'), thread('done', { latestRunId: 'run', latestRunCompletedAt: at(3), status: 'completed' })] });
    const entry = background([thread('empty', { projectId: 'r1' }), thread('done', { projectId: 'r1', latestRunId: 'run', latestRunCompletedAt: at(3), status: 'completed' })]);
    obj(obj(entry.config.environment).capabilities).threadVisitedTracking = false;
    for (const id of ['thread-1', 'done', 'fleet:env-remote:empty', 'fleet:env-remote:done']) await sidebarCommand(client, native, files, 'legacy-thread-choice', id, 'mark-unread');
    expect(sidebarPrefs(client).visited).toMatchObject({ done: '2026-03-09T10:02:59.999Z', 'fleet:env-remote:done': '2026-03-09T10:02:59.999Z' });
    expect(sidebarPrefs(client).visited['thread-1']).toBeUndefined();
    expect(sidebarPrefs(client).visited['fleet:env-remote:empty']).toBeUndefined();
    expect(toasts(client)).toEqual([]);
    expect(rpcs).toEqual([]);
  });
});

describe('row drop ownership during an environment handoff', () => {
  test('a queued background drop waits for synchronization and attaches once in the destination, preserving file order', async () => {
    const { client, native, ops } = rig({ presentation: { rowDrops: ['fleet:env-remote:thread-1'] } });
    const staged = ['notes.txt', 'other.txt'].map((name, i) => ({ kind: 'file', name, id: `11111111-1111-4111-8111-11111111111${i}`, sizeBytes: 5, mimeType: 'text/plain' }));
    const original = native.later.bind(native);
    native.later = async request => {
      if (obj(request).op !== 'rowDropTake') return original(request);
      ops.push(obj(request)); client.presentation = { rowDrops: [] };
      return { ok: true, generation: client.generation, value: { files: staged } };
    };
    expect(await takeRowDrops(client, native)).toBe(false); // same raw thread ID on the current environment
    T3Client.prototype.adoptStatus.call(client, { state: 'connected', origin: 'http://studio.local:3773', environmentId: 'env-remote', message: '', presentation: client.presentation }, 1);
    client.threadId = 'thread-1'; client.projectId = 'r1';
    expect(await takeRowDrops(client, native)).toBe(false); // selected, but shell/thread/config not ready
    expect(opsOf(ops, 'rowDropTake')).toEqual([]);
    expect(opsOf(ops, 'rowDropClear')).toEqual([]);
    Object.assign(client, { configLive: true, shellLive: true, threadLive: true });
    const owner = client.snapshotOwner;
    expect(await takeRowDrops(client, native)).toBe(true);
    expect(draftFiles(client.local).map(file => [file.name, file.draftKey])).toEqual([['notes.txt', 'env-remote:thread-1'], ['other.txt', 'env-remote:thread-1']]);
    expect(opsOf(ops, 'editorInsert')[0]?.owner).toBe(owner);
    expect(client.local.drafts['local:thread-1']).toBeUndefined();
    expect(await takeRowDrops(client, native)).toBe(false);
    expect(opsOf(ops, 'rowDropTake')).toHaveLength(1);
  });
  test('a failed row press clears its queue even before the next status snapshot reports it', async () => {
    const { client, native, ops } = rig();
    await clearRowDrops(client, native, ['fleet:env-remote:gone']);
    expect(opsOf(ops, 'rowDropClear').map(op => op.keys)).toEqual([['fleet:env-remote:gone']]);
  });
  test('navigation while rowDropTake replies releases the staged files instead of attaching to the new draft', async () => {
    const { client, native, ops } = rig({ presentation: { rowDrops: ['fleet:local:thread-1'] } });
    const original = native.later.bind(native);
    native.later = async request => {
      if (obj(request).op !== 'rowDropTake') return original(request);
      client.threadId = 'thread-2';
      return { ok: true, generation: client.generation, value: { files: [{ kind: 'file', name: 'notes.txt', id: '11111111-1111-4111-8111-111111111111', sizeBytes: 5 }] } };
    };
    expect(await takeRowDrops(client, native)).toBe(false);
    expect(draftFiles(client.local)).toEqual([]);
    expect(opsOf(ops, 'editorInsert')).toEqual([]);
    expect(opsOf(ops, 'composerAttachRemove')).toHaveLength(1);
  });
  test('navigation while clearing an abandoned drop cannot attach it on a later destination revisit', async () => {
    const { client, native, ops } = rig({ presentation: { rowDrops: ['fleet:local:abandoned'] } });
    await takeRowDrops(client, native);
    client.threadId = 'thread-2'; client.presentation = { rowDrops: ['fleet:local:abandoned', 'fleet:local:thread-2'] };
    const original = native.later.bind(native), queued = new Set(['fleet:local:abandoned', 'fleet:local:thread-2']);
    let release: (() => void) | undefined;
    native.later = async request => {
      const op = obj(request);
      if (op.op === 'rowDropClear') {
        if (!release) await new Promise<void>(resolve => { release = resolve; });
        for (const key of op.keys as string[]) queued.delete(key);
      }
      if (op.op === 'rowDropTake') {
        ops.push(op);
        const files = (op.keys as string[]).some(key => queued.delete(key)) ? [{ kind: 'file', name: 'abandoned.txt', id: '11111111-1111-4111-8111-111111111111', sizeBytes: 5 }] : [];
        return { ok: true, generation: client.generation, value: { files } };
      }
      return original(request);
    };
    const taking = takeRowDrops(client, native);
    expect(release).toBeDefined();
    client.threadId = 'thread-3';
    release!();
    expect(await taking).toBe(false);
    client.presentation = { rowDrops: [...queued] };
    expect(await takeRowDrops(client, native)).toBe(false);
    client.threadId = 'thread-2'; client.presentation = { rowDrops: [...queued] };
    expect(await takeRowDrops(client, native)).toBe(false);
    expect([...queued]).toEqual([]);
    expect(opsOf(ops, 'rowDropTake')).toEqual([]);
    expect(opsOf(ops, 'rowDropClear').map(op => op.keys)).toEqual([['fleet:local:abandoned'], ['fleet:local:thread-2']]);
    expect(draftFiles(client.local)).toEqual([]);
  });
  test('an image-only drop keeps its inline fallback chip with its original shelf after editor navigation', async () => {
    const { client, native, ops } = rig({ presentation: { rowDrops: ['fleet:local:thread-1'] } });
    client.local.drafts['local:thread-1'] = 'Keep this image here';
    const original = native.later.bind(native), owner = client.snapshotOwner;
    let release: (() => void) | undefined;
    let entered: (() => void) | undefined;
    const inserting = new Promise<void>(resolve => { entered = resolve; });
    native.later = async request => {
      const op = obj(request);
      if (op.op === 'rowDropTake') return { ok: true, generation: client.generation, value: { files: [{ kind: 'image', name: 'shot.png', id: '11111111-1111-4111-8111-111111111111', sizeBytes: 64 }] } };
      if (op.op === 'editorInsert') await new Promise<void>(resolve => { release = resolve; entered!(); });
      return original(request);
    };
    const taking = takeRowDrops(client, native);
    await inserting;
    expect(release).toBeDefined();
    client.threadId = 'thread-2';
    release!();
    expect(await taking).toBe(true);
    expect(client.local.snapshotDrafts['local:thread-1']?.map(image => image.name)).toEqual(['shot.png']);
    expect(client.local.drafts['local:thread-1']).toContain('shot.png');
    expect(client.local.drafts['local:thread-2']).toBeUndefined();
    expect(client.local.snapshotDrafts['local:thread-2']).toBeUndefined();
    expect(opsOf(ops, 'editorInsert')[0]?.owner).toBe(owner);
  });
});

describe('mounted initial focus survives late thread-open completion', () => {
  const source = readFileSync(appPath('legacy-sidebar.contract'), 'utf8');
  const root = readFileSync(appPath('app.contract'), 'utf8');
  const guard = root.split('\n').find(line => line.trim().startsWith('if result.message.startsWith("focus:")'))!.trim().slice(3).split(' //')[0]!;
  for (const title of ['Branch work', '한글🙂']) test(`rename selects the complete UTF-16 title once: ${title}`, () => {
    const component = contractComponent('dialog-trap.contract', 'FieldOpenSelect'), state = component.state();
    const calls: unknown[][] = [], target = 'rename-thread-2';
    const host = { target, length: title.length, focus: (id: string) => calls.push(['focus', id]), setSelectionRange: (...range: unknown[]) => calls.push(['selection', ...range]) };
    component.call('laidOut', state, host, [0, 0]);
    expect(calls).toEqual([]);
    component.call('laidOut', state, host, [1, 1]);
    expect(calls).toEqual([['focus', target], ['selection', target, 0, title.length]]);
    component.call('laidOut', state, host, [1, 1]);
    expect(calls).toHaveLength(2);
    expect(value(guard, { result: { message: 'focus:composer' }, data: { sidebar: { renameId: 'thread-2' } }, renameAfterOp: '' })).toBe(false);
    expect(value(guard, { result: { message: 'focus:composer' }, data: { sidebar: { renameId: '' } }, renameAfterOp: '' })).toBe(true);
    // A double-click's rename held until the open lands (sidebar-rename-after-open.test.ts) owns the focus too.
    expect(value(guard, { result: { message: 'focus:composer' }, data: { sidebar: { renameId: '' } }, renameAfterOp: 'legacy-rename-start' })).toBe(false);
    expect(value(guard, { result: { message: 'focus:other-dialog' }, data: { sidebar: { renameId: 'thread-2' } }, renameAfterOp: '' })).toBe(true);
    expect(source).toContain('FieldOpenSelect(target=`rename-${t.id}`, length=t.title.length)');
    expect(source).toContain('input id=`rename-${t.id}`');
  });
  test('Confirm focuses once per opening, and zero layout permits a later reopening', () => {
    const component = contractComponent('dialog-trap.contract', 'DialogOpenFocus'), state = component.state(), focused: string[] = [];
    const host = { target: 'thread-archive-confirm-thread-1', focus: (id: string) => focused.push(id) };
    for (const bounds of [[0, 0], [1, 1], [1, 1]]) component.call('laidOut', state, host, bounds);
    expect(focused).toEqual([host.target]);
    component.call('laidOut', state, host, [0, 0]);
    component.call('laidOut', state, host, [1, 1]);
    expect(focused).toEqual([host.target, host.target]);
    expect(source).toContain('if confirmShown\n          DialogOpenFocus(target=`thread-archive-confirm-${t.id}`)');
    expect(source).toContain('id=`thread-archive-confirm-${t.id}`');
  });
});

describe('project identity is included in local and remote dialog copy', () => {
  test('a project-menu reply carries its destination even while the snapshot still contains the previous navigation', async () => {
    for (const remote of [false, true]) {
      const { client, native } = rig();
      if (remote) background([], [{ id: 'r1', title: 'solo', workspaceRoot: '/srv/solo' }]);
      const message = await sidebarCommand(client, native, files, 'legacy-project-choice', remote ? 'env-remote:/srv/solo' : 'github.com/acme/app', 'project-settings');
      const root = readFileSync(appPath('app.contract'), 'utf8');
      const branch = root.split('\n').find(line => line.includes('if result.message') && (line.includes('sidebar:navigate') || line.includes('sidebar:project-settings:')))!.trim().slice(3);
      expect(value(branch, { result: { message }, data: { sidebar: { navigateKind: '', navigateProject: 'old-project' } } })).toBe(true);
      expect(message).toBe(`sidebar:project-settings:${remote ? 'fleet:env-remote:r1' : 'p1'}`);
    }
  });
  test('rename and removal show the focused environment label, and grouping uses the requested ellipsis', async () => {
    const { client, native } = rig();
    await sidebarCommand(client, native, files, 'legacy-project-choice', 'github.com/acme/app', 'rename:local:/work/app');
    expect(legacySidebarSnapshot(client, Date.parse(at(30)), { projectIdentity: identity })).toMatchObject({ dialog: 'rename-project', dialogDetail: 'Environment: This Mac' });
    await sidebarCommand(client, native, files, 'legacy-remove-anyway', 'p1', '');
    expect(legacySidebarSnapshot(client, Date.parse(at(30)), { projectIdentity: identity }).dialogDescription).toContain('Environment: This Mac');
  });
});

describe('LEG-TIP: the root validates the rendered owner, including delayed tips without leave', () => {
  const root = readFileSync(appPath('app.contract'), 'utf8');
  const owner = readFileSync(appPath('legacy-sidebar.contract'), 'utf8').split('\n').find(line => line.startsWith('fn legacyTipOwner('))!.split(' = ')[1]!;
  const valid = root.split('\n').find(line => line.startsWith('  derive hoverOwnerValid = '))!.split(' = ')[1]!;
  const shown = root.split('\n').find(line => line.startsWith('  derive hoverShown = '))!.split(' = ')[1]!;
  const legacyTipOwner = (testId: string, projects: unknown[]) => value(owner, { testId, projects });
  test('a valid unrelated hover card retains its open delay and complete close delay', () => {
    const component = contractComponent('app.contract', 'T3Code');
    const helpers = readFileSync(appPath('hover-layer.contract'), 'utf8').split('\n');
    const helper = (name: string, kind: string) => value(helpers.find(line => line.startsWith(`fn ${name}(`))!.split(' = ')[1]!, { kind });
    const tip = { key: 'connection-scopes', kind: 'scopes' };
    const state: Env = { hoverWait: 0, hoverEnd: 0, hoverTip: { key: '' }, hoverOn: '', hoverOpen: 0, hoverUntil: 0 };
    const visible = () => value(shown, { ...state, hoverOwnerValid: true });
    const scope = () => ({ hoverOwnerValid: true, hoverShown: visible(), hoverOpenDelay: (kind: string) => helper('hoverOpenDelay', kind),
      hoverDelay: (kind: string) => helper('hoverDelay', kind), hoverHold: (end: number) => { state.hoverEnd = end; } });
    component.call('hoverTipAt', state, scope(), [tip, 'trigger', true]);
    expect(state.hoverEnd).toBe(250);
    expect(visible()).toBe(false);
    for (let tick = 0; tick < 24; tick++) component.call('hoverTicked', state, scope(), []);
    expect(visible()).toBe(false);
    component.call('hoverTicked', state, scope(), []);
    expect(visible()).toBe(true);
    component.call('hoverTipAt', state, scope(), [tip, 'trigger', false]);
    expect(state.hoverEnd).toBe(350);
    for (let tick = 0; tick < 9; tick++) component.call('hoverTicked', state, scope(), []);
    expect(visible()).toBe(true);
    component.call('hoverTicked', state, scope(), []);
    expect(visible()).toBe(false);
    expect(state.hoverTip).toEqual(tip);
  });
  test('removal, collapse, hiding and rename invalidate thread owners; reordered rows and header owners remain valid', () => {
    const { client } = rig();
    const projection = () => legacySidebarSnapshot(client, Date.parse(at(30)), { projectIdentity: identity });
    const testId = 'legacy-tip-title-thread-2-bubble';
    expect(legacyTipOwner(testId, projection().projects)).toBe(true);
    client.shell.threads.reverse();
    expect(legacyTipOwner(testId, projection().projects)).toBe(true);
    sidebarSession(client).renameId = 'thread-2';
    expect(legacyTipOwner(testId, projection().projects)).toBe(false);
    sidebarSession(client).renameId = '';
    sidebarPrefs(client).projectExpanded['github.com/acme/app'] = false;
    expect(legacyTipOwner(testId, projection().projects)).toBe(false);
    expect(legacyTipOwner('legacy-tip-new-github.com/acme/app-bubble', projection().projects)).toBe(true);
    sidebarPrefs(client).projectExpanded['github.com/acme/app'] = true;
    client.shell.threads.find(t => t.id === 'thread-2')!.archivedAt = at(20);
    expect(legacyTipOwner(testId, projection().projects)).toBe(false);
    client.shell.projects = [];
    expect(legacyTipOwner('legacy-tip-new-github.com/acme/app-bubble', projection().projects)).toBe(false);
  });
  test('a missing owner hides immediately and clears delayed state; an unrelated tooltip stays valid', () => {
    const { client } = rig();
    const data = { legacy: legacySidebarSnapshot(client, Date.parse(at(30)), { projectIdentity: identity }), sidebarOpen: true };
    const hoverTip = { key: 'legacy-tip', testId: 'legacy-tip-title-thread-2-bubble' };
    const env = { data, hoverTip, legacyTipOwner };
    expect(value(valid, env)).toBe(true);
    data.legacy.projects = [];
    const hoverOwnerValid = value(valid, env);
    expect(hoverOwnerValid).toBe(false);
    for (const hoverWait of [0, 600]) expect(value(shown, { ...env, hoverOwnerValid, hoverOn: 'trigger', hoverWait, hoverOpen: 600, hoverUntil: 0 })).toBe(false);
    const component = contractComponent('app.contract', 'T3Code'), state = { hoverWait: 0 }, cleared: unknown[] = [];
    component.call('hoverTicked', state, { hoverOwnerValid, noHoverTip: () => ({ key: '' }), hoverTipAt: (...args: unknown[]) => cleared.push(args) }, []);
    expect(cleared).toEqual([[{ key: '' }, '', true]]);
    for (const key of ['usage', 'composer-model', 'sidebar-row']) expect(value(valid, { ...env, hoverTip: { ...hoverTip, key } })).toBe(true);
    data.legacy = legacySidebarSnapshot(client, Date.parse(at(30)), { projectIdentity: identity });
    data.legacy.enabled = false;
    expect(value(valid, env)).toBe(false);
    data.legacy.enabled = true; data.sidebarOpen = false;
    expect(value(valid, env)).toBe(false);
  });
});

describe('the complete Sidebar options focus boundary', () => {
  test('radio, field and step-button transitions stay open; outside focus and no target close without redirecting focus', () => {
    const options = contractComponent('legacy-sidebar.contract', 'LegacyOptions'), state = options.state();
    const hidden: string[] = [], focused: string[] = [];
    const scope = { ...options.derives(state, { KmItem: (item: { id: string; label: string }) => item }),
      hidePopover: (id: string) => hidden.push(id), focus: (id: string) => focused.push(id) };
    const members = ['legacy-sort-projects-updated_at', 'legacy-sort-projects-created_at', 'legacy-sort-projects-manual',
      'legacy-sort-threads-updated_at', 'legacy-sort-threads-created_at', 'legacy-preview-count', 'legacy-preview-decrease',
      'legacy-preview-increase', 'legacy-sidebar-options-keys', 'legacy-sidebar-options-keys-first', 'legacy-sidebar-options-keys-last'];
    expect(scope.focusIds).toEqual(expect.arrayContaining(members));
    for (const target of members) for (const relatedTarget of members) {
      options.call('left', state, scope, [{ target, relatedTarget }]);
    }
    expect(hidden).toEqual([]);
    for (const relatedTarget of ['legacy-options', 'composer', '', null]) {
      options.call('left', state, scope, [{ target: 'legacy-preview-count', relatedTarget }]);
    }
    expect(hidden).toEqual(Array(4).fill('legacy-sidebar-options'));
    expect(focused).toEqual([]);
    const source = readFileSync(appPath('legacy-sidebar.contract'), 'utf8');
    expect(source).toContain('row id="legacy-sidebar-options" focusout=left popover="auto"');
    for (const id of ['legacy-preview-count', 'legacy-preview-decrease', 'legacy-preview-increase']) {
      expect(source).toContain(`id="${id}"`);
    }
  });

  test('clamping and stepping reconcile the draft on the same input and preserve the open boundary', async () => {
    const { client, native } = rig(), count = contractComponent('legacy-sidebar.contract', 'LegacyCount'), state = count.state();
    const pending: Promise<unknown>[] = [], writes: unknown[][] = [];
    const scope = () => ({ count: legacySidebarSnapshot(client, Date.parse(at(10)), { projectIdentity: identity }).previewCount,
      local: (op: string, id: string, text: string) => { writes.push([op, id, text]); pending.push(sidebarLocal(client, native, op, id, text)); } });
    const act = async (name: string, ...args: unknown[]) => {
      const props = scope(); count.call(name, state, { ...props, ...count.derives(state, props) }, args);
      await Promise.all(pending.splice(0));
    };
    for (const [text, expected] of [['99', '15'], ['0', '1'], ['', '1']] as const) {
      await act('edit', text);
      expect(count.derives(state, scope()).value).toBe(text);
      await act('submitDraft');
      expect(count.derives(state, scope()).value).toBe(expected);
      expect(count.drawn('id="legacy-preview-count"', state, scope())).toBe(true);
    }
    await act('edit', '12'); await act('commit', '12');
    expect(count.derives(state, scope()).value).toBe('12');
    await act('step', '1'); expect(count.derives(state, scope()).value).toBe('13');
    await act('step', '-1'); expect(count.derives(state, scope()).value).toBe('12');
    expect(writes).toEqual([['legacy-preview', '', '99'], ['legacy-preview', '', '0'], ['legacy-preview', '', ''],
      ['legacy-preview', '', '12'], ['legacy-preview-step', '', '1'], ['legacy-preview-step', '', '-1']]);
    const source = readFileSync(appPath('legacy-sidebar.contract'), 'utf8');
    expect(source).toContain('LegacyCount(count=legacy.previewCount, local=local)');
    expect(source).not.toContain('for n in [legacy.previewCount]');
  });

  // With 22 typed, Tab went to the increase stepper, the clamp disabled it,
  // and the focus stayed on that disabled button. Base UI 1.4.1's steppers (useNumberFieldButton: tabIndex -1) are
  // no Tab stops and keep the focus in the field, so the field is the popup's last stop and Tab leaves the popup.
  test('the steppers are no Tab stops: Tab from a clamped field leaves the popup instead of landing on a disabled stepper', () => {
    const count = contractComponent('legacy-sidebar.contract', 'LegacyCount'), state = count.state();
    const source = readFileSync(appPath('legacy-sidebar.contract'), 'utf8').split('\n');
    const stepper = (id: string) => source.find(line => line.includes(`button cursor="pointer" id="${id}"`))!;
    for (const [id, disabled] of [['legacy-preview-decrease', 'count <= 1'], ['legacy-preview-increase', 'count >= 15']] as const) {
      expect(stepper(id)).toContain(' tabindex=-1 disabled=');
      expect(stepper(id)).toContain(`disabled=(${disabled})`);
    }
    // The field stays the one stop, at the clamp's maximum and minimum alike, and closes the popup on its way out.
    expect(source.find(line => line.includes('input id="legacy-preview-count"'))).not.toContain('tabindex');
    for (const n of [15, 1]) expect(count.drawn('id="legacy-preview-count"', state, { count: n, local: () => {} })).toBe(true);
    const options = contractComponent('legacy-sidebar.contract', 'LegacyOptions'), hidden: string[] = [];
    const scope = { ...options.derives(options.state(), { KmItem: (item: { id: string; label: string }) => item }), hidePopover: (id: string) => hidden.push(id) };
    options.call('left', options.state(), scope, [{ target: 'legacy-preview-count', relatedTarget: 'sidebar-add-project-trigger' }]);
    expect(hidden).toEqual(['legacy-sidebar-options']);
  });
});

describe('"No projects yet" (LegacySidebar.tsx `projectsLength === 0` over useProjects, every enabled environment\'s projects)', () => {
  const empty = (client: T3Client) => legacySidebarSnapshot(client, Date.parse(at(30)), { projectIdentity: identity }).noProjects;
  test('a background environment\'s project keeps it away while this environment has none', () => {
    const { client } = rig({ projects: [], threads: [] });
    expect(empty(client)).toBe(true);
    const entry = background();
    expect(empty(client)).toBe(false);
    expect(legacySidebarSnapshot(client, Date.parse(at(30)), { projectIdentity: identity }).projects.map(project => project.name)).toEqual(['solo']);
    entry.shell.projects = [];
    expect(empty(client)).toBe(true);
  });
  test('this environment\'s own project keeps it away; it is never shown while the legacy sidebar is off', () => {
    expect(empty(rig().client)).toBe(false);
    expect(empty(rig({ projects: [], threads: [], settings: { legacySidebarEnabled: false } }).client)).toBe(false);
  });
});
