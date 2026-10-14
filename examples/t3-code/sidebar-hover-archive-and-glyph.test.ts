// sidebar-hover-archive-and-glyph (T3 Code 1e2ecbd975, MIT; see LICENSE-T3): the realinput-1011c rows of the main
// sidebar the user asked to fix on 2026-10-14.
// SW-2: a row whose control's tooltip ends with the pointer still in the row has its hover and card back (Sidebar.tsx:
// the row is the details tooltip's trigger; the reference's card closes for a frame and reopens). Since main #327 the
// macOS hover is the hit's ancestor path, so the row keeps the host's hover under its controls and hears no new enter;
// the window's `sidebarHover` (app-window.contract) gives it back. The window's and the root's halves run here with
// Contract's action semantics (contract-action-fixture.ts).
// SW-3a: a press on Settle or Snooze (and Wake, Un-settle) closes its tooltip at once, as Base UI's do.
// R11-X63: a pick from a row's menu lets go of the composer's focus, so ⌘Z after Archive is the app's thread undo.
// G4-14b: Settings › Archive's delete confirm names "this thread" and Unarchive is named by its text.
// B12-PR-GLYPH: a thread with no pull request link draws its branch's (useLinkedThreadPullRequest's fallback), read by
// `pullRequests.summary`; the data is the B12 lane's "Studio branch" as its server holds it.
import { afterEach, describe, expect, test } from 'bun:test';
import { T3Client } from './client';
import type { Obj } from './domain';
import type { Native } from './protocol';
import { contractComponent, type Env } from './contract-action-fixture';
import { contractFn } from './contract-fn-fixture';
import { fleet, type FleetEntry } from './settings-b-fleet';
import { sidebarSnapshot } from './sidebar-view';
import { legacySidebarSnapshot } from './legacy-sidebar-view';
import { branchPullRequest, fallbackReference, syncBranchPullRequests } from './sidebar-branch-pr';
import { archiveCommand, archiveConfirmation } from './settings-a-archive';
import { sidebarCommand } from './sidebar-commands';

const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();

// ---- SW-2, SW-3a: the window's hover over a row, its controls and their tips ----
const windowHalf = contractComponent('app-window.contract', 'T3Window');
const rootHalf = contractComponent('app.contract', 'T3Code');
// Rows 78 pt tall at y 100 and 180; each control's tip anchor is a zero-size box at its row's top.
const frames: Record<string, { y: number; height: number }> = {
  'row-a~active': { y: 100, height: 78 }, 'row-a~active#settle': { y: 100, height: 0 }, 'row-a~active#snooze': { y: 100, height: 0 },
  'row-b~active': { y: 180, height: 78 }, 'row-b~active#settle': { y: 180, height: 0 },
  'row-draft:p1': { y: 20, height: 78 }, 'row-draft:p1#discard': { y: 20, height: 0 }, 'row-sidebar-settings': { y: 800, height: 28 },
};
async function hoverRig() {
  const tipRow = await contractFn('sidebar.contract', 'sidebarTipRow', ['id'], { indexOf: (text: string, part: string) => text.indexOf(part) });
  const win: Env = { sidebarHoverSeq: 0, sidebarHoverLast: '', sidebarHoverY: 0, sidebarHoverH: 0, sidebarHoverHeld: '' };
  const root: Env = { sidebarHoverId: '' };
  const rootScope = { data: { sidebar: { searching: false } } };
  // The host's events, as the window's `hover=hoverCard(key)` and the presses' `sidebarHover("", false)` call them.
  const hover = (id: string, over: boolean) => windowHalf.call('sidebarHover', win, {
    frame: (key: string) => { const f = frames[key]; if (!f) throw new Error(`no frame ${key}`); return f; },
    sidebarScrollY: 0, sidebarHoverId: root.sidebarHoverId, sidebarTipRow: tipRow,
    sidebarHoverRoot: (rid: string, rover: boolean) => rootHalf.call('sidebarHover', root, rootScope, [rid, rover]),
  }, [id, over]);
  return {
    hover,
    /** What the overlay draws (sidebar-overlays.contract): the card is the hovered row's at `y`, a tip its control's. */
    get shown() { return { id: root.sidebarHoverId, last: win.sidebarHoverLast, y: win.sidebarHoverY, h: win.sidebarHoverH, held: win.sidebarHoverHeld }; },
  };
}

