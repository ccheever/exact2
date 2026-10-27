// Ported from T3 Code 7c3b3b3605ef (MIT, see LICENSE-T3):
// apps/web/src/components/ProviderUpdateLaunchNotification.logic.test.ts — "provider update
// launch notification logic" (every case but the sidebar pill's, which sidebar-provider-pill
// covers), the incompatible latest-version case and "getProviderUpdateRunToastView", with their
// original names, and two earlier candidate cases. AsyncResult.success/failure are
// `success`/`failure`; Cause.interrupt() is a failure marked interrupted.
import { describe, expect, it } from 'bun:test';
import type { Obj } from '../../src/client/domain';
import {
  canOneClickUpdateProviderCandidate, collectProviderUpdateCandidates, failure, getProviderUpdateRunToastView, hasOneClickUpdateProviderCandidate,
  isProviderSettingsUpdateCandidate, isProviderUpdateCandidate, success,
} from '../../src/providers/provider-updates';

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
  const run = (machineLabel: string, providerDriver: string, result: ReturnType<typeof success<{ providers: Obj[] }>>) => ({ environmentId: machineLabel, machineLabel, driver: providerDriver, instanceId: providerDriver, result });

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
