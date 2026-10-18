import { expect, test } from 'bun:test';
import { buildScript, decodeInput, parseProjectScripts, primaryScript, releaseClaimedRoles, scriptMenuLabel } from '../../src/settings/settings-b-actions';
import { cardScripts } from '../../src/details/project-scripts';
import { isWorkingThread, shellRuntime } from '../../src/sidebar/sidebar-model';
import { effectiveWorktreeRules } from '../../src/client/domain';
import { storagePatch } from '../../src/settings/settings-rest-commands';
import { isProjectScopedSettingKey, planScopedSettingsPatch } from '../../src/settings/scoped-settings-plan';

test('settle actions preserve independent setup roles and stay reachable without becoming a one-click action', () => {
  const input = decodeInput('name=Clean&command=rm%20-rf%20build&setup=false&settle=true&wait=true');
  const clean = buildScript('clean', input), both = { ...clean, runOnWorktreeCreate: true };
  expect(clean).toMatchObject({ runOnSettle: true, runOnWorktreeCreate: false });
  expect(clean.async).toBeUndefined();
  expect(releaseClaimedRoles(both, input)).toMatchObject({ runOnSettle: false, runOnWorktreeCreate: true });
  expect(releaseClaimedRoles(both, { ...input, runOnSettle: false, runOnWorktreeCreate: true })).toMatchObject({ runOnSettle: true, runOnWorktreeCreate: false });
  expect(primaryScript([clean])).toBeUndefined();
  const normal = { ...clean, id: 'test', name: 'Test', runOnSettle: false };
  expect(primaryScript([clean, normal], 'clean')?.id).toBe('test');
  expect(scriptMenuLabel(both)).toBe('Clean (setup, on settle)');
  const view = cardScripts({ defaultProjectScripts: [clean], projectSettingsFolded: true }, { id: 'p' }, 'clean');
  expect(view.scriptName).toBe('');
  expect(view.scripts).toEqual([{ id: 'clean', label: 'Clean (on settle)', icon: 'play' }]);
  expect(parseProjectScripts('{"scripts":[{"name":"Clean","command":"true","runOnSettle":true}]}').scripts[0]?.runOnSettle).toBe(true);
  expect(parseProjectScripts('{"scripts":[{"name":"Clean","command":"true","runOnSettle":"yes"}]}').status).toBe('invalid');
});

test('a never-run thread held by a PR watch stays Working until the server removes its monitor', () => {
  const thread = { latestRunId: null, activeProviderThreadId: null, status: 'idle', pendingBackgroundTasks: [{ kind: 'monitor', taskId: 'pull-request-watch:example', description: 'Watching pull request #12' }] };
  expect(shellRuntime(thread)?.status).toBe('idle');
  expect(isWorkingThread(thread)).toBe(true);
  expect(shellRuntime({ ...thread, pendingBackgroundTasks: [] })).toBeNull();
});

test('settled retention accepts zero, keep rules survive edits, and off clears both retention rules', () => {
  const settings = { storageCleanup: { worktreeKeepWhen: 'tracked-changes', worktreeSettledAfterDays: 0, worktreeAfterDays: 8 } };
  expect(storagePatch(settings, '', 'worktreeSettledAfterDays', '0')).toEqual({ storageCleanup: { worktreeSettledAfterDays: 0 } });
  expect(storagePatch(settings, '', 'worktreeAfterDays', '0')).toEqual({ storageCleanup: { worktreeAfterDays: 1 } });
  expect(storagePatch(settings, '', 'worktreeKeepWhen', 'any-local-files')).toEqual({ storageCleanup: { worktreeKeepWhen: 'any-local-files' } });
  expect(() => storagePatch(settings, '', 'worktreeKeepWhen', 'none')).toThrow('local changes');
  expect(effectiveWorktreeRules(settings, {})).toMatchObject({ worktreeKeepWhen: 'tracked-changes', worktreeSettledAfterDays: 0 });
  expect(effectiveWorktreeRules(settings, { worktreeCleanup: { mode: 'off' } })).toMatchObject({ worktreeAfterDays: null, worktreeSettledAfterDays: null });
  expect(storagePatch(settings, '', 'worktreesDirectory', '  D:\\worktrees  ')).toEqual({ worktreesDirectory: 'D:\\worktrees' });
  expect(() => storagePatch(settings, 'p', 'worktreesDirectory', '~/worktrees')).toThrow('environment');
});

test('keeping PR threads is a project setting and worktree location remains environment-wide', () => {
  expect(isProjectScopedSettingKey('sidebarAutoSettleKeepPullRequests')).toBe(true);
  expect(isProjectScopedSettingKey('worktreesDirectory')).toBe(false);
  const plan = planScopedSettingsPatch({ kind: 'all', environmentIds: ['env'] }, [{ environmentId: 'env', label: 'Mac', connection: { phase: 'connected' }, serverConfig: { settings: {} } }], { sidebarAutoSettleKeepPullRequests: true, worktreesDirectory: '~/worktrees' });
  expect(plan.serverWrites[0]?.patch).toEqual({ sidebarAutoSettleKeepPullRequests: true, worktreesDirectory: '~/worktrees' });
});

