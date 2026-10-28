import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { appPath } from '../support/app-files';
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
  const thread = { latestRunId: null, activeProviderThreadId: null, status: 'idle', pendingBackgroundTasks: [{ kind: 'monitor', taskId: 'pull-request-watch:example', description: 'Watching pull request #12', awaited: true }] }; // pullRequestWatchTasks marks it awaited
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
  expect(webhookSchedule({ ...Object.fromEntries(Object.entries(draft).map(([key, value]) => [key, String(value)])), signatureSecret: '' }, existing)).toEqual({ type: 'webhook', maxDeliveryAgeMinutes: 45, signature: { header: 'X-Signature', encoding: 'base64', prefix: '' }, dedupe: {} });
  expect(() => webhookSchedule({ signatureEnabled: 'true', signatureHeader: 'x-signature' })).toThrow('Signing secret is required');
  expect(webhookSchedule({ signatureEnabled: 'true', signatureHeader: ' x-signature ', signatureSecret: ' secret ', signaturePrefix: 'sha256=' })).toMatchObject({ signature: { secret: 'secret', encoding: 'hex', header: 'x-signature' } });
  expect(webhookSchedule({ signatureEnabled: 'false' }, existing).signature).toBeNull();
  expect(() => webhookSchedule({ maxDeliveryAgeMinutes: '1441' })).toThrow('Invalid age limit');
});

test('new and existing task editor projections match the strict Contract draft shape', () => {
  const contract = readFileSync(appPath('settings-scheduled.contract'), 'utf8');
  const fields = contract.match(/shape TaskDraftView\n([\s\S]*?)(?=\nshape )/)?.[1];
  expect(fields).toBeDefined();
  const declared = [...(fields ?? '').matchAll(/^  ([A-Za-z][A-Za-z0-9]*):/gm)].map(match => match[1]).sort();
  const existing = { id: 'task', title: 'Webhook', runtimeMode: 'full-access', interactionMode: 'default',
    modelSelection: { instanceId: 'codex', model: 'm', options: { reasoning: 'high' } },
    workspaceStrategy: { type: 'existing_worktree', worktreePath: '/fixture/checkout' },
    schedule: { type: 'webhook', signature: { header: 'x-signature', encoding: 'hex', prefix: 'sha256=' }, maxDeliveryAgeMinutes: 60 }, webhook: { hasSecret: true } };
  for (const task of [undefined, existing]) {
    const draft = editorDraft(task, 'project', 'codex:m', 'environment');
    const projected = { ...draft, ...webhookAddress(task?.webhook, 'http://127.0.0.1:16771'), key: 'editor', missing: false };
    expect(Object.keys(projected).sort()).toEqual(declared);
    expect(draft.signatureSecret).toBe('');
    expect(draft.hasSecret).toBe(task !== undefined);
  }
});

test('the webhook editor round-trips its trigger and reports Listening instead of a next run', () => {
  const task = { id: 't', title: 'Webhook', prompt: '{{body.issue.title}}', enabled: true, schedule: { type: 'webhook', signature: null }, projectId: 'p', workspaceStrategy: { type: 'root' }, modelSelection: { instanceId: 'codex', model: 'm' } };
  const draft = editorDraft(task, 'p', 'codex:m');
  expect(draft.scheduleMode).toBe('webhook');
  const input = Object.fromEntries(Object.entries(draft).map(([key, value]) => [key, String(value)]));
  expect(taskInput(input, task).schedule).toEqual({ type: 'webhook', signature: null, maxDeliveryAgeMinutes: null, dedupe: {} });
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
  expect(calls).toEqual([['scheduledTasks.listWebhookDeliveries', { id: 'task', acceptAnyOutcome: true }], ['scheduledTasks.getWebhookDelivery', { id: 'task', deliveryId: 'd', acceptAnyOutcome: true }]]);
});


import { cleanupEntries, cleanupHasRules, cleanupPermitted, databaseCompactionNotices, formatBytes } from '../../src/settings/storage-maintenance';
import { adoptShellPrefs, shellPrefs } from '../../src/shell/shell-prefs';

