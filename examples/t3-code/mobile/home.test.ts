import { describe, expect, test } from 'bun:test';
import { mobileHome, mobileHomeEmpty, mobileHomeProjects, mobileHomeSources, mobileRelativeTime, projectMobileHome, type HomeSource } from './home';
import { T3Client } from './shared/client';
import { initialShell, type Obj } from './shared/domain';
import { EnvironmentFleet, type FleetEntry } from './shared/settings-b-fleet';

const now = Date.parse('2026-10-07T12:00:00Z');
const at = (minutes: number) => new Date(now + minutes * 60_000).toISOString();
const thread = (id: string, fields: Obj = {}): Obj => ({ id, projectId: 'project', title: id, createdAt: at(-60), updatedAt: at(-5), archivedAt: null, ...fields });
const source = (id: string, threads: Obj[] = [], caps: Obj = {}): HomeSource => ({ environmentId: id, label: `Machine ${id}`, machine: 'laptop', focused: id === 'one',
  config: { environment: { capabilities: caps } }, shell: { ...initialShell(), projects: [{ id: 'project', title: 'Project', workspaceRoot: '/repo' }], threads } });
const rows = (result: ReturnType<typeof projectMobileHome>) => result.items.filter(item => item.kind === 'thread');
const capabilities = { threadSettlement: true, threadSnooze: true, threadPinning: true };

describe('mobile V2 home projection', () => {
  test('static active order ignores last activity; archived and subagent rows are excluded', () => {
    const input = source('one', [thread('older', { createdAt: at(-40), updatedAt: at(0) }), thread('newer', { createdAt: at(-20), updatedAt: at(-10) }),
      thread('archived', { archivedAt: at(-2) }), thread('child', { lineage: { relationshipToParent: 'subagent' } })]);
    const before = JSON.stringify(input);
    expect(rows(projectMobileHome([input], now)).map(row => row.id)).toEqual(['newer', 'older']);
    expect(JSON.stringify(input)).toBe(before);
  });

  test('capability checks belong to each environment; scoped ids never collide', () => {
    const first = source('one', [thread('same', { settledOverride: 'settled' })], capabilities);
    const second = source('two', [thread('same', { settledOverride: 'settled' })]);
    const result = projectMobileHome([first, second], now, { settledExpanded: true });
    expect(rows(result).map(row => [row.key, row.id, row.section])).toEqual([
      ['two:same', 'fleet:two:same', 'active'], ['one:same', 'same', 'settled'],
    ]);
    expect(rows(result)[0]?.environmentLabel).toBe('Machine two');
    expect(rows(projectMobileHome([first], now, { settledExpanded: true }))[0]?.environmentLabel).toBe('');
  });

  test('snooze wins, then settlement, then pin; queued messages keep settled work active', () => {
    const input = source('one', [thread('snoozed', { snoozedUntil: at(20), pinnedAt: at(-1), settledOverride: 'settled' }),
      thread('settled', { settledOverride: 'settled', pinnedAt: at(-1) }), thread('pinned', { pinnedAt: at(-1) }),
      thread('outbox', { settledOverride: 'settled' })], capabilities);
    const result = projectMobileHome([input], now, { snoozedExpanded: true, settledExpanded: true, queuedThreadKeys: new Set(['one:outbox']) });
    expect(rows(result).map(row => [row.threadId, row.section])).toEqual([
      ['pinned', 'pinned'], ['outbox', 'active'], ['snoozed', 'snoozed'], ['settled', 'settled'],
    ]);
    expect(result.nextSnoozeWakeAt).toBe(now + 20 * 60_000);
    expect(rows(result).find(row => row.threadId === 'snoozed')?.time).toBe('20m');
    expect(rows(result).find(row => row.threadId === 'outbox')?.queued).toBe(true);
  });

  test('collapsed shelves retain selected thread; settled paging adds the selected tail only once', () => {
    const input = source('one', Array.from({ length: 14 }, (_, index) => thread(`s${index}`, { settledOverride: 'settled', settledAt: at(-index) })), capabilities);
    const collapsed = projectMobileHome([input], now);
    expect(rows(collapsed)).toEqual([]);
    expect(collapsed.items[0]?.title).toBe('Settled (14)');
    const selected = projectMobileHome([input], now, { selectedThreadKey: 'one:s13', settledExpanded: true });
    expect(rows(selected)).toHaveLength(11);
    expect(rows(selected).at(-1)?.threadId).toBe('s13');
    expect(selected.hiddenSettledCount).toBe(3);
    expect(selected.items.at(-1)?.title).toBe('Show more (3 settled hidden)');
    expect(selected.items.at(-1)?.last).toBe(true);
    expect(rows(projectMobileHome([input], now, { selectedThreadKey: 'one:s13' })).map(row => row.id)).toEqual(['s13']);
  });

  test('working beta sorts by authored send and preserves mobile Goal/Done/Waiting presentation', () => {
    const input = source('one', [thread('workingOld', { status: 'running', latestRunId: 'r', activeRunId: 'r', latestUserAuthoredMessageAt: at(-10) }),
      thread('goal', { status: 'running', latestRunId: 'r', activeRunId: 'r', latestUserAuthoredMessageAt: at(-1), goal: { status: 'active' } }),
      thread('waiting', { status: 'idle', latestRunId: 'r' }),
      thread('done', { status: 'completed', latestRunId: 'r', latestRunCompletedAt: at(-1), lastVisitedAt: at(-5) })], capabilities);
    const result = projectMobileHome([input], now, { workingEnabled: true, workingExpanded: true });
    expect(rows(result).filter(row => row.section === 'working').map(row => row.id)).toEqual(['goal', 'workingOld', 'waiting']);
    expect(rows(result).find(row => row.id === 'goal')?.status).toBe('Goal');
    expect(rows(result).find(row => row.id === 'done')?.status).toBe('Done');
    expect(rows(result).find(row => row.id === 'done')?.time).toBe('');
    expect(rows(result).find(row => row.id === 'waiting')?.status).toBe('');
    expect(rows(result).find(row => row.id === 'waiting')?.time).toBe('');
    expect(rows(projectMobileHome([input], now)).filter(row => row.section === 'working')).toHaveLength(0);
  });

  test('project repository groups span machines and physical scope references narrow correctly', () => {
    const first = source('one', [thread('one')]), second = source('two', [thread('two')]);
    for (const item of [first, second]) item.shell.projects[0] = { id: 'project', title: 'repo', workspaceRoot: '/repo',
      repositoryIdentity: { canonicalKey: 'github:team/repo', rootPath: '/repo', name: 'repo', displayName: 'team/repo' } };
    const groups = mobileHomeProjects([first, second], {});
    expect(groups).toHaveLength(1); expect(groups[0]?.title).toBe('team/repo');
    expect(groups[0]?.projectKeys).toEqual(['one:project', 'two:project']);
    expect(rows(projectMobileHome([first, second], now, { projectKey: 'two:project' }))).toHaveLength(2);
    expect(rows(projectMobileHome([first, second], now, { environmentId: 'two', projectKey: 'two:project' })).map(row => row.id)).toEqual(['fleet:two:two']);
    expect(mobileHomeProjects([first, second], { groupingMode: 'separate' })).toHaveLength(2);
  });

  test('title, PR and server-message queries filter without crossing environment ownership', () => {
    const first = source('one', [thread('same', { title: 'Fix cache', pullRequests: [{ number: 42, title: 'Cache PR' }] })]);
    const second = source('two', [thread('same', { title: 'Unrelated' })]);
    expect(rows(projectMobileHome([first, second], now, { query: '#42' })).map(row => row.key)).toEqual(['one:same']);
    const result = projectMobileHome([first, second], now, { query: 'needle', messageMatches: new Map([['two:same', { snippet: 'the needle is here' }]]) });
    expect(rows(result).map(row => row.key)).toEqual(['two:same']);
    expect(rows(result)[0]?.searchExcerpt).toBe('the needle is here');
    const empty = projectMobileHome([first, second], now, { query: 'absent' });
    expect(empty.emptyTitle).toBe('No results'); expect(empty.emptyDetail).toBe('No threads matching "absent".');
  });

  test('mobile timestamps and project override fallbacks use source presentation rules', () => {
    expect(mobileRelativeTime(at(1), now)).toBe('<1m'); expect(mobileRelativeTime('invalid', now)).toBe('<1m');
    expect(mobileRelativeTime(at(-60), now)).toBe('1h'); expect(mobileRelativeTime(at(-1500), now)).toBe('1d');
    const input = source('one', [thread('icon')]);
    input.shell.projects[0] = { id: 'project', title: 'T3 Code', projectIcon: { kind: 'lucide', name: 'code', color: 'blue' } };
    const icon = rows(projectMobileHome([input], now))[0]!;
    expect(icon.iconKind).toBe('monogram'); expect(icon.iconText).toBe('T3'); expect(icon.iconColor).toBe('#2b7fff');
  });
});