import { parseMaxDeliveryAge, webhookDraft, webhookSchedule, webhookAddress } from '../../src/settings/scheduled-webhook';
import { editorDraft, taskInput } from '../../src/settings/scheduled-view';
import { scheduleLabel, taskStatus } from '../../src/settings/scheduled-tasks';
import { deliveriesPage } from '../../src/settings/scheduled-deliveries';
import type { T3Client } from '../../src/client/client';
import type { LiveEnvironment } from '../../src/client/live-streams';

test('webhook age and signature validation preserve write-only existing secrets', () => {
  expect(['', '0', '1', '1.5', '1440', '1441'].map(parseMaxDeliveryAge)).toEqual([null, undefined, 1, undefined, 1440, undefined]);
  const existing = { schedule: { type: 'webhook', signature: { header: 'X-Signature', encoding: 'base64', prefix: '' }, maxDeliveryAgeMinutes: 45 }, webhook: { hasSecret: true } };
  const draft = webhookDraft(existing);
  expect(draft).toMatchObject({ signatureEnabled: true, signatureSecret: '', maxDeliveryAgeMinutes: '45', signaturePrefix: '' });
  expect(webhookSchedule({ ...Object.fromEntries(Object.entries(draft).map(([key, value]) => [key, String(value)])), signatureSecret: '' }, existing)).toEqual({ type: 'webhook', maxDeliveryAgeMinutes: 45, signature: { header: 'X-Signature', encoding: 'base64', prefix: '' } });
  expect(() => webhookSchedule({ signatureEnabled: 'true', signatureHeader: 'x-signature' })).toThrow('Signing secret is required');
  expect(webhookSchedule({ signatureEnabled: 'true', signatureHeader: ' x-signature ', signatureSecret: ' secret ', signaturePrefix: 'sha256=' })).toMatchObject({ signature: { secret: 'secret', encoding: 'hex', header: 'x-signature' } });
  expect(webhookSchedule({ signatureEnabled: 'false' }, existing).signature).toBeNull();
  expect(() => webhookSchedule({ maxDeliveryAgeMinutes: '1441' })).toThrow('Invalid age limit');
});

test('the webhook editor round-trips its trigger and reports Listening instead of a next run', () => {
  const task = { id: 't', title: 'Webhook', prompt: '{{body.issue.title}}', enabled: true, schedule: { type: 'webhook', signature: null }, projectId: 'p', workspaceStrategy: { type: 'root' }, modelSelection: { instanceId: 'codex', model: 'm' } };
  const draft = editorDraft(task, 'p', 'codex:m');
  expect(draft.scheduleMode).toBe('webhook');
  const input = Object.fromEntries(Object.entries(draft).map(([key, value]) => [key, String(value)]));
  expect(taskInput(input, task).schedule).toEqual({ type: 'webhook', signature: null, maxDeliveryAgeMinutes: null });
  expect(scheduleLabel(task.schedule)).toBe('On webhook');
  expect(taskStatus(task, 0)).toBe('On webhook · Listening');
  expect(webhookAddress({ path: '/hooks/token', url: null }, 'http://127.0.0.1:16500')).toMatchObject({ address: 'http://127.0.0.1:16500/hooks/token', copyable: true, note: expect.stringContaining('Only this computer') });
  expect(webhookAddress({ path: '/hooks/token', url: 'https://relay.example/hook' }, '')).toMatchObject({ address: 'https://relay.example/hook', note: '' });
});

test('deliveries load through their owner, retain the list, and expose the rendered prompt and redacted request', async () => {
  const calls: unknown[] = [], client = {} as T3Client;
  const environment = { environmentId: 'remote', key: 'remote-transport', request: async (method: string, payload: unknown) => {
    calls.push([method, payload]);
    return method.endsWith('listWebhookDeliveries') ? { deliveries: [{ id: 'd', outcome: 'rejected_signature', method: 'POST', receivedAt: '2026-10-18T00:00:00Z', missingFields: [] }] }
      : { delivery: { id: 'd', outcome: 'accepted', signatureVerified: true, headers: { authorization: '[redacted]' }, body: '{"issue":12}', renderedPrompt: 'Review issue 12', missingFields: ['body.title'] } };
  } } as unknown as LiveEnvironment;
  const task = { id: 'task', title: 'Review' };
  expect((await deliveriesPage(client, environment, task, '', 0, Date.parse('2026-10-18T00:01:00Z'))).items[0]).toMatchObject({ id: 'd', label: 'Bad signature', method: 'POST', age: '1m ago' });
  const detail = await deliveriesPage(client, environment, task, 'd', 0, 0);
  expect(detail.blocks).toContainEqual({ title: 'Prompt sent to the agent', value: 'Review issue 12' });
  expect(detail.blocks).toContainEqual({ title: 'Headers', value: 'authorization: [redacted]' });
  expect(detail.missing).toBe('Empty placeholders: body.title');
  await deliveriesPage(client, environment, task, '', 0, 0);
  expect(calls).toEqual([['scheduledTasks.listWebhookDeliveries', { id: 'task' }], ['scheduledTasks.getWebhookDelivery', { id: 'task', deliveryId: 'd' }]]);
});