test('cleanup recognizes zero-day settled and custom project rules while off suppresses inherited worktree rules', () => {
  expect(cleanupHasRules({ storageCleanup: { worktreeSettledAfterDays: 0 } })).toBe(true);
  expect(cleanupHasRules({ storageCleanup: { worktreeAfterDays: 3 }, worktreeCleanup: { mode: 'off' } })).toBe(false);
  expect(cleanupHasRules({ projectSettingsOverrides: { p: { worktreeCleanup: { mode: 'custom', rules: { worktreeOnDelete: true } } } } })).toBe(true);
});

test('one database notice lists every machine with a GiB to reclaim, largest first, and dismissal is per environment', () => {
  const status = (gib: number, extra: Record<string, unknown> = {}) => ({ reclaimableBytes: gib * 1024 ** 3, incrementalVacuumEnabled: false, compactCommand: `t3 doctor --compact # ${gib}`, ...extra });
  const machines = [{ environmentId: 'mac', label: 'Mac', status: status(1.5) }, { environmentId: 'remote', label: 'Remote', status: status(3.25) },
    { environmentId: 'small', label: 'Small', status: { ...status(1), reclaimableBytes: 1024 ** 3 - 1 } }, { environmentId: 'vacuum', label: 'Vacuum', status: status(9, { incrementalVacuumEnabled: true }) },
    { environmentId: 'loading', label: 'Loading', status: undefined }];
  expect(databaseCompactionNotices(machines, [])).toEqual([{ environmentId: 'remote', title: '3.3 GiB on Remote', command: 't3 doctor --compact # 3.25' },
    { environmentId: 'mac', title: '1.5 GiB on Mac', command: 't3 doctor --compact # 1.5' }]);
  const local = {}; adoptShellPrefs(local, { shell: { databaseStorageDismissals: ['mac', 3] } });
  const dismissed = shellPrefs({ local }).databaseStorageDismissals;
  expect(dismissed).toEqual(['mac']);
  expect(databaseCompactionNotices(machines, dismissed).map(notice => notice.environmentId)).toEqual(['remote']);
});

test('cleanup results name the machine, project, folder and thread of each entry (StorageSettings CleanupResults)', () => {
  const report = { omittedCount: 0, entries: [
    { kind: 'worktree', outcome: 'removed', reason: 'Thread settled 9 days ago', path: '/Users/me/.t3/worktrees/app/t3-feature', projectPath: '/Users/me/app', threadId: 't1', threadTitle: 'Add login', bytes: 5 * 1024 ** 2, files: null },
    { kind: 'worktree', outcome: 'kept', reason: 'No thread uses this folder, and it isn\'t a git worktree', path: '/Users/me/.t3/worktrees/app/orphan', projectPath: null, threadId: null, threadTitle: null, bytes: 2048, files: null },
    { kind: 'logs', outcome: 'removed', reason: 'Removed 3 rotated logs', path: null, threadId: null, threadTitle: null, bytes: 1500, files: 3 },
    { kind: 'browser-artifacts', outcome: 'failed', reason: 'Permission denied', path: null, threadId: null, threadTitle: null, bytes: null, files: null },
  ] };
  const { entries, omitted, empty } = cleanupEntries(report, 'Lane Mac');
  expect(entries.map(entry => [entry.name, entry.reason, entry.failed, entry.size])).toEqual([
    ['t3-feature', 'Thread settled 9 days ago', false, '5.00 MB'],
    ['/Users/me/.t3/worktrees/app/orphan', 'No thread uses this folder, and it isn\'t a git worktree', false, '2.00 KB'],
    ['Rotated logs', 'Removed 3 rotated logs', false, '3 files · 1.46 KB'],
    ['Browser artifacts', 'Permission denied', true, ''],
  ]);
  expect(entries[0]!.details.map(detail => [detail.label, detail.value, detail.code])).toEqual([['Machine', 'Lane Mac', false], ['Project', '/Users/me/app', true],
    ['Folder', '/Users/me/.t3/worktrees/app/t3-feature', true], ['Thread', 'Add login', false]]);
  expect(entries[3]!.details.map(detail => detail.label)).toEqual(['Machine']);
  expect([omitted, empty]).toEqual(['', '']);
  // A server older than projectPath leaves the Project line out.
  expect(cleanupEntries({ entries: [{ ...report.entries[0], projectPath: undefined }], omittedCount: 1234 }, 'Lane Mac')).toMatchObject({ omitted: 'Lane Mac: 1,234 more results.', empty: '' });
  expect(cleanupEntries({ entries: [], omittedCount: 0 }, 'Lane Mac')).toMatchObject({ entries: [], omitted: '', empty: 'Lane Mac: Nothing needed cleanup.' });
  expect([formatBytes(512), formatBytes(10.5 * 1024), formatBytes(150 * 1024 ** 3)]).toEqual(['512 B', '10.5 KB', '150 GB']);
  // The fold's first line takes its divider from the header: the first report's first entry, or its empty line.
  expect(cleanupEntries(report, 'Lane Mac', true).entries.map(entry => entry.first)).toEqual([true, false, false, false]);
  expect(cleanupEntries(report, 'Lane Mac').entries.some(entry => entry.first)).toBe(false);
  expect(cleanupEntries({ entries: [], omittedCount: 0 }, 'Lane Mac', true)).toMatchObject({ emptyFirst: true, omittedFirst: true });
});

