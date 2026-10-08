// Ported from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3):
// apps/web/src/components/ProviderUpdateLaunchNotification.logic.test.ts — "provider update
// launch notification logic" (every case but the sidebar pill's, which sidebar-provider-pill
// covers), "multi-backend update outcomes", "isTerminalProviderUpdatePhase", the incompatible
// latest-version case and "getProviderUpdateRunToastView", with their original names. The
// WSL grouping describes are out of scope. AsyncResult.success/failure are `success`/`failure`;
// Cause.interrupt() is a failure marked interrupted.
import { describe, expect, it } from 'bun:test';
import type { Obj } from './domain';
import {
  canOneClickUpdateProviderCandidate, collectProviderUpdateCandidates, collectProviderUpdateOutcomeSnapshots, collectUpdatedProviderSnapshots,
  failure, firstFailedProviderUpdateMessage, firstRejectedProviderUpdateMessage, getProviderUpdateInitialToastView, getProviderUpdateProgressToastView,
  getProviderUpdateRejectedToastView, getProviderUpdateRunToastView, hasOneClickUpdateProviderCandidate, isProviderSettingsUpdateCandidate,
  isProviderUpdateCandidate, isTerminalProviderUpdatePhase, providerUpdateNotificationKey, shouldShowPrimaryProviderUpdateToast, success,
  type LocalProviderUpdateOutcome, type SettledResult,
} from './provider-updates';

const checkedAt = '2026-04-23T10:00:00.000Z';
const laterCheckedAt = '2026-04-23T10:01:00.000Z';

function provider(input: { driver: string; instanceId?: string; enabled?: boolean; version?: string | null; latestVersion?: string | null; canUpdate?: boolean;
  updateCommand?: string | null; updateState?: Obj; advisoryStatus?: string }): Obj {
  const result: Obj = {
    instanceId: input.instanceId ?? input.driver, driver: input.driver, enabled: input.enabled ?? true, installed: true, version: input.version ?? '1.0.0',
    status: 'ready', auth: { status: 'authenticated' }, checkedAt, models: [], slashCommands: [], skills: [],
    versionAdvisory: { status: input.advisoryStatus ?? 'behind_latest', currentVersion: input.version ?? '1.0.0',
      latestVersion: 'latestVersion' in input ? input.latestVersion ?? null : '1.1.0',
      updateCommand: 'updateCommand' in input ? input.updateCommand ?? null : 'npm install -g provider', canUpdate: input.canUpdate ?? true, checkedAt, message: 'Update available.' },
  };
  return input.updateState ? { ...result, updateState: input.updateState } : result;
}
const updateCandidate = provider;
const state = (status: string, message: string, extra: Obj = {}): Obj => ({ status, startedAt: checkedAt, finishedAt: status === 'running' ? null : checkedAt, message, output: null, ...extra });