describe('SW-2: a control\'s tooltip ending inside its row gives the row its hover and card back', () => {
  test('the row, onto its Settle, back onto the row: hovered with its card again', async () => {
    const r = await hoverRig();
    r.hover('a~active', true);
    expect(r.shown).toEqual({ id: 'a~active', last: 'a~active', y: 100, h: 78, held: 'a~active' }); // the card
    r.hover('a~active#settle', true);
    expect(r.shown).toMatchObject({ id: 'a~active#settle', last: 'a~active#settle', held: 'a~active' }); // the tip; the row hovered under it
    r.hover('a~active#settle', false); // the pointer back on the row's title: the row hears nothing (ancestor path)
    // The base cleared the hover here for good ({ id: '' }): no surface, no actions, no card.
    expect(r.shown).toEqual({ id: 'a~active', last: 'a~active', y: 100, h: 78, held: 'a~active' });
    r.hover('a~active#snooze', true); r.hover('a~active#snooze', false);
    expect(r.shown.id).toBe('a~active');
  });

  test('leaving the row through a control, in either order of the two leaves, leaves nothing hovered', async () => {
    const tipFirst = await hoverRig();
    tipFirst.hover('a~active', true); tipFirst.hover('a~active#settle', true);
    tipFirst.hover('a~active#settle', false); tipFirst.hover('a~active', false);
    expect(tipFirst.shown).toMatchObject({ id: '', held: '' });
    const rowFirst = await hoverRig();
    rowFirst.hover('a~active', true); rowFirst.hover('a~active#settle', true);
    rowFirst.hover('a~active', false); rowFirst.hover('a~active#settle', false);
    expect(rowFirst.shown).toMatchObject({ id: '', held: '' });
  });

  test('from a row\'s control straight onto the next row: that row, whatever the order', async () => {
    const r = await hoverRig();
    r.hover('a~active', true); r.hover('a~active#settle', true);
    r.hover('b~active', true); r.hover('a~active#settle', false); r.hover('a~active', false);
    expect(r.shown).toMatchObject({ id: 'b~active', last: 'b~active', y: 180, held: 'b~active' });
  });

  test('a draft row\'s Discard does the same', async () => {
    const r = await hoverRig();
    r.hover('draft:p1', true); r.hover('draft:p1#discard', true); r.hover('draft:p1#discard', false);
    expect(r.shown).toEqual({ id: 'draft:p1', last: 'draft:p1', y: 20, h: 78, held: 'draft:p1' });
  });

  test('the sidebar chrome\'s tips (no row) end as before', async () => {
    const r = await hoverRig();
    r.hover('sidebar-settings', true); r.hover('sidebar-settings', false);
    expect(r.shown).toMatchObject({ id: '', held: '' });
    expect(await contractFn('sidebar.contract', 'sidebarTipRow', ['id'], { indexOf: (t: string, p: string) => t.indexOf(p) }).then(f => [f('a~active#settle'), f('a~active'), f('')])).toEqual(['a~active', '', '']);
  });
});

describe('SW-3a: a press on Settle or Snooze closes its tooltip at once', () => {
  test('the press moves the tip and card away; the row stays hovered; leaving the control gives the row back, card still away', async () => {
    const r = await hoverRig();
    r.hover('a~active', true); r.hover('a~active#settle', true);
    r.hover('', false); // pointerdown=hoverCard("", false)
    // The tip is drawn at hoverY + its top, the card at hoverY: both off the window. The hover id still names the
    // control, so the row keeps its surface and actions (ThreadRow `hovering`) and the press its gesture.
    expect(r.shown).toMatchObject({ id: 'a~active#settle', y: -10000 });
    r.hover('a~active#settle', false); // released or cancelled, the pointer moved off Settle inside the row
    expect(r.shown).toEqual({ id: 'a~active', last: 'a~active', y: -10000, h: 78, held: 'a~active' });
    r.hover('b~active', true); // the next enter places the card again
    expect(r.shown).toMatchObject({ id: 'b~active', y: 180 });
  });

  test('Settle, Snooze, Wake and Un-settle close their tips on pointerdown; the press and the sweep keep their gestures', async () => {
    const text = await source('sidebar-row.contract');
    const controls = [
      /button cursor="pointer" id=`sweep-\$\{t\.id\}` press=run\("settle", t\.id, ""\) focus=hoverCard\("", false\) pointerdown=hoverCard\("", false\) pan=sweep\(t\.id, "settle"\)/,
      /button cursor="pointer" press=hoverCard\("", false\) key=snoozeKey [^\n]*popovertarget=`snooze-\$\{t\.id\}` pointerdown=hoverCard\("", false\) [^\n]*aria-label="Snooze thread"/,
      /button cursor="pointer" id=`sweep-\$\{t\.id\}` press=run\("unsnooze", t\.id, ""\) focus=hoverCard\("", false\) pointerdown=hoverCard\("", false\) pan=sweep\(t\.id, "unsnooze"\)/,
      /button cursor="pointer" id=`sweep-\$\{t\.id\}` press=run\("unsettle", t\.id, ""\) focus=hoverCard\("", false\) pointerdown=hoverCard\("", false\) pan=sweep\(t\.id, "unsettle"\)/,
    ];
    for (const control of controls) expect(text).toMatch(control);
    expect(text.match(/pointerdown=/g)?.length).toBe(4);
  });
});

