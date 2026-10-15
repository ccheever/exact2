// The index landing across environments: _chat.index.tsx IndexDraftLanding over
// sortScopedProjectsForSidebar and allEnvironmentShellsBootstrappedAtom (T3 Code 1e2ecbd975).
import { describe, expect, test } from 'bun:test';
import { T3Client } from './client';
import type { Obj, Shell } from './domain';
import { EnvironmentFleet, type FleetEntry, type FleetPhase } from './settings-b-fleet';
import { welcomeLocal } from './pages-welcome';
import { backgroundShellsBootstrapped, homeTarget, homeWaiting, landHome, openHome, scopedProjectsByActivity, settleHome } from './home-landing';

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
  test('waits while a background environment connects, then picks across every environment', () => {
    const client = landing(), connecting = entry('remote', remoteShell, 'connecting');
    expect(landHome(client, () => 'fallback', fleetOf(connecting))).toBe('');
    expect(client.homePending).toBe(true);
    expect(homeWaiting({ ...client, projectId: '' })).toBe(true);
    // The remote project is the most recent: the focused project stays empty and the move is kept for the refresh.
    expect(landHome(client, () => 'fallback', fleetOf(entry('remote', remoteShell)))).toBe('');
    expect(client).toMatchObject({ homePending: false, homeMove: { environmentId: 'remote', projectId: 'r' } });
    expect(homeWaiting({ ...client, projectId: '' })).toBe(true);
    // After the landing, a project that went away falls back to the focused environment's own rule.
    expect(landHome(client, () => 'fallback', fleetOf(entry('remote', remoteShell)))).toBe('fallback');
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
