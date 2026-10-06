import { test, expect, describe, beforeEach } from 'bun:test';
import { resolveRemoteOpenState, shouldShowOpenInPicker, canUseMarkdownFileShellActions, parseSafeExternalUrl, remoteEditorsFrom, remoteCapableEditors,
  resetRemoteEditorsForTests, openInView, openInEditorHere, rememberSshAlias, remoteOpenFor, openFavoriteEnabled } from './remote-open';
import { buildRemoteOpenUrl, REMOTE_CAPABLE_EDITOR_IDS } from './editors';
import { obj, type Obj } from './domain';
import type { Native } from './protocol';
import type { T3Client } from './client';

const TAILSCALE_TARGETS = [{ kind: 'tailscale', host: 'sol.tail1234.ts.net' }, { kind: 'mdns', host: 'sol.local' }];
const primary = (httpBaseUrl: string) => ({ kind: 'primary' as const, httpBaseUrl });

// apps/web/src/remoteOpen.test.ts
describe('resolveRemoteOpenState', () => {
  test('keeps exec behavior for a loopback primary target', () => {
    expect(resolveRemoteOpenState({ target: primary('http://127.0.0.1:8000'), sshAlias: null, isDesktopRenderer: false, remoteOpenTargets: TAILSCALE_TARGETS })).toEqual({ mode: 'local-exec' });
  });
  test('uses deep links for a primary target reached over the network', () => {
    expect(resolveRemoteOpenState({ target: primary('https://sol.tail1234.ts.net'), sshAlias: null, isDesktopRenderer: false, remoteOpenTargets: TAILSCALE_TARGETS }))
      .toEqual({ mode: 'remote-links', host: { kind: 'tailscale', host: 'sol.tail1234.ts.net' } });
  });
  test("keeps exec behavior for the desktop app's own primary even on a NAT URL", () => {
    expect(resolveRemoteOpenState({ target: primary('http://172.29.112.1:14369'), sshAlias: null, isDesktopRenderer: true, remoteOpenTargets: TAILSCALE_TARGETS })).toEqual({ mode: 'local-exec' });
  });
  test('keeps exec behavior for desktop-local secondary backends', () => {
    expect(resolveRemoteOpenState({ target: { kind: 'desktop-local' }, sshAlias: null, isDesktopRenderer: false, remoteOpenTargets: TAILSCALE_TARGETS })).toEqual({ mode: 'local-exec' });
  });
  test('prefers the desktop SSH alias over server-advertised hosts', () => {
    expect(resolveRemoteOpenState({ target: { kind: 'ssh' }, sshAlias: 'sol', isDesktopRenderer: true, remoteOpenTargets: TAILSCALE_TARGETS })).toEqual({ mode: 'remote-links', host: { kind: 'ssh-alias', host: 'sol' } });
  });
  test('reports unavailable when a remote environment advertises no hosts', () => {
    for (const remoteOpenTargets of [[], undefined]) {
      expect(resolveRemoteOpenState({ target: { kind: 'relay' }, sshAlias: null, isDesktopRenderer: false, remoteOpenTargets })).toEqual({ mode: 'remote-unavailable' });
    }
  });
  test('falls back to exec when the environment has no catalog entry', () => {
    expect(resolveRemoteOpenState({ target: null, sshAlias: null, isDesktopRenderer: false, remoteOpenTargets: undefined })).toEqual({ mode: 'local-exec' });
  });
});

describe('buildRemoteOpenUrl', () => {
  test('builds a vscode-remote deep link', () => {
    expect(buildRemoteOpenUrl({ editor: 'vscode', host: 'sol.tail1234.ts.net', absolutePath: '/home/theo/code/my repo' })).toBe('vscode://vscode-remote/ssh-remote+sol.tail1234.ts.net/home/theo/code/my%20repo');
  });
  test("uses the fork's scheme", () => {
    expect(buildRemoteOpenUrl({ editor: 'cursor', host: 'sol', absolutePath: '/tmp/x' })).toBe('cursor://vscode-remote/ssh-remote+sol/tmp/x');
  });
  test('roots Windows paths', () => {
    expect(buildRemoteOpenUrl({ editor: 'vscode', host: 'sol', absolutePath: 'C:\\Users\\theo' })).toBe('vscode://vscode-remote/ssh-remote+sol/C%3A/Users/theo');
  });
  test("builds Zed's ssh deep link", () => {
    expect(buildRemoteOpenUrl({ editor: 'zed', host: 'sol.tail1234.ts.net', absolutePath: '/home/theo/code/my repo' })).toBe('zed://ssh/sol.tail1234.ts.net/home/theo/code/my%20repo');
  });
  test('drops the Windows drive letter for Zed', () => {
    expect(buildRemoteOpenUrl({ editor: 'zed', host: 'sol', absolutePath: 'C:\\Users\\theo' })).toBe('zed://ssh/sol/Users/theo');
    expect(buildRemoteOpenUrl({ editor: 'zed', host: 'sol', absolutePath: '/C:/project' })).toBe('zed://ssh/sol/C%3A/project');
  });
  test('returns undefined for editors without remote support', () => {
    expect(buildRemoteOpenUrl({ editor: 'idea', host: 'sol', absolutePath: '/tmp/x' })).toBe(undefined);
  });
});

