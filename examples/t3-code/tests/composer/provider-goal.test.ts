import { expect, test } from 'bun:test';
import { opened, thread } from '../support/composer-controls-fixture';
import { composerCommand } from '../../src/composer/composer-controls-commands';
import { composerNotices } from '../../src/composer/composer-controls-view';
import { topStatus } from '../../src/sidebar/sidebar-model';
import { obj } from '../../src/client/domain';
test('goal banner keeps usage beside the title, shows only idle actions and preserves the draft', async () => {
  const { client, native, disk } = await opened();
  client.shell.threads = [thread('t1', { goal: { objective: 'Fix parser', status: 'paused', tokensUsed: 12000, checks: 2 } })];
  client.local.drafts[client.draftKey] = 'Keep my current draft';
  client.local.snapshotDrafts[client.draftKey] = [{ id: 'kept-image' }];
  const notice = composerNotices(client, 0).find(notice => notice.id === 'goal:t1');
  expect(notice).toMatchObject({ title: 'Goal paused · 12k tokens · 2 checks', description: 'Fix parser', action: 'cc:goal-resume', action2: 'cc:goal-clear' });
  await composerCommand(client, native, disk, 'goal-resume', '', '');
  expect(native.committed.at(-1)).toMatchObject({ type: 'message.dispatch', text: '/goal resume', attachments: [], dispatchMode: { type: 'start_immediately' } });
  expect(client.draft).toBe('Keep my current draft'); expect(client.snapshotDrafts).toEqual([{ id: 'kept-image' }]);
  client.projection.runs = [{ id: 'running', status: 'running' }];
  expect(composerNotices(client, 0).find(notice => notice.id === 'goal:t1')).toMatchObject({ action: '', action2: '' });
  await expect(composerCommand(client, native, disk, 'goal-clear', '', '')).rejects.toThrow('Wait for the current turn');
  expect(topStatus('working', false, false, true)?.label).toBe('Goal');
  expect(topStatus('working', false, false, false)?.label).toBe('Working');
  expect(obj(native.committed.at(-1)).modelSelection).toBeDefined();
});

test('runless provider-native work hides and refuses goal actions', async () => {
  const { client, native, disk } = await opened();
  client.shell.threads = [thread('t1', { goal: { objective: 'Inspect parser', status: 'active' } })];
  client.projection.runs = [];
  Object.assign(obj(client.projection.thread), { creationSource: 'provider', lineage: { relationshipToParent: 'subagent' } });
  client.projection.nodes = [{ id: 'root', kind: 'root_turn', runId: null, status: 'running', startedAt: '2026-01-01T00:00:00Z' }];
  expect(composerNotices(client, 0).find(notice => notice.id === 'goal:t1')).toMatchObject({ title: 'Pursuing goal', action: '', action2: '' });
  await expect(composerCommand(client, native, disk, 'goal-clear', '', '')).rejects.toThrow('Wait for the current turn');
  expect(native.committed).toHaveLength(0);
});
