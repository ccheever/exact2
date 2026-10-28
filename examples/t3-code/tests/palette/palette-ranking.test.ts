// ⌘K's results as CommandPalette.logic.ts ranks them at T3 Code 7c3b3b3605ef: subagents left out, word starts above
// mid-word matches, typos, recents by last use (a pick from a search counts), hints for a match outside the title,
// submenu options at the root, a submenu's fallback to the root, host terms, the element a Settings result opens its
// page at, group limits, the copy-reference words, and titles in their own direction.
import { describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { obj, type Obj } from '../../src/client/domain';
import type { Files, Native } from '../../src/client/protocol';
import { commandView, settingTarget, type CommandContext } from '../../src/palette/palette';
import { paletteCommand } from '../../src/palette/palette-commands';
import { CATALOG } from '../../src/settings/settings-catalog';
import { searchSettings, settingsNavigation } from '../../src/settings/settings-search';

const NOW = Date.parse('2026-10-27T12:00:00.000Z');
const at = (hoursAgo: number) => new Date(NOW - hoursAgo * 3_600_000).toISOString();
// This client's opens are kept per environment for the app's life, so each test that picks names its own.
function client(threads: Obj[], environmentId = 'ranking-env'): T3Client {
  const c = new T3Client();
  Object.assign(c, { connection: 'connected', configLive: true, shellLive: true, threadLive: true, generation: 1, environmentId });
  c.config = { environment: { label: 'Local', capabilities: { pullRequests: true } }, keybindings: [], settings: {} };
  c.shell = { sequence: 1, projects: [{ id: 'p1', title: 'Exact', workspaceRoot: '/repo' }], threads: threads.map(thread => ({ projectId: 'p1', ...thread })) };
  c.projectId = 'p1';
  return c;
}
const view = (c: T3Client, query: string, page = '', extra: Partial<CommandContext> = {}) =>
  commandView(c, { page, query, now: NOW, scheme: 'dark', matches: new Map(), matchQuery: '', searching: false, ...extra });
const group = (rows: ReturnType<typeof view>['rows'], value: string) => rows.filter(entry => entry.key.startsWith(`${value}:`));
const files: Files = { fs: { async mkdir() {}, async readFile() { throw new Error('none'); }, async atomicWriteFile() {} } };
/** A thread row picked at `moment`: the command it runs, with its moment and the root search's query. */
const pick = (c: T3Client, id: string, query: string, moment = NOW) => paletteCommand(c, null, files, 'thread', id, `${moment} ${query}`);
const recents = (c: T3Client) => group(view(c, '').rows, 'recent-threads').map(entry => entry.arg);
/**
 * A connected transport that records the commands it is asked to dispatch. As the real one, a request with a `deliver`
 * key answers once sent; `refuse` answers a dispatch as the server's failure.
 */
class Transport implements Native {
  available = true; dispatched: Obj[] = []; delivered: string[] = []; refuse = false;
  watch() {}
  async later(input: unknown): Promise<unknown> {
    const request = obj(input);
    if (request.op === 'ids') return { ok: true, generation: 1, value: Array.from({ length: Number(request.count) }, (_, i) => `id-${this.dispatched.length}-${i}`) };
    if (request.method !== 'orchestration.dispatchCommand') return { ok: true, generation: 1, value: {} };
    this.dispatched.push(obj(request.payload)); this.delivered.push(String(request.deliver ?? ''));
    return this.refuse ? { ok: false, generation: 1, error: { kind: 'server', message: 'Thread is deleted.', uncertain: false } } : { ok: true, generation: 1, value: { id: 7 } };
  }
}
/** A client connected to a visit-tracking server (its threads carry `lastVisitedAt`), the open thread still loading. */
function visiting(threads: Obj[], environmentId: string): T3Client {
  const c = client(threads, environmentId);
  Object.assign(c, { origin: 'http://127.0.0.1:16540', threadId: 'open', threadLive: false });
  return c;
}
const visitThreads = [{ id: 'old', title: 'Old', updatedAt: at(48), lastVisitedAt: at(48) }, { id: 'quiet', title: 'Quiet', updatedAt: at(5) },
  { id: 'open', title: 'Open', updatedAt: at(1), lastVisitedAt: at(1) }];

describe('threads', () => {
  test('subagent threads are left out, of the results and of the recents', () => {
    const c = client([{ id: 'a', title: 'Sidebar perf', updatedAt: at(1) }, { id: 'b', title: 'Sidebar perf probe', updatedAt: at(0), lineage: { relationshipToParent: 'subagent', parentThreadId: 'a' } }]);
    expect(group(view(c, 'sidebar').rows, 'threads-search').map(entry => entry.arg)).toEqual(['a']);
    expect(group(view(c, '').rows, 'recent-threads').map(entry => entry.arg)).toEqual(['a']);
  });
  test('a word start outranks a newer match inside a word; a misspelt title still matches', () => {
    const c = client([{ id: 'oauth', title: 'Fix oauth redirect', updatedAt: at(0) }, { id: 'auth', title: 'Share auth across worktrees', updatedAt: at(400) }]);
    expect(group(view(c, 'auth').rows, 'threads-search').map(entry => entry.arg)).toEqual(['auth', 'oauth']);
    expect(group(view(c, 'worktres').rows, 'threads-search').map(entry => entry.arg)).toEqual(['auth']);
  });
  test('a thread picked from a search counts as used here: first in Recent Threads, and above an equal match', async () => {
    const c = client([{ id: 'old', title: 'Old', updatedAt: at(48), latestUserMessageAt: at(48) }, { id: 'old-too', title: 'Old', updatedAt: at(2), latestUserMessageAt: at(2) },
      { id: 'new', title: 'New', updatedAt: at(1), latestUserMessageAt: at(1) }], 'picked-env');
    expect(group(view(c, '').rows, 'recent-threads').map(entry => entry.arg)).toEqual(['new', 'old-too', 'old']);
    expect(group(view(c, 'old').rows, 'threads-search').map(entry => entry.arg)).toEqual(['old-too', 'old']);
    expect(await pick(c, 'old', 'old')).toMatchObject({ close: true, thread: 'old' });
    expect(group(view(c, '').rows, 'recent-threads').map(entry => entry.arg)).toEqual(['old', 'new', 'old-too']);
    expect(group(view(c, 'old').rows, 'threads-search').map(entry => entry.arg)).toEqual(['old', 'old-too']);
  });
  test('a pick counts at its own moment: picked ten seconds after a message elsewhere, it leads Recent Threads', async () => {
    const c = client([{ id: 'a', title: 'A', updatedAt: '2026-10-27T12:00:40.000Z', latestUserMessageAt: '2026-10-27T12:00:40.000Z' },
      { id: 'old', title: 'Old', updatedAt: at(48), latestUserMessageAt: at(48) }], 'moment-env');
    await pick(c, 'old', 'old', Date.parse('2026-10-27T12:00:50.000Z'));
    expect(recents(c)).toEqual(['old', 'a']);
  });
  test('no use is recorded for a pick without a query, from a ">" filter, or of the open thread', async () => {
    const c = client([{ id: 'old', title: 'Old', updatedAt: at(48), latestUserMessageAt: at(48) }, { id: 'new', title: 'New', updatedAt: at(1), latestUserMessageAt: at(1) },
      { id: 'open', title: 'Old open', updatedAt: at(72), latestUserMessageAt: at(72) }], 'unpicked-env');
    c.threadId = 'open';
    // The query is "" for a submenu's row and a pull request's linked thread (palette-members.contract). Upstream's thread
    // view also records an open for every thread it shows (ChatView.tsx's visit, threadVisits.ts), which counts these
    // too; the clone's visit tracker records no opens yet, so this is today's behaviour.
    for (const [id, query] of [['old', ''], ['old', '  '], ['old', '>old'], ['open', 'old']] as const) expect(await pick(c, id, query)).toMatchObject({ close: true, thread: id });
    expect(recents(c)).toEqual(['new', 'old']);
    c.threadId = '';
    expect(recents(c)).toEqual(['new', 'old', 'open']);
  });
  test('a pick tells the server its query, and the pick it keeps ranks that thread first among word matches next time', async () => {
    const threads = [{ id: 'share', title: 'Share auth across worktrees', updatedAt: at(400), lastVisitedAt: at(400) },
      { id: 'token', title: 'Auth token refresh', updatedAt: at(0), lastVisitedAt: at(0) }, { id: 'oauth', title: 'Fix oauth redirect', updatedAt: at(0), lastVisitedAt: null }];
    const c = visiting(threads, 'kept-pick-env'), transport = new Transport();
    expect(group(view(c, 'auth').rows, 'threads-search').map(entry => entry.arg)).toEqual(['token', 'share', 'oauth']);
    await paletteCommand(c, transport, files, 'thread', 'share', `${NOW} auth`);
    // The server keeps the visit's query as a pick in the thread's opens (Orchestrator.ts recordThreadOpen).
    const [visit] = transport.dispatched;
    c.shell.threads = c.shell.threads.map(thread => thread.id === visit!.threadId ? { ...thread, opens: { openedAt: NOW, score: 1, picks: [{ query: visit!.searchQuery, at: NOW }] } } : thread);
    c.threadId = '';
    // The pick counts for its query and for the start of it; a match inside a word stays below either way.
    expect(group(view(c, 'auth').rows, 'threads-search').map(entry => entry.arg)).toEqual(['share', 'token', 'oauth']);
    expect(group(view(c, 'aut').rows, 'threads-search').map(entry => entry.arg)).toEqual(['share', 'token']);
    expect(group(view(c, 'refresh').rows, 'threads-search').map(entry => entry.arg)).toEqual(['token']);
  });
  test('the pick sends its open without waiting for the reply, while the open thread still loads too', async () => {
    const c = visiting(visitThreads, 'visit-env'), transport = new Transport();
    expect(c.writable).toBe(false);
    expect(await paletteCommand(c, transport, files, 'thread', 'old', `${NOW} Old Things`)).toMatchObject({ close: true, thread: 'old' });
    expect(transport.dispatched).toEqual([{ type: 'thread.visit', commandId: 'id-0-0', threadId: 'old', visitedAt: at(48), opened: true, searchQuery: 'old things' }]);
    expect(transport.delivered).toEqual(['palette-visit:id-0-0']);
    // None without a query, from a ">" filter, for the open thread, or to a server that keeps no visits.
    for (const [id, query] of [['old', ''], ['old', '>old'], ['open', 'open'], ['quiet', 'quiet']]) await paletteCommand(c, transport, files, 'thread', id!, `${NOW} ${query}`);
    expect(transport.dispatched.length).toBe(1);
  });
  test('picking the same thread again sends its open again', async () => {
    const c = visiting(visitThreads, 'twice-env'), transport = new Transport();
    await paletteCommand(c, transport, files, 'thread', 'old', `${NOW} old`);
    await paletteCommand(c, transport, files, 'thread', 'old', `${NOW + 1000} old`);
    expect(transport.dispatched.map(visit => [visit.threadId, visit.searchQuery])).toEqual([['old', 'old'], ['old', 'old']]);
  });
  test('a refused open, or no transport, still opens the picked thread', async () => {
    const c = visiting(visitThreads, 'refused-env'), transport = new Transport();
    transport.refuse = true;
    expect(await paletteCommand(c, transport, files, 'thread', 'old', `${NOW} old`)).toMatchObject({ ok: true, close: true, thread: 'old' });
    expect(transport.dispatched.length).toBe(1);
    expect(await paletteCommand(c, null, files, 'thread', 'old', `${NOW} old`)).toMatchObject({ ok: true, close: true, thread: 'old' });
    expect(await paletteCommand(c, { ...transport, available: false, later: transport.later.bind(transport), watch() {} }, files, 'thread', 'old', `${NOW} old`))
      .toMatchObject({ close: true, thread: 'old' });
    expect(transport.dispatched.length).toBe(1);
  });
  test('a match outside the title says which field matched, with the project beside the title', () => {
    const c = client([{ id: 'a', title: 'Weekly notes', updatedAt: at(1), branch: 't3/cleanup-payloads' }]);
    const [row] = group(view(c, 'cleanup').rows, 'threads-search');
    expect(row).toMatchObject({ matchLabel: 'Branch:', context: 'Exact', height: 48 });
    expect(row!.matchParts.map(part => [part.text, part.hit])).toEqual([['cleanup', true], ['-payloads', false]]);
  });
});

describe('options inside submenus', () => {
  test('"Grove" lists "Change theme: Grove", which runs the theme change', () => {
    const c = client([]);
    const [grove] = group(view(c, 'Grove').rows, 'nested-search');
    expect(grove).toMatchObject({ header: 'Suggestions', title: 'Change theme: Grove', op: 'device', arg: 'theme', arg2: 'grove' });
    expect(grove!.orbs.length).toBe(2);
    // A query that only names the submenu wants the submenu row itself.
    expect(group(view(c, 'change theme').rows, 'nested-search')).toEqual([]);
    expect(group(view(c, 'change theme').rows, 'actions')[0]!.title).toBe('Change theme');
  });
  test('a submenu where nothing matches searches the root', () => {
    const c = client([]);
    const theme = view(c, 'usage', 'theme');
    expect(theme.back).toBe(true);
    expect(group(theme.rows, 'actions').map(entry => entry.title)).toEqual(['Open usage']);
    expect(view(c, 'grove', 'theme').rows.map(entry => entry.title)).toEqual(['Grove']);
  });
});

describe('commands and settings', () => {
  test('"merge requests" finds Open pull requests by the hosts\' own terms', () => {
    const c = client([]);
    for (const query of ['merge requests', 'mrs', 'pull requests', 'gitcafe']) expect(view(c, query).rows.some(entry => entry.op === 'pull-requests'), query).toBe(true);
    expect(view(c, 'gitea').rows.some(entry => entry.arg === 'add-project'), 'gitea').toBe(true);
  });
  test('a Settings result opens where the Settings search opens it; a keybinding command at its row, else at Keybindings', () => {
    const c = client([]);
    c.config.keybindings = [{ command: 'sidebar.toggle', key: 'mod+b' }, { command: 'thread.stop', key: 'mod+.' }];
    const result = (query: string, id: string) => view(c, query).rows.find(entry => entry.key === `settings-search:setting:${id}`);
    // A core settings page gives a row's control `setting-<targetId ?? id>`; elsewhere the rule maps six rows' elements.
    expect(result('send shortcut', 'send-shortcut')).toMatchObject({ op: 'settings', arg: 'chat', arg2: 'setting-send-shortcut', description: 'Settings · Chat' });
    expect(result('keybindings', 'keybindings')).toMatchObject({ arg: 'keybindings', arg2: 'keybindings-filter' });
    // The rest open at their page's top: rows whose page carries no ids yet, and rows aimed at another one (This machine's
    // section, Worktree cleanup).
    expect(result('git fetch interval', 'git-fetch-interval')).toMatchObject({ arg: 'source-control', arg2: '' });
    expect(result('network access', 'network-access')).toMatchObject({ arg: 'connections', arg2: '' });
    expect(result('keep worktrees with local changes', 'storage-worktree-keep-when')).toMatchObject({ arg: 'worktrees', arg2: '' });
    // A command with a default key opens at its row (keybindingSearchAnchorId); one without, at the Keybindings page.
    expect(result('sidebar toggle', 'keybinding-sidebar.toggle')).toMatchObject({ arg: 'keybindings', arg2: 'keybinding-sidebar.toggle' });
    expect(result('thread stop', 'keybinding-thread.stop')).toMatchObject({ arg: 'keybindings', arg2: 'keybindings' });
  });
  test('⌘K opens every setting at the element the Settings search opens it at', () => {
    const context = { connected: true, autoSettle: true, scopeKind: 'all', keybindings: [], localBackend: true, localEnvironment: true };
    let compared = 0;
    for (const [id, title] of CATALOG) {
      const searched = settingsNavigation(title, context).items.find(item => item.id === `search-result-${id}`);
      if (!searched) continue; // not on this platform
      expect([id, settingTarget(searchSettings(title, context).find(item => item.id === id)!)]).toEqual([id, searched.target]);
      compared++;
    }
    expect(compared).toBeGreaterThan(100);
  });
});

describe('group limits', () => {
  test('Threads show twenty rows, other groups three beside matching threads, In messages ten', () => {
    const c = client([...Array.from({ length: 25 }, (_, i) => ({ id: `t${i}`, title: `Fixture ${i}`, updatedAt: at(i + 1) })),
      ...Array.from({ length: 12 }, (_, i) => ({ id: `m${i}`, title: `Notes ${i}`, updatedAt: at(i + 1) }))]);
    c.shell.projects.push(...Array.from({ length: 5 }, (_, i) => ({ id: `fp${i}`, title: `Fixture project ${i}`, workspaceRoot: `/fixture-${i}` })));
    const matches = new Map(Array.from({ length: 12 }, (_, i) => [`m${i}`, { threadId: `m${i}`, source: 'user', snippet: `the fixture ${i}` }] as const));
    const rows = view(c, 'fixture', '', { matches, matchQuery: 'fixture' }).rows;
    expect([group(rows, 'threads-search').length, group(rows, 'projects-search').length, group(rows, 'threads-messages').length]).toEqual([20, 3, 10]);
    // Without a matching thread, a group lists every match.
    const projects = view(c, 'fixture project').rows;
    expect([group(projects, 'threads-search').length, group(projects, 'projects-search').length]).toEqual([0, 5]);
  });
});

describe('copy the thread reference', () => {
  test('Copy thread ID and Copy PR link are found by their own words', () => {
    const c = client([{ id: 'a', title: 'Alpha', updatedAt: at(1) }]);
    c.threadId = 'a';
    const copy = (query: string) => view(c, query).rows.find(entry => entry.op === 'copy-thread')?.title ?? null;
    expect([copy('thread id'), copy('reference'), copy('pull request link')]).toEqual(['Copy thread ID', 'Copy thread ID', null]);
    c.shell.threads[0]!.linkedPullRequest = { number: 12, url: 'https://github.com/acme/web/pull/12' };
    expect([copy('thread id'), copy('reference'), copy('pull request link')]).toEqual([null, 'Copy PR link', 'Copy PR link']);
  });
});

describe('text direction', () => {
  test('an Arabic or Hebrew title reads right to left; a styled title keeps the page\'s direction', () => {
    const c = client([{ id: 'ar', title: 'اكتب دالة تجمع الأرقام', updatedAt: at(1) }, { id: 'en', title: 'fix the failing test', updatedAt: at(2) }]);
    const rows = view(c, '').rows;
    expect(rows.find(entry => entry.arg === 'ar')!.titleDir).toBe('rtl');
    expect(rows.find(entry => entry.arg === 'en')!.titleDir).toBe('');
    c.shell.projects[0]!.title = 'مشروع';
    expect(view(c, '').rows.find(entry => entry.op === 'new-thread')).toMatchObject({ bold: 'مشروع', titleDir: '' });
  });
});