import { cleanupHasRules, databaseNotice, cleanupPermitted } from '../../src/settings/storage-maintenance';
import { adoptShellPrefs, shellPrefs } from '../../src/shell/shell-prefs';

test('cleanup recognizes zero-day settled and custom project rules while off suppresses inherited worktree rules', () => {
  expect(cleanupHasRules({ storageCleanup: { worktreeSettledAfterDays: 0 } })).toBe(true);
  expect(cleanupHasRules({ storageCleanup: { worktreeAfterDays: 3 }, worktreeCleanup: { mode: 'off' } })).toBe(false);
  expect(cleanupHasRules({ projectSettingsOverrides: { p: { worktreeCleanup: { mode: 'custom', rules: { worktreeOnDelete: true } } } } })).toBe(true);
});

test('database reclaim notices require a GiB without incremental vacuum and persist dismissal per environment', () => {
  const status = { reclaimableBytes: 1.5 * 1024 ** 3, incrementalVacuumEnabled: false, compactCommand: 't3 doctor --compact' };
  expect(databaseNotice(status, 'mac', 'Mac', [])).toMatchObject({ show: true, title: 'Reclaim 1.5 GiB on Mac', command: 't3 doctor --compact' });
  expect(databaseNotice({ ...status, reclaimableBytes: 1024 ** 3 - 1 }, 'mac', 'Mac', []).show).toBe(false);
  expect(databaseNotice({ ...status, incrementalVacuumEnabled: true }, 'mac', 'Mac', []).show).toBe(false);
  const local = {}; adoptShellPrefs(local, { shell: { databaseStorageDismissals: ['mac', 3] } });
  const dismissed = shellPrefs({ local }).databaseStorageDismissals;
  expect(dismissed).toEqual(['mac']);
  expect(databaseNotice(status, 'mac', 'Mac', dismissed).show).toBe(false);
  expect(databaseNotice(status, 'remote', 'Remote', dismissed).show).toBe(true);
});


import { composerCommand } from '../../src/composer/composer-controls-commands';
import { presentBackgroundWork } from '../../src/composer/composer-controls-view';
import type { Files, Native } from '../../src/client/protocol';

test('Stop ends visible PR watches even when a thread has never run', async () => {
  const commands: unknown[] = []; let ids = 0;
  const client = { threadId: 'thread', thread: {}, projection: { runs: [], thread: { pullRequests: [
    { host: 'github.com', repository: 'org/repo', number: 12, watch: {} },
    { host: 'github.com', repository: 'org/repo', number: 13, watch: {}, source: 'stack-dismissed' },
    { host: 'github.com', repository: 'org/repo', number: 14 },
  ] } }, restAccess: () => ({ ids: async () => [`command-${++ids}`], dispatch: async (_storage: Files, payload: unknown) => { commands.push(payload); } }) } as unknown as T3Client;
  await composerCommand(client, {} as Native, {} as Files, 'stop-background', '', '');
  expect(commands).toEqual([{ type: 'thread.pull-request.watch', commandId: 'command-1', threadId: 'thread', host: 'github.com', repository: 'org/repo', number: 12, watching: false }]);
  expect(presentBackgroundWork([{ kind: 'monitor', description: 'Watching pull request #12' }])?.title).toBe('Watching pull request #12');
});

test('cleanup uses its maintenance permission independently of orchestration writes', () => {
  const focused = { focused: true, key: '' };
  expect(cleanupPermitted({ scopes: ['orchestration:operate'] }, focused)).toBe(false);
  expect(cleanupPermitted({ scopes: ['environment:maintain'] }, focused)).toBe(true);
});

import { contractComponent, type Env } from '../support/contract-action-fixture';
test('webhook copy and rotation complete without closing the editor or showing their internal completion marker', () => {
  const root = contractComponent('app.contract', 'T3Code');
  for (const message of ['', 'task-webhook-keep']) {
    const state: Env = { noticeExit: '', routeKeyboard: '', pendingModal: 'rest', restEditor: 'task', modalError: 'old', settingsRefresh: 7, commandPending: true, renameAfterOp: '', keybindingOpen: true, keybindingPrevious: 'old', keybindingRecording: true, lineageStopping: false };
    root.call('commandCompleted', state, { changed: { message, relaunch: false, routeReorder: 0 }, routeKeyboardComplete: () => '', routeKeyboardOwned: false }, []);
    expect(state).toMatchObject({ modalError: '', commandPending: false, pendingModal: '', settingsRefresh: 8, restEditor: message ? 'task' : '' });
  }
});
