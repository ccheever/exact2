// Lane composer-controls: selection memory, follow-up dispatch, primary
// actions, docked banners and the footer layout, against a fake T3 backend.
import { describe, expect, test } from 'bun:test';
import { T3Client } from './client';
import { snapshot, modelCatalog } from './presentation';
import { arr, obj, str, type Obj } from './domain';
import type { Native, Files } from './protocol';
import { resolveDispatchMode, withDispatchMode, submissionIntent, resolvePlanSubmission, proposedPlanTitle, decodeComposerControls, latestProposedPlan, resumeState } from './composer-controls';
import { applyOptionChoice, providerLock, wokeAt } from './composer-controls-commands';
import { footerLayout, textWidth, traitsDisplay, presentBackgroundWork, tasksProgress } from './composer-controls-view';
import { traitsMenu } from './composer-presentation';
import { toasts } from './toast';
import { queueState, queuedEdit } from './composer-controls-queue';
import { composerBranches } from './composer-controls-branch';

import { at, effort, fast, provider, thread, projection, Fake, storage, connected, opened, running } from './composer-controls-fixture';

describe('sticky model and next-turn selection', () => {
  test('a new draft opens on the last picked model and its remembered traits, ahead of the default', async () => {
    const { client, command } = await opened();
    await command('model', 'model-b', 'codex-work');
    expect([client.providerId, client.modelId]).toEqual(['codex-work', 'model-b']);
    await command('model-option', 'reasoningEffort', 'high');
    await command('new-thread');
    expect([client.threadId, client.providerId, client.modelId]).toEqual(['', 'codex-work', 'model-b']);
    expect(client.modelOptions).toEqual([{ id: 'reasoningEffort', value: 'high' }]);
    // Coming back to a model restores its own traits, not the previous model's.
    await command('model', 'model-a', 'codex-work');
    expect(client.modelOptions).toEqual([]);
    await command('model', 'model-b', 'codex-work');
    expect(client.modelOptions).toEqual([{ id: 'reasoningEffort', value: 'high' }]);
  });
  test('sticky memory survives a reload and ignores an unavailable provider', () => {
    const saved = decodeComposerControls({ stickyProvider: 'gone', stickyByProvider: { gone: { model: 'x', options: [{ id: 'a', value: true }, { bad: 1 }] } } });
    expect(saved.stickyByProvider.gone).toEqual({ model: 'x', options: [{ id: 'a', value: true }] });
  });
  test('traits and runtime stay enabled during a run: changes stage for the next message', async () => {
    const { client, native, command } = await opened();
    running(client);
    const before = native.committed.length;
    await command('model-option', 'reasoningEffort', 'low');
    await command('runtime', '', 'approval-required');
    expect(native.committed.length).toBe(before);
    expect(client.error).toBe('');
    expect(snapshot(client).composer).toMatchObject({ traitsLabel: 'Low', traitsSpeed: '', runtimeLabel: 'Supervised' });
    // A thread event re-reads the thread's own selection; the staged choice still owns the composer.
    native.emit('thread', { kind: 'event', sequence: 2, event: { id: 'e2', type: 'thread.updated', threadId: 't1', occurredAt: at, payload: {} } });
    await client.refresh(native, storage());
    expect(client.runtimeMode).toBe('approval-required');
    client.thread!.projection.runs = [{ id: 'r1', ordinal: 1, status: 'completed' }];
    await command('send', '', 'Follow up');
    const sent = native.committed.slice(before);
    expect(sent.map(entry => entry.type)).toEqual(['thread.runtime-mode.set', 'message.dispatch']);
    expect(sent[1]).toMatchObject({ modelSelection: { instanceId: 'codex', model: 'model-a', options: [{ id: 'reasoningEffort', value: 'low' }] },
      deliveryIntent: 'auto', dispatchMode: { type: 'start_immediately' } });
  });
  test('boolean traits are offered and applied', () => {
    expect(applyOptionChoice([effort, fast], [], 'fastMode', 'true')).toEqual([{ id: 'fastMode', value: true }]);
    expect(() => applyOptionChoice([effort, fast], [], 'fastMode', 'maybe')).toThrow('no longer offered');
    const menu = traitsMenu([effort, fast], [{ id: 'fastMode', value: true }]);
    expect(menu.items.filter(item => item.descriptor === 'fastMode').map(item => [item.kind, item.label, item.selected])).toEqual([
      ['divider', '', false], ['header', 'Fast mode', false], ['option', 'On', true], ['option', 'Off', false]]);
    expect(traitsDisplay('codex', [effort, fast], [{ id: 'fastMode', value: true }])).toEqual({ label: 'Medium', speed: 'fast' });
    expect(traitsDisplay('claudeAgent', [fast], [])).toEqual({ label: 'Normal', speed: '' });
    const tier = { id: 'serviceTier', type: 'select', options: [{ id: 'default', label: 'Normal' }, { id: 'priority', label: 'Fast' }, { id: 'flex', label: 'Ultrafast' }] };
    expect(traitsDisplay('codex', [effort, tier], [{ id: 'serviceTier', value: 'flex' }])).toEqual({ label: 'Medium', speed: 'ultrafast' });
    expect(traitsDisplay('codex', [effort, { id: 'thinking', label: 'Thinking', type: 'boolean', currentValue: true }], [])).toEqual({ label: 'Medium · Thinking On', speed: '' });
  });
  test('a started thread locks the picker to its driver; drafts are unrestricted', async () => {
    const { client, command } = await opened();
    expect(modelCatalog(client, '', '').providers.map(rail => [rail.id, rail.ready])).toEqual([['codex', true], ['codex-work', true], ['claude', true]]);
    client.thread!.projection.runs = [{ id: 'r1', ordinal: 1, status: 'completed' }];
    expect(providerLock(client)).toEqual({ driver: 'codex', group: '' });
    expect(modelCatalog(client, '', '').providers.map(rail => [rail.id, rail.ready])).toEqual([['codex', true], ['codex-work', true], ['claude', false]]);
    expect(modelCatalog(client, '', 'model').models.every(row => row.providerId !== 'claude')).toBe(true);
    await command('model', 'model-a', 'claude');
    expect(client.error).toContain('continues with its current provider');
    client.thread!.projection.providerSessions = [{ id: 's1', capabilities: { sessions: { supportsProviderSwitchingViaHandoff: true } } }];
    client.thread!.projection.providerThreads = [{ id: 'pt', appThreadId: 't1', providerSessionId: 's1' }];
    expect(providerLock(client)).toBeNull();
  });
});

