// Lineage rows at T3 Code 5d3d78ab93 (MIT, see LICENSE-T3): an idle child the shell left out stays enabled and
// keeps its parent edge once opened (#26762), a subagent's mark carries its account badge (#24833), and every row's
// tooltip repeats the title it truncates (#28326).
import { expect, test } from 'bun:test';
import { lineageView } from '../../src/details/shell-lineage';
import type { Obj } from '../../src/client/domain';

const at = (minutes: number) => new Date(Date.UTC(2026, 10, 1, 10, minutes)).toISOString();
const shell = (id: string, extra: Obj = {}): Obj => ({ id, title: `T ${id}`, status: 'idle', createdAt: at(0), lineage: { parentThreadId: null, relationshipToParent: null }, ...extra });

test('an idle child left out of the shell is a row that opens, with the plain hint', () => {
  const projection = { thread: { id: 'parent' }, subagents: [{ id: 'agent', childThreadId: 'omitted', status: 'completed', title: 'Idle child', origin: 'app_owned' }] };
  const view = lineageView([shell('parent')], projection, 'parent', [], Date.parse(at(5)));
  expect(view.lineage).toHaveLength(1);
  expect(view.lineage[0]).toMatchObject({ id: 'omitted', title: 'Idle child', missing: true, hint: 'Open subagent in this chat' });
});

test.each(['fork', 'subagent'] as const)('an opened %s omitted from the shell keeps its parent edge', relationship => {
  const thread = { id: 'omitted', title: 'Child', forkedFrom: relationship === 'fork' ? { type: 'run', threadId: 'parent', runId: 'run' } : null,
    lineage: { rootThreadId: 'parent', parentThreadId: 'parent', relationshipToParent: relationship } };
  const view = lineageView([shell('parent')], { thread, subagents: [], contextTransfers: [] }, 'omitted', [], Date.parse(at(5)));
  expect(view.lineage.map(row => [row.id, row.icon])).toEqual([['parent', 'corner-left-up']]);
  expect(view.mergeTargetId).toBe(relationship === 'fork' ? 'parent' : '');
});

test('a subagent on one of several accounts draws its badge; a fork tooltip shows its title over the hint', () => {
  const providers = [{ instanceId: 'codex', driver: 'codex', displayName: 'Codex', enabled: true },
    { instanceId: 'codex_work', driver: 'codex', displayName: 'Work Codex', enabled: true }];
  const projection = { thread: { id: 'me' }, subagents: [{ id: 'a', childThreadId: 'kid', status: 'completed', title: 'Review', origin: 'app_owned',
    driver: 'codex', providerInstanceId: 'codex_work', model: 'gpt-5', startedAt: at(1), completedAt: at(3), result: 'All good' }] };
  const threads = [shell('me'), shell('kid', { lineage: { parentThreadId: 'me', relationshipToParent: 'subagent' } }),
    shell('fork', { title: 'A fork with a long title', lineage: { parentThreadId: 'me', relationshipToParent: 'fork' } })];
  const rows = lineageView(threads, projection, 'me', providers, Date.parse(at(5))).lineage;
  const agent = rows.find(row => row.id === 'kid')!, fork = rows.find(row => row.id === 'fork')!;
  expect(agent).toMatchObject({ driver: 'codex', badge: 'WC', accent: '', markIcon: '' });
  expect(agent.tipLines).toEqual(['GPT-5 · Work Codex', 'Done', '2m 00s', 'All good', 'codex']);
  expect(fork).toMatchObject({ driver: '', badge: '', tipLines: ['Open fork in this chat'] });
  const single = lineageView(threads, projection, 'me', providers.slice(1), Date.parse(at(5))).lineage.find(row => row.id === 'kid')!;
  expect(single.badge).toBe('');
});
