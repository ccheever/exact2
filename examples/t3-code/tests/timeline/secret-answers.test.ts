import { expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { Backend, storage } from '../support/client-fixture';
import { obj, type Obj } from '../../src/client/domain';
import { secretAnswerKey, secretAnswerState, secretAnswersNeeded, refreshSecretAnswer } from '../../src/timeline/secret-answers';

class SecretBackend extends Backend {
  answers: Obj[] = [];
  error = '';
  async later(input: unknown): Promise<unknown> {
    const request = obj(input);
    if (request.op === 'request' && request.method === 'secrets.answerRequest') {
      this.answers.push(structuredClone(obj(request.payload)));
      return this.error ? this.bad(this.error) : this.good({});
    }
    return super.later(input);
  }
}
async function fixture(visibility = 'local') {
  const client = new T3Client(), native = new SecretBackend(), disk = storage();
  await client.refresh(native, disk.files); await client.command('select-thread', 't1', '', 0, native, disk.files); await client.refresh(native, disk.files);
  const item = { id: 'secret-card', threadId: 't1', type: 'secret_request', secretStatus: 'pending', label: 'Webhook key', reason: 'Sign deliveries' };
  client.projection.visibleTurnItems = [{ sourceThreadId: 't1', sourceItemId: item.id, position: 0, visibility, item }];
  return { client, native, disk, key: secretAnswerKey(client, 't1', item.id), item };
}
test('private answers bypass persisted pending operations and never enter thread drafts', async () => {
  const { client, native, disk, key } = await fixture();
  await client.command('secretlocal:save', key, ' private-fixture-value ', 0, native, disk.files);
  expect(secretAnswersNeeded(client)).toBe(true);
  expect(JSON.stringify(disk.data())).not.toContain('private-fixture-value');
  expect(client.pending).toBeUndefined(); expect(client.draft).toBe('');
  await refreshSecretAnswer(client, native);
  expect(native.answers).toEqual([{ threadId: 't1', turnItemId: 'secret-card', answer: { type: 'save', secret: 'private-fixture-value' } }]);
  expect(secretAnswerState(client, 't1', 'secret-card')).toEqual({ pending: false, submitted: true, error: '' });
  expect(JSON.stringify(disk.data())).not.toContain('private-fixture-value');
});
test('duplicate submissions coalesce, inherited requests reject answers, and payload-bearing failures stay private', async () => {
  const { client, native, disk, key } = await fixture();
  native.error = 'Bad payload contained private-fixture-value';
  await client.command('secretlocal:save', key, 'private-fixture-value', 0, native, disk.files);
  await client.command('secretlocal:save', key, 'second-value', 0, native, disk.files);
  await refreshSecretAnswer(client, native);
  expect(native.answers).toHaveLength(1);
  expect(secretAnswerState(client, 't1', 'secret-card').error).toBe('Could not answer the request. Try again.');
  expect(client.error).not.toContain('private-fixture-value');
  const inherited = await fixture('inherited');
  const result = await inherited.client.command('secretlocal:save', inherited.key, 'never-sent', 0, inherited.native, inherited.disk.files);
  expect(result.message).toBe('This secret request no longer accepts an answer.');
  expect(secretAnswersNeeded(inherited.client)).toBe(false);
});
test('queued answers are discarded when their environment generation changes', async () => {
  const { client, native, disk, key } = await fixture();
  await client.command('secretlocal:decline', key, '', 0, native, disk.files);
  client.generation++;
  await refreshSecretAnswer(client, native);
  expect(native.answers).toHaveLength(0); expect(secretAnswersNeeded(client)).toBe(false);
});

test('projected local and inherited secret requests stay standalone through native view serialization', async () => {
  const { transcriptRows, timelineMessages } = await import('../../src/timeline/timeline-presentation');
  for (const visibility of ['local', 'inherited']) {
    const { client, item } = await fixture(visibility);
    Object.assign(item, { ordinal: 1, runId: null, nodeId: null, status: 'pending', startedAt: '2026-01-01T00:00:00.000Z', updatedAt: '2026-01-01T00:00:00.000Z' });
    const rows = timelineMessages(client, transcriptRows(client), Date.parse('2026-01-01T00:00:00.000Z'));
    expect(rows.filter(row => row.kind === 'secret')).toHaveLength(1);
    expect(rows.find(row => row.kind === 'secret')?.secret[0]).toMatchObject({ label: 'Webhook key', kind: visibility === 'local' ? 'pending' : 'pending-elsewhere',
      privacy: 'Stored securely, never shown to the agent' });
    expect(JSON.stringify(rows)).not.toContain('private-fixture-value');
  }
});

test("a file secret's card does not promise to hide a value the agent's commands can read (#24809)", async () => {
  const { transcriptRows, timelineMessages } = await import('../../src/timeline/timeline-presentation');
  const { client, item } = await fixture('local');
  Object.assign(item, { ordinal: 1, runId: null, nodeId: null, status: 'pending', delivery: 'file', startedAt: '2026-01-01T00:00:00.000Z', updatedAt: '2026-01-01T00:00:00.000Z' });
  const card = timelineMessages(client, transcriptRows(client), Date.parse('2026-01-01T00:00:00.000Z')).find(row => row.kind === 'secret')?.secret[0];
  expect(card?.privacy).toBe("Kept out of chat, in a private file this agent's commands can read for 24 hours");
});
