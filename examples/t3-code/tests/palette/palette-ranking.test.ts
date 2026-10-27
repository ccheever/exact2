// ⌘K's results as CommandPalette.logic.ts ranks them at T3 Code 7c3b3b3605ef: subagents left out, word starts above
// mid-word matches, typos, recents by last use, hints for a match outside the title, submenu options at the root, a
// submenu's fallback to the root, host terms, Settings rows that open at their row, and titles in their own direction.
import { describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import { commandView, type CommandContext } from '../../src/palette/palette';
import { claimThreadOpen } from '../../src/palette/palette-search';

const NOW = Date.parse('2026-10-27T12:00:00.000Z');
const at = (hoursAgo: number) => new Date(NOW - hoursAgo * 3_600_000).toISOString();
function client(threads: Obj[]): T3Client {
  const c = new T3Client();
  Object.assign(c, { connection: 'connected', configLive: true, shellLive: true, threadLive: true, generation: 1, environmentId: 'ranking-env' });
  c.config = { environment: { label: 'Local', capabilities: { pullRequests: true } }, keybindings: [], settings: {} };
  c.shell = { sequence: 1, projects: [{ id: 'p1', title: 'Exact', workspaceRoot: '/repo' }], threads: threads.map(thread => ({ projectId: 'p1', ...thread })) };
  c.projectId = 'p1';
  return c;
}
const view = (c: T3Client, query: string, page = '', extra: Partial<CommandContext> = {}) =>
  commandView(c, { page, query, now: NOW, scheme: 'dark', matches: new Map(), matchQuery: '', searching: false, ...extra });
const group = (rows: ReturnType<typeof view>['rows'], value: string) => rows.filter(entry => entry.key.startsWith(`${value}:`));

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
  test('recent threads are the ones used last, written in or opened here', () => {
    const c = client([{ id: 'old', title: 'Old', updatedAt: at(48), latestUserMessageAt: at(48) }, { id: 'new', title: 'New', updatedAt: at(1), latestUserMessageAt: at(1) }]);
    expect(group(view(c, '').rows, 'recent-threads').map(entry => entry.arg)).toEqual(['new', 'old']);
    claimThreadOpen('ranking-env', 'old', NOW);
    expect(group(view(c, '').rows, 'recent-threads').map(entry => entry.arg)).toEqual(['old', 'new']);
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
  test('a Settings result opens its page at its row on every page, as the Settings search does', () => {
    const c = client([]);
    const chat = view(c, 'send shortcut').rows.find(entry => entry.key === 'settings-search:setting:send-shortcut');
    expect(chat).toMatchObject({ op: 'settings', arg: 'chat', arg2: 'setting-send-shortcut', description: 'Settings · Chat' });
    const keybindings = view(c, 'keybindings').rows.find(entry => entry.key === 'settings-search:setting:keybindings');
    expect(keybindings).toMatchObject({ arg: 'keybindings', arg2: 'keybindings-filter' });
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
