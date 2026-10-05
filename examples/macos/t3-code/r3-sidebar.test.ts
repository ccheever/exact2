// Lane r3-sidebar: the round-3 sidebar ports (upstream c5a929e1ac woke dismiss
// and Project order, f68e24fb41 commands-only background work) and the
// integration fixes (hover keys, utility-page deselection, settled ink, glyphs).
import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { Obj } from './domain';
import type { Files, Native } from './protocol';
import { sidebarStatus, shellRuntime, topStatus, unseenCompletion, lastVisited, isWorkingThread } from './sidebar-model';
import { projectScopes, projectSortOrder, sidebarSnapshot } from './sidebar-view';
import { acknowledgeWoke, sidebarCommand, sidebarRefreshed } from './sidebar-commands';
import { sidebarPrefs } from './sidebar-state';
import { faviconFromAnswer, faviconSrc, projectGlyph, syncFavicons } from './r3-sidebar-glyph';
import { threadTransitions } from './shell-notify';

const NOW = Date.parse('2026-10-04T12:00:00.000Z');
const iso = (offset: number) => new Date(NOW + offset).toISOString();
const ALL_CAPS = { threadSettlement: true, threadSnooze: true, threadPinning: true, threadPinReorder: true, threadActiveReorder: true,
  threadAutoSettleOptOut: true, threadTitleRegeneration: true, threadVisitedTracking: true };
const shell = (id: string, extra: Obj = {}): Obj => ({ id, projectId: 'p1', title: `Thread ${id}`, status: 'idle', latestRunId: null,
  activeProviderThreadId: null, pendingRuntimeRequest: null, createdAt: iso(-3_600_000), updatedAt: iso(-3_600_000), archivedAt: null,
  settledOverride: null, settledAt: null, modelSelection: { instanceId: 'codex', model: 'm' }, lineage: { relationshipToParent: null }, ...extra });
const identity = (name: string) => ({ projectMark: name.slice(0, 2).toUpperCase(), projectInk: 'light-dark(#155dfc, #51a2ff)', projectSurface: 'surface' });
const helpers = { projectIdentity: identity, providerBadge: () => ({ providerBadge: '', providerBadgeColor: '' }) };

function fake(threads: Obj[], options: { caps?: Obj; settings?: Obj; threadId?: string; projects?: Obj[]; groups?: { key: string; name: string; members: Obj[] }[] } = {}) {
  const dispatched: Obj[] = [], rpcs: Obj[] = [];
  let ids = 0;
  const projects = options.projects ?? [{ id: 'p1', title: 'Parity fixture', workspaceRoot: '/fixture' }, { id: 'p2', title: 'Other', workspaceRoot: '/other' }];
  const client = {
    shell: { projects, threads, sequence: 1 },
    config: { environment: { capabilities: { ...ALL_CAPS, ...(options.caps ?? {}) } }, providers: [], keybindings: [] },
    environmentId: 'env', threadId: options.threadId ?? '', projectId: 'p1', query: '', connection: 'connected', writable: true, ready: true,
    generation: 3, origin: 'http://127.0.0.1:14821',
    presentation: {}, local: { drafts: {}, snapshotDrafts: {}, snapshotReleases: [], deviceSettings: { timestampFormat: '24-hour' }, clientSettings: { confirmThreadArchive: false, confirmThreadDelete: true, confirmThreadUnpin: false, sidebarWorkingShelfEnabled: false, ...(options.settings ?? {}) }, sidebarWidth: 256 },
    projectGroups() { return options.groups ?? projects.map(project => ({ key: `g-${String(project.id)}`, name: String(project.title), members: [project] })); },
    restAccess: () => ({
      ids: async (count: number) => Array.from({ length: count }, () => `c${ids++}`),
      request: async (method: string, payload: Obj) => { if (method === 'orchestration.dispatchCommand') dispatched.push(payload); return {}; },
      call: async () => ({}),
    }),
    rpc: async (_native: Native, method: string, payload: Obj) => { rpcs.push({ method, ...payload }); return { relativeUrl: `/api/assets/${String((payload.resource as Obj).cwd).slice(1)}.png?sig=1` }; },
  } as unknown as T3Client;
  return { client, dispatched, rpcs };
}
const notified: Obj[] = [];
const native = { available: true, watch() {}, later: async (request: Obj) => { notified.push(request); return {}; } } as unknown as Native;
const strip = (payloads: Obj[]) => payloads.map(({ commandId: _id, ...rest }) => rest);

