// A sent draft still starting its thread: ChatView onSend marks it promoted at once, so the new-thread shortcut opens a
// fresh draft beside the launch; a failed launch restores the draft as "Could not start thread" (T3 Code 74541bab17, MIT;
// see LICENSE-T3).
import { describe, expect, test } from 'bun:test';
import { connected } from '../support/composer-controls-fixture';
import { toasts } from '../../src/shell/toast';
import { obj, type Obj } from '../../src/client/domain';
import { draftLaunching } from '../../src/composer/draft-launch';
import { draftThreadId } from '../../src/composer/draft-thread-id';
import { contractComponent, type Env } from '../support/contract-action-fixture';

/** Holds the launch's reply until `release`, as a worktree setup does; `answer` replaces the reply. */
async function launching(answer?: (reply: unknown) => unknown) {
  const context = await connected();
  let release: () => void = () => {};
  const gate = new Promise<void>(resolve => { release = resolve; });
  const later = context.native.later.bind(context.native);
  context.native.later = async (input: unknown) => {
    const request = obj(input);
    if (request.op === 'request' && request.method === 'orchestration.launchThread') {
      await gate;
      const reply = await later(input);
      return answer ? answer(reply) : reply;
    }
    return later(input);
  };
  return { ...context, release: () => release() };
}
const settle = () => new Promise(resolve => setTimeout(resolve, 10));

describe('the new-thread shortcut while a sent draft starts its thread', () => {
  test('opens a fresh draft; the launch finishes beside it and does not take the window', async () => {
    const { client, native, disk, release } = await launching();
    client.local.composerControls.draftThreads = { 'env1:new:p1': 'reserved-thread' };
    const sending = client.command('send', '', 'Set up the worktree', 0, native, disk);
    await settle();
    expect(draftLaunching(client)).toBe(true);
    expect(client.draft).toBe('Set up the worktree');
    const fresh = await client.command('new-thread', '', '', 0, native, disk);
    expect(fresh.message).toBe('');
    // The composer is the project's fresh draft: none of the sent text, a new thread id next time, not uncertain.
    expect(client).toMatchObject({ threadId: '', projectId: 'p1', draft: '' });
    expect(draftThreadId(client, 'env1:new:p1')).toBe('');
    expect(client.pending?.uncertain).toBe(false);
    expect(draftLaunching(client)).toBe(false);
    release();
    expect((await sending).message).toBe('');
    expect(client.pending).toBeUndefined();
    expect(client).toMatchObject({ threadId: '', error: '' });
    expect(native.committed.filter(command => command.method === 'orchestration.launchThread')).toHaveLength(1);
  });
  test('a launch that then fails restores its draft and offers it in a toast', async () => {
    const { client, native, disk, release } = await launching(reply => ({ ...(reply as Obj), ok: false, error: { kind: 'server', message: 'Worktree setup failed.' } }));
    const sending = client.command('send', '', 'Set up the worktree', 0, native, disk);
    await settle();
    await client.command('new-thread', '', '', 0, native, disk);
    client.local.drafts['env1:new:p1'] = 'Typed meanwhile';
    release();
    expect((await sending).message).toBe('');
    expect(client.pending).toBeUndefined();
    expect(client.error).toBe('');
    expect(client.draft).toBe('Typed meanwhile\n\nSet up the worktree');
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'error', title: 'Could not start thread', description: 'Worktree setup failed.', action: { label: 'Open draft', op: 'new-thread' } });
  });
  test('any other command while the launch is in flight still leaves it uncertain', async () => {
    const { client, native, disk, release } = await launching();
    const sending = client.command('send', '', 'Set up the worktree', 0, native, disk);
    await settle();
    await client.command('history', '', '', 0, native, disk).catch(() => undefined);
    expect(client.pending?.uncertain).toBe(true);
    release();
    await sending;
  });
});

describe('app.contract: the shortcut while the launch is in flight', () => {
  test('opens the fresh draft on its own send; with nothing launching it waits for the running command', () => {
    const root = contractComponent('app.contract', 'T3Code');
    const calls: string[] = [];
    const record = (name: string) => (...args: unknown[]) => { calls.push(`${name}:${args.join(',')}`); return name; };
    const state: Env = { commandPending: true, draftOwner: '', jumpRequest: 0, utilityPage: 'usage' };
    const scope = { command: record('command'), focus: record('focus') } as Env;
    scope.draftBeside = () => root.call('draftBeside', state, scope, []);
    root.call('newThread', state, { ...scope, commandPending: true, data: { projectId: 'p1', draftLaunching: false } }, []);
    expect(calls).toEqual([]);
    root.call('newThread', state, { ...scope, commandPending: true, data: { projectId: 'p1', draftLaunching: true } }, []);
    expect(calls).toEqual(['command:new-thread,,,0', 'focus:composer']);
    expect(state).toMatchObject({ commandPending: true, draftOwner: '__unset__', jumpRequest: 1, utilityPage: '' });
  });
});
