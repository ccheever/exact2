// What 0.1.2-nightly.20261021.3457, 0.1.4-nightly.20261027.3770 and 0.1.5 servers add (T3 Code 74541bab17, 7c3b3b3605ef
// and 6d450a4284, MIT, see LICENSE-T3: packages/contracts): optional fields decode without failing, forward-compatible arrays and
// reasons drop what this build does not know, a legacy writing-style string reads as the struct, and EAS hosts join
// the device state's hosts.
import { describe, expect, test } from 'bun:test';
import { applyShell, applyThread, decodeChangeBreakdown, decodeDeviceState, decodeEasAccount, decodeIgnoredFiles, initialShell, obj, threadSnapshot, visibleTurnItems, type Obj } from '../../src/client/domain';
import { allowedMergeMethods, resolveBaseFreshness } from '../../src/details/pr-row-logic';
import { contextCompactionLabel } from '../../src/timeline/timeline-worklog';
import { failure } from '../../src/transport/transport-wire';
import { rpcPermissions } from '../../src/connections/command-permissions';
import { scopedSettingPatch, writingStyleOf } from '../../src/settings/scoped-settings';
import { canSnooze, pendingInput, raisedHand, sidebarStatus } from '../../src/sidebar/sidebar-model';
import { mergeUsage, providersWithUsage } from '../../src/usage/pages-usage';
import { readDiscoveredServers } from '../../src/browser/browser-targets';
import { readPreviewEvent } from '../../src/browser/browser-automation';
import { applyTerminalAttachStreamEvent, EMPTY_TERMINAL_BUFFER_STATE, type TerminalAttachStreamEvent } from '../../src/terminal/terminal-session';
import { resolveServerConfigVersionMismatch } from '../../src/connections/version-skew';
import { decodeClientPrefs } from '../../src/settings/settings-core';
import { advancedBackgroundValue, profileOption, resolveBackground } from '../../src/settings/settings-a-background';

const at = '2026-10-21T00:00:00.000Z';
const usage = { usedTokens: 10, updatedAt: at, autoCompactThreshold: 200_000, promptCacheExpiresAt: at };

describe('new optional fields', () => {
  test('a thread with every new field loads', () => {
    const user = { id: 'i1', ordinal: 1, threadId: 't', runId: 'r', nodeId: 'n', type: 'user_message', status: 'completed', messageId: 'm1', text: 'run', inputIntent: 'turn_start', attachments: [], workflowScript: true };
    const question = { id: 'i2', ordinal: 2, threadId: 't', runId: 'r', nodeId: 'n', type: 'user_input_request', status: 'pending', requestId: 'q', questions: [{ id: 'a', header: 'H', question: 'Q?', options: [{ label: 'One', description: 'First', preview: '<b>1</b>' }] }] };
    const state = threadSnapshot({ snapshotSequence: 3, projection: {
      thread: { id: 't', title: 'T', pullRequests: [{ host: 'github.com', repository: 'o/r', number: 1, url: 'https://github.com/o/r/pull/1', repositoryWebUrl: 'https://github.com/o/r', source: 'agent', linkedAt: at, snapshot: null, stack: null, endedWatch: { startedAt: at } }] },
      runs: [], attempts: [], nodes: [], subagents: [{ id: 's', workflow: { agents: [{ index: 0, label: 'A', state: 'completed', completedAt: 1 }] } }], providerSessions: [], providerThreads: [], providerTurns: [],
      runtimeRequests: [], messages: [{ id: 'm1', text: 'run', role: 'user', workflowScript: true }], plans: [], turnItems: [user, question], checkpointScopes: [], checkpoints: [], contextHandoffs: [],
      contextTransfers: [{ id: 'x', sourceTokenUsage: usage }], visibleTurnItems: [user, question].map((item, position) => ({ position, visibility: 'local', sourceThreadId: 't', sourceItemId: item.id, item })), updatedAt: at,
    } });
    expect(visibleTurnItems(state).map(row => obj(row.item).id)).toEqual(['i1', 'i2']);
    expect(state.projection.contextTransfers).toEqual([{ id: 'x', sourceTokenUsage: usage }]);
  });

  test('a shell thread with the new run fields loads', () => {
    const shell = applyShell(initialShell(), { snapshotSequence: 1, projects: [], threads: [{ id: 't', projectId: 'p', title: 'T', status: 'running', latestRunStopRequested: true, blockedOnThreadWait: true,
      pendingRuntimeRequest: { id: 'q', kind: 'user_input', createdAt: at, blocking: false } }] });
    expect(shell.threads[0]).toMatchObject({ latestRunStopRequested: true, blockedOnThreadWait: true });
  });

  test('the new RPCs decode, a change breakdown from before the docs split included', () => {
    expect(decodeIgnoredFiles({ files: [{ path: '.env', byteLength: 12 }], truncated: false })).toEqual({ files: [{ path: '.env', byteLength: 12 }], truncated: false });
    const lines = { additions: 1, deletions: 2 };
    expect(decodeChangeBreakdown({ code: lines, tests: lines, mocks: lines, generated: lines })?.docs).toEqual({ additions: 0, deletions: 0 });
    expect(decodeChangeBreakdown(null)).toBeNull();
    expect(decodeEasAccount({ signedInAs: 'me', credentialSource: 'env', apps: [{ id: 'a', fullName: '@me/app' }] }).apps).toEqual([{ id: 'a', fullName: '@me/app' }]);
  });
});

