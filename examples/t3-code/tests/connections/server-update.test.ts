// Ports of T3 Code's update tests (1e2ecbd975, MIT, see LICENSE-T3), case names kept:
// packages/client-runtime/src/state/server.test.ts ("server state projection" update cases,
// "update restart reconnect nudges") and apps/web/src/components/ServerUpdateAction.test.tsx
// (the single-environment ServerUpdateAction cases and ServerUpdateProgress). The update runs
// as an outdated-hosts job here (transport-outdated.ts), so "the shared update flow" is the job table plus announceServerUpdates.
import { beforeEach, describe, expect, it } from 'bun:test';
import { obj, type Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';
import { T3Client } from '../../src/client/client';
import { toasts } from '../../src/shell/toast';
import { setJobs, type OutdatedJob } from '../../src/connections/settings-b-outdated';
import {
  announceServerUpdates, isLegacyUpdateHandoffLoss, matchesServerUpdateReadyEvent, matchesServerUpdateResumeEvent, nudgesReconnectDuringUpdateRestart,
  serverUpdateStageLabel, serverUpdateStateFor, serverUpdateStateForProgressEvent, serverUpdateStateForServerVersion, updateEnvironment, updateTargetFromConfig,
  validateServerUpdateReadyEvent, type ServerLifecycleReadyEvent, type ServerUpdateTarget, type UpdateDeps,
} from '../../src/connections/server-update';

const socket = (tag: string) => ({ kind: 'fail' as const, error: { _tag: 'RpcClientError', reason: { _tag: tag } } });
const ready = (serverVersion: string, outcome?: ServerLifecycleReadyEvent['payload']['updateOutcome']): ServerLifecycleReadyEvent =>
  ({ type: 'ready', payload: { environment: { serverVersion }, ...(outcome ? { updateOutcome: outcome } : {}) } });

describe('update restart reconnect nudges', () => {
  it('holds the retry cadence flat through a backoff or a rejected first credential only', () => {
    expect(nudgesReconnectDuringUpdateRestart({ phase: 'backoff' })).toBe(true);
    expect(nudgesReconnectDuringUpdateRestart({ phase: 'blocked', lastFailure: { reason: 'authentication' } })).toBe(true);
    expect(nudgesReconnectDuringUpdateRestart({ phase: 'blocked', lastFailure: { reason: 'permission' } })).toBe(false);
    expect(nudgesReconnectDuringUpdateRestart({ phase: 'connected' })).toBe(false);
  });
});

describe('server state projection', () => {
  it('only treats a legacy transport interruption as an unacknowledged handoff', () => {
    expect(isLegacyUpdateHandoffLoss([{ kind: 'interrupt' }])).toBe(true);
    expect(isLegacyUpdateHandoffLoss([socket('SocketCloseError')])).toBe(true);
    expect(isLegacyUpdateHandoffLoss([socket('SocketOpenError')])).toBe(false);
    expect(isLegacyUpdateHandoffLoss([{ kind: 'fail', error: { _tag: 'RpcClientError', reason: { _tag: 'RpcClientDefect', message: 'incompatible protocol' } } }])).toBe(false);
    expect(isLegacyUpdateHandoffLoss([{ kind: 'fail', error: new Error('Install failed.') }])).toBe(false);
  });

  it('projects streamed update milestones into the shared operation state', () => {
    expect(serverUpdateStateForProgressEvent('0.0.30', '0.0.31', { type: 'progress', stage: 'installing' }))
      .toEqual({ status: 'running', stage: 'installing', fromVersion: '0.0.30', targetVersion: '0.0.31' });
    expect(serverUpdateStateForProgressEvent('0.0.30', '0.0.31', { type: 'complete', result: { targetVersion: '0.0.31', method: 'respawn' } }))
      .toEqual({ status: 'running', stage: 'resuming', fromVersion: '0.0.30', targetVersion: '0.0.31' });
  });

  it('keeps active update state and hides stale failures after a version change', () => {
    const running = { status: 'running' as const, stage: 'resuming' as const, fromVersion: '0.0.30', targetVersion: '0.0.31' };
    const failed = { status: 'failed' as const, stage: 'installing' as const, fromVersion: '0.0.30', targetVersion: '0.0.31', message: 'Install failed.' };
    expect(serverUpdateStateForServerVersion(running, '0.0.31')).toBe(running);
    expect(serverUpdateStateForServerVersion(failed, '0.0.30')).toBe(failed);
    expect(serverUpdateStateForServerVersion(failed, null)).toBe(failed);
    expect(serverUpdateStateForServerVersion(failed, '0.0.31')).toEqual({ status: 'idle' });
  });

  it('correlates launcher outcomes and fails immediately after rollback', () => {
    const result = { targetVersion: '0.0.31', method: 'boot-service', updateId: 'update-1' };
    const outcome = (status: 'committed' | 'rolled-back') => ready(status === 'committed' ? '0.0.31' : '0.0.30',
      { id: 'update-1', targetVersion: '0.0.31', status, ...(status === 'rolled-back' ? { reason: 'prepared-timeout' } : {}) });
    expect(matchesServerUpdateReadyEvent(result, outcome('committed'))).toBe(true);
    expect(validateServerUpdateReadyEvent(result, outcome('committed'))).toBeNull();
    expect(validateServerUpdateReadyEvent(result, outcome('rolled-back'))).toBe('prepared-timeout');
  });

  it('requires tokenless desktop updates to reach the target version', () => {
    expect(matchesServerUpdateResumeEvent({ targetVersion: '0.0.31', method: 'desktop-app' }, ready('0.0.30'))).toBe(false);
    expect(matchesServerUpdateResumeEvent({ targetVersion: '0.0.31', method: 'desktop-app', desktopUpdateToken: 'update-1' }, ready('0.0.30'))).toBe(true);
  });
});

// ── ServerUpdateAction ─────────────────────────────────────────────────────
type Harness = { deps: UpdateDeps; started: Obj[]; copied: string[]; toasted: Obj[]; finish: (() => void) | null };
function harness(reply: Obj = { started: true, attempt: 'a1' }, hold = false): Harness {
  const state: Harness = { started: [], copied: [], toasted: [], finish: null, deps: null as unknown as UpdateDeps };
  state.deps = {
    start: (key, request) => {
      state.started.push({ key, ...request });
      return hold ? new Promise(resolve => { state.finish = () => resolve(reply); }) : Promise.resolve(reply);
    },
    copy: async text => { state.copied.push(text); },
    toast: toast => { state.toasted.push(toast); },
  };
  return state;
}
const target = (patch: Partial<ServerUpdateTarget> = {}): ServerUpdateTarget => ({ environmentKey: 'http://10.0.0.2:3773\nenv-test', environmentId: 'env-test',
  serverLabel: 'Test server', selfUpdate: 'boot-service', targetVersion: '0.0.31', fromVersion: '0.0.30', ...patch });
const job = (patch: Partial<OutdatedJob> = {}): OutdatedJob => ({ status: 'running', stage: 'downloading', fromVersion: '0.0.30', targetVersion: '0.0.31', message: '',
  resultVersion: '', label: 'Test server', mode: 'connected', attempt: 'a1', ...patch });
const key = 'http://10.0.0.2:3773\nenv-test';
function announcer() {
  const calls: Obj[] = [];
  const native: Native = { available: true, watch() {}, later: async input => { calls.push(obj(input)); return { ok: true, generation: 0, value: { jobs: {} } }; } };
  const client = new T3Client();
  Object.assign(client, { environmentId: 'env-test', connection: 'connected' });
  return { native, client, calls };
}

describe('ServerUpdateAction', () => {
  beforeEach(() => setJobs({}));

  it.each([
    [{ kind: 'npm-global', prefix: '/opt/node' } as const, "npm install --global --prefix '/opt/node' t3@0.0.45", 'Update command copied', 'then restart t3'],
    [{ kind: 'npx' } as const, 'npx t3@0.0.45', 'Relaunch command copied', 'This does not update an installed t3 command.'],
    [undefined, 'npx t3@0.0.45', 'Relaunch command copied', 'This does not update an installed t3 command.'],
  ])('copies an honest manual command for %j without invoking remote update', async (installation, command, title, guidance) => {
    const state = harness();
    expect(await updateEnvironment(target({ selfUpdate: null, installation, targetVersion: '0.0.45' }), state.deps)).toBe('copied');
    expect(state.copied).toEqual([command]);
    expect(state.toasted.at(-1)).toMatchObject({ title });
    expect(String(state.toasted.at(-1)!.description)).toContain(guidance);
    expect(state.started).toEqual([]);
  });

  it('reports success only after the shared update flow reconnects', async () => {
    const state = harness(), { native, client } = announcer();
    expect(await updateEnvironment(target(), state.deps)).toBe('started');
    expect(state.started[0]).toMatchObject({ key, mode: 'connected', targetVersion: '0.0.31', extras: {} });
    setJobs({ [key]: job({ status: 'done', resultVersion: '0.0.31' }) });
    let version = '0.0.30';
    await announceServerUpdates(native, client, () => version);
    expect(toasts(client)).toEqual([]);
    expect(serverUpdateStateFor('env-test', '0.0.30')).toMatchObject({ status: 'running', stage: 'resuming' });
    version = '0.0.31';
    await announceServerUpdates(native, client, () => version);
    expect(toasts(client).map(toast => [toast.kind, toast.title, toast.description])).toEqual([['success', 'Test server updated', 'Reconnected on t3@0.0.31.']]);
    expect(serverUpdateStateFor('env-test', '0.0.31')).toEqual({ status: 'idle' });
  });

  it('reports one result when the update action is double-clicked', async () => {
    const state = harness({ started: true, attempt: 'a2' }, true), { native, client } = announcer();
    const first = updateEnvironment(target(), state.deps), second = updateEnvironment(target(), state.deps);
    expect(await second).toBe('busy');
    expect(state.started).toHaveLength(1);
    state.finish!();
    expect(await first).toBe('started');
    setJobs({ [key]: job({ status: 'done', resultVersion: '0.0.31', attempt: 'a2' }) });
    await announceServerUpdates(native, client, () => '0.0.31');
    await announceServerUpdates(native, client, () => '0.0.31');
    expect(toasts(client)).toHaveLength(1);
  });

  it('quietly releases the action when the operation is interrupted', async () => {
    // The native job was already running (or vanished): the click starts nothing and says nothing.
    const state = harness({ started: false });
    expect(await updateEnvironment(target(), state.deps)).toBe('busy');
    expect(state.toasted).toEqual([]);
    expect(await updateEnvironment(target(), harness().deps)).toBe('started');
  });

  it('keeps the manual instruction for desktop servers without remote update support', async () => {
    const state = harness();
    expect(await updateEnvironment(target({ selfUpdate: 'desktop-managed' }), state.deps)).toBe('manual');
    expect(state.started).toEqual([]);
  });

  it('updates remote desktop apps through the shared update flow', async () => {
    const state = harness({ started: true, attempt: 'a3' }), { native, client } = announcer();
    expect(await updateEnvironment(target({ selfUpdate: 'desktop-managed', desktopAppUpdate: true, targetVersion: '0.0.34' }), state.deps)).toBe('started');
    expect(state.started[0]).toMatchObject({ targetVersion: '0.0.34' });
    setJobs({ [key]: job({ status: 'done', resultVersion: '0.0.34', targetVersion: '0.0.34', attempt: 'a3' }) });
    await announceServerUpdates(native, client, () => '0.0.34');
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'success', title: 'Test server updated', description: 'Desktop app relaunched on 0.0.34.' });
  });

  it('asks for the target version alone, whatever the saved thread continuation (ServerUpdateAction at 6d450a4284)', async () => {
    const state = harness();
    await updateEnvironment(updateTargetFromConfig({ environment: { serverVersion: '0.1.4', capabilities: { serverSelfUpdate: 'respawn', serverUpdateThreadContinuation: true } },
      settings: { continueThreadsAfterServerUpdate: true } }, key, 'env', 'Test server'), state.deps);
    expect(state.started[0]!.extras).toEqual({});
  });

  it('toasts a failed attempt once and keeps it as the failed state', async () => {
    const { native, client } = announcer();
    setJobs({ [key]: job({ status: 'failed', stage: 'installing', message: 'Server update failed: The package could not be verified.', attempt: 'a4' }) });
    await announceServerUpdates(native, client, () => '0.0.30');
    await announceServerUpdates(native, client, () => '0.0.30');
    expect(toasts(client).map(toast => [toast.kind, toast.title])).toEqual([['error', 'Test server update failed']]);
    expect(serverUpdateStateFor('env-test', '0.0.30')).toMatchObject({ status: 'failed', stage: 'installing', attempt: 'a4' });
  });
});

