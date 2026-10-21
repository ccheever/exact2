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

/** Holds the first launch's reply until `release`, as a worktree setup does; `answer` replaces that reply. */
async function launching(answer?: (reply: unknown) => unknown) {
  const context = await connected();
  let release: () => void = () => {};
  const gate = new Promise<void>(resolve => { release = resolve; });
  let enter: () => void = () => {};
  const entered = new Promise<void>(resolve => { enter = resolve; });
  const later = context.native.later.bind(context.native);
  let launches = 0;
  const ops: string[] = [];
  context.native.later = async (input: unknown) => {
    const request = obj(input);
    ops.push(String(request.op));
    if (request.op === 'request' && request.method === 'orchestration.launchThread' && ++launches === 1) {
      enter();
      await gate;
      const reply = await later(input);
      return answer ? answer(reply) : reply;
    }
    return later(input);
  };
  return { ...context, ops, entered, release: () => release() };
}

describe('the new-thread shortcut while a sent draft starts its thread', () => {
  test('opens a fresh draft; the launch finishes beside it and does not take the window', async () => {
    const { client, native, disk, ops, entered, release } = await launching();
    client.local.composerControls.draftThreads = { 'env1:new:p1': 'reserved-thread' };
    const sending = client.command('send', '', 'Set up the worktree', 0, native, disk);
    await entered;
    expect(draftLaunching(client)).toBe(true);
    expect(client.draft).toBe('Set up the worktree');
    const fresh = await client.command('new-thread', '', '', 0, native, disk);
    expect(fresh.message).toBe('');
    // The composer is the project's fresh draft: none of the sent text, a new thread id next time, not uncertain.
    expect(client).toMatchObject({ threadId: '', projectId: 'p1', draft: '' });
    expect(draftThreadId(client, 'env1:new:p1')).toBe('');
    // The launch left the window's pending write for its own map: nothing blocks the fresh draft.
    expect(client.pending).toBeUndefined();
    expect(Object.values(client.local.launches)).toHaveLength(1);
    expect(draftLaunching(client)).toBe(false);
    expect(ops).toContain('snapshotNews');
    release();
    expect((await sending).message).toBe('');
    expect(client.local.launches).toEqual({});
    expect(client).toMatchObject({ threadId: '', error: '' });
    expect(native.committed.filter(command => command.method === 'orchestration.launchThread')).toHaveLength(1);
  });
  test('a launch that then fails restores its draft and offers it in a toast', async () => {
    const { client, native, disk, entered, release } = await launching(reply => ({ ...(reply as Obj), ok: false, error: { kind: 'server', message: 'Worktree setup failed.' } }));
    const sending = client.command('send', '', 'Set up the worktree', 0, native, disk);
    await entered;
    await client.command('new-thread', '', '', 0, native, disk);
    client.local.drafts['env1:new:p1'] = 'Typed meanwhile';
    release();
    expect((await sending).message).toBe('');
    expect(client.pending).toBeUndefined();
    expect(client.error).toBe('');
    expect(client.draft).toBe('Typed meanwhile\n\nSet up the worktree');
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'error', title: 'Could not start thread', description: 'Worktree setup failed.', action: { label: 'Open draft', op: 'new-thread' } });
  });
  test('the fresh draft sends meanwhile; the first launch\'s failure still restores its draft and toasts', async () => {
    const { client, native, disk, entered, release } = await launching(reply => ({ ...(reply as Obj), ok: false, error: { kind: 'server', message: 'Worktree setup failed.' } }));
    const sending = client.command('send', '', 'Set up the worktree', 0, native, disk);
    await entered;
    await client.command('new-thread', '', '', 0, native, disk);
    const second = await client.command('send', '', 'Another thread', 0, native, disk);
    expect(second.message).toBe('');
    expect(native.committed.filter(command => command.method === 'orchestration.launchThread')).toHaveLength(1); // the held one is not in yet
    release();
    await sending;
    expect(client.local.launches).toEqual({});
    expect(client.local.drafts['env1:new:p1']).toBe('Set up the worktree');
    expect(toasts(client).at(-1)).toMatchObject({ title: 'Could not start thread', description: 'Worktree setup failed.' });
  });
  test('a launch whose reply was let go settles once its thread is in the shell, and releases its images', async () => {
    const { client, native, disk, entered } = await launching();
    const sending = client.command('send', '', 'Set up the worktree', 0, native, disk);
    await entered;
    await client.command('new-thread', '', '', 0, native, disk);
    const [launch] = Object.values(client.local.launches);
    launch!.detached = { images: [{ id: '0f0e0d0c-0b0a-4908-8706-050403020100' }] };
    client.shell.threads.push({ id: launch!.payload.threadId, projectId: 'p1', title: 'Set up the worktree' });
    expect(client.reconcilePending()).toBe(true);
    expect(client.local.launches).toEqual({});
    expect(client.local.snapshotReleases).toContain('0f0e0d0c-0b0a-4908-8706-050403020100');
    void sending;
  });
  test('any other command while the launch is in flight still leaves it uncertain', async () => {
    const { client, native, disk, entered, release } = await launching();
    const sending = client.command('send', '', 'Set up the worktree', 0, native, disk);
    await entered;
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
    expect(state).toMatchObject({ commandPending: false, draftOwner: '__unset__', jumpRequest: 1, utilityPage: '' });
  });
});