describe('what a 0.1.4 server adds', () => {
  const row = (item: Obj, position: number) => ({ position, visibility: 'local', sourceThreadId: 't', sourceItemId: item.id, item });
  const base = { threadId: 't', runId: 'r', nodeId: 'n' };
  const items: Obj[] = [
    { ...base, id: 'u', ordinal: 1, type: 'user_message', status: 'completed', messageId: 'm', text: '/compact', inputIntent: 'turn_start', attachments: [], idleCompaction: 'onReturn' },
    { ...base, id: 'c', ordinal: 2, type: 'command_execution', status: 'running', input: 'grok monitor', monitor: true },
    { ...base, id: 'a', ordinal: 3, type: 'approval_request', status: 'pending', requestId: 'q', requestKind: 'command', prompt: 'Run it?', reason: 'An ask rule matched.', fullAccessOverride: 'ask_rule' },
    { ...base, id: 's', ordinal: 4, type: 'secret_request', status: 'pending', label: 'TOKEN', reason: '', secretStatus: 'pending', delivery: 'file' },
  ];

  test('a thread with an idle compaction, a monitor, a full-access approval reason and a file secret opens', () => {
    const state = threadSnapshot({ snapshotSequence: 3, projection: {
      thread: { id: 't', title: 'T', opens: { openedAt: 1, score: 2, picks: [{ query: 'api', at: 1 }] },
        pullRequests: [{ host: 'github.com', repository: 'o/r', number: 1, url: 'https://github.com/o/r/pull/1', watch: { owedChecks: ['ci'], checksAwaitedSince: at } }] },
      runs: [{ id: 'r', status: 'running', restartCancelledBackgroundWork: [{ kind: 'monitor', label: 'grok monitor', awaited: true }] }], attempts: [], nodes: [], subagents: [], providerSessions: [],
      providerThreads: [{ id: 'pt', claudeAttachmentBudget: { bytesUsed: 1, pagesUsed: 1 } }], providerTurns: [],
      runtimeRequests: [{ id: 'q', status: 'pending', responseCapability: { type: 'live', providerSessionId: 'ps', supportsFeedback: true }, feedback: 'Use the staging bucket.' }],
      messages: [{ id: 'm', text: '/compact', role: 'user', idleCompaction: 'onReturn' }], plans: [], turnItems: items, checkpointScopes: [], checkpoints: [], contextHandoffs: [],
      contextTransfers: [], visibleTurnItems: items.map(row), updatedAt: at,
    } });
    expect(visibleTurnItems(state).map(entry => obj(entry.item).id)).toEqual(['u', 'c', 'a', 's']);
  });

  test('a shell thread with an error code, open stats and awaited background work loads', () => {
    const shell = applyShell(initialShell(), { snapshotSequence: 1, projects: [{ id: 'p', title: 'P', workspaceRoot: '/repo' }], threads: [{ id: 't', projectId: 'p', title: 'T', status: 'error',
      lastError: 'Rate limited', lastErrorCode: 'rate_limit', opens: null, pendingBackgroundTasks: [{ taskId: 'w', kind: 'monitor', awaited: true }] }] });
    expect(shell.threads[0]).toMatchObject({ lastErrorCode: 'rate_limit', pendingBackgroundTasks: [{ awaited: true }] });
  });

  test('a pull request read at a time, that always reports checks and takes ranged comments reads as before; Muse Code usage is a provider', () => {
    const detail: Obj = { state: 'open', observedAt: 1_000, mergeCapabilities: { merge: true, squash: true }, capabilities: { mergeMethods: ['merge', 'squash'], checksAlwaysReport: true,
      review: { inlineComment: true, rangeComment: true, reply: true, resolve: true, verdicts: ['comment'] } }, viewerPermissions: {} };
    expect(allowedMergeMethods(detail)).toEqual(['merge', 'squash']);
    const bucket = { day: '2026-10-27', provider: 'muse', model: 'muse-1', totals: { uncachedInputTokens: 1, cachedInputTokens: 0, cacheCreationTokens: 0, outputTokens: 1, reasoningTokens: 0 },
      costUsd: 1, cacheSavingsUsd: 0, costSource: 'modelPriced', records: 1, unpricedRecords: 0, sessions: 1 };
    const merged = mergeUsage([{ id: 'env', label: 'Mac', summary: { contractVersion: 6, readAt: at, buckets: [bucket],
      sources: [{ fingerprint: { hostId: 'mac', provider: 'muse', resolvedHomePath: '/m', volumeId: '1:2' }, status: 'ok', scannedFiles: 1, skippedFiles: 0, malformedRecords: 0, distinctSessions: 1, message: null }] } }]);
    expect(merged.providers.map(entry => [entry.provider, entry.costUsd, entry.sessions])).toEqual([['muse', 1, 1]]);
    // A provider the page knows (usage-providers.ts), so it draws: Muse Code's series and its row.
    expect(providersWithUsage(merged)).toEqual(['muse']);
  });

  test('a guest share update carries the whole thread and applies as a thread event', () => {
    const state = threadSnapshot({ snapshotSequence: 3, projection: { thread: { id: 't', title: 'T' }, runs: [], attempts: [], nodes: [], subagents: [], providerSessions: [],
      providerThreads: [], providerTurns: [], runtimeRequests: [], messages: [], plans: [], turnItems: [], checkpointScopes: [], checkpoints: [], contextHandoffs: [],
      contextTransfers: [], visibleTurnItems: [], updatedAt: at } });
    const later = '2026-10-27T12:00:00.000Z';
    const next = applyThread(state, { kind: 'event', sequence: 4, event: { id: 'e', type: 'thread.guest-share-updated', threadId: 't', occurredAt: later,
      payload: { id: 't', title: 'T', guestShare: { status: 'active' } } } });
    expect([next.sequence, obj(next.projection.thread).guestShare, next.projection.updatedAt]).toEqual([4, { status: 'active' }, later]);
  });
});

