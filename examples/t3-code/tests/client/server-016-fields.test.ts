// What a 0.1.6 server adds that this client must read before the features that use it (T3 Code 2359643cff, MIT, see
// LICENSE-T3: packages/contracts orchestrationV2.ts, project.ts; packages/client-runtime projectGrouping.ts, threadShell.ts,
// promptLinks.ts, composerThreadItems.ts): side chats stay out of thread lists, archived projects out of the pickers and
// the project filter, a checkpoint's `filesTotal` and an interrupt's `unresponsiveAt` decode.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import { checkpointFileTotals, isListedThreadLineage, messages, pickableProjectGroups, projectGroupOrganization, threadSnapshot, type Obj } from '../../src/client/domain';
import { sidebarVisible } from '../../src/sidebar/sidebar-model';
import { resolveSidebarRowThreadId } from '../../src/sidebar/sidebar-lineage';
import { chosenScopes, pickableScopes, projectScopes } from '../../src/sidebar/sidebar-view';
import { threadMenuState } from '../../src/sidebar/sidebar-menu';
import { sidebarPrefs } from '../../src/sidebar/sidebar-state';
import { heroGroups } from '../../src/sidebar/project-groups';
import { pickerProjects } from '../../src/palette/palette-projects';
import { threadRows } from '../../src/composer/composer-editor-menu';
import { findPromptLinkPullRequestThread } from '../../src/prompt-links/prompt-link-model';
import { heroProjects, mostRecentProjectId } from '../../src/pages/pages-home';

const at = '2026-10-31T00:00:00.000Z';
const thread = (id: string, extra: Obj = {}): Obj => ({ id, projectId: 'p1', title: `Thread ${id}`, status: 'idle', createdAt: at, updatedAt: at, archivedAt: null,
  lineage: { parentThreadId: null, relationshipToParent: null, rootThreadId: id }, ...extra });
const sideChat = (id: string, parent: string, extra: Obj = {}) => thread(id, { lineage: { parentThreadId: parent, relationshipToParent: 'fork', rootThreadId: parent, sideChat: true }, ...extra });

function fake(projects: Obj[], threads: Obj[] = [], projectId = 'p1') {
  return {
    shell: { projects, threads, sequence: 1 }, config: { environment: { capabilities: {} } }, environmentId: 'env', projectId, threadId: '', revision: 1,
    local: { clientSettings: {}, deviceSettings: { timestampFormat: '24-hour' }, groupingMode: 'repository', groupingOverrides: {} }, ready: true,
    projectGroups() { return projects.map(project => ({ key: `g-${String(project.id)}`, name: String(project.title), members: [project] })); },
  } as unknown as T3Client;
}

describe('side chats are hidden forks (lineage.sideChat)', () => {
  test('isListedThreadLineage leaves out subagents and side chats', () => {
    expect([{ relationshipToParent: null }, { relationshipToParent: 'fork' }, { relationshipToParent: 'subagent' }, { relationshipToParent: 'fork', sideChat: true }, { sideChat: false }]
      .map(isListedThreadLineage)).toEqual([true, true, false, false, true]);
  });

  test('no sidebar row, so the open side chat selects its parent; no palette or composer @ row; no prompt link target', () => {
    const parent = thread('t1', { title: 'Parent', updatedAt: '2026-10-30T00:00:00.000Z' }), side = sideChat('t2', 't1', { title: 'Parent side chat' });
    expect([parent, side].map(sidebarVisible)).toEqual([true, false]);
    expect(resolveSidebarRowThreadId('t2', [parent, side])).toBe('t1');
    expect(threadRows([parent, side], '', 'Parent', Date.parse(at)).map(row => row.id)).toEqual(['thread:t1']);
    const link = { host: 'github.com', repository: 'o/r', number: 7, url: 'https://github.com/o/r/pull/7' };
    // Side chats inherit their parent's links; the newer side chat is skipped for its parent.
    expect(findPromptLinkPullRequestThread([{ ...parent, pullRequests: [link] }, { ...side, pullRequests: [link] }], { owner: 'o', name: 'r', number: 7 })?.id).toBe('t1');
  });
});