// apps/desktop/src/electron/ElectronShell.test.ts (the URL gate; T3RemoteEditors.swift repeats it before NSWorkspace)
describe('parseSafeExternalUrl', () => {
  test('opens safe external URLs', () => { expect(parseSafeExternalUrl('https://example.com/path')).toBe('https://example.com/path'); });
  test('opens remote SSH editor URLs', () => {
    expect(parseSafeExternalUrl('vscode://vscode-remote/ssh-remote+example.com/home/user/project')).toBe('vscode://vscode-remote/ssh-remote+example.com/home/user/project');
  });
  test("opens Zed's ssh deep link", () => {
    expect([parseSafeExternalUrl('zed://ssh/example.com/home/user/project'), parseSafeExternalUrl('zed://ssh/example.com/')]).toEqual(['zed://ssh/example.com/home/user/project', 'zed://ssh/example.com/']);
  });
  test('does not open editor URLs that mix up link shapes', () => {
    expect([parseSafeExternalUrl('zed://extension/attacker'), parseSafeExternalUrl('vscode://ssh/example.com/home/user/project')]).toEqual([null, null]);
  });
  test('does not open remote editor URLs with userinfo', () => {
    expect(['vscode://user@vscode-remote/ssh-remote+example.com/home/user/project', 'vscode://:secret@vscode-remote/ssh-remote+example.com/home/user/project', 'zed://ssh/user@example.com/home/user/project'].map(parseSafeExternalUrl)).toEqual([null, null, null]);
  });
  test('does not open unsafe external URLs', () => { expect(parseSafeExternalUrl('file:///etc/passwd')).toBe(null); });
  test('does not open non-remote editor URLs', () => { expect(parseSafeExternalUrl('vscode://ms-python.python/some-command?argument=attacker')).toBe(null); });
  test('returns false when Electron rejects openExternal', async () => {
    // The module answers opened=false (NSWorkspace refused); the open does not count as the first remote open.
    const { native, calls } = fakeNative(request => request.op === 'remoteEditorsOpen' ? { opened: false } : request.op === 'sshHosts' ? { targets: { 'http://127.0.0.1:41001': { alias: 'devbox' } } } : {});
    resetRemoteEditorsForTests();
    expect(await openInEditorHere(client('http://127.0.0.1:41001'), native, '/srv/app', 'vscode')).toBe(false);
    expect(calls.some(call => call.op === 'remoteEditorsHint' && call.seen === true)).toBe(false);
  });
});

function fakeNative(handler: (request: Obj) => Obj) {
  const calls: Obj[] = [];
  const native: Native = { available: true, watch() {}, later: async (input: unknown) => { const request = obj(input); calls.push(request); return { ok: true, generation: 0, value: handler(request) }; } };
  return { native, calls };
}
const rest: Obj[] = [];
function client(origin: string, config: Obj = {}): T3Client {
  return { origin, environmentId: 'env-1', config: { environment: { label: 'devbox' }, availableEditors: ['cursor', 'idea'], ...config }, restAccess: () => ({ request: async (method: string, body: Obj) => { rest.push({ method, ...body }); return {}; } }) } as unknown as T3Client;
}