describe('what a 0.1.5 server adds', () => {
  test('a discovered server an agent started reads as before', () => {
    const { servers } = readDiscoveredServers({ servers: [{ host: '127.0.0.1', port: 5173, url: 'http://127.0.0.1:5173', processName: 'vite', pid: 42, terminal: null, agent: { threadId: 't' } }], configuredUrlProbing: false });
    expect(servers).toEqual([{ host: '127.0.0.1', port: 5173, url: 'http://127.0.0.1:5173', processName: 'vite', pid: 42, terminal: null }]);
  });

  test('the new environment capabilities leave a matching server current', () => {
    const capabilities = { localServerStop: true, triage: true, threadProjectFromScratch: true, desktopAppChannelSwitch: true, databaseMigration: 72 };
    expect(resolveServerConfigVersionMismatch({ environment: { serverVersion: '0.1.5', capabilities } })).toBeNull();
  });

  test('a headless preview tab opens', () => {
    const snapshot = { threadId: 't', tabId: 'tab', navStatus: { _tag: 'Success', url: 'https://example.com', title: 'Example' }, canGoBack: false, canGoForward: false, updatedAt: at, runtime: 'server', headless: true };
    expect(readPreviewEvent({ type: 'opened', threadId: 't', tabId: 'tab', createdAt: at, serverEpoch: 'e', revision: 1, snapshot })).toMatchObject({ type: 'opened', snapshot: { tabId: 'tab', runtime: 'server' } });
  });

  test('a terminal snapshot and its activity carry the shared grid', () => {
    const snapshot = { threadId: 't', terminalId: 'term', cwd: '/repo', worktreePath: null, status: 'running', pid: 7, history: 'ready\n', exitCode: null, exitSignal: null, updatedAt: at, cols: 120, rows: 30 };
    const opened = applyTerminalAttachStreamEvent(EMPTY_TERMINAL_BUFFER_STATE, { type: 'snapshot', snapshot } as unknown as TerminalAttachStreamEvent);
    expect(opened.status).toBe('running');
    const activity = { type: 'activity', threadId: 't', terminalId: 'term', createdAt: at, hasRunningSubprocess: false, cols: 100, rows: 40 } as unknown as TerminalAttachStreamEvent;
    expect(applyTerminalAttachStreamEvent(opened, activity)).toBe(opened);
  });

  test('a pull request with maintainer edits reads as before', () => {
    const detail: Obj = { state: 'open', maintainerCanModify: true, mergeCapabilities: { merge: true }, capabilities: { mergeMethods: ['merge'], edit: { maintainerEdits: true } },
      viewerPermissions: { maintainerEdits: true } };
    expect(allowedMergeMethods(detail)).toEqual(['merge']);
  });

  test('a write still works beside the new server settings', () => {
    const settings = { snoozePresets: [{ kind: 'after', minutes: 60 }, { kind: 'at', day: 'monday', time: '09:00' }], fallbackModelSelection: { instanceId: 'codex', model: 'gpt-6' },
      backgroundActivity: { schemaVersion: 1, profile: 'balanced', overrides: {} }, projectSettingsOverrides: {} };
    expect(scopedSettingPatch(settings, 'p', 'defaultAutoPull', true, false)).toEqual({ projectSettingsOverrides: { p: { defaultAutoPull: true } } });
  });

  test('the new client settings, and the removed ones a saved file may hold, decode', () => {
    const prefs = decodeClientPrefs({ effortDisplay: 'bars', threadDetailsItems: { usage: true }, composerCollapseOnScroll: false, sidebarThreadPreviewCount: 9,
      dismissedProviderUpdateNotificationKeys: ['codex:0.42.0'], chatWidth: 'wide' });
    expect(prefs.chatWidth).toBe('wide');
  });

  // BackgroundActivityOverrides has no idleClientTtl at 6d450a4284, so the server drops the key an Advanced write
  // still carries. No control sets it, so it only ever holds the preset's value, which the read falls back to.
  test('an Advanced write reads back the same once the server drops its idleClientTtl', () => {
    const settings: Obj = { backgroundActivity: { schemaVersion: 1, profile: 'balanced', overrides: {} } };
    const written = obj(advancedBackgroundValue(settings, 'git', '60'));
    const { idleClientTtl, ...kept } = obj(written.overrides);
    expect(idleClientTtl).toBe(45_000);
    const stored = { backgroundActivity: { ...written, overrides: kept } };
    expect(resolveBackground(stored)).toEqual(resolveBackground({ backgroundActivity: written }));
    expect([resolveBackground(stored).automaticGitFetchInterval, profileOption(stored)]).toEqual([60, 'advanced']);
  });
});

