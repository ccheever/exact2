// Lane r8-pointer: real-input pointer/state defects D9, D14, D15, D16 and the
// ⌘] minor, at the logic level (the AppKit halves are macos/tests/sidebar and
// macos/tests/r8-pointer).
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { setRuntimeClock } from './sidebar-state';
import { T3Client } from './client';
import { epochNow } from './r8-pointer-clock';
import { launchFocus, launchFocusKey, reconnectOnLaunch, relaunchTarget } from './r8-pointer-reconnect';
import { settingsForward } from './r8-pointer-forward';
import { connectionsProjection } from './connections';
import { keyboardDispatch } from './keyboard-dispatch';
import { timelineMessages } from './timeline-presentation';
import { welcomeView } from './pages-welcome';
import { pagesPrefs } from './pages-prefs';
import { obj, type Message, type Obj } from './domain';
import type { Native } from './protocol';

// The data runtime has no clock (sidebar-state.ts `clock`); these tests stand in for a host clock that reads Date.now.
beforeEach(() => setRuntimeClock(() => Date.now()));
afterEach(() => setRuntimeClock(() => Number.NaN));

class Fake implements Native {
  available = true; calls: Obj[] = [];
  constructor(private saved: Obj[] = []) {}
  watch() {}
  async later(input: unknown): Promise<unknown> {
    const request = obj(input); this.calls.push(request);
    if (request.op === 'environments') return { ok: true, generation: 0, value: { saved: this.saved } };
    return { ok: true, generation: 0, value: {} };
  }
}

describe('D16: completion time is a real epoch', () => {
  test('a window time counted from launch (exactTime unresolved) falls back to the runtime clock', () => {
    const before = Date.now();
    expect(epochNow(118_000)).toBeGreaterThanOrEqual(before);
    expect(epochNow(0)).toBeGreaterThanOrEqual(before);
    expect(epochNow(Date.parse('2026-10-04T00:00:00.000Z'))).toBe(Date.parse('2026-10-04T00:00:00.000Z'));
  });
  test('the welcome decision persists onboardingCompletedAt now, not at 1970-01-01T00:01:58Z', async () => {
    // A hydrated client that already has an environment records setup as done on its first decision.
    const client = { local: {}, origin: '', environmentId: '', connection: 'disconnected', statusMessage: '', scopes: [], config: {}, shell: { projects: [], threads: [], sequence: 0 } } as unknown as T3Client;
    await welcomeView(client, new Fake([{ origin: 'http://127.0.0.1:1', environmentId: 'e' }]), { step: 'connect', now: 118_000 });
    const saved = Date.parse(pagesPrefs(client).onboardingCompletedAt);
    expect(saved).toBeGreaterThan(Date.parse('2026-01-01T00:00:00.000Z'));
    expect(Math.abs(saved - Date.now())).toBeLessThan(60_000);
  });
});