describe('picker visibility and editors', () => {
  beforeEach(() => { resetRemoteEditorsForTests(); rest.length = 0; });
  test('shouldShowOpenInPicker: the primary always, other environments only outside local-exec', () => {
    const base = { activeProjectName: 'app', activeThreadEnvironmentId: 'b', primaryEnvironmentId: 'a' };
    expect(shouldShowOpenInPicker({ ...base, activeThreadEnvironmentId: 'a', remoteOpenMode: 'local-exec' })).toBe(true);
    expect(shouldShowOpenInPicker({ ...base, remoteOpenMode: 'local-exec' })).toBe(false);
    expect(shouldShowOpenInPicker({ ...base, remoteOpenMode: 'remote-links' })).toBe(true);
    expect(shouldShowOpenInPicker({ ...base, remoteOpenMode: 'remote-unavailable' })).toBe(true);
    expect(shouldShowOpenInPicker({ ...base, activeProjectName: undefined, remoteOpenMode: 'remote-links' })).toBe(false);
  });
  test('Markdown file links get editor actions only in local-exec once resolved', () => {
    expect(canUseMarkdownFileShellActions('a', 'local-exec', true)).toBe(true);
    expect(canUseMarkdownFileShellActions('a', 'local-exec', false)).toBe(false);
    expect(canUseMarkdownFileShellActions('a', 'remote-links', true)).toBe(false);
    expect(canUseMarkdownFileShellActions(null, 'local-exec', true)).toBe(false);
  });
  test('lists only remote-capable editors that resolve', async () => {
    expect(REMOTE_CAPABLE_EDITOR_IDS).toEqual(['cursor', 'vscode', 'vscode-insiders', 'vscodium', 'zed']);
    expect(remoteEditorsFrom(['zed', 'idea', 'cursor', 7])).toEqual(['zed', 'cursor']);
    const { native, calls } = fakeNative(() => ({ editors: ['cursor', 'zed'] }));
    expect(await remoteCapableEditors(native)).toEqual(['cursor', 'zed']);
    expect(await remoteCapableEditors(native)).toEqual(['cursor', 'zed']);
    expect(calls.length).toBe(1);
  });
  test('falls back to VS Code when none resolve', async () => {
    expect(remoteEditorsFrom([])).toEqual(['vscode']);
    expect(remoteEditorsFrom(['idea'])).toEqual(['vscode']);
    expect(await remoteCapableEditors(fakeNative(() => ({ editors: [] })).native)).toEqual(['vscode']);
  });
  test('an SSH environment opens over its alias; the hint shows until the first accepted open', async () => {
    const { native, calls } = fakeNative(request => request.op === 'sshHosts' ? { targets: { 'http://127.0.0.1:41001': { alias: 'devbox', hostname: 'devbox' } } }
      : request.op === 'remoteEditorsProbe' ? { editors: ['vscode', 'zed'] } : request.op === 'remoteEditorsOpen' ? { opened: true } : request.op === 'remoteEditorsHint' ? { seen: request.seen === true } : {});
    const ssh = client('http://127.0.0.1:41001/');
    const view = await openInView(ssh, native, ['cursor', 'idea'], 'app');
    expect(view).toMatchObject({ mode: 'remote-links', show: true, editors: ['vscode', 'zed'], hint: 'Opens over SSH. Needs your key on devbox', unavailable: '', empty: false });
    expect(await openInEditorHere(ssh, native, '/home/tester/my app', 'vscode')).toBe(true);
    expect(calls.find(call => call.op === 'remoteEditorsOpen')?.url).toBe('vscode://vscode-remote/ssh-remote+devbox/home/tester/my%20app');
    expect(rest).toEqual([]);
    expect((await openInView(ssh, native, [], 'app')).hint).toBe('');
    expect(openFavoriteEnabled(ssh, 'app')).toBe(true);
  });
  test('a loopback primary runs the server editor; a remote one with no route opens nothing', async () => {
    const { native, calls } = fakeNative(request => request.op === 'sshHosts' ? { targets: {} } : {});
    const local = client('http://127.0.0.1:3774');
    expect(await openInView(local, native, ['cursor', 'idea'], 'app')).toMatchObject({ mode: 'local-exec', show: true, editors: ['cursor', 'idea'], hint: '' });
    expect(await openInEditorHere(local, native, '/w', 'cursor')).toBe(true);
    expect(rest).toEqual([{ method: 'shell.openInEditor', cwd: '/w', editor: 'cursor' }]);
    const far = client('https://far.example.com');
    expect(await openInView(far, native, ['cursor'], 'app')).toMatchObject({ mode: 'remote-unavailable', show: true, unavailable: 'No SSH route to devbox' });
    expect(await openInEditorHere(far, native, '/w', 'vscode')).toBe(false);
    expect(calls.some(call => call.op === 'remoteEditorsOpen')).toBe(false);
    const advertised = client('https://far.example.com', { remoteOpenTargets: TAILSCALE_TARGETS });
    expect(remoteOpenFor(advertised).state).toEqual({ mode: 'remote-links', host: { kind: 'tailscale', host: 'sol.tail1234.ts.net' } });
    rememberSshAlias('https://far.example.com', 'farbox');
    expect(remoteOpenFor(advertised).state).toEqual({ mode: 'remote-links', host: { kind: 'ssh-alias', host: 'farbox' } });
    expect(openFavoriteEnabled(local, '')).toBe(false);
  });
});