describe('ServerUpdateProgress', () => {
  it('shows one calm status row for the restart wait', () => {
    expect(serverUpdateStageLabel('resuming')).toBe('Restarting…');
  });
  it('folds the sub-second installing handoff into the download phase', () => {
    expect(serverUpdateStageLabel('installing')).toBe('Downloading…');
    expect(serverUpdateStageLabel('downloading')).toBe('Downloading…');
  });
  it('keeps the failure visible with its retryable error', () => {
    setJobs({ [key]: job({ status: 'failed', stage: 'installing', message: 'The package could not be verified.' }) });
    expect(serverUpdateStateFor('env-test', '0.0.30')).toMatchObject({ status: 'failed', message: 'The package could not be verified.' });
    setJobs({});
  });
});

it('quiet Update everything hears a fast completion before the start reply returns', async () => {
  const { awaitServerUpdate } = await import('../../src/connections/server-update');
  let returnStart!: (value: Obj) => void;
  const toasts: unknown[] = [], key = 'https://fast-update\nfast-update';
  const native: Native = { available: true, watch() {}, later: async () => ({ ok: true, value: { jobs: {} } }) };
  const target: ServerUpdateTarget = { environmentId: 'fast-update', environmentKey: key, serverLabel: 'Fast', selfUpdate: 'respawn',
    fromVersion: '0.0.40', targetVersion: '0.0.50' };
  const outcome = awaitServerUpdate(target, { start: async () => new Promise<Obj>(resolve => { returnStart = resolve; }), copy: async () => {},
    toast: toast => { toasts.push(toast); } }, { quiet: true });
  setJobs({ [key]: { mode: 'connected', status: 'done', stage: 'resuming', attempt: 'fast-before-reply', label: 'Fast',
    fromVersion: '0.0.40', targetVersion: '0.0.50', resultVersion: '0.0.50', message: '' } });
  await announceServerUpdates(native, null, () => '0.0.50');
  returnStart({ started: true, attempt: 'fast-before-reply' });
  expect(await outcome).toEqual({ status: 'updated' });
  expect(toasts).toEqual([]);
  setJobs({});
});
