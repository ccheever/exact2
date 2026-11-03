// A parent thread's view of its children: approvals a subagent waits on, answered above the composer
// (ChildThreadApprovals.tsx, #28516), and an Antigravity subagent's native children (NativeChildWorkDialog.tsx, #28629),
// at T3 Code 5d3d78ab93.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import { childApprovalCards, childApprovalsPrepare, respondToChildApproval } from '../../src/composer/child-approvals';
import { nativeChildWork, nativeChildWorkView } from '../../src/composer/native-child-work';

const projection = { runtimeRequests: [{ id: 'req-1', status: 'pending', kind: 'approval', createdAt: '2026-11-01T00:00:00Z', responseCapability: { type: 'live' } }],
  turnItems: [{ type: 'approval_request', requestId: 'req-1', prompt: 'rm -rf build', reason: 'Cleans the build', appName: '' }] };
function fixture(requests: Record<string, () => unknown>) {
  const sent: unknown[] = [], reads: string[] = [];
  const client = { ready: true, connection: 'connected', environmentId: 'env', generation: 1, threadId: 'parent', scopes: ['orchestration:operate'],
    request: async (_native: Native, method: string, payload: { threadId: string }) => { reads.push(`${method} ${payload.threadId}`); return requests[method]!(); },
    shell: { threads: [{ id: 'parent' }, { id: 'child', title: 'Reviewer', lineage: { parentThreadId: 'parent', sideChat: true }, pendingRuntimeRequest: { kind: 'approval' }, updatedAt: '1' }] },
    restAccess: () => ({ request: async (method: string, payload: { threadId: string }) => { reads.push(`${method} ${payload.threadId}`); return requests[method]!(); },
      ids: async () => ['command-1'], dispatch: async (_storage: unknown, payload: unknown) => { sent.push(payload); } }) } as unknown as T3Client;
  return { client, sent, reads, native: { available: true } as Native };
}

describe('child approvals', () => {
  test('a card per pending approval of a child, read once per change, answering the child\'s own request', async () => {
    const { client, sent, reads, native } = fixture({ 'orchestration.getThreadProjection': () => projection });
    await childApprovalsPrepare(client, native);
    await childApprovalsPrepare(client, native);
    expect(reads).toEqual(['orchestration.getThreadProjection child']);
    expect(childApprovalCards(client)).toMatchObject([{ threadId: 'child', requestId: 'req-1', title: 'Reviewer', detail: 'rm -rf build', reason: 'Cleans the build',
      live: true, operable: true, options: [{ id: 'decline' }, { id: 'accept' }], more: [{ id: 'cancel' }, { id: 'acceptForSession' }] }]);
    await respondToChildApproval(client, native, {} as never, 'child\treq-1', 'accept');
    expect(sent).toEqual([{ type: 'runtime-request.respond', commandId: 'command-1', threadId: 'child', requestId: 'req-1', decision: 'accept' }]);
    // The answered child is read again.
    await childApprovalsPrepare(client, native);
    expect(reads).toHaveLength(2);
  });
});

describe('subagent work', () => {
  test('reads the native children while open; an empty answer and a failure read as the reference does', async () => {
    const answers: unknown[] = [{ children: [{ conversationId: 'c1', role: '', prompt: 'Find the bug', truncated: true,
      steps: [{ index: 0, type: 'TOOL_CALL', status: 'RUNNING', text: null, toolCalls: [{ name: 'grep', input: 'foo' }] }, { index: 1, type: 'PLANNER_RESPONSE', status: 'DONE', text: 'Found it', toolCalls: [] }] },
      { conversationId: 'c2', role: 'Writer', prompt: 'Write', truncated: false, steps: null }] }];
    const { client, native } = fixture({ 'orchestration.getNativeChildWork': () => { const next = answers.shift(); if (next instanceof Error) throw next; return next; } });
    expect(nativeChildWorkView(client).open).toBe(false);
    await nativeChildWork(client, native, 'open');
    expect(nativeChildWorkView(client)).toEqual({ open: true, loading: false, error: '', children: [
      { key: 'c1', role: 'Subagent', prompt: 'Find the bug', truncated: true, transcript: true, steps: [
        { key: '0', label: 'Tool call · Running', calls: 'grep foo', text: '', hasText: false }, { key: '1', label: 'Planner response', calls: '', text: 'Found it', hasText: true }] },
      { key: 'c2', role: 'Writer', prompt: 'Write', truncated: false, transcript: false, steps: [] }] });
    answers.push(new Error('Unknown method'));
    await nativeChildWork(client, native, 'refresh');
    expect(nativeChildWorkView(client)).toMatchObject({ open: true, error: 'Unknown method' });
    await nativeChildWork(client, native, 'close');
    expect(nativeChildWorkView(client).open).toBe(false);
  });
});


test('a failed child approval read retries without a shell change', async () => {
  let attempts = 0;
  const { client, native } = fixture({ 'orchestration.getThreadProjection': () => {
    if (attempts++ === 0) throw new Error('Disconnected');
    return projection;
  } });
  await childApprovalsPrepare(client, native);
  expect(childApprovalCards(client)).toEqual([]);
  await childApprovalsPrepare(client, native);
  expect(childApprovalCards(client)).toHaveLength(1);
});

test('native child work does not cross an environment change or reopen on return', async () => {
  let finish!: (value: unknown) => void;
  const { client, native } = fixture({ 'orchestration.getNativeChildWork': () => new Promise(resolve => { finish = resolve; }) });
  const loading = nativeChildWork(client, native, 'open');
  expect(nativeChildWorkView(client).loading).toBe(true);
  client.environmentId = 'other';
  expect(nativeChildWorkView(client).open).toBe(false);
  finish({ children: [{ conversationId: 'old', role: 'Old environment', steps: null }] });
  await loading;
  client.environmentId = 'env';
  expect(nativeChildWorkView(client)).toEqual({ open: false, loading: false, error: '', children: [] });
});