describe('f68e24fb41: commands left running do not hold the thread', () => {
  test('a commands-only roster reads ready and shows the unseen completion; subagents, monitors and unknown work still wait', () => {
    const base = { latestRunId: 'r', status: 'completed', latestRunCompletedAt: iso(-60_000) };
    const devServer = shell('a', { ...base, pendingBackgroundTasks: [{ id: 't', kind: 'command' }] });
    expect(shellRuntime(devServer)?.status).toBe('completed');
    expect(sidebarStatus(devServer)).toBe('ready');
    expect(isWorkingThread(devServer)).toBe(false);
    const visited = lastVisited(devServer, iso(-120_000));
    expect(unseenCompletion(devServer, visited)).toBe(true);
    expect(topStatus(sidebarStatus(devServer), false, true)).toMatchObject({ label: 'Done', icon: 'circle-check', color: '#00bc7d' });
    for (const kind of ['subagent', 'monitor', 'background_task', 'something-new']) {
      const held = shell('b', { ...base, pendingBackgroundTasks: [{ id: 't', kind: 'command' }, { id: 'u', kind }] });
      expect(sidebarStatus(held)).toBe('waiting');
      expect(isWorkingThread(held)).toBe(true);
    }
    // A failed run outranks the roster either way.
    expect(sidebarStatus(shell('c', { latestRunId: 'r', status: 'failed', pendingBackgroundTasks: [{ id: 'u', kind: 'subagent' }] }))).toBe('failed');
  });

  test('the completion notification and the sidebar agree on a commands-only thread', () => {
    const { client } = fake([shell('a', { latestRunId: 'r', status: 'running' })]);
    const first = threadTransitions(client, null);
    client.shell.threads = [shell('a', { latestRunId: 'r', status: 'completed', latestRunCompletedAt: iso(0), pendingBackgroundTasks: [{ id: 't', kind: 'command' }] })];
    expect(threadTransitions(client, first.next).transitions).toEqual([expect.objectContaining({ threadId: 'a', kind: 'completion', status: 'ready' })]);
  });
});

describe('c5a929e1ac: Woke dismiss syncs through thread.visit', () => {
  const snoozed = () => shell('a', { latestRunId: 'r', status: 'completed', latestRunCompletedAt: iso(-7_200_000), snoozedAt: iso(-3_600_000), snoozedUntil: iso(-60_000), lastVisitedAt: iso(-3_000_000) });

  test('a visited-tracking server gets thread.visit at the wake time; nothing is written locally', async () => {
    const { client, dispatched } = fake([snoozed()]);
    await sidebarCommand(client, native, {} as Files, 'wake-dismiss', 'a', '', NOW);
    expect(strip(dispatched)).toEqual([{ type: 'thread.visit', threadId: 'a', visitedAt: iso(-60_000) }]);
    expect(sidebarPrefs(client).visited.a).toBeUndefined();
  });

  test('an older server keeps the local watermark, which clears the pill', async () => {
    const { client, dispatched } = fake([snoozed()], { caps: { threadVisitedTracking: false } });
    await sidebarCommand(client, native, {} as Files, 'wake-dismiss', 'a', '', NOW);
    expect(dispatched).toEqual([]);
    expect(sidebarPrefs(client).visited.a).toBe(iso(-60_000));
  });

  test('a failed visit is not reported (reportFailure: false)', async () => {
    const { client } = fake([snoozed()]);
    (client as unknown as { writable: boolean }).writable = false;
    await acknowledgeWoke(client, native, 'a', iso(-60_000));
    expect(sidebarPrefs(client).visited.a).toBeUndefined();
  });

  test('the server watermark clears Woke on every device once the visit lands', () => {
    const thread = snoozed();
    const before = sidebarSnapshot(fake([thread]).client, NOW, helpers).threads[0]!;
    expect(before.woke).toBe(true);
    const after = sidebarSnapshot(fake([{ ...thread, lastVisitedAt: iso(-60_000) }]).client, NOW, helpers).threads[0]!;
    expect(after.woke).toBe(false);
  });
});