describe('provider update launch notification logic', () => {
  it('detects enabled providers with a latest-version advisory', () => {
    expect(isProviderUpdateCandidate(provider({ driver: 'codex' }))).toBe(true);
    expect(isProviderUpdateCandidate(provider({ driver: 'codex', enabled: false }))).toBe(false);
    expect(isProviderUpdateCandidate(provider({ driver: 'codex', advisoryStatus: 'current', latestVersion: null }))).toBe(false);
    expect(isProviderUpdateCandidate(provider({ driver: 'codex', latestVersion: null }))).toBe(false);
  });

  it('deduplicates multi-instance provider candidates by driver', () => {
    expect(collectProviderUpdateCandidates([
      provider({ driver: 'codex', instanceId: 'codex_personal', latestVersion: '1.1.0' }),
      provider({ driver: 'codex', instanceId: 'codex', latestVersion: '1.1.0' }),
      provider({ driver: 'cursor', latestVersion: '0.3.0' }),
    ])).toHaveLength(2);
  });

  it('disables one-click updates when provider instances disagree on the update command', () => {
    const candidate = updateCandidate({ driver: 'claudeAgent', instanceId: 'claude_personal', latestVersion: '2.1.123' });
    expect(canOneClickUpdateProviderCandidate(candidate, [candidate,
      provider({ driver: 'claudeAgent', instanceId: 'claude_work', latestVersion: '2.1.123', canUpdate: true, updateCommand: 'bun add -g @anthropic-ai/claude-code@latest' })])).toBe(false);
  });

  it('keeps one-click updates enabled when sibling instances are already current', () => {
    const candidate = updateCandidate({ driver: 'claudeAgent', instanceId: 'claude_personal', latestVersion: '2.1.123', updateCommand: 'npm install -g @anthropic-ai/claude-code@latest' });
    const sibling = provider({ driver: 'claudeAgent', instanceId: 'claude_work', version: '2.1.123', latestVersion: '2.1.123', advisoryStatus: 'current', canUpdate: false, updateCommand: null });
    expect(hasOneClickUpdateProviderCandidate(candidate, [candidate, sibling])).toBe(true);
    expect(canOneClickUpdateProviderCandidate(candidate, [candidate, sibling])).toBe(true);
  });

  it('keeps the inline update action available while a provider update is already running', () => {
    const candidate = updateCandidate({ driver: 'codex', updateState: state('running', 'Updating provider.') });
    expect(hasOneClickUpdateProviderCandidate(candidate, [candidate])).toBe(true);
    expect(canOneClickUpdateProviderCandidate(candidate, [candidate])).toBe(false);
  });

  it('builds a notification key from provider latest versions', () => {
    const codex = updateCandidate({ driver: 'codex', version: '1.0.0', latestVersion: '1.1.0' });
    const cursor = updateCandidate({ driver: 'cursor', version: '0.2.0', latestVersion: '0.3.0' });
    expect(providerUpdateNotificationKey([codex, cursor])).toBe('codex:1.1.0|cursor:0.3.0');
    expect(providerUpdateNotificationKey([])).toBeNull();
  });

  it('keeps the same notification key while the published update version is unchanged', () => {
    const first = updateCandidate({ driver: 'codex', version: '1.0.0', latestVersion: '1.2.0' });
    const second = updateCandidate({ driver: 'codex', version: '1.1.0', latestVersion: '1.2.0' });
    const nextPublishedVersion = updateCandidate({ driver: 'codex', version: '1.1.0', latestVersion: '1.3.0' });
    expect(providerUpdateNotificationKey([first])).toBe(providerUpdateNotificationKey([second]));
    expect(providerUpdateNotificationKey([nextPublishedVersion])).not.toBe(providerUpdateNotificationKey([first]));
  });

  it('tracks updated provider snapshots by instance instead of collapsing to a sibling driver', () => {
    const updatedPersonal = provider({ driver: 'codex', instanceId: 'codex_personal', version: '1.1.0', latestVersion: '1.1.0', advisoryStatus: 'current', updateState: state('succeeded', 'Provider updated.') });
    const currentDefaultSibling = provider({ driver: 'codex', instanceId: 'codex', version: '1.1.0', latestVersion: '1.1.0', advisoryStatus: 'current' });
    expect(collectUpdatedProviderSnapshots({ results: [success({ providers: [updatedPersonal, currentDefaultSibling] })], providerInstanceIds: new Set(['codex_personal']) }))
      .toEqual([updatedPersonal]);
  });

  it('describes a single one-click update', () => {
    const view = getProviderUpdateInitialToastView({ updateProviders: [updateCandidate({ driver: 'codex', latestVersion: '1.1.0' })],
      oneClickProviders: [updateCandidate({ driver: 'codex', latestVersion: '1.1.0' })] });
    expect(view).toMatchObject({ phase: 'initial', type: 'warning', title: 'Update Available: Codex v1.1.0', description: 'Install the update now or review provider settings.' });
  });

  it('describes settings-only updates without one-click support', () => {
    const view = getProviderUpdateInitialToastView({ updateProviders: [updateCandidate({ driver: 'codex', canUpdate: false }), updateCandidate({ driver: 'cursor', canUpdate: false })], oneClickProviders: [] });
    expect(view.description).toBe('Codex and Cursor can be updated from provider settings.');
  });

  it('uses server update state for running progress', () => {
    const view = getProviderUpdateProgressToastView({ providers: [provider({ driver: 'codex', updateState: state('running', 'Updating provider.') })], providerCount: 1 });
    expect(view).toMatchObject({ phase: 'running', type: 'loading', title: 'Updating provider' });
    expect(shouldShowPrimaryProviderUpdateToast(view)).toBe(false);
  });

  it('keeps the initial prompt and terminal outcomes visible as toasts', () => {
    expect(shouldShowPrimaryProviderUpdateToast(getProviderUpdateInitialToastView({ updateProviders: [updateCandidate({ driver: 'codex' })], oneClickProviders: [updateCandidate({ driver: 'codex' })] }))).toBe(true);
    expect(shouldShowPrimaryProviderUpdateToast(getProviderUpdateRejectedToastView(1, 'boom'))).toBe(true);
  });

  it('uses server failure state for failed progress', () => {
    const view = getProviderUpdateProgressToastView({ providers: [provider({ driver: 'codex', updateState: state('failed', 'command failed', { output: 'stderr' }) })], providerCount: 1 });
    expect(view).toMatchObject({ phase: 'failed', type: 'error', title: 'Provider update failed', description: 'command failed' });
  });

  it('keeps unchanged providers actionable from settings', () => {
    const view = getProviderUpdateProgressToastView({ providers: [provider({ driver: 'cursor', updateState: state('unchanged', 'still old') })], providerCount: 1 });
    expect(view).toMatchObject({ phase: 'unchanged', type: 'warning', title: 'Provider still needs an update', description: 'Cursor still appears outdated. Check provider settings for details.' });
  });

  it('marks progress succeeded once every attempted provider is no longer outdated', () => {
    const view = getProviderUpdateProgressToastView({ providers: [provider({ driver: 'codex', version: '1.1.0', latestVersion: '1.1.0', advisoryStatus: 'current', updateState: state('succeeded', 'Provider updated.') })], providerCount: 1 });
    expect(view).toMatchObject({ phase: 'succeeded', type: 'success', title: 'Provider updated', description: 'New sessions will use the updated provider.', dismissAfterVisibleMs: 3_000 });
  });

  it('falls back to a rejected RPC message for transport-level failures', () => {
    const results = [failure(new Error('WebSocket closed'))];
    expect(firstFailedProviderUpdateMessage(results)).toBe('WebSocket closed');
    expect(getProviderUpdateRejectedToastView(2, 'WebSocket closed')).toMatchObject({ phase: 'failed', title: 'Provider updates failed', description: 'WebSocket closed' });
  });

  it('collects only attempted provider snapshots from update responses', () => {
    const codex = provider({ driver: 'codex' }), cursor = provider({ driver: 'cursor' });
    expect(collectUpdatedProviderSnapshots({ results: [success({ providers: [codex, cursor] })], providerInstanceIds: new Set(['cursor']) })).toEqual([cursor]);
  });

  describe('multi-backend update outcomes', () => {
    const fulfilledOutcome = (isPrimary: boolean, snapshot: Obj | null, environment = 'env'): SettledResult<LocalProviderUpdateOutcome> => ({
      status: 'fulfilled', value: { environmentId: environment, isPrimary, driver: String(snapshot?.driver ?? 'codex'), instanceId: String(snapshot?.instanceId ?? 'codex'), provider: snapshot } });

    it("surfaces a secondary backend's failed update over the primary's success", () => {
      const snapshots = collectProviderUpdateOutcomeSnapshots([fulfilledOutcome(true, provider({ driver: 'codex', updateState: state('succeeded', 'Provider updated.') })),
        fulfilledOutcome(false, provider({ driver: 'codex', updateState: state('failed', 'npm: NotFound') }))]);
      expect(snapshots).toHaveLength(1);
      expect((snapshots[0]?.updateState as Obj | undefined)?.status).toBe('failed');
      expect(getProviderUpdateProgressToastView({ providers: snapshots, providerCount: 1 })).toMatchObject({ phase: 'failed' });
    });

    it("surfaces a secondary backend that stayed outdated over the primary's success", () => {
      const snapshots = collectProviderUpdateOutcomeSnapshots([fulfilledOutcome(true, provider({ driver: 'codex', updateState: state('succeeded', 'Provider updated.') })),
        fulfilledOutcome(false, provider({ driver: 'codex', updateState: state('unchanged', 'still outdated') }))]);
      expect((snapshots[0]?.updateState as Obj | undefined)?.status).toBe('unchanged');
      expect(getProviderUpdateProgressToastView({ providers: snapshots, providerCount: 1 })).toMatchObject({ phase: 'unchanged' });
    });

    it('reports success only when every backend succeeded', () => {
      const snapshots = collectProviderUpdateOutcomeSnapshots([fulfilledOutcome(true, provider({ driver: 'codex', updateState: state('succeeded', 'Provider updated.') })),
        fulfilledOutcome(false, provider({ driver: 'codex', updateState: state('succeeded', 'Provider updated.') }))]);
      expect(getProviderUpdateProgressToastView({ providers: snapshots, providerCount: 1 })).toMatchObject({ phase: 'succeeded' });
    });

    it('ignores backends that did not return the targeted instance', () => {
      const primary = provider({ driver: 'codex', updateState: state('succeeded', 'Provider updated.') });
      expect(collectProviderUpdateOutcomeSnapshots([fulfilledOutcome(true, primary), fulfilledOutcome(false, null)])).toEqual([primary]);
    });

    it('treats a rejected dispatch as not contributing a snapshot', () => {
      const primary = provider({ driver: 'codex', updateState: state('succeeded', 'Provider updated.') });
      const results: SettledResult<LocalProviderUpdateOutcome>[] = [fulfilledOutcome(true, primary), { status: 'rejected', reason: new Error('WebSocket closed') }];
      expect(collectProviderUpdateOutcomeSnapshots(results)).toEqual([primary]);
      expect(firstRejectedProviderUpdateMessage(results)).toBe('WebSocket closed');
    });
  });

  describe('isTerminalProviderUpdatePhase', () => {
    it('treats succeeded/failed/unchanged as terminal', () => {
      expect(isTerminalProviderUpdatePhase('succeeded')).toBe(true);
      expect(isTerminalProviderUpdatePhase('failed')).toBe(true);
      expect(isTerminalProviderUpdatePhase('unchanged')).toBe(true);
    });
    it('treats running/initial as non-terminal so they are not persisted', () => {
      expect(isTerminalProviderUpdatePhase('running')).toBe(false);
      expect(isTerminalProviderUpdatePhase('initial')).toBe(false);
    });
  });
});

