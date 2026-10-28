// The composer's sends a 0.1.4 server reads (T3 Code 7c3b3b3605ef, MIT, see LICENSE-T3): a typed `/compact` with
// instructions is the compaction itself (ChatView.logic.ts COMPACT_COMMAND_PATTERN), T3 Code's own `/compact` says
// it ran after the thread was idle (useComposerSend.ts `idleCompaction: "onReturn"`), and a message sent over an
// approval whose agent takes feedback declines it with that feedback (threadRequests.ts sendApprovalFeedback).
import { describe, expect, test } from 'bun:test';
import { arr, obj, str, type Obj } from '../../src/client/domain';
import { snapshot } from '../../src/client/presentation';
import { connected, opened } from '../support/composer-controls-fixture';
import { toasts } from '../../src/shell/toast';

const NOW = Date.parse('2026-10-03T12:00:30.000Z');
const minutesAgo = (minutes: number) => new Date(NOW - minutes * 60_000).toISOString();

/** A Claude thread idle six minutes past its five-minute cache, 234k tokens in: the Compact chip is offered. */
async function staleClaudeThread() {
  const context = await connected();
  const claude = arr(context.native.config.providers).find(provider => provider.instanceId === 'claude')!;
  claude.slashCommands = [{ name: 'compact' }];
  const projection = obj(context.native.details.t1!.projection);
  Object.assign(obj(projection.thread), { modelSelection: { instanceId: 'claude', model: 'model-a' }, activeProviderThreadId: 'pt' });
  projection.providerThreads = [{ id: 'pt', appThreadId: 't1', providerInstanceId: 'claude', updatedAt: minutesAgo(6), contextUsage: { usedTokens: 234_567, maxTokens: 1_000_000 } }];
  await context.command('select-thread', 't1');
  await context.client.refresh(context.native, context.disk);
  const loaded = context.client.thread!.projection;
  loaded.providerTurns = [{ tokenUsage: { usedTokens: 234_567, maxTokens: 1_000_000, updatedAt: minutesAgo(6), promptCacheTtlMs: 300_000 } }];
  loaded.runs = [{ id: 'r1', ordinal: 1, status: 'completed', startedAt: minutesAgo(10), completedAt: minutesAgo(6) }];
  loaded.visibleTurnItems = [{ item: { type: 'user_message', text: 'Build the parser', attachments: [] } }];
  snapshot(context.client, NOW);
  return context;
}
const dispatched = (committed: Obj[]) => committed.filter(entry => entry.type === 'message.dispatch');

describe('the compact-first send', () => {
  test('/compact with instructions sends no bare /compact first', async () => {
    for (const text of ['/compact keep the API notes', '/COMPACT', '/compact\n  tests']) {
      const { native, command } = await staleClaudeThread();
      await command('send', '', text);
      expect(dispatched(native.committed).map(entry => entry.text)).toEqual([text.trim()]);
    }
  });

  test('T3 Code\'s own /compact before a message says it ran on return', async () => {
    const { native, command } = await staleClaudeThread();
    await command('send', '', '/compactfoo');
    const sent = dispatched(native.committed);
    expect(sent.map(entry => [entry.text, entry.idleCompaction])).toEqual([['/compact', 'onReturn'], ['/compactfoo', undefined]]);
  });
});

describe('feedback on an approval', () => {
  async function waiting(supportsFeedback: boolean) {
    const context = await opened();
    const projection = context.client.thread!.projection;
    projection.runs = [{ id: 'r1', ordinal: 1, status: 'waiting', startedAt: '2026-10-03T00:00:00Z' }];
    projection.runtimeRequests = [{ id: 'a', kind: 'command', status: 'pending', createdAt: '2026-10-03T00:00:01Z', responseCapability: { type: 'live', providerSessionId: 'ps', ...(supportsFeedback ? { supportsFeedback: true } : {}) } }];
    projection.turnItems = [{ id: 'item-a', type: 'approval_request', requestId: 'a', prompt: 'rm -rf build' }];
    return context;
  }
  const writes = (committed: Obj[]) => committed.filter(entry => entry.method === 'orchestration.dispatchCommand')
    .map(entry => entry.type === 'runtime-request.respond' ? { decision: entry.decision, requestId: entry.requestId, feedback: entry.feedback } : { type: entry.type, text: entry.text });

  test('the message declines the approval as its feedback, and the draft clears', async () => {
    const { client, native, command } = await waiting(true);
    await command('draft', '', '  Use the staging bucket instead.  ');
    await command('send', '', '  Use the staging bucket instead.  ');
    expect(writes(native.committed)).toEqual([{ decision: 'decline', requestId: 'a', feedback: 'Use the staging bucket instead.' }]);
    expect(client.draft).toBe('');
  });

  test('an approval without feedback is declined and the message redirects the agent, as before', async () => {
    const { native, command } = await waiting(false);
    await command('send', '', 'Use the other file');
    expect(writes(native.committed)).toEqual([{ decision: 'decline', requestId: 'a', feedback: undefined }, { type: 'message.dispatch', text: 'Use the other file' }]);
  });

  // useComposerSend.ts: a draft edited since Send, or an approval already being answered, ends the send unsent, without a toast.
  test('a draft edited since Send is not sent as feedback, and stays', async () => {
    const { client, native, command } = await waiting(true);
    const later = native.later.bind(native);
    native.later = async (input: unknown) => {
      if (obj(input).op === 'ids') client.local.drafts[client.draftKey] = 'Use the staging bucket, and';
      return later(input);
    };
    await command('draft', '', 'Use the staging bucket');
    await command('send', '', 'Use the staging bucket');
    expect(writes(native.committed)).toEqual([]);
    expect([client.draft, client.error, toasts(client).length]).toEqual(['Use the staging bucket, and', '', 0]);
  });

  test('an approval already being answered takes no feedback, and the draft stays', async () => {
    const { client, native, command } = await waiting(true);
    const later = native.later.bind(native);
    native.later = async (input: unknown) => {
      if (obj(input).op === 'ids') client.local.pending[client.environmentId] = { method: 'orchestration.dispatchCommand', description: 'Approve',
        payload: { type: 'runtime-request.respond', threadId: client.threadId, requestId: 'a', decision: 'accept' }, threadId: client.threadId, text: '', uncertain: false };
      return later(input);
    };
    await command('draft', '', 'Use the staging bucket');
    await command('send', '', 'Use the staging bucket');
    expect(writes(native.committed)).toEqual([]);
    expect([client.draft, client.error, toasts(client).length]).toEqual(['Use the staging bucket', '', 0]);
  });

  test('feedback refuses attachments', async () => {
    const { client, native, command } = await waiting(true);
    client.local.snapshotDrafts[client.draftKey] = [{ id: 'img-1', name: 'shot.png', mimeType: 'image/png', sizeBytes: 9, uploadId: 'up-1' }];
    const result = await command('send', '', 'Use the staging bucket');
    expect(str(result.message) || client.error).toContain('Send approval feedback without attachments or added context.');
    expect(writes(native.committed)).toEqual([]);
  });
});