describe('follow-up dispatch', () => {
  test('the configured behavior and its alternate pick queue or steer', () => {
    expect(resolveDispatchMode(false, 'queue', true)).toBe('auto');
    expect(resolveDispatchMode(true, 'queue', false)).toBe('queue');
    expect(resolveDispatchMode(true, 'queue', true)).toBe('steer');
    expect(resolveDispatchMode(true, 'steer', false)).toBe('steer');
    expect(withDispatchMode({ deliveryIntent: 'auto', dispatchMode: { type: 'start_immediately' } }, 'queue')).toEqual({ dispatchMode: { type: 'queue_after_active' } });
    expect(withDispatchMode({}, 'steer')).toEqual({ deliveryIntent: 'steer', dispatchMode: { type: 'start_immediately' } });
    expect(submissionIntent({ modifiers: 'meta', source: 'key', ageMs: 5 }, true, false)).toBe('alternate');
    expect(submissionIntent({ modifiers: 'meta', source: 'pointer', ageMs: 5 }, false, false)).toBe('foreground');
    expect(submissionIntent({ modifiers: 'meta+alt', source: 'key', ageMs: 5 }, false, true)).toBe('background');
    expect(submissionIntent({ modifiers: 'meta', source: 'key', ageMs: 5000 }, true, false)).toBe('foreground');
  });
  test('a message sent during a run queues by default, steers with ⌘ or the Steer setting', async () => {
    const { client, native, command } = await opened();
    running(client);
    await command('send', '', 'Queued one');
    expect(native.committed.at(-1)).toMatchObject({ type: 'message.dispatch', dispatchMode: { type: 'queue_after_active' } });
    expect(native.committed.at(-1)!.deliveryIntent).toBeUndefined();
    native.gesture = { modifiers: 'meta', source: 'pointer', ageMs: 10 };
    await command('send', '', 'Steer one');
    expect(native.committed.at(-1)).toMatchObject({ deliveryIntent: 'steer', dispatchMode: { type: 'start_immediately' } });
    await command('device-setting', 'followUpBehavior', 'steer');
    client.local.clientSettings.followUpBehavior = 'steer';
    await command('send', '', 'Steer two');
    expect(native.committed.at(-1)).toMatchObject({ deliveryIntent: 'steer' });
    expect(snapshot(client).composer).toMatchObject({ sendRunning: true, sendLabel: 'Steer message', sendIcon: 'corner-up-right' });
    client.local.clientSettings.followUpBehavior = 'queue';
    expect(snapshot(client).composer).toMatchObject({ sendLabel: 'Queue message', sendIcon: 'list-plus', sendTooltip: 'Click to queue, Ctrl/⌘-click or ⌘Enter to steer' });
    client.presentation = { ...client.presentation, modifiers: 'meta' };
    expect(snapshot(client).composer).toMatchObject({ sendLabel: 'Steer message', sendIcon: 'corner-up-right' });
  });
  test('⌥⌘↩ in a draft starts the thread in the background and keeps a fresh draft', async () => {
    const { client, native, command } = await connected();
    native.gesture = { modifiers: 'meta+alt', source: 'key', ageMs: 3 };
    await command('send', '', 'Background work');
    expect(native.committed.at(-1)).toMatchObject({ method: 'orchestration.launchThread' });
    expect(client.threadId).toBe('');
    expect(client.draft).toBe('');
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'success', title: 'Started in background', action: { label: 'Open', op: 'select-thread' } });
  });
  test('Resume continues an interrupted run, then releases a held queue', async () => {
    const { client, native, command } = await opened();
    client.thread!.projection.runs = [{ id: 'r1', ordinal: 1, status: 'interrupted', startedAt: at }, { id: 'r2', ordinal: 2, status: 'queued', queueHeld: true }];
    expect(resumeState(client.projection)).toEqual({ runId: 'r1', heldQueue: true });
    expect(snapshot(client).composer.canResume).toBe(true);
    await command('cc:resume');
    expect(native.committed.slice(-2)).toEqual([
      expect.objectContaining({ type: 'message.dispatch', text: 'Continue where you left off.', manualContinuationOfRunId: 'r1', dispatchMode: { type: 'start_immediately' } }),
      expect.objectContaining({ type: 'queue.resume', threadId: 't1' })]);
  });
});