test('the Last run fold is FoldedSettingsSection: its title and summary in the trigger, a chevron that turns when open', async () => {
  const source = await Bun.file(appPath('settings-storage.contract')).text();
  const fold = source.slice(source.indexOf('component CleanupLastRun'), source.indexOf('component CleanupResultRow'));
  expect(fold).toContain('CnFolded(title="Last run", summary=summary, open=open, sectionId="storage-cleanup-last-run", toggle=toggle');
  const kit = await Bun.file(appPath('settings-b-kit.contract')).text();
  const folded = kit.slice(kit.indexOf('component CnFolded'), kit.indexOf('component CnSelect'));
  // The title never shrinks away and the summary truncates after it (FoldedSettingsSection's shrink-0 / min-w-0 truncate).
  expect(folded).toContain('text title font-size="0.875rem" line-height="1.25rem" font-weight=500 flex-shrink=0');
  expect(folded).toContain('text summary font-size="0.75rem" line-height="1rem" min-width=0 line-clamp=1');
  expect(folded).toContain('CnGlyph(name=(open ? "chevron-down" : "chevron-right")');
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
  // The server marks a pull request's watch awaited (pullRequestWatchTasks).
  expect(presentBackgroundWork([{ kind: 'monitor', description: 'Watching pull request #12', awaited: true }])?.title).toBe('Watching pull request #12');
});

test('cleanup uses settings permission independently of orchestration writes', () => {
  const focused = { focused: true, key: '' };
  expect(cleanupPermitted({ scopes: ['orchestration:operate'] }, focused)).toBe(false);
  expect(cleanupPermitted({ scopes: ['environment:maintain'] }, focused)).toBe(false);
  expect(cleanupPermitted({ scopes: ['settings:write'] }, focused)).toBe(true);
});

import { contractComponent, type Env } from '../support/contract-action-fixture';
test('webhook copy and rotation complete without closing the editor or showing their internal completion marker', () => {
  const root = contractComponent('app.contract', 'T3Code');
  for (const message of ['', 'task-webhook-keep']) {
    const state: Env = { noticeExit: '', routeKeyboard: '', pendingModal: 'rest', restEditor: 'task', modalError: 'old', settingsRefresh: 7, commandPending: true, renameAfterOp: '', keybindingOpen: true, keybindingPrevious: 'old', keybindingRecording: true, keybindingSaving: '', keybindingDrafts: [], lineageStopping: false };
    root.call('commandCompleted', state, { changed: { message, relaunch: false, routeReorder: 0 }, routeKeyboardComplete: () => '', routeKeyboardOwned: false }, []);
    expect(state).toMatchObject({ modalError: '', commandPending: false, pendingModal: '', settingsRefresh: 8, restEditor: message ? 'task' : '' });
  }
});