describe('D14: a relaunch reconnects to the saved environment', () => {
  const loop = { origin: 'http://127.0.0.1:14922', environmentId: 'env-local', label: 'Fixture', enabled: true };
  const remote = { origin: 'https://box.example.com', environmentId: 'env-box', label: 'Box', enabled: true };
  test('the last origin wins, then this machine, then the first saved; switched-off ones never', () => {
    expect(relaunchTarget([remote, loop], 'https://box.example.com/')).toBe(remote);
    expect(relaunchTarget([remote, loop], '')).toBe(loop);
    expect(relaunchTarget([{ ...loop, enabled: false }, remote], '')).toBe(remote);
    expect(relaunchTarget([{ ...loop, enabled: false }], 'http://127.0.0.1:14922')).toBeNull();
    expect(relaunchTarget([], '')).toBeNull();
  });
  test('the first disconnected status connects once, with Keychain’s credential; the fleet leaves it alone', async () => {
    const client = { origin: 'http://127.0.0.1:14922', environmentId: '', connection: 'disconnected' };
    const native = new Fake([remote, loop]);
    expect(await reconnectOnLaunch(client, native, { state: 'disconnected', origin: 'http://127.0.0.1:14922', environmentId: '' })).toBe(true);
    expect(native.calls.filter(call => call.op === 'connect')).toEqual([{ op: 'connect', origin: 'http://127.0.0.1:14922', credential: '' }]);
    // While the socket opens, the fleet's focus is that environment (no second, background transport).
    client.connection = 'connecting';
    expect(launchFocusKey(client)).toBe('http://127.0.0.1:14922\nenv-local');
    // Once the server names it, the client is the focus again.
    client.environmentId = 'env-local'; client.connection = 'connected';
    expect(launchFocus(client)).toBe(client);
    // Never again in this process: a later Disconnect stays disconnected.
    expect(await reconnectOnLaunch(client, native, { state: 'disconnected', origin: 'http://127.0.0.1:14922', environmentId: '' })).toBe(false);
    expect(native.calls.filter(call => call.op === 'connect')).toHaveLength(1);
  });
  test('an already connected or never-paired launch asks nothing', async () => {
    const connected = new Fake([loop]);
    expect(await reconnectOnLaunch({}, connected, { state: 'connected', origin: loop.origin, environmentId: 'env-local' })).toBe(false);
    expect(connected.calls).toEqual([]);
    const empty = new Fake([]);
    expect(await reconnectOnLaunch({}, empty, { state: 'disconnected', origin: '', environmentId: '' })).toBe(false);
    expect(empty.calls.map(call => call.op)).toEqual(['environments']);
  });
  test('Connections lists the paired loopback environment as a saved row under Environments (r9-connect), with its Remove (Forget) menu', () => {
    const host = { connection: 'connected', origin: loop.origin, environmentId: 'env-local', statusMessage: '', scopes: [], config: { environment: { label: 'Fixture' } } };
    const page = connectionsProjection(host, [loop, remote]);
    expect(page.environments.map(row => row.label)).toEqual(['Fixture', 'Box']);
    expect(page.environments[0]).toMatchObject({ label: 'Fixture', origin: loop.origin, environmentId: 'env-local', active: true, subtitle: 'http://127.0.0.1:14922/ · Connected' });
    expect(connectionsProjection({ ...host, connection: 'disconnected', environmentId: '' }, [remote]).environments.map(row => row.label)).toEqual(['Box']);
  });
});

describe('D9: an answer and its attached changed files share one hover', () => {
  const at = '2026-10-03T13:13:00.000Z';
  const row = (id: string, kind: string, extra: Partial<Message> = {}): Message => ({ id, kind, title: kind, body: '', createdAt: at, runId: 'r1', sourceThreadId: 't1', completed: true, ...extra });
  test('the attached card takes the answer’s hover key; other rows keep their own', () => {
    const client = { threadId: 't1', projection: { checkpoints: [{ id: 'cp', appRunOrdinal: 1 }] }, local: { deviceSettings: { timestampFormat: '12-hour' } } } as unknown as T3Client;
    const rows = timelineMessages(client, [row('u', 'user'), row('a', 'assistant'), row('c', 'checkpoint', { checkpointId: 'cp', files: [{ path: 'x.md', additions: 1, deletions: 0 }] })], Date.parse(at));
    expect(rows.map(message => [message.id, message.hoverKey, message.actionsId])).toEqual([['u', 'u', ''], ['a', 'a', ''], ['c', 'a', 'a']]);
    expect(rows[2]!.actionsTimeTip).toMatch(/^\d{1,2}:\d{2}\s?[AP]M, 3rd October 2026$/);
  });
});

describe('⌘] returns to Settings after ⌘[ left it', () => {
  const context = { composerFocus: false, editableFocus: false, turnRunning: false, modelPickerOpen: false, draftThreadRoute: false, modalOpen: false, settingsOpen: false, diffOpen: false };
  function client() {
    const c = new T3Client();
    c.config = { keybindings: [{ command: 'navigation.back', shortcut: { key: '[', modKey: true } }, { command: 'navigation.forward', shortcut: { key: ']', modKey: true } }], settings: {} };
    c.shell = { sequence: 1, projects: [{ id: 'p1', title: 'P' }], threads: [{ id: 't1', projectId: 'p1', title: 'One' }, { id: 't2', projectId: 'p1', title: 'Two' }] } as never;
    c.projectId = 'p1'; c.threadId = 't1';
    return c;
  }
  test('Forward reopens the route Settings was left on, until another location is visited', () => {
    const c = client();
    const forward = () => keyboardDispatch(c, [], '', '', context).find(item => item.command === 'navigation.forward');
    expect(forward()).toBeUndefined();
    keyboardDispatch(c, [], '', '', { ...context, modalOpen: true, settingsOpen: true, settingsRoute: 'connections' });
    expect(forward()).toMatchObject({ kind: 'palette-run', target: 'settings', extra: 'connections', chord: 'Meta+]' });
    c.threadId = 't2';
    expect(forward()).toBeUndefined();
    expect(settingsForward({ threadId: 't2', projectId: 'p1' }, false)).toBe('');
  });
});