describe('plan follow-up', () => {
  const plan = (status = 'active'): Obj => ({ id: 'plan-1', kind: 'proposed_plan', runId: 'r1', status, markdown: '# Ship the parser\n\n- step' });
  test('Implement sends the plan in default mode; Refine sends the draft in plan mode', async () => {
    expect(proposedPlanTitle('intro\n## Ship it\nbody')).toBe('Ship it');
    expect(resolvePlanSubmission('', '# T\nbody')).toEqual({ text: 'PLEASE IMPLEMENT THIS PLAN:\n# T\nbody', interactionMode: 'default' });
    expect(resolvePlanSubmission(' tighten step 2 ', 'x')).toEqual({ text: 'tighten step 2', interactionMode: 'plan' });
    const { client, native, command } = await opened();
    client.thread!.projection.runs = [{ id: 'r1', ordinal: 1, status: 'completed' }];
    client.thread!.projection.plans = [plan()];
    (client.thread!.projection.thread as Obj).interactionMode = 'plan';
    client.interactionMode = 'plan';
    expect(latestProposedPlan(client.projection)?.id).toBe('plan-1');
    expect(snapshot(client).composer).toMatchObject({ planReady: true, planTitle: 'Ship the parser',
      placeholder: 'Add feedback to refine the plan, or leave this blank to implement it' });
    await command('send', '', '');
    expect(native.committed.slice(-2).map(entry => entry.type)).toEqual(['thread.interaction-mode.set', 'message.dispatch']);
    expect(native.committed.at(-1)).toMatchObject({ text: 'PLEASE IMPLEMENT THIS PLAN:\n# Ship the parser\n\n- step', sourcePlanRef: { threadId: 't1', planId: 'plan-1' } });
    client.interactionMode = 'plan';
    await command('send', '', 'Refine step 2');
    expect(native.committed.at(-1)).toMatchObject({ type: 'message.dispatch', text: 'Refine step 2' });
    expect(native.committed.at(-1)!.sourcePlanRef).toBeUndefined();
    client.thread!.projection.plans = [plan('implemented')];
    expect(snapshot(client).composer.planReady).toBe(false);
  });
  test('Implement in a new thread launches a default-mode thread titled after the plan', async () => {
    const { client, native, command } = await opened();
    client.thread!.projection.runs = [{ id: 'r1', ordinal: 1, status: 'completed' }];
    client.thread!.projection.plans = [plan()];
    client.interactionMode = 'plan';
    await command('cc:implement-new');
    expect(native.committed.at(-1)).toMatchObject({ method: 'orchestration.launchThread', title: 'Implement Ship the parser', interactionMode: 'default',
      initialMessage: expect.objectContaining({ text: 'PLEASE IMPLEMENT THIS PLAN:\n# Ship the parser\n\n- step' }) });
  });
});

