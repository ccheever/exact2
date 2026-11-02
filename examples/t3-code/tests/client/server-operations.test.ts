// The client operations 0.1.5 and 0.1.6 servers add (server-operations.ts; T3 Code 2359643cff, MIT, see LICENSE-T3:
// packages/contracts preview.ts, server.ts, project.ts, pullRequest.ts, orchestrationV2.ts, clientRpcPermissions.ts): each request's
// method and payload, the scope it needs, and the reference's sentence for each typed refusal.
import { describe, expect, test } from 'bun:test';
import { obj, type Obj } from '../../src/client/domain';
import { failure } from '../../src/transport/transport-wire';
import { CLIENT_VERSION } from '../../src/connections/version-skew';
import { connected } from '../support/client-fixture';

async function recording(scopes: string[], answer: (method: string) => Obj = () => ({})) {
  const context = await connected(), calls: Obj[] = [], later = context.native.later.bind(context.native);
  context.client.scopes = scopes;
  context.native.later = async request => {
    const call = obj(request);
    if (call.op === 'request' && ['preview.stopLocalServer', 'server.prepareTriage', 'projects.createFromThread', 'pullRequests.update', 'orchestration.forceStopThread'].includes(String(call.method))) {
      calls.push({ method: call.method, payload: call.payload });
      return context.native.good(answer(String(call.method)));
    }
    return later(request);
  };
  return { ...context, calls };
}

describe('the operations a 0.1.5 server adds', () => {
  test('each sends its method and payload', async () => {
    const { client, native, calls } = await recording(['orchestration:read', 'orchestration:operate', 'terminal:operate', 'source-control:write'],
      method => method === 'server.prepareTriage' ? { prompt: 'Read [triage-playbook.md](/t3/triage/triage-playbook.md)' } : method === 'projects.createFromThread' ? { projectId: 'p9' } : {});
    await client.stopLocalServer(native, { pid: 4242, port: 5173 });
    expect(await client.prepareTriage(native)).toBe('Read [triage-playbook.md](/t3/triage/triage-playbook.md)');
    expect(await client.createProjectFromThread(native, 't1')).toBe('p9');
    const reference = { host: 'github.com', repository: 'o/r', number: 7 };
    await client.setMaintainerEdits(native, reference, false);
    expect(calls).toEqual([
      { method: 'preview.stopLocalServer', payload: { pid: 4242, port: 5173 } },
      { method: 'server.prepareTriage', payload: { client: { label: 'T3 Code Desktop', deviceType: 'desktop', os: 'macOS', surface: 'desktop', appVersion: CLIENT_VERSION } } },
      { method: 'projects.createFromThread', payload: { threadId: 't1' } },
      { method: 'pullRequests.update', payload: { ...reference, maintainerCanModify: false } },
    ]);
  });

  test('a port out of range or a missing thread never reaches the server', async () => {
    const { client, native, calls } = await recording(['orchestration:read', 'orchestration:operate', 'terminal:operate']);
    await expect(client.stopLocalServer(native, { pid: 1, port: 70_000 })).rejects.toThrow('That server is not a running local server.');
    await expect(client.createProjectFromThread(native, '')).rejects.toThrow('Choose a thread.');
    expect(calls).toEqual([]);
  });

  test('stopping a server needs terminal operate, as closing a terminal does', async () => {
    const { client, native, calls } = await recording(['orchestration:read', 'orchestration:operate']);
    await expect(client.stopLocalServer(native, { pid: 4242, port: 5173 })).rejects.toThrow();
    expect(calls).toEqual([]);
  });

  test('each refusal reads the reference sentence', () => {
    const stop = (reason: string) => failure({ _tag: 'Fail', error: { _tag: 'PreviewStopLocalServerError', reason } }).message;
    expect(['not-found', 'shared-group', 'unsupported', 'signal-failed'].map(stop)).toEqual([
      'This server is no longer running.',
      "This server shares a process group with T3 Code, an agent, or a terminal shell, so it can't be stopped on its own.",
      "Stopping servers is not supported on this environment's platform.",
      "The server's processes could not be signaled.",
    ]);
    expect(failure({ _tag: 'Fail', error: { _tag: 'ServerPrepareTriageError' } }).message).toBe('Could not write the triage session files.');
  });
});

describe('the operation a 0.1.6 server adds', () => {
  test('Force stop names the thread and the run the user saw ignore Stop, and reads the outcome', async () => {
    const { client, native, calls } = await recording(['orchestration:read', 'orchestration:operate'],
      () => ({ outcome: 'shared', sharedWithThreadIds: ['t2', 7] }));
    expect(await client.forceStopThread(native, 't1', 'r3')).toEqual({ outcome: 'shared', sharedWithThreadIds: ['t2'] });
    await client.forceStopThread(native, 't1');
    expect(calls).toEqual([{ method: 'orchestration.forceStopThread', payload: { threadId: 't1', runId: 'r3' } }, { method: 'orchestration.forceStopThread', payload: { threadId: 't1' } }]);
  });

  test('it needs orchestration operate; an unknown outcome is refused; the refusals read the reference sentences', async () => {
    const read = await recording(['orchestration:read']);
    await expect(read.client.forceStopThread(read.native, 't1')).rejects.toThrow('This connection requires orchestration:operate.');
    expect(read.calls).toEqual([]);
    const odd = await recording(['orchestration:read', 'orchestration:operate'], () => ({ outcome: 'vanished' }));
    await expect(odd.client.forceStopThread(odd.native, 't1')).rejects.toThrow('The server did not say how the agent was stopped.');
    const refusal = (error: Obj) => failure({ _tag: 'Fail', error: { _tag: 'OrchestrationV2ForceStopThreadError', threadId: 't1', ...error } }).message;
    expect([refusal({ reason: 'superseded' }), refusal({})]).toEqual(['A new turn started, so nothing was force stopped.', 'Failed to force stop the agent.']);
  });
});
