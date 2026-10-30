// The sidebar as a keyboard tree (lib/sidebarTree.ts at T3 Code 6d450a4284): the rows sidebar-view.ts lists for it,
// and sidebar.contract's arrows, Home and End, shelf open and close, and its one Tab stop.
import { expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import { sidebarSnapshot } from '../../src/sidebar/sidebar-view';
import { sidebarPrefs } from '../../src/sidebar/sidebar-state';
import { contractComponent } from '../support/contract-action-fixture';
import { appText } from '../support/app-files';

const lines = (file: string, marker: string): string[] => appText(file).split('\n').filter(text => text.includes(marker));

const NOW = Date.parse('2026-10-04T12:00:00.000Z');
const iso = (offset: number) => new Date(NOW + offset).toISOString();
const CAPS = { threadSettlement: true, threadSnooze: true, threadPinning: true, threadPinReorder: true };
const shell = (id: string, extra: Obj = {}): Obj => ({ id, projectId: 'p1', title: `Thread ${id}`, status: 'idle', latestRunId: null,
  activeProviderThreadId: null, pendingRuntimeRequest: null, createdAt: iso(-3_600_000), updatedAt: iso(-3_600_000), archivedAt: null,
  settledOverride: null, settledAt: null, lineage: { relationshipToParent: null }, ...extra });
const helpers = { projectIdentity: (name: string) => ({ projectMark: name.slice(0, 2), projectInk: '', projectSurface: '' }), providerBadge: () => ({ providerBadge: '', providerBadgeColor: '' }) };
function fake(threads: Obj[], threadId = ''): T3Client {
  return { shell: { projects: [{ id: 'p1', title: 'P', workspaceRoot: '/p' }], threads, sequence: 1 }, config: { environment: { capabilities: CAPS }, providers: [], keybindings: [] },
    environmentId: 'env', threadId, projectId: 'p1', query: '', connection: 'connected', writable: true, ready: true, presentation: {},
    local: { drafts: {}, snapshotDrafts: {}, deviceSettings: { timestampFormat: '24-hour' }, clientSettings: {} },
    projectGroups: () => [{ key: 'g1', name: 'P', members: [{ id: 'p1' }] }] } as unknown as T3Client;
}

test('the tree lists the rows as painted: cards, then each shelf header with its rows, a closed shelf only its open row', () => {
  const client = fake([shell('a'), shell('p', { pinnedAt: iso(-1) }), shell('z', { snoozedUntil: iso(3_600_000) }),
    shell('s1', { settledOverride: 'settled', settledAt: iso(-60_000) }), shell('s2', { settledOverride: 'settled', settledAt: iso(-120_000) })], 's2');
  let view = sidebarSnapshot(client, NOW, helpers).sidebar;
  expect(view.tree.map(row => [row.id, row.parent, row.expanded])).toEqual([['thread-p', '', ''], ['thread-a', '', ''],
    ['shelf-snoozed', '', 'false'], ['shelf-settled', '', 'false'], ['thread-s2', 'shelf-settled', '']]);
  expect(view.treeCurrent).toBe('thread-s2');
  sidebarPrefs(client).snoozedExpanded = true;
  view = sidebarSnapshot(client, NOW, helpers).sidebar;
  expect(view.tree.filter(row => row.parent === 'shelf-snoozed').map(row => row.id)).toEqual(['thread-z']);
  expect(view.tree.find(row => row.id === 'shelf-snoozed')?.expanded).toBe('true');
  // An empty list has no shelves: the empty state stands in for them.
  expect(sidebarSnapshot(fake([]), NOW, helpers).sidebar.tree).toEqual([]);
});

test('the arrows, Home and End move the focus through the rows; Right and Left open, enter, close and leave a shelf', () => {
  const tree = [{ id: 'thread-a', parent: '', expanded: '' }, { id: 'thread-b', parent: '', expanded: '' }, { id: 'shelf-settled', parent: '', expanded: 'true' },
    { id: 'thread-s', parent: 'shelf-settled', expanded: '' }, { id: 'shelf-snoozed', parent: '', expanded: 'false' }];
  const sidebar = contractComponent('sidebar.contract', 'Sidebar');
  const press = (id: string, k: string, mods: Obj = {}) => {
    const done: unknown[][] = [], state = sidebar.state();
    const e = { key: k, shiftKey: false, metaKey: false, ctrlKey: false, altKey: false, isComposing: false, repeat: false, ...mods };
    const writes = sidebar.call('treeKey', state, { data: { sidebar: { tree, treeCurrent: '' } }, min: Math.min, max: Math.max,
      preventDefault: () => done.push(['prevent']), stopPropagation: () => {}, focus: (target: string) => done.push(['focus', target]),
      scrollIntoView: (target: string) => done.push(['scroll', target]), local: (...args: unknown[]) => done.push(['local', ...args]) }, [id, k, e]) as Obj | undefined;
    return { done, writes };
  };
  const focused = (id: string, k: string, mods: Obj = {}) => press(id, k, mods).done.filter(entry => entry[0] === 'focus').map(entry => entry[1]);
  expect(focused('thread-a', 'ArrowDown')).toEqual(['thread-b']);
  expect(focused('thread-b', 'ArrowUp')).toEqual(['thread-a']);
  expect(focused('thread-a', 'ArrowUp')).toEqual(['thread-a']);
  expect(focused('thread-a', 'End')).toEqual(['shelf-snoozed']);
  expect(focused('shelf-snoozed', 'Home')).toEqual(['thread-a']);
  // A target is revealed, then focused.
  expect(press('thread-a', 'ArrowDown').done).toEqual([['prevent'], ['scroll', 'thread-b'], ['focus', 'thread-b']]);
  expect(focused('shelf-settled', 'ArrowRight')).toEqual(['thread-s']);
  expect(focused('thread-s', 'ArrowLeft')).toEqual(['shelf-settled']);
  expect(press('shelf-snoozed', 'ArrowRight').done).toEqual([['prevent'], ['local', 'shelf', 'snoozed', '']]);
  expect(press('shelf-settled', 'ArrowLeft').done).toEqual([['prevent'], ['local', 'shelf', 'settled', '']]);
  // F2 renames a focused thread in place (SidebarThreadRow handleKeyDown); a shelf has no title to rename.
  expect(press('thread-a', 'F2').done).toEqual([['prevent'], ['local', 'rename-start', 'a', '']]);
  expect(press('shelf-settled', 'F2').done).toEqual([]);
  expect(press('thread-a', 'F2', { metaKey: true }).done).toEqual([]);
  // Modified keys and others are the rows' own (Enter opens, a ⇧ click selects).
  expect(press('thread-a', 'ArrowDown', { shiftKey: true }).done).toEqual([]);
  expect(press('thread-a', 'Enter').done).toEqual([]);
});

test('the list is one Tab stop: a row is a tree item that takes the stop only while it holds it', () => {
  expect(lines('sidebar.contract', 'role="tree" aria-label="Threads"')).toHaveLength(1);
  const rows = lines('sidebar-row.contract', 'id=`thread-${t.id}` press=openPress');
  expect(rows).toHaveLength(2); // the card and the slim row
  for (const row of rows) for (const part of ['role="treeitem"', 'aria-level=treeLevel', 'tabindex=(sidebarTreeStop == `thread-${t.id}` ? 0 : -1)', 'key=treeKey(`thread-${t.id}`)']) expect(row).toContain(part);
  expect(lines('sidebar.contract', 'id=`shelf-${section}`')[0]).toContain('role="treeitem"');
  // Each tree item draws the reference's focus-visible ring (ring-2 ring-inset ring-ring) beneath its own line.
  for (const [file, marker] of [['sidebar-row.contract', 'id=`thread-${t.id}` press=openPress'], ['sidebar-row.contract', 'id=`draft-${d.id}`'],
    ['sidebar.contract', 'id=`shelf-${section}`'], ['sidebar.contract', 'id="settled-more"']] as const) {
    const text = appText(file).split('\n');
    for (const [index, line] of text.entries()) if (line.includes(marker) && line.trimStart().startsWith('button ')) {
      const block = text.slice(index + 1, index + 4).map(entry => entry.trim());
      expect(block).toContain(':focus-visible');
      expect(block.find(entry => entry.startsWith('box-shadow='))).toContain('inset 0 0 0 2px');
    }
  }
});

test("Copy › Thread's chip is the composer's own link for that thread, which a paste reads back as a thread chip", async () => {
  const { threadChipCopyTarget } = await import('../../src/sidebar/thread-reference');
  const { contextId, contextLink, contextReferences } = await import('../../src/composer/composer-editor-menu');
  const id = '0b7c1d2e-3f40-4a5b-8c6d-7e8f9a0b1c2d';
  expect(threadChipCopyTarget(id, '  A  [title] ').value).toBe(contextLink('thread', contextId('thread', id), 'A title'));
  expect(contextReferences(threadChipCopyTarget(id, 'Notes').value)).toEqual([{ kind: 'thread', id: `thread_${id}`, label: 'Notes' }]);
});

test('Turn into project: a No project thread with its own folder, on a server that offers it; the toasts name the outcome', async () => {
  const { canTurnIntoProject } = await import('../../src/sidebar/sidebar-menu');
  const { sidebarCommand } = await import('../../src/sidebar/sidebar-commands');
  const { toasts } = await import('../../src/shell/toast');
  const client = fake([shell('a', { projectId: 'scratch', worktreePath: '/scratch/a' }), shell('b', { projectId: 'scratch' }), shell('c', { worktreePath: '/p/c' })]);
  (client.shell.projects as Obj[]).push({ id: 'scratch', title: 'Scratch', workspaceRoot: '/scratch' });
  Object.assign(client.config, { scratchWorkspaceRoot: '/scratch', environment: { capabilities: { ...CAPS, threadProjectFromScratch: true } } });
  const [a, b, c] = client.shell.threads as Obj[];
  expect([canTurnIntoProject(client, a!), canTurnIntoProject(client, b!), canTurnIntoProject(client, c!)]).toEqual([true, false, false]);
  const requests: unknown[][] = [];
  let refuse = false;
  Object.assign(client, { createProjectFromThread: async (_native: unknown, threadId: string) => { requests.push([threadId]); if (refuse) throw new Error('Not a scratch thread.'); return 'p9'; } });
  const native = { available: true, watch() {}, later: async () => ({}) } as never;
  await sidebarCommand(client, native, {} as never, 'menu-choice', 'a', 'turn-into-project');
  expect(requests).toEqual([['a']]);
  expect(toasts(client).at(-1)).toMatchObject({ kind: 'success', title: 'Project created', description: 'Thread a' });
  refuse = true;
  await sidebarCommand(client, native, {} as never, 'menu-choice', 'a', 'turn-into-project');
  expect(toasts(client).at(-1)).toMatchObject({ kind: 'error', title: 'Could not turn thread into a project', description: 'Not a scratch thread.' });
  (client.config.environment as Obj).capabilities = CAPS;
  expect(canTurnIntoProject(client, a!)).toBe(false);
});
