// Where hidden tagged threads leave (T3 Code 30654b3e91, MIT; see LICENSE-T3): the sidebar's list, the open route
// (ThreadRouteView) and the archive (archivedThreads.ts), read through the clone's filter.
import { expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import { EnvironmentFleet, environmentKey, type FleetEntry } from '../../src/connections/settings-b-fleet';
import { partition } from '../../src/sidebar/sidebar-view';
import { hiddenArchivedKeys, routeHiddenByTags, threadKey } from '../../src/sidebar/thread-tag-filter';

const at = '2026-06-01T00:00:00.000Z';
const thread = (id: string, extra: Obj = {}): Obj => ({ id, projectId: 'p1', createdAt: at, updatedAt: at, status: 'idle', latestRunId: null, archivedAt: null,
  lineage: { relationshipToParent: null, parentThreadId: null }, ...extra });
let revision = 0;
function client(threads: Obj[], settings: Obj, extra: Obj = {}): T3Client {
  return { shell: { projects: [{ id: 'p1', title: 'P' }], threads, sequence: 1 }, config: { environment: { capabilities: {} }, providers: [], keybindings: [], settings },
    environmentId: 'env', connection: 'connected', shellLive: true, revision: ++revision, threadId: '', thread: null, local: { drafts: {}, clientSettings: {} },
    projectGroups: () => [{ key: 'g1', name: 'P', members: [{ id: 'p1' }] }], ...extra } as unknown as T3Client;
}
const HIDING = { hideTaggedThreads: true, hiddenThreadTags: ['private'] };
const threads = [thread('secret', { tags: ['private'] }), thread('kid', { lineage: { relationshipToParent: 'subagent', parentThreadId: 'secret' } }), thread('plain')];

test('the sidebar lists no thread the filter hides; with the switch off every one returns', () => {
  const listed = (settings: Obj) => Object.values(partition(client(threads, settings), Date.parse(at))).flat().map(entry => entry.id);
  expect(listed(HIDING)).toEqual(['plain']);
  expect(listed({ ...HIDING, hideTaggedThreads: false }).sort()).toEqual(['plain', 'secret']);
});

test('the open thread leaves its route once hidden; off the shell, only a child of a loaded, visible parent stays open', () => {
  expect(routeHiddenByTags(client(threads, HIDING, { threadId: 'secret' }))).toBe(true);
  expect(routeHiddenByTags(client(threads, HIDING, { threadId: 'plain' }))).toBe(false);
  expect(routeHiddenByTags(client(threads, {}, { threadId: 'secret' }))).toBe(false);
  const detail = (lineage: Obj) => ({ projection: { thread: { id: 'off', lineage } } });
  const offShell = (lineage: Obj) => routeHiddenByTags(client(threads, HIDING, { threadId: 'off', thread: detail(lineage) }));
  expect(offShell({ relationshipToParent: 'subagent', parentThreadId: 'plain' })).toBe(false);
  expect(offShell({ relationshipToParent: 'subagent', parentThreadId: 'secret' })).toBe(true);
  expect(offShell({ relationshipToParent: null, parentThreadId: null })).toBe(true);
});

test('the archive leaves out hidden threads and the archived children of a hidden live parent', () => {
  const source = new EnvironmentFleet(), key = environmentKey('http://b.test', 'env-b');
  source.entries.set(key, { key, environmentId: 'env-b', config: { settings: {} }, shell: { projects: [], threads: [], sequence: 1 } } as unknown as FleetEntry);
  const archived = [thread('old', { tags: ['private'] }), thread('task', { delegatedFrom: { environmentId: 'env', threadId: 'secret' } }), thread('fine')];
  const hidden = hiddenArchivedKeys(client(threads, HIDING), [{ environmentId: 'env', threads: archived.slice(0, 1) }, { environmentId: 'env-b', threads: archived.slice(1) }], source);
  expect([...hidden].sort()).toEqual([threadKey('env', 'old'), threadKey('env-b', 'task')]);
});
