// A Claude native task is stopped by its own identifier; its owner's turn must keep running.
import { expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import { ClientError, type Files, type Native } from '../../src/client/protocol';
import { shellCommand } from '../../src/shell/shell-commands';
import { toasts } from '../../src/shell/toast';
const native = { available: true } as unknown as Native, files = {} as Files;
const agent: Obj = { id: 'native-task', childThreadId: 'native-child', threadId: 'owner', runId: 'owner-run', origin: 'provider_native', driver: 'claudeAgent',
  nativeTaskRef: { nativeId: 'claude-task', strength: 'strong' }, status: 'running', startedAt: '2026-10-18T12:00:00.000Z' };
function fixture(options: { fresh?: Obj; writable?: boolean; refuse?: boolean } = {}) {
  const requests: unknown[][] = [], writes: Obj[] = [];
  const client = { threadId: 'owner', projection: { subagents: [agent] }, writable: options.writable !== false, error: '', generation: 1,
    shell: { threads: [{ id: 'owner' }, { id: 'native-child' }] },
    restAccess: () => ({
      request: async (method: string, payload: Obj, write = false) => { requests.push([method, payload, write]);
        if (method === 'orchestration.getThreadProjection') return { subagents: [options.fresh ?? agent], runs: [{ id: 'owner-run', status: 'running' }] };
        if (options.refuse) { client.error = 'Workflow cannot be stopped.'; throw new ClientError(client.error, 'server'); }
        return {};
      },
      ids: async () => ['command'], dispatch: async (_storage: Files, payload: Obj) => { writes.push(payload); return {}; },
    }),
  };
  return { client: client as unknown as T3Client, requests, writes };
}

test('native Stop dispatches subagent.stop with the fresh owner projection and never run.interrupt', async () => {
  const { client, requests, writes } = fixture({ fresh: { ...agent, runId: 'fresh-run' } });
  await shellCommand(client, native, files, 'stop-subagent', 'native-child', '');
  expect(requests).toEqual([['orchestration.getThreadProjection', { threadId: 'owner' }, false]]);
  expect(writes).toEqual([{ type: 'subagent.stop', commandId: 'command', threadId: 'owner', runId: 'fresh-run', subagentId: 'native-task' }]);
  for (const change of [{ status: 'completed' }, { nativeTaskRef: { nativeId: 'task', strength: 'weak' } }, { runId: null }]) {
    const stale = fixture({ fresh: { ...agent, ...change } });
    await shellCommand(stale.client, native, files, 'stop-subagent', 'native-child', '');
    expect(stale.writes).toEqual([]);
  }
});

test('workflow Stop calls only its coordinator RPC as a write, respects read-only access, and toasts a refusal', async () => {
  const happy = fixture();
  await shellCommand(happy.client, native, files, 'stop-workflow', 'native-task', '');
  expect(happy.requests).toEqual([['orchestration.stopWorkflow', { threadId: 'owner', subagentId: 'native-task' }, true]]);
  expect(happy.writes).toEqual([]);
  const nativeReadonly = fixture({ writable: false });
  await shellCommand(nativeReadonly.client, native, files, 'stop-subagent', 'native-child', '');
  expect(nativeReadonly.requests).toEqual([]);
  const readonly = fixture({ writable: false });
  await shellCommand(readonly.client, native, files, 'stop-workflow', 'native-task', '');
  expect(readonly.requests).toEqual([]);
  const failure = fixture({ refuse: true });
  await shellCommand(failure.client, native, files, 'stop-workflow', 'native-task', '');
  expect(toasts(failure.client).map(toast => [toast.title, toast.description])).toEqual([['Could not stop workflow', '']]);
  expect(failure.client.error).toBe('');
});