describe('docked banners, activity and the provider control', () => {
  test('snoozed and woke notices, with Wake now and dismissal', async () => {
    const { client, native, command } = await opened();
    const now = Date.now();
    Object.assign(client.shell.threads[0]!, { snoozedUntil: new Date(now + 3_600_000).toISOString(), snoozedAt: new Date(now - 1000).toISOString() });
    expect(snapshot(client, now).composer.notices.map(notice => [notice.title, notice.description, notice.actionLabel, notice.front])).toEqual([
      ['This thread is snoozed', 'Send a message to wake', 'Wake now', true]]);
    await command('cc:unsnooze');
    expect(native.committed.at(-1)).toMatchObject({ type: 'thread.unsnooze', threadId: 't1', reason: 'user' });
    Object.assign(client.shell.threads[0]!, { snoozedUntil: new Date(now - 60_000).toISOString() });
    expect(wokeAt(client.shell.threads[0]!, now)).toBe(new Date(now - 60_000).toISOString());
    expect(snapshot(client, now).composer.notices.map(notice => [notice.title, notice.dismissLabel])).toEqual([['Thread woke from snooze', 'Dismiss Woke notification']]);
    await command('cclocal:dismiss-woke');
    expect(snapshot(client, now).composer.notices).toEqual([]);
    Object.assign(client.shell.threads[0]!, { settledOverride: 'settled', snoozedUntil: null });
    expect(snapshot(client, now).composer.notices.map(notice => [notice.title, notice.actionLabel, notice.action])).toEqual([['This thread is settled', 'Un-settle', 'unsettle']]);
  });
  test('several notices: the first stays attached, the rest wait behind the peek; activity leads when present', async () => {
    const { client } = await opened();
    Object.assign(client.shell.threads[0]!, { settledOverride: 'settled', pendingBackgroundTasks: [{ taskId: 'a', kind: 'command', description: 'npm test' }] });
    const notices = snapshot(client).composer.notices;
    expect(notices.map(notice => [notice.title, notice.front])).toEqual([['Running: npm test', true], ['This thread is settled', false]]);
    running(client);
    client.thread!.projection.plans = [{ id: 'todo', kind: 'todo_list', runId: 'r1', steps: [{ text: 'Read', status: 'completed', durationMs: 1200 }, { text: 'Write', status: 'running' }, { text: 'Test', status: 'pending' }] }];
    const view = snapshot(client).composer;
    expect(view).toMatchObject({ activity: 'tasks', tasksTotal: 3, tasksDone: 1, tasksCurrent: 'Write' });
    expect(view.tasks.map(step => [step.text, step.status, step.duration])).toEqual([['Read', 'completed', '1.2s'], ['Write', 'inProgress', 'now'], ['Test', 'pending', '']]);
    expect(view.notices.every(notice => !notice.front)).toBe(true);
    expect(presentBackgroundWork([{ kind: 'subagent' }, { kind: 'command' }, { kind: 'command' }])!.title).toBe('Waiting on 1 subagent and 2 commands');
    client.thread!.projection.runs = [];
    expect(tasksProgress(client).tasksTotal).toBe(0);
  });
  test('the provider control replaces the picker when nothing is available', async () => {
    const { client, command } = await opened();
    for (const entry of arr(client.config.providers)) entry.status = 'error';
    expect(snapshot(client).composer).toMatchObject({ noProvider: 'Open provider settings', providerSetupId: 'codex' });
    client.config.providers = [];
    // The thread still asks for its own instance (unavailableProviderInstanceId).
    expect(snapshot(client).composer).toMatchObject({ noProvider: 'Open provider settings', providerSetupId: 'codex' });
    await command('new-thread');
    client.providerId = ''; client.config.settings = {};
    expect(snapshot(client).composer).toMatchObject({ noProvider: 'No provider available', providerSetupId: '' });
    client.config.providers = [{ instanceId: 'claude', driver: 'claudeAgent', enabled: true, status: 'error', setup: { canAuthenticate: true }, models: [] }];
    expect(snapshot(client).composer).toMatchObject({ noProvider: 'Open provider settings', providerSetupId: 'claude' });
  });
  test('trailing labels compact before the model name', () => {
    expect(Math.round(textWidth('Medium'))).toBe(53);
    const base = { model: 'GPT-5.6-Luna', traits: 'Medium', traitsIcon: false, runtime: 'Full access', plan: '' };
    expect(footerLayout({ ...base, host: 420 })).toMatchObject({ runtimeIconOnly: false, traitsIconOnly: false });
    expect(footerLayout({ ...base, host: 340 })).toMatchObject({ runtimeIconOnly: true, traitsIconOnly: false });
    expect(footerLayout({ ...base, host: 290 })).toMatchObject({ runtimeIconOnly: true, traitsIconOnly: true });
    expect(footerLayout({ ...base, host: 0 })).toMatchObject({ runtimeIconOnly: false });
  });
});

