// The composer while an approval is pending (T3 Code 74541bab17 a93b03c8a5): it stays usable, and Send declines the
// shown approvals before it sends the message as a redirect.
import { describe, expect, test } from 'bun:test';
import { arr, obj, str, type Obj } from '../../src/client/domain';
import { snapshot } from '../../src/client/presentation';
import { toasts } from '../../src/shell/toast';
import { noteComposerTyping } from '../../src/composer/composer-typing-guard';
import { composerInput, requestPresentation } from '../../src/composer/requests';
import { stageTextAttachment } from '../../src/composer/composer-editor-files';
import { applyStaged, decodeComposerControls, reconcileStagedModels, stagedFor } from '../../src/composer/composer-controls';
import { opened } from '../support/composer-controls-fixture';
import { EnvironmentFleet, fleet, type FleetEntry } from '../../src/connections/settings-b-fleet';
import { contextId, contextLink } from '../../src/composer/composer-editor-menu';
import { composerChipFaces } from '../../src/composer/composer-chip-presentation';
import { UNREADABLE_THREAD_TIP, attachedThreads, machineLabel, readsEveryThread, unreadableThreads } from '../../src/composer/composer-thread-machines';

const approval = (id: string, capability = 'live'): Obj => ({ id, kind: 'command', status: 'pending', createdAt: `2026-10-03T00:00:0${id.length}Z`, responseCapability: { type: capability } });

async function waiting(...approvals: Obj[]) {
  const context = await opened();
  const projection = context.client.thread!.projection;
  projection.runs = [{ id: 'r1', ordinal: 1, status: 'waiting', startedAt: '2026-10-03T00:00:00Z' }];
  projection.runtimeRequests = approvals;
  projection.turnItems = approvals.map(entry => ({ id: `item-${str(entry.id)}`, type: 'approval_request', requestId: entry.id, prompt: 'rm -rf build' }));
  return context;
}
const writes = (committed: Obj[]) => committed.filter(entry => entry.method === 'orchestration.dispatchCommand')
  .map(entry => entry.type === 'runtime-request.respond' ? `${str(entry.decision)} ${str(entry.requestId)}` : `${str(entry.type)} ${str(entry.text)} ${str(entry.deliveryIntent) || str(obj(entry.dispatchMode).type)}`);
const resolve = (projection: Obj, id: string) => { for (const request of arr(projection.runtimeRequests)) if (request.id === id) request.status = 'resolved'; };