describe('forward compatibility', () => {
  test('pull request capability arrays drop methods this build does not know', () => {
    const detail: Obj = { state: 'open', baseComparison: 'behind', mergeability: 'mergeable', behindBy: 2, mergeCapabilities: { merge: true, squash: true, 'fast-forward': true },
      capabilities: { mergeMethods: ['merge', 'fast-forward', 'squash'], updateMethods: ['rebase', 'cherry-pick'] }, viewerPermissions: { updateMethods: ['rebase', 'cherry-pick'] } };
    expect(allowedMergeMethods(detail)).toEqual(['merge', 'squash']);
    expect(resolveBaseFreshness(detail)?.methods).toEqual(['rebase']);
  });

  test('a git failure names a known reason, ref_not_found included, and drops an unknown one', () => {
    const git = (reason: string) => failure({ _tag: 'GitCommandError', operation: 'switchRef', command: 'git', cwd: '/repo', detail: 'exit 128', reason });
    expect(git('ref_not_found').message).toBe('Git command failed in switchRef (/repo): exit 128 (ref_not_found)');
    expect([git('newer_reason').message, git('newer_reason').reason]).toEqual(['Git command failed in switchRef (/repo): exit 128', '']);
  });

  test('a legacy writing style string reads as the struct, and a write sends the struct', () => {
    expect(writingStyleOf('Use the imperative.')).toEqual({ mode: 'custom', customInstructions: 'Use the imperative.', followChangeRequestTemplates: true });
    expect(writingStyleOf('')).toEqual({ mode: 'repo_conventions', customInstructions: '', followChangeRequestTemplates: true });
    const settings = { sourceControlWritingStyle: 'Use the imperative.', projectSettingsOverrides: {} };
    expect(scopedSettingPatch(settings, '', 'sourceControlWritingStyle.followChangeRequestTemplates', false, false))
      .toEqual({ sourceControlWritingStyle: { mode: 'custom', customInstructions: 'Use the imperative.', followChangeRequestTemplates: false } });
  });

  test('EAS hosts travel as easHosts and join hosts', () => {
    const state = decodeDeviceState({ hosts: [{ id: 'local', kind: 'local' }], easHosts: [{ id: 'cloud', kind: 'eas' }], hostStatus: 'ready' });
    expect(state).toEqual({ hosts: [{ id: 'local', kind: 'local' }, { id: 'cloud', kind: 'eas' }], hostStatus: 'ready' });
    const older = { hosts: [{ id: 'local', kind: 'local' }] };
    expect(decodeDeviceState(older)).toBe(older);
  });
});