describe('queued messages control', () => {
  const queued = (client: T3Client) => {
    running(client, { activeAttemptId: 'a1', providerThreadId: 'pt' });
    Object.assign(client.thread!.projection, {
      runs: [...arr(client.thread!.projection.runs), { id: 'q2', ordinal: 3, status: 'queued', queuePosition: 2, userMessageId: 'm2' },
        { id: 'q1', ordinal: 2, status: 'queued', queuePosition: 1, userMessageId: 'm1' }, { id: 'auto', ordinal: 4, status: 'queued', userMessageId: 'm3' }],
      messages: [{ id: 'm1', text: 'First  follow-up' }, { id: 'm2', text: 'Second', attachments: [{ type: 'image', id: 'i' }] }, { id: 'm3', text: 'done', notification: {} }],
      providerThreads: [{ id: 'pt', providerSessionId: 's' }], providerSessions: [{ id: 's', capabilities: { turns: { supportsQueuedMessages: true, supportsActiveSteering: true } } }],
      providerTurns: [{ runAttemptId: 'a1', status: 'running' }] });
  };
  test('lists the user queue in order with steer, edit and remove', async () => {
    const { client, native, command } = await opened();
    queued(client);
    const state = queueState(client.projection);
    expect(state.queued.map(entry => [entry.run.id, entry.text, entry.images])).toEqual([['q1', 'First  follow-up', 0], ['q2', 'Second', 1]]);
    expect(state).toMatchObject({ canReorder: true, canSteer: true });
    const view = snapshot(client).composer;
    expect(view.queued.map(row => [row.runId, row.text, row.index, row.last])).toEqual([['q1', 'First follow-up', 0, false], ['q2', 'Second', 1, true]]);
    expect(view).toMatchObject({ queueSteer: true, queueSteerReason: '' });
    await command('cc:queued-steer', 'q1');
    expect(native.committed.at(-1)).toMatchObject({ type: 'queued-message.promote-to-steer', queuedRunId: 'q1', targetRunId: 'r1' });
    await command('cc:queued-remove', 'q2');
    expect(native.committed.at(-1)).toMatchObject({ type: 'queued-run.cancel', runId: 'q2' });
    client.thread!.projection.runs = arr(client.thread!.projection.runs).filter(run => run.status === 'queued');
    expect(snapshot(client).composer).toMatchObject({ queueSteer: false, queueSteerReason: 'There is no active run to steer' });
  });
  test('editing loads the composer, saves with queued-run.edit and restores the draft; cancel restores too', async () => {
    const { client, native, command } = await opened();
    queued(client);
    await command('draft', '', 'my own draft');
    await command('cclocal:queued-edit', 'q2');
    expect(client.draft).toBe('Second');
    expect(snapshot(client)).toMatchObject({ requestKey: 'queued-edit:q2', composer: { queueEditing: 'q2', sendLabel: 'Update queued message', sendIcon: 'check' } });
    await command('send', '', 'Second, revised');
    expect(native.committed.at(-1)).toMatchObject({ type: 'queued-run.edit', runId: 'q2', text: 'Second, revised' });
    expect(client.draft).toBe('my own draft');
    expect(queuedEdit(client)).toBeUndefined();
    await command('cclocal:queued-edit', 'q1');
    await command('cclocal:queued-cancel');
    expect(client.draft).toBe('my own draft');
    // A run that leaves the queue ends the edit; a dirty edit is dropped while the thread has a draft.
    await command('cclocal:queued-edit', 'q1');
    await command('draft', '', 'changed');
    client.thread!.projection.runs = [];
    expect(snapshot(client).composer.queueEditing).toBe('');
    expect(client.draft).toBe('my own draft');
    expect(toasts(client).at(-1)?.title).toBe('Queued message is no longer queued');
  });
});

