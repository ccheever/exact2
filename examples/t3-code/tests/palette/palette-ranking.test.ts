// ⌘K's results as CommandPalette.logic.ts ranks them at T3 Code 7c3b3b3605ef: subagents left out, word starts above
// mid-word matches, typos, recents by last use (a pick from a search counts), hints for a match outside the title,
// submenu options at the root, a submenu's fallback to the root, host terms, the element a Settings result opens its
// page at, group limits, the copy-reference words, and titles in their own direction.
import { describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import type { Files } from '../../src/client/protocol';
import { commandView, type CommandContext } from '../../src/palette/palette';
import { paletteCommand } from '../../src/palette/palette-commands';
import { paletteView } from '../../src/palette/palette-view';

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
/** The palette drawn for `query` at NOW, then its row for `id` picked: the command a thread row runs, with the root search's query. */
async function pick(c: T3Client, id: string, query: string) {
  await paletteView(c, null, [true, 'command', '', query, '', false, NOW, 'dark']);
  return paletteCommand(c, null, files, 'thread', id, query);
}

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
  test('no use is recorded for a pick without a query, from a ">" filter, or of the open thread', async () => {
    const c = client([{ id: 'old', title: 'Old', updatedAt: at(48), latestUserMessageAt: at(48) }, { id: 'new', title: 'New', updatedAt: at(1), latestUserMessageAt: at(1) },
      { id: 'open', title: 'Old open', updatedAt: at(72), latestUserMessageAt: at(72) }], 'unpicked-env');
    c.threadId = 'open';
    // The contract passes "" for a pick outside the root page, as upstream searches threads only there.
    for (const [id, query] of [['old', ''], ['old', '  '], ['old', '>old'], ['open', 'old']] as const) expect(await pick(c, id, query)).toMatchObject({ close: true, thread: id });
    expect(group(view(c, '').rows, 'recent-threads').map(entry => entry.arg)).toEqual(['new', 'old']);
    c.threadId = '';
    expect(group(view(c, '').rows, 'recent-threads').map(entry => entry.arg)).toEqual(['new', 'old', 'open']);
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
  test('every Settings result names the element its page opens at: targetId ?? id, a command\'s own row', () => {
    const c = client([]);
    c.config.keybindings = [{ command: 'sidebar.toggle', key: 'mod+b' }];
    const result = (query: string, id: string) => view(c, query).rows.find(entry => entry.key === `settings-search:setting:${id}`);
    // A page drawn from settings-core gives a row's control `setting-<id>`.
    expect(result('send shortcut', 'send-shortcut')).toMatchObject({ op: 'settings', arg: 'chat', arg2: 'setting-send-shortcut', description: 'Settings · Chat' });
    // Pages outside settings-core take the same name; until their rows carry it, they open at their top.
    expect(result('git fetch interval', 'git-fetch-interval')).toMatchObject({ arg: 'source-control', arg2: 'setting-git-fetch-interval' });
    expect(result('browser profiles', 'browser-profiles')).toMatchObject({ arg: 'browser', arg2: 'setting-browser' });
    // A target the page focuses another element for, from its own row and from the rows aimed at it.
    expect(result('keybindings', 'keybindings')).toMatchObject({ arg: 'keybindings', arg2: 'keybindings-filter' });
    expect(result('network access', 'network-access')).toMatchObject({ arg: 'connections', arg2: 'manage-connection' });
    expect(result('keep worktrees with local changes', 'storage-worktree-keep-when')).toMatchObject({ arg: 'worktrees', arg2: 'storage-toggle-worktreeOnDelete' });
    // A keybinding command opens at its own row (keybindingSearchAnchorId).
    expect(result('sidebar toggle', 'keybinding-sidebar.toggle')).toMatchObject({ arg: 'keybindings', arg2: 'keybinding-sidebar.toggle' });
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