describe('c5a929e1ac: the sidebar project picker follows Project order', () => {
  const projects = [
    { id: 'p1', title: 'Alpha', workspaceRoot: '/a', createdAt: iso(-9_000_000), updatedAt: iso(-9_000_000) },
    { id: 'p2', title: 'Beta', workspaceRoot: '/b', createdAt: iso(-1_000_000), updatedAt: iso(-1_000_000) },
    { id: 'p3', title: 'Gamma', workspaceRoot: '/c', createdAt: iso(-5_000_000), updatedAt: iso(-100) },
  ];
  const threads = [
    shell('a', { projectId: 'p1', createdAt: iso(-8_000_000), latestUserMessageAt: iso(-10_000) }),
    shell('b', { projectId: 'p2', createdAt: iso(-900_000), latestUserMessageAt: iso(-500_000) }),
  ];
  test('Last user message (default), Created at and Manual', () => {
    // Gamma has no threads: its own update stamp is the newest.
    expect(projectScopes(fake(threads, { projects }).client).map(scope => scope.name)).toEqual(['Gamma', 'Alpha', 'Beta']);
    expect(projectScopes(fake(threads, { projects, settings: { sidebarProjectSortOrder: 'created_at' } }).client).map(scope => scope.name)).toEqual(['Beta', 'Gamma', 'Alpha']);
    expect(projectScopes(fake(threads, { projects, settings: { sidebarProjectSortOrder: 'manual' } }).client).map(scope => scope.name)).toEqual(['Alpha', 'Beta', 'Gamma']);
    expect(projectSortOrder(fake([], { settings: { sidebarProjectSortOrder: 'bogus' } }).client)).toBe('updated_at');
  });
  test('ties fall back to the title, then the key', () => {
    const tied = [{ id: 'p1', title: 'Zeta', workspaceRoot: '/z', updatedAt: iso(0) }, { id: 'p2', title: 'Eta', workspaceRoot: '/e', updatedAt: iso(0) }];
    expect(projectScopes(fake([], { projects: tied }).client).map(scope => scope.name)).toEqual(['Eta', 'Zeta']);
  });
});

describe('integration: hover keys, utility pages and the settled tail', () => {
  test('a row hover key carries its shelf, so a moved row is not hovered; search results key by id', () => {
    const { client } = fake([shell('a'), shell('b', { settledOverride: 'settled' })]);
    sidebarPrefs(client).settledExpanded = true;
    const rows = sidebarSnapshot(client, NOW, helpers).threads;
    expect(rows.map(row => row.hoverKey)).toEqual(['a~active', 'b~settled']);
    client.query = 'Thread';
    expect(sidebarSnapshot(client, NOW, helpers).threads.map(row => row.hoverKey)).toEqual(['a', 'b']);
  });

  test('the open settled row keeps secondary/70 ink at rest; idle styling paints the open row as any other', () => {
    const { client } = fake([shell('a', { latestRunId: 'r', status: 'completed', latestRunCompletedAt: iso(-60_000) }), shell('b', { settledOverride: 'settled' })], { threadId: 'b' });
    const [active, settled] = sidebarSnapshot(client, NOW, helpers).threads;
    expect(settled).toMatchObject({ selected: true, section: 'settled', titleColor: 'light-dark(#71717bb3, #818181b3)', titleWeight: 500 });
    const open = sidebarSnapshot(fake([shell('a', { latestRunId: 'r', status: 'completed', latestRunCompletedAt: iso(-60_000) })], { threadId: 'a' }).client, NOW, helpers).threads[0]!;
    expect(open).toMatchObject({ selected: true, recede: false, titleWeight: 500, idleRecede: true, idleTitleColor: 'light-dark(#71717b, #818181)', idleTitleWeight: 400 });
    expect(active!.recede).toBe(active!.idleRecede);
  });
});