describe('workspace and branch strip', () => {
  test('a draft shows Current checkout and the branch; switching checks the ref out on the server', async () => {
    const { client, native, command } = await connected();
    expect(await composerBranches(client, native, false, '')).toMatchObject({ show: true, envMode: 'local', envLabel: 'Current checkout', branchLabel: 'main' });
    const open = await composerBranches(client, native, true, '');
    expect(open.refs.map(ref => [ref.name, ref.badge, ref.selected])).toEqual([['main', 'current', true], ['feature', '', false], ['origin/main', 'remote', false]]);
    expect((await composerBranches(client, native, true, 'feat')).refs.map(ref => ref.name)).toEqual(['feature']);
    expect((await composerBranches(client, native, true, 'brand-new')).creatable).toBe('brand-new');
    await command('cc:branch', '', 'feature');
    expect(native.committed.at(-1)).toMatchObject({ method: 'vcs.switchRef', cwd: '/repo', refName: 'feature' });
    expect((await composerBranches(client, native, false, '')).branchLabel).toBe('feature');
    await command('send', '', 'On feature');
    expect(native.committed.at(-1)).toMatchObject({ method: 'orchestration.launchThread', workspaceStrategy: { type: 'root', branch: 'feature' } });
  });
  test('New worktree launches from its base ref; a started thread hides the strip unless it persists', async () => {
    const { client, native, command } = await connected();
    await command('cclocal:env-mode', '', 'worktree');
    const view = await composerBranches(client, native, true, '');
    expect(view).toMatchObject({ envMode: 'worktree', envLabel: 'New worktree', branchLabel: 'From main' });
    await command('cc:branch', '', 'feature');
    expect(native.committed.some(entry => entry.method === 'vcs.switchRef')).toBe(false);
    expect((await composerBranches(client, native, false, '')).branchLabel).toBe('From feature');
    await command('send', '', 'In a worktree');
    expect(native.committed.at(-1)).toMatchObject({ workspaceStrategy: { type: 'worktree', baseRef: 'feature' } });
    await command('select-thread', 't1');
    expect((await composerBranches(client, native, false, '')).show).toBe(false);
    client.local.clientSettings.persistComposerContextStrip = true;
    expect((await composerBranches(client, native, false, '')).show).toBe(true);
  });
});

