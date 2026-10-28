// Lane composer-controls: selection memory, follow-up dispatch, primary
// actions, docked banners and the footer layout, against a fake T3 backend.
import { describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { snapshot, modelCatalog } from '../../src/client/presentation';
import { arr, obj, str, type Obj } from '../../src/client/domain';
import type { Native, Files } from '../../src/client/protocol';
import { noteNow, composerNow, resolveDispatchMode, withDispatchMode, resolvePlanSubmission, proposedPlanTitle, decodeComposerControls, latestProposedPlan, resumeState } from '../../src/composer/composer-controls';
import { applyOptionChoice, providerLock, wokeAt } from '../../src/composer/composer-controls-commands';
import { footerLayout, textWidth, traitsDisplay, presentBackgroundWork } from '../../src/composer/composer-controls-view';
import { queuedTurnRows, threadNowRows } from '../../src/composer/thread-end';
import { traitsMenu, withNoticeDetails } from '../../src/composer/composer-presentation';
import { probeKey } from '../../src/composer/composer-menus';
import { antigravitySendBlockReason } from '../../src/composer/composer-provider-selection';
import { threadStatusLine } from '../../src/timeline/thread-status-line';
import { toasts } from '../../src/shell/toast';
import { queueState, queuedEdit } from '../../src/composer/composer-controls-queue';
import { branchMismatchNotice, composerBranches, resolveLocalCheckoutBranchMismatch, restoreThreadBranch, stripWorkspace } from '../../src/composer/composer-controls-branch';

import { vcsStatusEvent, watchVcsStatus, VCS_STATUS_KEY } from '../../src/details/shell-vcs';
import { at, effort, fast, provider, thread, projection, Fake, storage, connected, opened, running } from '../support/composer-controls-fixture';

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
    expect(snapshot(client).composer).toMatchObject({ traitsLabel: 'Low', runtimeLabel: 'Supervised' });
    // A thread event re-reads the thread's own selection; the staged choice still owns the composer.
    native.emit('thread', { kind: 'event', sequence: 2, event: { id: 'e2', type: 'thread.updated', threadId: 't1', occurredAt: at, payload: {} } });
    await client.refresh(native, storage());
    expect(client.runtimeMode).toBe('approval-required');
    client.thread!.projection.runs = [{ id: 'r1', ordinal: 1, status: 'completed' }];
    await command('send', '', 'Follow up');
    const sent = native.committed.slice(before);
    expect(sent.map(entry => entry.type)).toEqual(['thread.runtime-mode.set', 'message.dispatch']);
    // withImplicitFastModeDefault: model-a offers Fast and the user never chose it, so Normal is explicit.
    expect(sent[1]).toMatchObject({ modelSelection: { instanceId: 'codex', model: 'model-a', options: [{ id: 'reasoningEffort', value: 'low' }, { id: 'fastMode', value: false }] },
      deliveryIntent: 'auto', dispatchMode: { type: 'start_immediately' } });
  });
  test('boolean traits are offered and applied', () => {
    expect(applyOptionChoice([effort, fast], [], 'fastMode', 'true')).toEqual([{ id: 'fastMode', value: true }]);
    expect(() => applyOptionChoice([effort, fast], [], 'fastMode', 'maybe')).toThrow('no longer offered');
    const menu = traitsMenu([effort, fast], [{ id: 'fastMode', value: true }]);
    expect(menu.items.filter(item => item.descriptor === 'fastMode').map(item => [item.kind, item.label, item.selected])).toEqual([
      ['divider', '', false], ['header', 'Fast mode', false], ['option', 'On', true], ['option', 'Off', false]]);
    expect(traitsDisplay('codex', [effort, fast], [{ id: 'fastMode', value: true }])).toEqual({ label: 'Medium Fast' });
    expect(traitsDisplay('claudeAgent', [fast], [])).toEqual({ label: 'Normal' });
    const tier = { id: 'serviceTier', type: 'select', options: [{ id: 'default', label: 'Normal' }, { id: 'priority', label: 'Fast' }, { id: 'flex', label: 'Ultrafast' }] };
    expect(traitsDisplay('codex', [effort, tier], [{ id: 'serviceTier', value: 'flex' }])).toEqual({ label: 'Medium Ultrafast' });
    expect(traitsDisplay('codex', [effort, { id: 'thinking', label: 'Thinking', type: 'boolean', currentValue: true }], [])).toEqual({ label: 'Medium · Thinking On' });
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

describe('the send: settings, title seed, provider and permission (ChatView onSend)', () => {
  test('on an idle server thread, mode and model picks send nothing until the message, which follows the set commands', async () => {
    const { client, native, command } = await opened();
    const before = native.committed.length;
    await command('interaction', '', 'plan');
    await command('runtime', '', 'approval-required');
    await command('model', 'model-b', 'codex');
    expect(native.committed.length).toBe(before);
    expect([client.interactionMode, client.runtimeMode, client.modelId]).toEqual(['plan', 'approval-required', 'model-b']);
    // The picks stay with the thread's draft across a switch.
    await command('select-thread', 't2');
    expect(client.interactionMode).toBe('default');
    await command('select-thread', 't1');
    await client.refresh(native, storage());
    expect([client.interactionMode, client.runtimeMode, client.modelId]).toEqual(['plan', 'approval-required', 'model-b']);
    expect(native.committed.length).toBe(before);
    await command('send', '', 'Now go');
    expect(native.committed.slice(before).map(entry => entry.type)).toEqual(['thread.runtime-mode.set', 'thread.interaction-mode.set', 'message.dispatch']);
    expect(native.committed.at(-1)).toMatchObject({ modelSelection: { instanceId: 'codex', model: 'model-b' } });
  });
  test('every message carries its title seed, on a thread with turns too', async () => {
    const { client, native, command } = await opened();
    await command('send', '', 'Hello **bold** from the clone');
    expect(native.committed.at(-1)).toMatchObject({ type: 'message.dispatch', text: 'Hello **bold** from the clone', titleSeed: 'Hello **bold** from the clone' });
    client.thread!.projection.runs = [{ id: 'r1', ordinal: 1, status: 'completed' }];
    client.thread!.projection.messages = [{ id: 'm1', role: 'user', text: 'Earlier' }];
    await command('send', '', `${'y'.repeat(60)} [notes.md](t3-context://v1/file/file_x)`);
    expect(native.committed.at(-1)!.titleSeed).toBe(`${'y'.repeat(50)}...`);
  });
  test('a listed provider whose probe failed still sends; only a missing provider blocks', async () => {
    const { client, native, command } = await opened();
    const codex = arr(client.config.providers)[0]!;
    Object.assign(codex, { installed: false, status: 'error', availability: 'unavailable', auth: { status: 'unknown' } });
    expect(snapshot(client).canSend).toBe(true);
    await command('send', '', 'Send anyway');
    expect(native.committed.at(-1)).toMatchObject({ type: 'message.dispatch', text: 'Send anyway' });
    codex.models = []; // a probe that failed before listing models (Codex at a missing binary)
    expect(snapshot(client).canSend).toBe(true);
    await command('send', '', 'Send without a catalog');
    expect(native.committed.at(-1)).toMatchObject({ type: 'message.dispatch', text: 'Send without a catalog' });
    for (const entry of arr(client.config.providers)) entry.enabled = false;
    const count = native.committed.length;
    await command('send', '', 'Nowhere to go');
    expect(native.committed.length).toBe(count);
    expect(client.error).toContain('This provider is unavailable');
  });
  test("Antigravity's block reason is the one provider state that stops a send", () => {
    const agy = (extra: Obj) => provider('agy', 'antigravity', extra);
    expect(antigravitySendBlockReason(provider('codex', 'codex', { installed: false, status: 'error' }), 'model-a')).toBe('');
    expect(antigravitySendBlockReason(agy({ installed: false }), 'model-a')).toBe('Install Antigravity in provider settings before sending.');
    expect(antigravitySendBlockReason(agy({ auth: { status: 'unauthenticated' } }), 'model-a')).toBe('Sign in to Antigravity in provider settings before sending.');
    expect(antigravitySendBlockReason(agy({}), ' ')).toBe('Choose an Antigravity model before sending.');
    expect(antigravitySendBlockReason(agy({ models: [] }), 'model-a')).toBe('Refresh Antigravity models in provider settings before sending.');
    expect(antigravitySendBlockReason(agy({}), 'gone')).toBe('That Antigravity model is no longer available. Choose another model.');
    expect(antigravitySendBlockReason(agy({}), 'antigravity-default')).toBe('');
  });
  test('without orchestration:operate Send is disabled with its reason and no banner shows', async () => {
    const client = new T3Client(), native = new Fake(), disk = storage();
    native.scopes = ['orchestration:read'];
    await client.refresh(native, disk);
    await client.command('select-thread', 't1', '', 0, native, disk);
    await client.refresh(native, disk);
    client.local.drafts[client.draftKey] = 'ro text';
    const view = snapshot(client);
    expect(client.error).toBe('');
    expect(view.canSend).toBe(false);
    expect(view.composer).toMatchObject({ sendStatus: 'This connection cannot change threads.', sendTooltip: 'This connection cannot change threads.' });
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
  });
  test('a message sent during a run queues by default, steers with ⌘ or the Steer setting', async () => {
    const { client, native, command } = await opened();
    running(client);
    await command('send', '', 'Queued one');
    expect(native.committed.at(-1)).toMatchObject({ type: 'message.dispatch', dispatchMode: { type: 'queue_after_active' } });
    expect(native.committed.at(-1)!.deliveryIntent).toBeUndefined();
    // A ⌘-click: the press's own modifiers (app.contract `send`), not the module's recorded gesture.
    await command('send', 'pointer:meta', 'Steer one');
    expect(native.committed.at(-1)).toMatchObject({ deliveryIntent: 'steer', dispatchMode: { type: 'start_immediately' } });
    await command('device-setting', 'followUpBehavior', 'steer');
    client.local.clientSettings.followUpBehavior = 'steer';
    await command('send', '', 'Steer two');
    expect(native.committed.at(-1)).toMatchObject({ deliveryIntent: 'steer' });
    expect(snapshot(client).composer).toMatchObject({ sendRunning: true, sendLabel: 'Steer message', sendIcon: 'corner-up-right' });
    client.local.clientSettings.followUpBehavior = 'queue';
    expect(snapshot(client).composer).toMatchObject({ sendLabel: 'Queue message', sendIcon: 'list-plus', sendTooltip: 'Click to queue, Ctrl/⌘-click or ⌘Enter to steer' });
    // ⌘ or Control held (the window's shortcutModifiers): the alternate's label and icon.
    expect(snapshot(client).composer).toMatchObject({ sendHeldLabel: 'Steer message', sendHeldIcon: 'corner-up-right' });
  });
  test('⌥⌘↩ in a draft starts the thread in the background and keeps a fresh draft', async () => {
    const { client, native, command } = await connected();
    await command('send', 'key:meta+alt', 'Background work');
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
    await command('model', 'model-b', 'codex');
    await command('cc:resume');
    expect(native.committed.slice(-2)).toEqual([
      expect.objectContaining({ type: 'message.dispatch', text: 'Continue where you left off.', manualContinuationOfRunId: 'r1', modelSelection: { instanceId: 'codex', model: 'model-b' }, dispatchMode: { type: 'start_immediately' } }),
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
    // On the minute: the label reads the clock's minute, as the sidebar's snoozed shelf does.
    const now = Math.floor(Date.now() / 60_000) * 60_000;
    Object.assign(client.shell.threads[0]!, { snoozedUntil: new Date(now + 3_600_000).toISOString(), snoozedAt: new Date(now - 1000).toISOString() });
    expect(threadStatusLine(client, now)).toMatchObject({ title: 'Snoozed, 1h left', actionLabel: 'Wake now', detail: 'cc:unsnooze' });
    expect(snapshot(client, now).composer.notices).toEqual([]);
    await command('cc:unsnooze');
    expect(native.committed.at(-1)).toMatchObject({ type: 'thread.unsnooze', threadId: 't1', reason: 'user' });
    Object.assign(client.shell.threads[0]!, { snoozedUntil: new Date(now - 60_000).toISOString() });
    expect(wokeAt(client.shell.threads[0]!, now)).toBe(new Date(now - 60_000).toISOString());
    expect(threadStatusLine(client, now)).toMatchObject({ title: 'Woke from snooze', actionLabel: 'Dismiss' });
    await command('cclocal:dismiss-woke');
    expect(snapshot(client, now).composer.notices).toEqual([]);
    Object.assign(client.shell.threads[0]!, { settledOverride: 'settled', snoozedUntil: null });
    expect(threadStatusLine(client, now)).toMatchObject({ title: 'Settled', actionLabel: 'Un-settle', detail: 'unsettle' });
  });
  test('what the thread is doing is not above the composer: background work, its goal and its tasks leave the dock', async () => {
    const { client } = await opened();
    Object.assign(client.shell.threads[0]!, { settledOverride: 'settled', pendingBackgroundTasks: [{ taskId: 'a', kind: 'command', description: 'npm test' }],
      goal: { objective: 'Ship it', status: 'active', tokensUsed: 0 } });
    expect(snapshot(client).composer.notices).toEqual([]);
    expect(threadNowRows(client).map(row => row.id)).toEqual(['background-work:t1', 'goal:t1']);
    running(client);
    client.thread!.projection.plans = [{ id: 'todo', kind: 'todo_list', runId: 'r1', steps: [{ text: 'Read', status: 'completed', durationMs: 1200 }, { text: 'Write', status: 'running' }] }];
    expect(snapshot(client).composer.notices).toEqual([]);
    expect(presentBackgroundWork([{ kind: 'subagent' }, { kind: 'command' }, { kind: 'command' }])!.title).toBe('Waiting on 1 subagent and 2 commands');
  });
  test('the usage-limit notice keeps its card while the goal waits at the end of the timeline', async () => {
    const { client } = await opened();
    Object.assign(client.shell.threads[0]!, { status: 'failed', lastErrorClass: 'usage_limit', usageLimitResetAt: new Date(Date.now() + 3_600_000).toISOString(), latestRunId: 'r9', updatedAt: new Date().toISOString(),
      goal: { objective: 'Ship it', status: 'active', tokensUsed: 0 } });
    const view = snapshot(client, Date.now()).composer;
    expect(view.notices.map(notice => [notice.id.split(':')[0], notice.group, notice.stacked])).toEqual([['usage-limit-recovery', 'attention', false]]);
    expect(threadNowRows(client).map(row => row.id)).toEqual(['goal:t1']);
  });
  test('a notice offers its details only while its description is clipped', () => {
    const text = 'Server update failed: The package could not be verified.';
    const base = { id: 'n', description: text, segments: [], redact: '', machines: [], details: false };
    const client = { presentation: { anchors: { [probeKey(text, 12, 400)]: [0, 300] } as Record<string, unknown> } } as unknown as T3Client;
    const details = (laidOut: number) => { client.presentation.anchors = { ...client.presentation.anchors as object, 'nd:n': [40, laidOut] }; return withNoticeDetails(client, [base] as never[], 16)[0]!.details; };
    expect(details(320)).toBe(false);
    expect(details(290)).toBe(true);
    // The button takes 28pt from the text; that room counts while it shows, so the button stays while the text
    // would still be clipped without it, and goes once it would fit.
    expect(details(262)).toBe(true);
    expect(details(273)).toBe(false);
    // Not laid out yet: no button.
    client.presentation.anchors = {};
    expect(withNoticeDetails(client, [base] as never[], 16)[0]!.details).toBe(false);
    // A roster measures its links (medium), plain items and commas: 120 + 4 + 90 = 214 natural.
    const roster = { ...base, id: 'r', description: 'Review, npm test', segments: [{ key: '0', label: 'Review', threadId: 'c', aria: '', last: false }, { key: '1', label: 'npm test', threadId: '', aria: '', last: true }] };
    client.presentation.anchors = { [probeKey('Review', 12, 500)]: [0, 120], [probeKey(', ', 12, 400)]: [0, 4], [probeKey('npm test', 12, 400)]: [0, 90], 'nd:r': [40, 200] };
    expect(withNoticeDetails(client, [roster] as never[], 16)[0]!.details).toBe(true);
    client.presentation.anchors = { ...client.presentation.anchors as object, 'nd:r': [40, 220] };
    expect(withNoticeDetails(client, [roster] as never[], 16)[0]!.details).toBe(false);
  });
  test('a lone subagent link reads Show, several name each subagent', () => {
    expect(presentBackgroundWork([{ kind: 'subagent', description: 'Review', childThreadId: 'c1' }])!.segments)
      .toEqual([{ key: '0:Review', label: 'Show', threadId: 'c1', aria: 'Show subagent Review', last: true }]);
    expect(presentBackgroundWork([{ kind: 'subagent', description: 'Review', childThreadId: 'c1' }, { kind: 'subagent', description: 'Lint', childThreadId: 'c2' }])!.segments
      .map(segment => [segment.label, segment.aria])).toEqual([['Review', 'Open subagent Review'], ['Lint', 'Open subagent Lint']]);
  });
  test('the provider control replaces the picker when nothing is available', async () => {
    const { client, command } = await opened();
    // resolveComposerProviderSelection: an enabled, available instance the thread asked for stays selected whatever its probe status.
    for (const entry of arr(client.config.providers)) entry.status = 'error';
    expect(snapshot(client).composer).toMatchObject({ noProvider: '', providerSetupId: '' });
    for (const entry of arr(client.config.providers)) entry.enabled = false;
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
    expect(footerLayout({ ...base, host: 250 })).toMatchObject({ runtimeIconOnly: true, traitsIconOnly: true });
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
    expect(state).toMatchObject({ canSteer: true });
    // The bubbles show the message in full, as a sent turn reads.
    expect(queuedTurnRows(client).map(row => [row.runId, row.body, row.actionLabel])).toEqual([['q1', 'First  follow-up', 'Send now'], ['q2', 'Second', 'Send now']]);
    await command('cc:queued-steer', 'q1');
    expect(native.committed.at(-1)).toMatchObject({ type: 'queued-message.promote-to-steer', queuedRunId: 'q1', targetRunId: 'r1' });
    await command('cc:queued-remove', 'q2');
    expect(native.committed.at(-1)).toMatchObject({ type: 'queued-run.cancel', runId: 'q2' });
    // With no active run, Send now starts the queued turn instead (queued-run.start).
    client.thread!.projection.runs = arr(client.thread!.projection.runs).filter(run => run.status === 'queued');
    expect(queuedTurnRows(client).map(row => [row.actionLabel, row.intentTip.replace(/ \(.*\)$/, '')])).toEqual([['Send now', 'Start a turn with this message'], ['Send now', 'Start a turn with this message']]);
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
  test('the workspace and host tooltips name their shortcuts', async () => {
    const { client, native } = await connected();
    client.config.keybindings = [{ command: 'composer.workspace', shortcut: { key: 'x', modKey: true, shiftKey: true } }, { command: 'composer.host', shortcut: { key: 'h', modKey: true, shiftKey: true } }];
    expect(await composerBranches(client, native, false, '')).toMatchObject({ keyWorkspace: '⇧⌘X', keyHost: '⇧⌘H' });
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
    expect(notice).toMatchObject({ description: 'Codex · Exact verification fixture · OpenAI API Key', dismissLabel: 'Dismiss usage limits', lines: [] });
    expect(notice.usage).toMatchObject([{ key: 'codex', showLabel: false, notice: 'This account has no subscription limits.', windows: [], showCredits: false }]);
    provider.usageLimits = { windows: [{ label: '5h', usedPercent: 25, resetsAt: new Date(Date.now() + 2 * 3_600_000 + 13 * 60_000 + 30_000).toISOString() }], checkedAt: at };
    await command('cclocal:usage-limits');
    expect(snapshot(client, Date.now()).composer.notices.find(entry => entry.title === 'Usage limits')!.usage[0]!.windows).toMatchObject([{ label: '5h', remaining: 75, timeLeft: -1, pace: '', resetsIn: 'resets in 2h 13m', summary: '5h: 75% left, resets in 2h 13m' }]);
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
    // A failure toasts instead of writing into the banner or the transcript.
    client.shell.threads[0]!.usageLimitResetAt = new Date(Date.now() - 60_000).toISOString();
    await command('cc:limit-snooze');
    expect(client.error).toBe('');
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'error', title: 'Could not snooze thread', description: 'The reset time has passed. Retry the thread manually.' });
  });
  test('snoozed, Snooze reads Wake now; the reset ends the snooze and disables Snooze; it says the reset in the Time format', async () => {
    const { client, native, command } = await opened();
    const now = Date.parse('2026-10-20T12:00:00.000Z'), reset = new Date(Date.parse('2026-10-20T12:00:00.000Z') + 30 * 60_000).toISOString();
    snapshot(client, now);
    Object.assign(client.shell.threads[0]!, { status: 'failed', lastErrorClass: 'usage_limit', usageLimitResetAt: reset, latestRunId: 'r9', updatedAt: new Date(now - 60_000).toISOString(),
      snoozedUntil: reset, limitRecovery: { runId: 'r9', resetAt: reset, snooze: true } });
    client.local.deviceSettings.timestampFormat = '24-hour';
    const notice = () => snapshot(client, now).composer.notices[0]!;
    const time = new Intl.DateTimeFormat('en-US', { hour: '2-digit', minute: '2-digit', hourCycle: 'h23' }).format(new Date(reset));
    expect(notice()).toMatchObject({ action2Label: 'Wake now', action2Disabled: false, description: `Resets ${time}` });
    await command('cc:limit-snooze');
    expect(native.committed.at(-1)).toMatchObject({ type: 'thread.metadata.update', limitRecovery: { runId: 'r9', resetAt: reset, snooze: false } });
    // Under a minute from the reset the root's clock ticks each second; past it the snooze is over.
    expect(snapshot(client, Date.parse(reset) - 30_000).composer).toMatchObject({ resetSoon: true, resetKey: `r9:${reset}` });
    expect(snapshot(client, Date.parse(reset) + 1_000).composer.notices[0]).toMatchObject({ action2Label: 'Snooze until reset', action2Disabled: true });
    expect(snapshot(client, Date.parse(reset) + 1_000).composer.resetSoon).toBe(false);
  });
});

test('older resource arguments cannot rewind the shared client clock', () => {
  const client = {};
  noteNow(client, 2000);
  noteNow(client, 1000);
  noteNow(client, Number.NaN);
  expect(composerNow(client)).toBe(2000);
  noteNow(client, 3000);
  expect(composerNow(client)).toBe(3000);
  expect(composerNow({})).toBe(0);
});


describe('local checkout branch mismatch', () => {
  const base = { effectiveEnvMode: 'local', activeWorktreePath: null, activeThreadBranch: 'feature', currentGitBranch: 'main' };
  test('only a local checkout with two different known branches mismatches', () => {
    expect(resolveLocalCheckoutBranchMismatch(base)).toEqual({ threadBranch: 'feature', currentBranch: 'main' });
    for (const patch of [{ effectiveEnvMode: 'worktree' }, { activeWorktreePath: '/worktree' }, { activeThreadBranch: null }, { currentGitBranch: null }, { currentGitBranch: 'feature' }]) {
      expect(resolveLocalCheckoutBranchMismatch({ ...base, ...patch })).toBeNull();
    }
  });
  async function mismatch(dirty = false) {
    const context = await opened();
    obj(context.client.projection.thread).branch = 'feature';
    await watchVcsStatus(context.client, context.native, '/repo', 0);
    const status = (branch: string, changed = dirty) => vcsStatusEvent(context.client, { subscriptionId: context.native.subs[VCS_STATUS_KEY],
      value: { _tag: 'snapshot', local: { isRepo: true, refName: branch, hasWorkingTreeChanges: changed } } });
    status('main');
    return { ...context, status };
  }
  test('content reveals the notice, clearing keeps it, resolution resets it, and dismissal survives a revisit', async () => {
    const { client, command, status } = await mismatch();
    expect(stripWorkspace(client)).toBe('/repo'); // still watched when both the card and strip are closed
    expect(branchMismatchNotice(client)).toBeNull();
    client.local.drafts[client.draftKey] = 'Continue';
    const notice = branchMismatchNotice(client)!;
    expect(notice).toMatchObject({ title: 'Branch changed from', code: 'feature', actionLabel: 'Restore branch', dismissLabel: 'Dismiss branch change notice' });
    client.local.drafts[client.draftKey] = '';
    expect(branchMismatchNotice(client)).not.toBeNull();
    status('feature'); expect(branchMismatchNotice(client)).toBeNull();
    status('main'); expect(branchMismatchNotice(client)).toBeNull();
    client.local.snapshotDrafts[client.draftKey] = [{ id: 'attachment' }];
    expect(branchMismatchNotice(client)).not.toBeNull();
    await command('cclocal:branch-mismatch-dismiss', notice.dismissId);
    expect(branchMismatchNotice(client)).toBeNull();
    client.threadId = 't2'; expect(branchMismatchNotice(client)).toBeNull();
    client.threadId = 't1'; expect(branchMismatchNotice(client)).toBeNull();
    status('another'); expect(branchMismatchNotice(client)).not.toBeNull();
  });
  test('clean restore switches the checkout and focuses the composer without redundant metadata', async () => {
    const { client, command, native } = await mismatch();
    const result = await command('cc:branch-restore');
    expect(result.message).toBe('focus:composer');
    expect(native.committed.at(-1)).toMatchObject({ method: 'vcs.switchRef', cwd: '/repo', refName: 'feature' });
    expect(native.committed.some(entry => entry.type === 'thread.metadata.update')).toBe(false);
    expect(branchMismatchNotice(client)).toBeNull();
  });
  test('dirty restore waits for confirmation and rejects confirmation after a mismatch changes', async () => {
    const { command, native, status } = await mismatch(true);
    expect((await command('cc:branch-restore')).message).toBe('branch-restore-confirm:feature');
    expect(native.committed.some(entry => entry.method === 'vcs.switchRef')).toBe(false);
    status('another');
    await command('cc:branch-restore-confirmed');
    expect(native.committed.some(entry => entry.method === 'vcs.switchRef')).toBe(false);
    await command('cc:branch-restore');
    expect((await command('cc:branch-restore-confirmed')).message).toBe('focus:composer');
    expect(native.branch).toBe('feature');
  });
  test.each(['environment', 'generation', 'origin'])('a changed %s cannot reuse another connection’s clean checkout status', async change => {
    const { client, native, disk, status } = await mismatch();
    client.local.drafts[client.draftKey] = 'Continue';
    await composerBranches(client, native, false, '', true); // populate both the stream and branch-picker caches
    expect(branchMismatchNotice(client)).not.toBeNull();
    if (change === 'environment') client.environmentId = 'env2';
    if (change === 'generation') { client.generation++; native.generation++; }
    if (change === 'origin') client.origin = 'http://env2:3773';
    client.local.drafts[client.draftKey] = 'Continue in the new connection';
    expect(branchMismatchNotice(client)).toBeNull();
    expect(await restoreThreadBranch(client, native, disk)).toBe('');
    await watchVcsStatus(client, native, '/repo', 1);
    expect(branchMismatchNotice(client)).toBeNull();
    expect(await restoreThreadBranch(client, native, disk)).toBe('');
    expect(native.committed.some(entry => entry.method === 'vcs.switchRef')).toBe(false);
    status('env2-branch', true);
    expect(branchMismatchNotice(client)).not.toBeNull();
    expect(await restoreThreadBranch(client, native, disk)).toBe('branch-restore-confirm:feature');
    expect(native.committed.some(entry => entry.method === 'vcs.switchRef')).toBe(false);
    expect(await restoreThreadBranch(client, native, disk, true)).toBe('focus:composer');
  });
  test('a reconnect cannot inherit dirty-checkout consent even when both branch names match', async () => {
    const { client, native, disk, status } = await mismatch(true);
    expect(await restoreThreadBranch(client, native, disk)).toBe('branch-restore-confirm:feature');
    client.generation++; native.generation++;
    await watchVcsStatus(client, native, '/repo', 1);
    status('main', true);
    expect(await restoreThreadBranch(client, native, disk, true)).toBe('');
    expect(native.committed.some(entry => entry.method === 'vcs.switchRef')).toBe(false);
    expect(await restoreThreadBranch(client, native, disk)).toBe('branch-restore-confirm:feature');
  });
  test.each(['switch', 'ids'])('an environment change during %s cannot dispatch metadata on the new environment', async stage => {
    const { client, native, disk } = await mismatch();
    const original = native.later.bind(native);
    native.later = async input => {
      const request = obj(input);
      const value = request.method === 'vcs.switchRef' ? native.ok({ refName: 'normalized-feature' }) : await original(input);
      if ((stage === 'switch' && request.method === 'vcs.switchRef') || (stage === 'ids' && request.op === 'ids')) {
        client.environmentId = 'env2'; client.generation++; client.threadId = 'other-thread';
      }
      return value;
    };
    expect(await restoreThreadBranch(client, native, disk)).toBe('');
    expect(native.committed.some(entry => entry.type === 'thread.metadata.update')).toBe(false);
    expect(toasts(client)).toHaveLength(0);
  });
  test('a normalized ref updates thread metadata; switch and metadata failures have distinct toasts', async () => {
    for (const failure of ['', 'switch', 'metadata']) {
      const { client, native, command } = await mismatch();
      const original = native.later.bind(native);
      native.later = async input => {
        const request = obj(input);
        if (request.method === 'vcs.switchRef') {
          if (failure === 'switch') return { ok: false, generation: 1, error: { kind: 'server', message: 'conflicting changes', uncertain: false } };
          return native.ok({ refName: 'local-feature' });
        }
        if (obj(request.payload).type === 'thread.metadata.update' && failure === 'metadata') return { ok: false, generation: 1, error: { kind: 'server', message: 'metadata unavailable', uncertain: false } };
        return original(input);
      };
      await command('cc:branch-restore');
      if (!failure) expect(native.committed.at(-1)).toMatchObject({ type: 'thread.metadata.update', threadId: 't1', branch: 'local-feature', worktreePath: null });
      else expect(toasts(client).at(-1)).toMatchObject({ kind: 'error', title: failure === 'switch' ? 'Failed to switch checkout' : 'Checkout switched, but the thread could not be updated' });
    }
  });
});