describe('composer during a pending approval', () => {
  test('the composer stays usable: its placeholder offers the redirect and Send steers', async () => {
    const { client } = await waiting(approval('a'));
    const composer = snapshot(client).composer;
    expect(composer.placeholder).toBe('Send a message to decline and redirect the agent');
    expect(composer.sendLabel).toBe('Steer message');
    expect(requestPresentation(client).requestMode).toBe('approval');
  });
  test('Send declines every shown approval first, then sends the message as a steer', async () => {
    const { client, native, command } = await waiting(approval('a'), approval('bb'));
    client.local.clientSettings.followUpBehavior = 'queue';
    const result = await command('send', '', 'Use the other file');
    expect(result.message).toBe('');
    expect(writes(native.committed)).toEqual(['decline a', 'decline bb', 'message.dispatch Use the other file steer']);
  });
  test('an approval the provider can no longer answer is not declined', async () => {
    const { native, command } = await waiting(approval('a', 'not_resumable'));
    await command('send', '', 'Try again');
    expect(writes(native.committed)).toEqual(['message.dispatch Try again steer']);
  });
  test('a refused decline sends nothing and keeps the draft', async () => {
    const { client, native, command } = await waiting(approval('a'));
    const later = native.later.bind(native);
    native.later = async input => {
      const payload = obj(obj(input).payload);
      if (payload.type === 'runtime-request.respond') return { ok: false, generation: native.generation, error: { kind: 'rejected', message: 'The request is no longer pending.', uncertain: false } };
      return later(input);
    };
    const result = await command('send', '', 'Use the other file');
    expect(result.message).toContain('no longer pending');
    expect(writes(native.committed)).toEqual([]);
    expect(client.draft).toBe('Use the other file');
  });
  test('a draft edited during the decline is not sent, and a toast says why', async () => {
    const { client, native, command } = await waiting(approval('a'));
    const later = native.later.bind(native);
    native.later = async input => {
      const reply = await later(input);
      if (obj(obj(input).payload).type === 'runtime-request.respond') client.local.drafts[client.draftKey] = 'Use the other file, and';
      return reply;
    };
    await command('send', '', 'Use the other file');
    expect(writes(native.committed)).toEqual(['decline a']);
    expect(client.draft).toBe('Use the other file, and');
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'info', title: "Approval declined. Your message changed, so it wasn't sent.", description: "Send again when it's ready." });
  });
  test('an approval that arrives during the decline keeps the draft for the next send', async () => {
    const { client, native, command } = await waiting(approval('a'));
    const projection = client.thread!.projection, later = native.later.bind(native);
    native.later = async input => {
      const reply = await later(input);
      if (obj(obj(input).payload).type === 'runtime-request.respond') {
        resolve(projection, 'a');
        projection.runtimeRequests = [...arr(projection.runtimeRequests), approval('ccc')];
      }
      return reply;
    };
    await command('send', '', 'Use the other file');
    expect(writes(native.committed)).toEqual(['decline a']);
    expect(client.draft).toBe('Use the other file');
    expect(toasts(client).at(-1)?.title).toBe("Another approval arrived, so your message wasn't sent.");
    native.later = later;
    await command('send', '', 'Use the other file');
    expect(writes(native.committed)).toEqual(['decline a', 'decline ccc', 'message.dispatch Use the other file steer']);
  });
  test('with a question pending too, the composer is the thread draft, and Send redirects with it', async () => {
    const { client, native, command } = await waiting(approval('a'));
    const projection = client.thread!.projection;
    projection.runtimeRequests = [...arr(projection.runtimeRequests), { id: 'q1', kind: 'user_input', status: 'pending', createdAt: '2026-10-03T00:00:09Z', responseCapability: { type: 'live' } }];
    projection.turnItems = [...arr(projection.turnItems), { id: 'item-q', type: 'user_input_request', requestId: 'q1', questions: [{ id: 's', header: 'S', question: 'Which?', options: [{ label: 'X', description: 'x' }] }] }];
    await command('draft', '', 'redirect text');
    expect(composerInput(client)).toBeUndefined();
    expect(client.draft).toBe('redirect text');
    await command('send', '', 'redirect text');
    expect(writes(native.committed)).toEqual(['decline a', 'message.dispatch redirect text steer']);
  });
  test('an approval answered while the send prepares keeps the draft unsent, and a toast says why', async () => {
    const { client, native, command } = await waiting(approval('a'));
    const later = native.later.bind(native);
    native.later = async input => {
      // The approval's own Approve press starts while the send takes its ids.
      if (obj(input).op === 'ids' && !client.local.pending[client.environmentId]) client.local.pending[client.environmentId] = { method: 'orchestration.dispatchCommand',
        payload: { type: 'runtime-request.respond', requestId: 'a', decision: 'accept' }, description: 'Approval', threadId: 't1', text: '', uncertain: false };
      return later(input);
    };
    await command('send', '', 'Use the other file');
    expect(writes(native.committed)).toEqual([]);
    expect(client.draft).toBe('Use the other file');
    expect(toasts(client).at(-1)?.title).toBe("An approval is already being answered, so your message wasn't sent.");
  });
  test('a file attached during the decline counts as a changed draft', async () => {
    const { client, native, command } = await waiting(approval('a'));
    const later = native.later.bind(native);
    native.later = async input => {
      const reply = await later(input);
      if (obj(obj(input).payload).type === 'runtime-request.respond') stageTextAttachment(client, 'notes', 'notes.txt', 'text/plain', 'late-file');
      return reply;
    };
    await command('send', '', 'Use the other file');
    expect(writes(native.committed)).toEqual(['decline a']);
    expect(toasts(client).at(-1)?.title).toBe("Approval declined. Your message changed, so it wasn't sent.");
  });
  test('leaving the thread during the decline ends the send silently', async () => {
    const { client, native, command } = await waiting(approval('a'), approval('bb'));
    const later = native.later.bind(native);
    native.later = async input => {
      const reply = await later(input);
      if (obj(obj(input).payload).type === 'runtime-request.respond') client.threadId = 't2';
      return reply;
    };
    const before = toasts(client).length;
    const result = await command('send', '', 'Use the other file');
    expect(result.message).toBe('');
    expect(writes(native.committed)).toEqual(['decline a']);
    expect(toasts(client).length).toBe(before);
  });
  test('an approval held while the user typed stays pending, and the send follows the setting', async () => {
    const { client, native, command } = await waiting();
    requestPresentation(client);
    noteComposerTyping(client, true);
    client.thread!.projection.runtimeRequests = [approval('a')];
    client.thread!.projection.turnItems = [{ id: 'item-a', type: 'approval_request', requestId: 'a', prompt: 'ls' }];
    expect(requestPresentation(client).requestMode).toBe('');
    await command('send', '', 'Keep going');
    expect(writes(native.committed)).toEqual(['message.dispatch Keep going queue_after_active']);
  });
});