// ---- R11-X63: the focus after a row menu's Archive ----
describe('R11-X63: a row menu\'s pick lets go of the composer, so ⌘Z after Archive is the thread undo', () => {
  const run = (op: string, composerFocused: boolean) => {
    const blurs: unknown[][] = [];
    // commandPending holds the send back: only the focus half runs here (the client's archive is sidebar-commands.ts').
    rootHalf.call('sidebarRun', { commandPending: true }, { composerFocused, blur: (...args: unknown[]) => { blurs.push(args); } }, [op, 'thread-1', 'archive']);
    return blurs.length;
  };
  test('the thread, draft and legacy row menus take the focus off the composer', () => {
    expect(['menu-choice', 'draft-choice', 'legacy-thread-menu'].map(op => run(op, true))).toEqual([1, 1, 1]);
  });
  test('nothing else does, and a menu leaves a focus that is not the composer\'s where it is', () => {
    expect(['row-key', 'settle', 'drop', 'open-draft'].map(op => run(op, true))).toEqual([0, 0, 0, 0]);
    expect(run('menu-choice', false)).toBe(0); // a keyboard-opened menu: the row keeps the focus Base UI gives back
  });
});

// ---- G4-14b: Settings › Archive ----
describe('G4-14b: the archived thread\'s delete confirm and its Unarchive button', () => {
  const client = (threads: Obj[] = []) => ({
    environmentId: 'env1', shell: { projects: [], threads, sequence: 1 },
    local: { clientSettings: { confirmThreadDelete: true } },
    restAccess: () => ({ call: async () => ({ clicked: 'delete' }) }),
  }) as unknown as T3Client;
  const native = { available: true } as unknown as Native;
  test('the confirm says "this thread": the live store holds no archived thread', async () => {
    const c = client([{ id: 't1', projectId: 'p1', title: 'Archive fixture B', archivedAt: '2026-10-12T00:00:00.000Z' }]);
    await archiveCommand(c, native, {} as never, 'archive-menu', 'env1:p1:t1', 'Archive fixture B');
    expect(archiveConfirmation(c)).toEqual({ id: 'env1:p1:t1', title: 'this thread' });
    const dialog = await source('settings-a-dialogs.contract');
    expect(dialog).toContain('SettingsConfirm(title=`Delete thread "${core.archiveConfirm.title}"?`');
  });
  test('a thread the live store still holds keeps its title (resolveThreadTarget)', async () => {
    const c = client([{ id: 't2', projectId: 'p1', title: 'Still live', archivedAt: null }]);
    await archiveCommand(c, native, {} as never, 'archive-menu', 'env1:p1:t2', 'Still live');
    expect(archiveConfirmation(c).title).toBe('Still live');
  });
  test('Unarchive is named by its text', async () => {
    const row = await source('settings-archive.contract');
    const button = /\n {8}button press=raw\("unarchive-thread"[^\n]*\n((?: {10}[^\n]*\n)*)/.exec(row)!;
    expect(button[0]).not.toContain('aria-label');
    expect(button[1]).toContain('text "Unarchive"');
  });
});

// ---- B12-PR-GLYPH: a linkless thread's branch pull request ----
const LANE_PROJECT = 'e2aeeb89-3b84-4239-a7a8-cf91e2abb483';
const PR_113 = 'https://github.com/daehyeonmun2021/playground/pull/113';
/** The B12 lane's "Studio branch" as its server's shell holds it (the GitHub lane, 2026-10-14). */
const studio = (extra: Obj = {}): Obj => ({ id: 'studio', projectId: LANE_PROJECT, title: 'Studio branch', status: 'idle', latestRunId: null,
  activeProviderThreadId: null, pendingRuntimeRequest: null, createdAt: '2026-10-14T01:00:00.000Z', updatedAt: '2026-10-14T01:00:00.000Z', archivedAt: null,
  settledOverride: null, lineage: { relationshipToParent: null }, modelSelection: { instanceId: 'codex', model: 'gpt-5.4' }, branch: 'feature/release-notes', worktreePath: null,
  linkedPullRequest: null, branchPullRequest: { projectId: LANE_PROJECT, repository: 'daehyeonmun2021/playground', number: 113, url: PR_113 }, pullRequests: [], ...extra });
/** The lane server's `pullRequests.summary` for #113 (its fields that the row reads). */
const SUMMARY = { provider: 'github', projectId: LANE_PROJECT, repository: 'daehyeonmun2021/playground', number: 113, title: 'Document the release steps',
  url: PR_113, state: 'open', isDraft: false, headBranch: 'feature/release-notes', baseBranch: 'main', updatedAt: '2026-10-09T02:51:35Z' };
const NOW = Date.parse('2026-10-14T03:00:00.000Z');
const identity = { projectIdentity: (name: string) => ({ projectMark: name.slice(0, 2), projectInk: 'ink', projectSurface: 'surface' }),
  providerBadge: () => ({ providerBadge: '', providerBadgeColor: '' }) };

function laneRig(threads: Obj[], caps: Obj = {}) {
  const client = new T3Client(), asks: { method: string; payload: Obj }[] = [];
  Object.assign(client, { loaded: true, connection: 'connected', origin: 'http://127.0.0.1:16523', configLive: true, shellLive: true, threadLive: true, scopes: ['orchestration:operate'], presentation: {} });
  client.environmentId = 'lane'; client.threadId = ''; client.projectId = LANE_PROJECT;
  client.shell = { projects: [{ id: LANE_PROJECT, title: 'playground', workspaceRoot: '/lane/repos/playground', createdAt: '2026-10-14T00:00:00.000Z', updatedAt: '2026-10-14T00:00:00.000Z' }], threads, sequence: 1 };
  client.config = { environment: { label: 'GitHub lane', capabilities: { pullRequests: true, threadPullRequests: true, threadSettlement: true, ...caps } }, settings: {}, providers: [], keybindings: [] };
  client.local.clientSettings = { ...client.local.clientSettings, legacySidebarEnabled: true } as typeof client.local.clientSettings; // both sidebars' rows are built
  client.request = async (_native, method, payload) => { asks.push({ method, payload }); return method === 'pullRequests.summary' ? SUMMARY : {}; };
  const native: Native = { available: true, watch() {}, async later(request) {
    const op = request as Obj;
    if (op.fleet && op.op === 'request') { asks.push({ method: String(op.method), payload: { ...(op.payload as Obj), fleet: op.fleet } }); return { ok: true, generation: 3, value: op.method === 'pullRequests.summary' ? SUMMARY : {} }; }
    return { ok: true, generation: Number(op.generation ?? 0), value: {} };
  } };
  return { client, native, asks };
}
const defaultRow = (client: T3Client, id = 'studio') => sidebarSnapshot(client, NOW, identity).threads.find(row => row.id === id)!;
const legacyRow = (client: T3Client, id = 'studio') => legacySidebarSnapshot(client, NOW, identity).projects.flatMap(project => project.threads).find(row => row.id === id)!;
afterEach(() => { fleet.entries.clear(); });

describe('B12-PR-GLYPH: a thread with no pull request link draws its branch\'s', () => {
  test('the reference that useLinkedThreadPullRequest reads: the branch\'s, only while no link is current', () => {
    const config = { environment: { capabilities: { threadPullRequests: true } } };
    expect(fallbackReference(studio(), config)).toEqual({ projectId: LANE_PROJECT, host: 'github.com', repository: 'daehyeonmun2021/playground', number: 113 });
    const link = { host: 'github.com', repository: 'daehyeonmun2021/playground', number: 132, url: 'https://github.com/daehyeonmun2021/playground/pull/132', source: 'manual', linkedAt: '2026-10-14T00:00:00.000Z', snapshot: null };
    expect(fallbackReference(studio({ pullRequests: [link] }), config)).toBeNull();
    // A server without many-per-thread links reads the legacy single link first.
    const legacy = { projectId: LANE_PROJECT, repository: 'daehyeonmun2021/playground', number: 7, url: 'https://github.com/daehyeonmun2021/playground/pull/7' };
    expect(fallbackReference(studio({ linkedPullRequest: legacy }), { environment: { capabilities: {} } })?.number).toBe(7);
    expect(fallbackReference(studio({ linkedPullRequest: legacy }), config)?.number).toBe(113);
    expect(fallbackReference(studio({ branchPullRequest: null }), config)).toBeNull();
  });

  test('the server\'s summary draws #113 in both sidebars, asked once a minute inside the refresh', async () => {
    const { client, native, asks } = laneRig([studio()]);
    // Before: the row read `pullRequests` alone, so the Studio branch row drew nothing.
    expect(defaultRow(client)).toMatchObject({ badge: '', badgeIcon: '' });
    await syncBranchPullRequests(client, native, NOW);
    expect(asks).toEqual([{ method: 'pullRequests.summary', payload: { projectId: LANE_PROJECT, host: 'github.com', repository: 'daehyeonmun2021/playground', number: 113 } }]);
    expect(branchPullRequest(client, studio())).toMatchObject({ number: 113, title: 'Document the release steps', state: 'open' });
    expect(defaultRow(client)).toMatchObject({ badge: '113', stacked: false, badgeIcon: 'git-pull-request-arrow', badgeState: 'open' });
    expect(legacyRow(client)).toMatchObject({ prIcon: 'git-pull-request-arrow', prState: 'open', prTooltip: 'PR #113 - Open: Document the release steps', prPending: false });
    await syncBranchPullRequests(client, native, NOW + 59_000);
    expect(asks.length).toBe(1);
    await syncBranchPullRequests(client, native, NOW + 61_000);
    expect(asks.length).toBe(2);
  });

  test('a background environment\'s thread is asked on its own server', async () => {
    const { client, native, asks } = laneRig([]);
    fleet.entries.set('gh', { key: 'gh', origin: 'http://127.0.0.1:16523', environmentId: 'env-gh', phase: 'connected', message: '', traceId: '', generation: 3, synchronized: 3, lastEvent: 0,
      subscriptions: {}, config: { environment: { label: 'GitHub lane', capabilities: { pullRequests: true, threadPullRequests: true } }, settings: {} },
      shell: { projects: [{ id: LANE_PROJECT, title: 'playground', workspaceRoot: '/lane/repos/playground' }], threads: [studio()], sequence: 1 }, scopes: [], error: '', requested: false } as unknown as FleetEntry);
    await syncBranchPullRequests(client, native, NOW);
    expect(asks).toEqual([{ method: 'pullRequests.summary', payload: { projectId: LANE_PROJECT, host: 'github.com', repository: 'daehyeonmun2021/playground', number: 113, fleet: 'gh' } }]);
    expect(defaultRow(client, 'fleet:env-gh:studio')).toMatchObject({ badge: '113', badgeState: 'open' });
    expect(legacyRow(client, 'fleet:env-gh:studio')).toMatchObject({ prTooltip: 'PR #113 - Open: Document the release steps' });
  });

  test('a failed read draws nothing, and a thread with a current link asks nothing', async () => {
    const { client, native, asks } = laneRig([studio(), studio({ id: 'linked', pullRequests: [{ host: 'github.com', repository: 'daehyeonmun2021/playground', number: 132,
      url: 'https://github.com/daehyeonmun2021/playground/pull/132', source: 'manual', linkedAt: '2026-10-14T00:00:00.000Z', snapshot: { state: 'open', title: 'Add a changelog', isDraft: false } }] })]);
    client.request = async (_native, method, payload) => { asks.push({ method, payload }); throw new Error('GitHub CLI command failed'); };
    await syncBranchPullRequests(client, native, NOW);
    expect(asks.length).toBe(1);
    expect(defaultRow(client)).toMatchObject({ badge: '' });
    expect(defaultRow(client, 'linked')).toMatchObject({ badge: '132', badgeState: 'open' });
  });

  test('the legacy glyph opens the branch\'s pull request (handlePrClick\'s prStatus.url)', async () => {
    const { client, native } = laneRig([studio()]);
    client.shell.projects[0]!.repositoryIdentity = { provider: 'github', canonicalKey: 'github.com/daehyeonmun2021/playground', displayName: 'daehyeonmun2021/playground' };
    expect(await sidebarCommand(client, native, {} as never, 'legacy-pr', 'studio', 'pull-requests')).toBe('');
    await syncBranchPullRequests(client, native, NOW);
    const opened = await sidebarCommand(client, native, {} as never, 'legacy-pr', 'studio', 'pull-requests');
    expect(JSON.parse(opened.slice('sidebar:pr-page:'.length))).toEqual({ projectId: LANE_PROJECT, host: 'github.com', repository: 'daehyeonmun2021/playground', number: 113 });
  });
});