describe('integration: project glyphs on sidebar rows (ProjectFavicon)', () => {
  test('overrides win: monogram letters and color, emoji, Lucide icon; else the automatic monogram', () => {
    const { client } = fake([]);
    expect(projectGlyph(client, { title: 'Parity fixture', projectIcon: { kind: 'lucide', name: 'folder-code', color: 'rose', monogramText: 'QX' } }, identity))
      .toMatchObject({ kind: 'monogram', text: 'QX', ink: 'light-dark(#ec003f, #ff637e)', surface: 'light-dark(#ec003f24, #ff637e24)' });
    expect(projectGlyph(client, { title: 'Parity fixture', projectIcon: { kind: 'emoji', emoji: '🚀' } }, identity)).toMatchObject({ kind: 'emoji', emoji: '🚀' });
    const lucide = projectGlyph(client, { title: 'Parity fixture', projectIcon: { kind: 'lucide', name: 'rocket', color: 'green' } }, identity);
    expect(lucide.kind).toBe('lucide');
    expect(lucide.d.length).toBeGreaterThan(10);
    expect(lucide.ink).toBe('light-dark(#00a63e, #05df72)');
    expect(projectGlyph(client, { title: 'Parity fixture' }, identity)).toMatchObject({ kind: 'monogram', text: 'PA', ink: 'light-dark(#155dfc, #51a2ff)' });
  });

  test('the favicon answer: a signed URL against the origin, or nothing for the fallback marker', () => {
    expect(faviconFromAnswer('http://h:1/', { relativeUrl: '/api/assets/x.png?sig=1' })).toBe('http://h:1/api/assets/x.png?sig=1');
    expect(faviconFromAnswer('http://h:1', { relativeUrl: '/api/assets/project-favicon-missing?sig=1' })).toBe('');
    expect(faviconFromAnswer('http://h:1', {})).toBe('');
  });

  test('a refresh awaits one ask per project without an override; the rows draw the image', async () => {
    const projects = [{ id: 'p1', title: 'Parity fixture', workspaceRoot: '/fixture' }, { id: 'p2', title: 'Other', workspaceRoot: '/other', projectIcon: { kind: 'emoji', emoji: '🧪' } }];
    const { client, rpcs } = fake([shell('a'), shell('b', { projectId: 'p2' })], { projects });
    await sidebarRefreshed(client, native);
    await syncFavicons(client, native);
    expect(rpcs).toEqual([{ method: 'assets.createUrl', resource: { _tag: 'project-favicon', cwd: '/fixture' } }]);
    expect(faviconSrc(client, projects[0]!)).toBe('http://127.0.0.1:14821/api/assets/fixture.png?sig=1');
    const snapshot = sidebarSnapshot(client, NOW, helpers);
    expect(snapshot.threads.map(row => row.glyph.kind)).toEqual(['image', 'emoji']);
    expect(snapshot.sidebar.scopes.map(scope => scope.glyph.kind)).toEqual(['monogram', 'emoji', 'image']);
  });
});