describe('question answer drafts (b177a16b69)', () => {
  const question = { id: 'scope', header: 'Scope', question: 'Which scope?', options: [{ label: 'Workspace', description: 'The workspace.' }] };
  const asking = (projection: Obj, id = 'q1') => {
    projection.runtimeRequests = [{ id, kind: 'user_input', status: 'pending', createdAt: '2026-10-03T00:00:00Z', responseCapability: { type: 'live' } }];
    projection.turnItems = [{ id: `item-${id}`, type: 'user_input_request', requestId: id, questions: [question] }];
  };
  test('a typed answer survives opening a new thread and coming back', async () => {
    const context = await opened();
    asking(context.client.thread!.projection);
    await context.command('draft', '', 'Only the parser');
    expect(requestPresentation(context.client).questions[0]?.answer).toBe('Only the parser');
    await context.command('new-thread', 'p1');
    expect(context.client.threadId).toBe('');
    await context.command('select-thread', 't1');
    await context.client.refresh(context.native, context.disk);
    expect(context.client.threadId).toBe('t1');
    asking(context.client.thread!.projection);
    expect(requestPresentation(context.client).questions[0]?.answer).toBe('Only the parser');
  });
  test('a resolved request drops its draft; a thread whose detail has not arrived keeps it', async () => {
    const { client } = await opened();
    const projection = client.thread!.projection;
    asking(projection);
    client.answers['q1::scope'] = { selected: [], custom: 'Draft', index: 0 };
    const kept = client.thread;
    client.thread = null;
    requestPresentation(client);
    expect(client.answers['q1::scope']?.custom).toBe('Draft');
    client.thread = kept;
    (projection.runtimeRequests as Obj[])[0]!.status = 'resolved';
    requestPresentation(client);
    expect(client.answers['q1::scope']).toBeUndefined();
  });
});