describe('projects gain pinnedAt and archivedAt', () => {
  const projects = [
    { id: 'p1', title: 'Alpha', workspaceRoot: '/alpha', updatedAt: '2026-10-29T00:00:00.000Z' },
    { id: 'p2', title: 'Beta', workspaceRoot: '/beta', updatedAt: '2026-10-31T00:00:00.000Z', archivedAt: at },
    { id: 'p3', title: 'Gamma', workspaceRoot: '/gamma', updatedAt: '2026-10-28T00:00:00.000Z', pinnedAt: at },
  ];

  test('a group is pinned when any member is and archived only when every member is; pickers offer open groups, pinned first', () => {
    expect(projectGroupOrganization([{ pinnedAt: at }, { archivedAt: at }])).toEqual({ pinned: true, archived: false });
    expect(projectGroupOrganization([{ archivedAt: at }, { archivedAt: at }])).toEqual({ pinned: false, archived: true });
    expect(projectGroupOrganization([])).toEqual({ pinned: false, archived: false });
    const groups = [{ key: 'a', pinned: false, archived: false }, { key: 'b', pinned: true, archived: true }, { key: 'c', pinned: true, archived: false }];
    expect(pickableProjectGroups(groups).map(group => group.key)).toEqual(['c', 'a']);
  });

  test('the palette, the hero and the index landing leave an archived project out; its scope reads as all projects', () => {
    const client = fake(projects);
    expect(projectScopes(client).map(scope => [scope.name, scope.pinned, scope.archived])).toEqual([['Beta', false, true], ['Alpha', false, false], ['Gamma', true, false]]);
    expect(pickableScopes(client).map(scope => scope.name)).toEqual(['Gamma', 'Alpha']);
    // The current project leads the unpinned ones.
    expect(pickerProjects(client).map(project => project.title)).toEqual(['Gamma', 'Alpha']);
    expect(heroGroups(client, heroProjects(client)).map(project => project.name)).toEqual(['Gamma', 'Alpha']);
    expect(mostRecentProjectId(client.shell)).toBe('p1');
    sidebarPrefs(client).scopes = ['g-p2'];
    expect(chosenScopes(client)).toEqual([]);
    sidebarPrefs(client).scopes = ['g-p1', 'g-p2'];
    expect(chosenScopes(client).map(scope => scope.key)).toEqual(['g-p1']);
  });

  test("a thread in an archived project stays in the sidebar, but its menu offers no project filter", () => {
    const client = fake(projects, [thread('t1', { projectId: 'p2' }), thread('t2', { projectId: 'p1' })]);
    expect(client.shell.threads.map(sidebarVisible)).toEqual([true, true]);
    const scopes = projectScopes(client);
    expect(threadMenuState(client, client.shell.threads[0]!, false, scopes, Date.parse(at)).projectFilter).toBeNull();
    expect(threadMenuState(client, client.shell.threads[1]!, false, scopes, Date.parse(at)).projectFilter).toEqual({ label: 'Alpha', isActive: false });
  });
});

describe('checkpoints and interrupts', () => {
  const row = (item: Obj, position: number) => ({ position, visibility: 'local', sourceThreadId: 't', sourceItemId: item.id, item });
  const base = { threadId: 't', runId: 'r', nodeId: 'n' };
  const items: Obj[] = [
    { ...base, id: 'cp', ordinal: 1, type: 'checkpoint', status: 'completed', checkpointId: 'c1', scopeId: 's', files: [{ path: 'a.ts', additions: 3, deletions: 1 }],
      filesTotal: { count: 4200, additions: 90_000, deletions: 12 } },
    { ...base, id: 'stop', ordinal: 2, type: 'run_interrupt_request', status: 'completed', message: 'Stop requested.', unresponsiveAt: at },
  ];
  const snapshot = (list: Obj[]) => threadSnapshot({ snapshotSequence: 1, projection: { thread: { id: 't', title: 'T' }, runs: [], attempts: [], nodes: [], subagents: [], providerSessions: [],
    providerThreads: [], providerTurns: [], runtimeRequests: [], messages: [], plans: [], turnItems: list, checkpointScopes: [], checkpoints: [], contextHandoffs: [],
    contextTransfers: [], visibleTurnItems: list.map(row), updatedAt: at } });

  test("a huge checkpoint's file prefix carries its totals; a malformed total is dropped", () => {
    expect(messages(snapshot(items))[0]).toMatchObject({ kind: 'checkpoint', files: [{ path: 'a.ts', additions: 3, deletions: 1 }], filesTotal: { count: 4200, additions: 90_000, deletions: 12 } });
    expect(checkpointFileTotals({ count: 1.5, additions: 0, deletions: 0 })).toBeNull();
    expect(checkpointFileTotals(undefined)).toBeNull();
  });

  test("an unconfirmed Stop's unresponsiveAt decodes, and a malformed one fails the thread as any known field does", () => {
    expect(snapshot(items).projection.turnItems).toContainEqual(expect.objectContaining({ id: 'stop', unresponsiveAt: at }));
    expect(() => snapshot([{ ...items[1]!, unresponsiveAt: 7 }])).toThrow('unresponsiveAt');
  });
});
