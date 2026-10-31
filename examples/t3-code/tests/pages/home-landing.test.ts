// The index landing across environments: _chat.index.tsx IndexDraftLanding over
// sortScopedProjectsForSidebar and allEnvironmentShellsBootstrappedAtom (T3 Code 74541bab17).
import { beforeEach, describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import type { Obj, Shell } from '../../src/client/domain';
import { EnvironmentFleet, fleet, type FleetEntry, type FleetPhase } from '../../src/connections/settings-b-fleet';
import { Backend, storage } from '../support/client-fixture';
import { welcomeLocal } from '../../src/pages/pages-welcome';
import { backgroundShellsBootstrapped, homeTarget, homeWaiting, landHome, localShellPending, openHome, scopedProjectsByActivity, settleHome, settleHomeElsewhere } from '../../src/pages/home-landing';
import { LocalPrimary, primary } from '../../src/connections/local-primary';
import { unknownLocalBackend, type LocalBackendState } from '../../src/connections/local-backend';

// No local backend unless a test makes one (another file may leave the shared primary ready).
beforeEach(() => { primary.update(unknownLocalBackend(), true); });
/** This machine's backend in `state`, its descriptor answered as `environmentId` ('' before it answered). */
const localBackend = (state: LocalBackendState, environmentId = 'local', setting = true) => {
  const local = new LocalPrimary();
  local.update({ ...unknownLocalBackend(), state, httpBaseUrl: 'http://127.0.0.1:16580/', wsBaseUrl: 'ws://127.0.0.1:16580/', environmentId, bearerReady: state === 'ready' }, setting);
  return local;
};

const shell = (projects: Obj[], threads: Obj[] = []): Shell => ({ projects, threads, sequence: 1 });
const entry = (environmentId: string, value: Shell, phase: FleetPhase = 'connected', synchronized = 1, requested = true): FleetEntry => ({
  key: `http://${environmentId}\n${environmentId}`, origin: `http://${environmentId}`, environmentId, phase, message: '', traceId: '', generation: 1, synchronized, lastEvent: 0,
  subscriptions: {}, config: {}, shell: value, scopes: [], error: '', requested });
const fleetOf = (...entries: FleetEntry[]) => { const source = new EnvironmentFleet(); for (const item of entries) source.entries.set(item.key, item); return source; };

const focusedShell = shell([{ id: 'a', title: 'Alpha', updatedAt: '2026-10-01T00:00:00Z' }, { id: 'b', title: 'Beta' }],
  [{ id: 't1', projectId: 'b', latestUserMessageAt: '2026-10-02T00:00:00Z' }]);
const remoteShell = shell([{ id: 'r', title: 'Remote', updatedAt: '2026-09-01T00:00:00Z' }],
  [{ id: 't9', projectId: 'r', latestUserMessageAt: '2026-10-05T00:00:00Z' }, { id: 't8', projectId: 'r', latestUserMessageAt: '2026-10-09T00:00:00Z', archivedAt: '2026-10-09T01:00:00Z' }]);

describe('sortScopedProjectsForSidebar over every environment', () => {
  test('a background environment\'s more recent project leads; archived threads count for nothing', () => {
    const client = { environmentId: 'local', shell: focusedShell };
    expect(scopedProjectsByActivity(client, fleetOf(entry('remote', remoteShell)))).toEqual([
      { environmentId: 'remote', projectId: 'r' }, { environmentId: 'local', projectId: 'b' }, { environmentId: 'local', projectId: 'a' }]);
    // Without the archived thread's later message the remote project is older than Beta's thread.
    const older = shell(remoteShell.projects, [{ id: 't9', projectId: 'r', latestUserMessageAt: '2026-10-01T12:00:00Z' }]);
    expect(homeTarget(client, fleetOf(entry('remote', older)))).toEqual({ environmentId: 'local', projectId: 'b' });
  });
  test('ties fall to the title, then the environment id, then the project id', () => {
    const same = '2026-10-01T00:00:00Z';
    const client = { environmentId: 'm-local', shell: shell([{ id: 'z', title: 'Same', updatedAt: same }]) };
    const source = fleetOf(entry('a-remote', shell([{ id: 'y', title: 'Same', updatedAt: same }, { id: 'x', title: 'Earlier title', updatedAt: same }])));
    expect(scopedProjectsByActivity(client, source).map(target => `${target.environmentId}/${target.projectId}`)).toEqual(['a-remote/x', 'a-remote/y', 'm-local/z']);
  });
  test('an environment that is not synchronized, or is the focused one, is not read', () => {
    const client = { environmentId: 'local', shell: focusedShell };
    expect(homeTarget(client, fleetOf(entry('remote', remoteShell, 'connected', 0)))).toEqual({ environmentId: 'local', projectId: 'b' });
    expect(homeTarget(client, fleetOf(entry('local', remoteShell)))).toEqual({ environmentId: 'local', projectId: 'b' });
    expect(homeTarget({ environmentId: 'local', shell: shell([]) }, fleetOf())).toBeNull();
  });
});

describe('allEnvironmentShellsBootstrappedAtom', () => {
  test('connecting, or connected without its snapshot, waits; disconnected or failed does not', () => {
    const client = { environmentId: 'local' };
    expect(backgroundShellsBootstrapped(client, fleetOf())).toBe(true);
    expect(backgroundShellsBootstrapped(client, fleetOf(entry('remote', remoteShell, 'connecting')))).toBe(false);
    expect(backgroundShellsBootstrapped(client, fleetOf(entry('remote', remoteShell, 'connected', 0)))).toBe(false);
    expect(backgroundShellsBootstrapped(client, fleetOf(entry('remote', remoteShell, 'available', -1, false)))).toBe(false); // not asked to connect yet
    expect(backgroundShellsBootstrapped(client, fleetOf(entry('remote', remoteShell, 'available', -1, true)))).toBe(true);
    expect(backgroundShellsBootstrapped(client, fleetOf(entry('remote', remoteShell, 'error')))).toBe(true);
    expect(backgroundShellsBootstrapped(client, fleetOf(entry('remote', remoteShell, 'reconnecting')))).toBe(true);
    expect(backgroundShellsBootstrapped(client, fleetOf(entry('local', remoteShell, 'connecting')))).toBe(true); // the focus is T3Client's
  });
});

describe('the launch landing', () => {
  const landing = () => ({ environmentId: 'local', shell: focusedShell, threadId: '', homePending: true, homeMove: null as { environmentId: string; projectId: string } | null });
  test('picks the focused project while another environment is still loading', () => {
    const client = landing();
    expect(landHome(client, () => 'fallback', fleetOf(entry('remote', remoteShell, 'connecting')))).toBe('b');
    expect(client).toMatchObject({ homePending: false, homeMove: null });
    expect(homeWaiting({ ...client, projectId: 'b' })).toBe(false);
    // Connected without its snapshot yet: its shell is not in, so it is not waited for either.
    const synchronizing = landing();
    expect(landHome(synchronizing, () => 'fallback', fleetOf(entry('remote', remoteShell, 'connected', 0)))).toBe('b');
  });
  test('an empty workspace waits for every environment, then picks across them', () => {
    const client = { ...landing(), shell: shell([]) }, connecting = entry('remote', remoteShell, 'connecting');
    expect(landHome(client, () => 'fallback', fleetOf(connecting))).toBe('');
    expect(client.homePending).toBe(true);
    expect(homeWaiting({ ...client, projectId: '' })).toBe(true);
    // The remote project is the only one: the focused project stays empty and the move is kept for the refresh.
    expect(landHome(client, () => 'fallback', fleetOf(entry('remote', remoteShell)))).toBe('');
    expect(client).toMatchObject({ homePending: false, homeMove: { environmentId: 'remote', projectId: 'r' } });
    expect(homeWaiting({ ...client, projectId: '' })).toBe(true);
    // Until settleHome moves there, nothing lands in the focused environment.
    expect(landHome(client, () => 'fallback', fleetOf(entry('remote', remoteShell)))).toBe('');
    // After the landing, a project that went away falls back to the focused environment's own rule.
    client.homeMove = null;
    expect(landHome(client, () => 'fallback', fleetOf(entry('remote', remoteShell)))).toBe('fallback');
  });
  test('with no project anywhere, the landing ends once every environment is in', () => {
    const client = { ...landing(), shell: shell([]) };
    expect(landHome(client, () => 'fallback', fleetOf(entry('remote', shell([]), 'connecting')))).toBe('');
    expect(client.homePending).toBe(true);
    expect(landHome(client, () => 'fallback', fleetOf(entry('remote', shell([]))))).toBe('');
    expect(client).toMatchObject({ homePending: false, homeMove: null });
  });
  test('a more recent project in a background environment that is in still leads', () => {
    const client = landing();
    expect(landHome(client, () => 'fallback', fleetOf(entry('remote', remoteShell), entry('slow', remoteShell, 'connecting')))).toBe('');
    expect(client).toMatchObject({ homePending: false, homeMove: { environmentId: 'remote', projectId: 'r' } });
  });
  test('a focused pick lands at once; a thread already open ends the landing', () => {
    const client = landing();
    expect(landHome(client, () => 'fallback', fleetOf())).toBe('b');
    expect(client).toMatchObject({ homePending: false, homeMove: null });
    const opened = { ...landing(), threadId: 't1' };
    expect(landHome(opened, () => 'fallback', fleetOf(entry('remote', remoteShell, 'connecting')))).toBe('fallback');
    expect(opened.homePending).toBe(false);
  });
  test('T3Client.ensureSelection keeps the focused rule when no background environment is saved', () => {
    const client = new T3Client();
    client.environmentId = 'local'; client.shell = focusedShell;
    client.ensureSelection();
    expect(client.projectId).toBe('b');
    expect(client.homePending).toBe(false);
  });

  const nativeLog = (calls: Obj[]) => ({ available: true, watch() {}, later: async (request: unknown) => {
    calls.push(request as Obj);
    return (request as Obj).op === 'connect'
      ? { ok: true, generation: 7, value: { state: 'connecting', origin: 'http://remote', environmentId: 'remote', message: '' } }
      : { ok: true, generation: 0, value: {} };
  } });
  test('settleHome focuses the background environment on its project\'s draft', async () => {
    const client = new T3Client(), calls: Obj[] = [], source = fleetOf(entry('remote', remoteShell));
    client.environmentId = 'local'; client.shell = focusedShell; client.generation = 3;
    client.homePending = false; client.homeMove = { environmentId: 'remote', projectId: 'r' };
    expect(await settleHome(client, nativeLog(calls), source)).toBe(true);
    expect(calls.map(call => call.op)).toEqual(['fleetStop', 'connect']);
    expect(client.local.selections.remote).toEqual({ projectId: 'r', threadId: '' });
    expect(client).toMatchObject({ environmentId: 'remote', projectId: 'r', threadId: '', homeMove: null });
  });
  test('a focused environment still loading does not hold the landing: a background project that is in leads', async () => {
    const client = new T3Client(), calls: Obj[] = [];
    client.environmentId = 'local'; client.connection = 'connecting'; client.generation = 3;
    expect(await settleHomeElsewhere(client, nativeLog(calls), fleetOf(entry('remote', remoteShell, 'connecting')))).toBe(false);
    expect(client).toMatchObject({ homePending: true, homeMove: null });
    expect(await settleHomeElsewhere(client, nativeLog(calls), fleetOf(entry('remote', remoteShell)))).toBe(true);
    expect(calls.map(call => call.op)).toEqual(['fleetStop', 'connect']);
    expect(client).toMatchObject({ environmentId: 'remote', projectId: 'r', homePending: false, homeMove: null });
    // A choice already made, or a landing already done, keeps the window where it is.
    const chosen = new T3Client();
    chosen.environmentId = 'local'; chosen.projectId = 'a';
    expect(await settleHomeElsewhere(chosen, nativeLog([]), fleetOf(entry('remote', remoteShell)))).toBe(false);
  });
  test('waits for This machine while the Local switch is on and it is still coming; never for a slow remote', async () => {
    // Focused on a remote whose project is in: the local backend still starting keeps the landing waiting.
    const remoteFocus = { ...landing(), environmentId: 'remote-a' };
    expect(landHome(remoteFocus, () => 'fallback', fleetOf(), localBackend('starting', ''))).toBe('');
    expect(remoteFocus.homePending).toBe(true);
    expect(landHome(remoteFocus, () => 'fallback', fleetOf(entry('local', shell([]), 'connecting')), localBackend('ready'))).toBe('');
    // Its shell in: the landing picks.
    expect(landHome(remoteFocus, () => 'fallback', fleetOf(entry('local', shell([]))), localBackend('ready'))).toBe('b');
    // Switched off, refused, failed or stopped: nothing to wait for.
    for (const local of [localBackend('starting', 'local', false), localBackend('refused'), localBackend('failed'), localBackend('stopped')]) {
      expect(localShellPending({ environmentId: 'remote-a' }, fleetOf(), local)).toBe(false);
    }
    // A local connection that failed is not waited for; the focused local one waits until it is ready.
    expect(localShellPending({ environmentId: 'remote-a' }, fleetOf(entry('local', shell([]), 'error')), localBackend('ready'))).toBe(false);
    expect(localShellPending({ environmentId: 'local', ready: false }, fleetOf(), localBackend('ready'))).toBe(true);
    expect(localShellPending({ environmentId: 'local', ready: true }, fleetOf(), localBackend('ready'))).toBe(false);
    // The focused local environment still connecting: no jump to a remote project that is in.
    const client = new T3Client(), calls: Obj[] = [];
    client.environmentId = 'local'; client.connection = 'connecting';
    expect(await settleHomeElsewhere(client, nativeLog(calls), fleetOf(entry('remote', remoteShell)), localBackend('ready'))).toBe(false);
    expect(calls).toEqual([]);
    expect(client.homePending).toBe(true);
  });
  test('a choice made before the move keeps it', async () => {
    const client = new T3Client(), calls: Obj[] = [];
    client.environmentId = 'local'; client.shell = focusedShell; client.projectId = 'a';
    client.homePending = false; client.homeMove = { environmentId: 'remote', projectId: 'r' };
    expect(await settleHome(client, nativeLog(calls), fleetOf(entry('remote', remoteShell)))).toBe(false);
    expect(calls).toEqual([]);
    expect(client).toMatchObject({ environmentId: 'local', projectId: 'a', homeMove: null });
  });
  test('the index route mid-session (no thread left, a #fragment link) opens the most recent project of every environment', async () => {
    const client = new T3Client(), calls: Obj[] = [];
    client.environmentId = 'local'; client.shell = focusedShell; client.projectId = 'a'; client.threadId = 't1';
    await openHome(client, nativeLog(calls), focusedShell, fleetOf(entry('remote', remoteShell)));
    expect(client).toMatchObject({ environmentId: 'remote', projectId: 'r', threadId: '' });
    const local = new T3Client();
    local.environmentId = 'local'; local.shell = focusedShell; local.projectId = 'a'; local.threadId = 't1';
    await openHome(local, nativeLog([]), focusedShell, fleetOf());
    expect(local).toMatchObject({ environmentId: 'local', projectId: 'b', threadId: '' });
  });
});

describe('a relaunch focused on an environment whose project is not the most recent', () => {
  // A relaunch reconnects the environment of the last focus (relaunch-reconnect.ts); the landing's pick may be another one's.
  test('the shell events of the same refresh keep the pick, and settleHome moves to that environment\'s draft', async () => {
    const native = new Backend(), disk = storage(), client = new T3Client();
    // The Backend refuses `environments`, so the refresh's fleet sync keeps exactly this entry.
    const others = new Map(fleet.entries), remote = entry('remote', remoteShell);
    fleet.entries.clear(); fleet.entries.set(remote.key, remote);
    try {
      // The focused shell's completion marker is drained after the synchronize's landing picked, before settleHome.
      await client.refresh(native, disk.files);
      // The move: the focused session connects to the remote environment with its project's draft as the selection.
      // (The Backend answers that connect with its own environment's status, so the client itself stays.)
      expect(native.calls.filter(call => call.op === 'connect').map(call => call.origin)).toEqual(['http://remote']);
      expect(client.local.selections.remote).toEqual({ projectId: 'r', threadId: '' });
      expect(client).toMatchObject({ projectId: '', threadId: '', homePending: false, homeMove: null });
    } finally { fleet.entries.clear(); for (const [key, value] of others) fleet.entries.set(key, value); }
  });
  test('a focus change before the move drops it', () => {
    const client = new T3Client();
    client.environmentId = 'local'; client.generation = 1; client.homePending = false;
    client.homeMove = { environmentId: 'remote', projectId: 'r' };
    client.adoptStatus({ state: 'connected', origin: 'http://other', environmentId: 'other', message: '' }, 2);
    expect(client).toMatchObject({ environmentId: 'other', homeMove: null });
  });
});

describe('the welcome wizard\'s end (WelcomeRouteView.onDone: navigate({ to: "/" }))', () => {
  const quiet = { available: true, watch() {}, later: async () => ({ ok: true, generation: 1, value: {} }) } as never;
  test('with no project picked, the index landing runs again and reaches an environment paired in the wizard', async () => {
    // The launch's landing ran before the pairing: no environment held a project, so it picked nothing.
    const client = new T3Client();
    client.environmentId = 'local'; client.shell = shell([]);
    expect(landHome(client, () => '', fleetOf())).toBe('');
    expect(client.homePending).toBe(false);
    await welcomeLocal(client, quiet, 'finish', '', String(Date.parse('2026-10-15T10:00:00Z')));
    expect(client).toMatchObject({ homePending: true, projectId: '' });
    expect(landHome(client, () => '', fleetOf(entry('remote', remoteShell)))).toBe('');
    expect(client.homeMove).toEqual({ environmentId: 'remote', projectId: 'r' });
  });
  test('a thread already open stays', async () => {
    const client = new T3Client();
    client.environmentId = 'local'; client.shell = focusedShell; client.threadId = 't1'; client.projectId = 'b'; client.homePending = false;
    await welcomeLocal(client, quiet, 'finish', '', String(Date.parse('2026-10-15T10:00:00Z')));
    expect(client).toMatchObject({ homePending: false, threadId: 't1', projectId: 'b' });
  });
});