describe('live home sources and empty-state catalog', () => {
  test('reads only synchronized background shells and keeps focused cached data on reconnect', () => {
    const client = new T3Client(), background = new EnvironmentFleet();
    client.environmentId = 'one'; client.shellLoaded = true; client.connection = 'reconnecting'; client.shell = source('one', [thread('real')]).shell;
    const entry: FleetEntry = { key: 'two-key', environmentId: 'two', origin: 'http://localhost:2', phase: 'connected', generation: 3, synchronized: 2,
      message: '', traceId: '', lastEvent: 0, subscriptions: {}, config: {}, shell: source('two').shell, scopes: [], error: '', requested: true };
    background.entries.set(entry.key, entry);
    expect(mobileHomeSources(client, background).map(item => item.environmentId)).toEqual(['one']);
    entry.synchronized = 3;
    expect(mobileHomeSources(client, background).map(item => item.environmentId)).toEqual(['one', 'two']);
    expect(mobileHome(now, {}, client, background).hasAnyThreads).toBe(true);
  });

  test('empty-state text follows actual connection/catalog facts', () => {
    const base = { loading: false, hasConnections: false, hasReadyEnvironment: false, hasLoadedShell: false, connecting: false, connectionState: 'available', error: '' };
    expect(mobileHomeEmpty(base, 0).title).toBe('No environments connected');
    expect(mobileHomeEmpty({ ...base, loading: true }, 0).loading).toBe(true);
    expect(mobileHomeEmpty({ ...base, hasConnections: true, connectionState: 'unsupported' }, 0).title).toBe('Client not supported');
    expect(mobileHomeEmpty({ ...base, hasConnections: true, connectionState: 'connecting', connecting: true }, 0).title).toBe('Connecting to environment');
    expect(mobileHomeEmpty({ ...base, hasConnections: true, hasLoadedShell: true }, 0).title).toBe('No projects found');
    expect(mobileHomeEmpty({ ...base, hasConnections: true, hasLoadedShell: true }, 1).title).toBe('No threads yet');
  });
});