describe('usage limits', () => {
  test('"/usage-limits" opens a local notice from the provider snapshot; a new turn or dismissal closes it', async () => {
    const { client, native, command } = await opened();
    const before = native.committed.length;
    await command('send', '', '/usage-limits');
    expect(native.committed.length).toBe(before + 1);
    expect(native.committed.at(-1)).toMatchObject({ type: 'message.dispatch', text: '/usage-limits' });
    const provider = arr(client.config.providers)[0]!;
    Object.assign(provider, { displayName: 'Exact verification fixture', auth: { status: 'authenticated', label: 'OpenAI API Key' },
      usageLimits: { windows: [], unavailable: { reason: 'unsupported' }, checkedAt: at } });
    const count = native.committed.length;
    await command('draft', '', '/usage-limits');
    await command('send', '', '/usage-limits');
    expect(native.committed.length).toBe(count);
    expect(client.draft).toBe('');
    const notice = snapshot(client, Date.now()).composer.notices.find(entry => entry.title === 'Usage limits')!;
    expect(notice).toMatchObject({ description: 'Codex · Exact verification fixture · OpenAI API Key', dismissLabel: 'Dismiss usage limits',
      lines: ['This account has no subscription limits.'] });
    provider.usageLimits = { windows: [{ label: '5h', usedPercent: 25, resetsAt: new Date(Date.now() + 2 * 3_600_000 + 13 * 60_000 + 30_000).toISOString() }], checkedAt: at };
    await command('cclocal:usage-limits');
    expect(snapshot(client, Date.now()).composer.notices.find(entry => entry.title === 'Usage limits')!.lines).toEqual(['5h · 75% left · resets in 2h 13m']);
    await command('cclocal:usage-limits-dismiss');
    expect(snapshot(client, Date.now()).composer.notices.some(entry => entry.title === 'Usage limits')).toBe(false);
  });
  test('a limit-stopped run offers Resume at reset and Snooze until reset', async () => {
    const { client, native, command } = await opened();
    const reset = new Date(Date.now() + 3_600_000).toISOString();
    Object.assign(client.shell.threads[0]!, { status: 'failed', lastErrorClass: 'usage_limit', usageLimitResetAt: reset, latestRunId: 'r9', updatedAt: new Date().toISOString() });
    const notice = snapshot(client, Date.now()).composer.notices[0]!;
    expect(notice).toMatchObject({ title: 'Usage limit reached', variant: 'warning', actionLabel: 'Resume at reset', action2Label: 'Snooze until reset' });
    await command('cc:limit-resume');
    expect(native.committed.at(-1)).toMatchObject({ type: 'thread.metadata.update', limitRecovery: { runId: 'r9', resetAt: reset, autoResume: true } });
    Object.assign(client.shell.threads[0]!, { limitRecovery: { runId: 'r9', resetAt: reset, autoResume: true } });
    expect(snapshot(client, Date.now()).composer.notices[0]!.actionLabel).toBe('Cancel auto-resume');
  });
});