it('does not offer incompatible latest versions and restores suggestions after policy relaxation', () => {
  const installed = provider({ driver: 'codex' });
  for (const latestVersionStatus of ['broken', 'unsupported', 'supported', 'unknown']) {
    const snapshot: Obj = { ...installed, compatibilityAdvisory: { status: 'supported', latestVersionStatus, message: null, recommendedRange: null, recommendedVersion: null } };
    const expected = latestVersionStatus === 'supported' || latestVersionStatus === 'unknown';
    expect(isProviderUpdateCandidate(snapshot)).toBe(expected);
    expect(isProviderSettingsUpdateCandidate(snapshot)).toBe(expected);
  }
});

describe('getProviderUpdateRunToastView', () => {
  const updateState = (status: string, message: string): Obj => ({ status, startedAt: checkedAt, finishedAt: laterCheckedAt, message, output: null });
  const run = (machineLabel: string, providerDriver: string, result: ReturnType<typeof success<{ providers: Obj[] }>>) => ({ machineLabel, driver: providerDriver, instanceId: providerDriver, result });

  it('lists every failed update and ignores interrupted ones', () => {
    const view = getProviderUpdateRunToastView([
      run('Mac Studio', 'codex', success({ providers: [provider({ driver: 'codex', updateState: updateState('succeeded', 'Provider updated.') })] })),
      run('Mac Studio', 'claudeAgent', success({ providers: [provider({ driver: 'claudeAgent', updateState: updateState('failed', 'npm exited with code 1.') })] })),
      run('Laptop', 'codex', failure(new Error('WebSocket closed'))),
      run('Server', 'codex', failure(new Error('interrupted'), true)),
    ]);
    expect(view).toEqual({ type: 'error', title: '2 of 3 provider updates failed', description: 'Mac Studio · Claude: npm exited with code 1.\nLaptop · Codex: WebSocket closed' });
  });

  it('reports success when every update succeeded', () => {
    const succeeded = success({ providers: [provider({ driver: 'codex', updateState: updateState('succeeded', 'Provider updated.') })] });
    expect(getProviderUpdateRunToastView([run('Mac Studio', 'codex', succeeded), run('Laptop', 'codex', succeeded)]))
      .toEqual({ type: 'success', title: '2 providers updated', description: 'New sessions will use the updated providers.' });
    expect(getProviderUpdateRunToastView([run('Server', 'codex', failure(new Error('interrupted'), true))])).toBeNull();
  });
});
