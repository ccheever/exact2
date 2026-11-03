// Ported from T3 Code 5d3d78ab93 packages/client-runtime/src/state/threadTagFilter.test.ts (MIT, see LICENSE-T3).
// exact2: a loaded PR row is judged by the set of loaded thread ids, and the clone's glue reads each server's settings.
import { describe, expect, it } from 'bun:test';
import type { Obj } from '../../src/client/domain';
import type { T3Client } from '../../src/client/client';
import { EnvironmentFleet, environmentKey, type FleetEntry } from '../../src/connections/settings-b-fleet';
import { hiddenTagsOf, hiddenThreadKeys, NO_HIDDEN_THREADS, threadKey as key, threadTagFilter, visibleLinkedThreads } from '../../src/sidebar/thread-tag-filter';

const local = 'local', linked = 'linked';
const thread = (id: string, tags: string[], parent?: { id: string; relationship: 'fork' | 'subagent' }, delegatedFrom?: { environmentId: string; id: string }): Obj => ({
  id, tags, lineage: { parentThreadId: parent?.id ?? null, relationshipToParent: parent?.relationship ?? null, rootThreadId: parent?.id ?? id },
  ...(delegatedFrom ? { delegatedFrom: { environmentId: delegatedFrom.environmentId, threadId: delegatedFrom.id, title: 'parent' } } : {}),
});
const localThreads = [
  thread('private', ['private']),
  thread('private-subagent', [], { id: 'private', relationship: 'subagent' }),
  thread('private-fork', [], { id: 'private', relationship: 'fork' }),
  thread('plain', []),
  thread('plain-private-side-chat', ['private'], { id: 'plain', relationship: 'subagent' }),
];
const linkedThreads = [thread('delegated-from-private', [], undefined, { environmentId: local, id: 'private' }), thread('linked-client', ['client'])];
const hiddenIds = (localTags: string[] | null, linkedTags: string[] | null) => [...hiddenThreadKeys([
  { environmentId: local, threads: localThreads, hiddenTags: localTags }, { environmentId: linked, threads: linkedThreads, hiddenTags: linkedTags }])].sort();

describe('hiddenThreadKeys', () => {
  it('hides nothing while every filter is off', () => {
    expect(hiddenIds(null, null)).toEqual([]);
    expect(hiddenThreadKeys([{ environmentId: local, threads: localThreads, hiddenTags: null }])).toBe(NO_HIDDEN_THREADS);
  });

  it('hides a tagged thread with its subagents and linked tasks, but not its forks', () => {
    expect(hiddenIds(['private'], null)).toEqual([key(local, 'private'), key(local, 'private-subagent'), key(linked, 'delegated-from-private')].sort());
  });

  it('judges an archive page against live hidden parents', () => {
    const archived = hiddenThreadKeys([{ environmentId: linked, threads: [linkedThreads[0]!], hiddenTags: null }], new Set([key(local, 'private')]));
    expect([...archived]).toEqual([key(linked, 'delegated-from-private')]);
  });

  it('hides a child whose parent is not loaded while any environment hides tags', () => {
    const orphan = thread('orphan-task', [], undefined, { environmentId: local, id: 'archived' });
    const keys = (localTags: string[] | null, linkedTags: string[] | null) => [...hiddenThreadKeys([
      { environmentId: local, threads: [], hiddenTags: localTags }, { environmentId: linked, threads: [orphan], hiddenTags: linkedTags }])];
    expect(keys(['private'], null)).toEqual([key(linked, 'orphan-task')]);
    expect(keys(null, ['client'])).toEqual([key(linked, 'orphan-task')]);
    expect(keys(null, null)).toEqual([]);
  });

  it('applies each environment\'s own tags', () => {
    expect(hiddenIds(null, ['client', 'private'])).toEqual([key(linked, 'linked-client')]);
  });

  it('judges loaded PR rows by visibility and leaves out unloaded rows while hiding', () => {
    const hidden = hiddenThreadKeys([{ environmentId: local, threads: localThreads, hiddenTags: ['private'] }]);
    const rows = [{ id: 'private-subagent' }, { id: 'plain-private-side-chat' }, { id: 'off-shell' }];
    const loaded = new Set(localThreads.map(entry => String(entry.id)));
    expect(visibleLinkedThreads(rows, local, loaded, hidden, true).map(row => row.id)).toEqual(['plain-private-side-chat']);
    // Another machine hides tags while nothing here is hidden: unloaded rows still stay out.
    expect(visibleLinkedThreads(rows, local, loaded, NO_HIDDEN_THREADS, true).map(row => row.id)).toEqual(['private-subagent', 'plain-private-side-chat']);
    expect(visibleLinkedThreads(rows, local, loaded, NO_HIDDEN_THREADS, false)).toBe(rows);
  });

  it('lets a live hidden parent win over the same parent being loaded elsewhere', () => {
    const parentKey = key(local, 'private');
    const archived = hiddenThreadKeys([{ environmentId: linked, threads: [linkedThreads[0]!], hiddenTags: null }], new Set([parentKey]), new Set([parentKey]));
    expect([...archived]).toEqual([key(linked, 'delegated-from-private')]);
  });
});

describe('each server\'s settings', () => {
  it('hide only while the switch is on, with trimmed tags; bad entries drop alone', () => {
    expect(hiddenTagsOf({ settings: { hideTaggedThreads: false, hiddenThreadTags: ['private'] } })).toBeNull();
    expect(hiddenTagsOf({ settings: { hideTaggedThreads: true, hiddenThreadTags: [' private ', 3, ''] } })).toEqual(['private']);
    expect(hiddenTagsOf({ settings: { hideTaggedThreads: true, hiddenThreadTags: [] } })).toBeNull();
    expect(hiddenTagsOf({})).toBeNull();
  });

  it('the focused environment and the background ones each apply their own', () => {
    const source = new EnvironmentFleet(), k = environmentKey('http://linked.test', linked);
    source.entries.set(k, { key: k, environmentId: linked, config: { settings: { hideTaggedThreads: true, hiddenThreadTags: ['client'] } },
      shell: { projects: [], threads: linkedThreads, sequence: 1 } } as unknown as FleetEntry);
    const client = { environmentId: local, revision: 1, config: { settings: {} }, shell: { projects: [], threads: localThreads, sequence: 1 } } as unknown as T3Client;
    expect(threadTagFilter(client, source)).toEqual({ hiding: true, keys: new Set([key(linked, 'linked-client')]) });
    source.entries.clear();
    expect(threadTagFilter(client, source)).toEqual({ hiding: false, keys: NO_HIDDEN_THREADS });
  });
});