describe('client behaviour', () => {
  test('an async question keeps a working thread Working, and still blocks snoozing', () => {
    const asking = (status: string, blocking?: boolean): Obj => ({ id: 't', status, latestRunId: 'r', pendingRuntimeRequest: { id: 'q', kind: 'user_input', createdAt: at, ...(blocking === undefined ? {} : { blocking }) } });
    expect([pendingInput(asking('running', false)), sidebarStatus(asking('running', false))]).toEqual([false, 'working']);
    expect([pendingInput(asking('waiting', false)), sidebarStatus(asking('waiting', false))]).toEqual([true, 'input']);
    expect(sidebarStatus(asking('running'))).toBe('input');
    expect(canSnooze(asking('running', false), Date.parse(at))).toBe(false);
    expect(raisedHand(asking('running', false))).toBe(true);
  });

  test('a compaction still pending or waiting reads as in progress', () => {
    expect(['pending', 'running', 'waiting', 'completed'].map(status => contextCompactionLabel({ status }))).toEqual(['Compacting context', 'Compacting context', 'Compacting context', 'Context compacted']);
  });

  test('a pull request created for a thread also needs orchestration read', () => {
    expect(rpcPermissions('git.runStackedAction', { action: 'create_pr', threadId: 't' })).toEqual(['source-control:write', 'orchestration:read']);
    expect(rpcPermissions('git.runStackedAction', { action: 'commit_push_pr', threadId: 't' })).toEqual(['source-control:write', 'orchestration:read']);
    expect(rpcPermissions('git.runStackedAction', { action: 'create_pr', projectId: 'p' })).toEqual(['source-control:write']);
    expect(rpcPermissions('git.runStackedAction', { action: 'commit', threadId: 't' })).toEqual(['source-control:write']);
    expect(['preview.close', 'projects.listIgnoredFiles', 'pullRequests.changeBreakdown', 'device.easAccount', 'pullRequests.filterOptions'].map(method => rpcPermissions(method, {})))
      .toEqual([['preview:operate'], ['filesystem:read'], ['orchestration:read'], ['settings:write'], ['orchestration:read']]);
  });

  test('the operations a 0.1.5 server adds need the reference scopes; the methods it removed are gone', () => {
    expect(['preview.stopLocalServer', 'server.prepareTriage', 'projects.createFromThread', 'pullRequests.update'].map(method => rpcPermissions(method, {})))
      .toEqual([['terminal:operate'], ['orchestration:operate'], ['orchestration:operate'], ['source-control:write']]);
    for (const method of ['server.getSettings', 'scheduledTasks.list', 'terminal.restart', 'preview.navigate', 'preview.refresh', 'server.reportHostPowerState', 'server.getBackgroundPolicy',
      'subscribeBackgroundPolicy', 'subscribeTerminalEvents', 'orchestration.subscribeArchivedShell', 'provider.codex.auth-callback.subscribe']) expect(rpcPermissions(method, {})).toEqual([]);
  });
});