describe('integration: row control tooltips (TooltipPopup side top, drawn by the window overlay)', () => {
  test('a live card offers Snooze and Settle at the right edge; a draft adds Discard and Unsent draft', async () => {
    const { rowTips, SETTLE_TEXT } = await import('./r3-sidebar-tips');
    const facts = { card: true, draft: false, pinned: false, canPin: true, woke: false, canSnooze: true, canSettle: true, canWake: false, canUnsettle: false };
    const settle = 30 + SETTLE_TEXT;
    expect(rowTips(facts)).toEqual([
      { name: 'settle', label: 'Settle thread', x: 14 + settle / 2, fromRight: true, top: 10, start: false },
      { name: 'snooze', label: 'Snooze thread', x: 14 + settle + 12, fromRight: true, top: 10, start: false },
    ]);
    expect(rowTips({ ...facts, draft: true, pinned: true }).map(tip => tip.name)).toEqual(['settle', 'snooze', 'discard', 'unpin', 'draft']);
    // The pen sits 24pt in: its bubble would cross the window edge, so it starts 5pt in (Base UI collision shift).
    expect(rowTips({ ...facts, draft: true }).find(tip => tip.name === 'draft')).toMatchObject({ label: 'Unsent draft', fromRight: false, x: 5, start: true, top: 15 });
    expect(rowTips({ ...facts, card: false, canSnooze: false, canSettle: false, draft: true }).find(tip => tip.name === 'draft')).toMatchObject({ x: 5, start: true, top: 12 });
  });
  test('slim rows: Un-settle on the settled tail, Woke and Unpin; snoozed rows offer no Wake tooltip', async () => {
    const { rowTips } = await import('./r3-sidebar-tips');
    const slim = { card: false, draft: false, pinned: false, canPin: true, woke: false, canSnooze: false, canSettle: false, canWake: false, canUnsettle: true };
    expect(rowTips(slim)).toEqual([{ name: 'unsettle', label: 'Un-settle thread', x: 27, fromRight: true, top: 6, start: false }]);
    expect(rowTips({ ...slim, canUnsettle: false, canWake: true })).toEqual([]);
    expect(rowTips({ ...slim, woke: true }).map(tip => tip.label)).toEqual(['Un-settle thread', 'Dismiss Woke notification']);
  });
  test('the projection carries each row its tips; search results carry none', () => {
    const { client } = fake([shell('a'), shell('b', { settledOverride: 'settled' })]);
    sidebarPrefs(client).settledExpanded = true;
    expect(sidebarSnapshot(client, NOW, helpers).threads.map(row => row.tips.map(tip => tip.name))).toEqual([['settle', 'snooze'], ['unsettle']]);
    client.query = 'Thread';
    expect(sidebarSnapshot(client, NOW, helpers).threads.every(row => row.tips.length === 0)).toBe(true);
  });
});

describe('custom snooze Duration: the NumberField stepper (CustomSnoozeDialog.tsx)', () => {
  test('Decrease and Increase move the amount by one and never below zero; they clear the error', async () => {
    const { client } = fake([shell('a')]);
    const { sidebarSession, openSnoozeDialog } = await import('./sidebar-state');
    const { sidebarLocal } = await import('./sidebar-commands');
    const session = sidebarSession(client);
    openSnoozeDialog(session, ['a'], 'Thread a', NOW);
    expect(session.dialogAmount).toBe('2');
    session.dialogError = 'Enter a positive duration.';
    await sidebarLocal(client, native, 'dialog-step', 'amount', '-1');
    expect([session.dialogAmount, session.dialogError]).toEqual(['1', '']);
    await sidebarLocal(client, native, 'dialog-step', 'amount', '-1');
    await sidebarLocal(client, native, 'dialog-step', 'amount', '-1');
    expect(session.dialogAmount).toBe('0');
    await sidebarLocal(client, native, 'dialog-step', 'amount', '+1');
    expect(session.dialogAmount).toBe('1');
    session.dialogAmount = '1.5';
    await sidebarLocal(client, native, 'dialog-step', 'amount', '+1');
    expect(session.dialogAmount).toBe('2.5');
  });
});