describe('drafts follow model changes made on other clients (2767012996)', () => {
  test('a staged model the thread now runs is dropped, and the composer follows the thread again', async () => {
    const { client, command } = await opened();
    await command('model', 'model-b');
    expect(stagedFor(client)).toMatchObject({ providerId: 'codex', modelId: 'model-b' });
    // Another client picks the same model; the web's fast mode off counts as unset.
    Object.assign(client.shell.threads[0]!, { modelSelection: { instanceId: 'codex', model: 'model-b', options: [{ id: 'fastMode', value: false }] } });
    client.shellLive = true;
    expect(reconcileStagedModels(client)).toBe(true);
    expect(stagedFor(client)).toBeUndefined();
  });
  test('staged modes stay when the model is dropped, and the thread model shows through', async () => {
    const { client, command } = await opened();
    await command('model', 'model-b');
    await command('runtime', '', 'approval-required');
    Object.assign(client.shell.threads[0]!, { modelSelection: { instanceId: 'codex', model: 'model-b' } });
    client.shellLive = true;
    expect(reconcileStagedModels(client)).toBe(true);
    expect(stagedFor(client)).toMatchObject({ providerId: '', runtimeMode: 'approval-required' });
    Object.assign(client, { providerId: 'codex', modelId: 'model-a' });
    applyStaged(client);
    expect([client.modelId, client.runtimeMode]).toEqual(['model-a', 'approval-required']);
    expect(decodeComposerControls({ staged: { 'env1:t1': stagedFor(client) } }).staged['env1:t1']).toMatchObject({ providerId: '', runtimeMode: 'approval-required' });
  });
  test('a different pick, or a shell that is not live, keeps the staged model', async () => {
    const { client, command } = await opened();
    await command('model', 'model-b');
    Object.assign(client.shell.threads[0]!, { modelSelection: { instanceId: 'codex', model: 'model-b', options: [{ id: 'fastMode', value: true }] } });
    client.shellLive = true;
    expect(reconcileStagedModels(client)).toBe(false);
    Object.assign(client.shell.threads[0]!, { modelSelection: { instanceId: 'codex', model: 'model-b' } });
    client.shellLive = false;
    expect(reconcileStagedModels(client)).toBe(false);
    expect(stagedFor(client)).toMatchObject({ modelId: 'model-b' });
  });
});

describe('attached threads on another machine (c1377176b1)', () => {
  const studio = { key: 'studio', origin: 'http://studio.local:3773', environmentId: 'env2', phase: 'connected', message: '', traceId: '', generation: 1, synchronized: 1,
    lastEvent: 0, subscriptions: {}, config: { environment: { label: 'Studio Mac' } }, shell: { projects: [], threads: [{ id: 'th-x', title: 'Parser work' }] }, scopes: [], error: '', requested: false } as unknown as FleetEntry;
  const chip = (threadId: string, label: string) => contextLink('thread', contextId('thread', threadId), label);
  test('a chip names the machine whose shell holds its thread', async () => {
    const { client } = await opened();
    const source = new EnvironmentFleet();
    source.entries.set('studio', studio);
    const text = `Compare ${chip('t2', 'Thread t2')} with ${chip('th-x', 'Parser work')} and ${chip('gone', 'Gone')}`;
    expect(attachedThreads(client, text, source).map(thread => [thread.threadId, thread.environmentId])).toEqual([['t2', 'env1'], ['th-x', 'env2']]);
    expect(unreadableThreads(client, text, source).map(thread => thread.title)).toEqual(['Parser work']);
    expect(machineLabel('env2', source)).toBe('Studio Mac');
    expect(machineLabel('env9', source)).toBe('its machine');
    const threads = attachedThreads(client, text, source);
    expect([readsEveryThread(threads, 'env1'), readsEveryThread(threads.slice(1), 'env2')]).toEqual([false, true]);
  });
  test('Send refuses a thread this machine cannot read, and its chip draws invalid', async () => {
    const { client, native, command } = await opened();
    fleet.entries.set('studio', studio);
    try {
      const text = `Use ${chip('th-x', 'Parser work')}`;
      await command('send', '', text);
      expect(writes(native.committed)).toEqual([]);
      expect(toasts(client).at(-1)).toMatchObject({ kind: 'warning', title: 'Attached thread is on another machine',
        description: 'This machine\'s agent can\'t read "Parser work". Remove it, or switch back to Studio Mac to send.' });
      const [face] = composerChipFaces(client, text, [{ kind: 'context', contextKind: 'thread', contextId: contextId('thread', 'th-x'), label: 'Parser work', start: 4, end: text.length, source: text.slice(4), image: false }] as never);
      expect(face).toMatchObject({ kind: 'invalid', icon: 'messages-square', tip: UNREADABLE_THREAD_TIP, interactive: false });
    } finally { fleet.entries.delete('studio'); }
  });
});
