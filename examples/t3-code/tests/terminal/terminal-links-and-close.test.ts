import { AUTH_STANDARD_CLIENT_SCOPES } from '../../src/connections/remote-scopes';
// T3 Code 1e2ecbd975 (MIT; see LICENSE-T3):
// - the welcome wizard's setup terminal is the drawer's TerminalViewport (WelcomeWizard.tsx AgentInstallTerminal), so its
//   links take the drawer's handleLinkActivate: an http(s) click or ⌘-click opens the system browser once, a path the
//   preferred editor once, resolved against the setup's cwd.
// - "Open links in" = T3 Code set at runtime reaches a drawer link: a Browser tab (openTerminalLinkInPreview), and an
//   in-app open that fails is kept as the drawer's failure (the reference logs TerminalLinkPreviewOpenError) before the
//   system-browser fallback.
// - while a terminal close confirm is open, ChatView's window key handler refuses the terminal close chord
//   (isTerminalCloseConfirmPending && preventTerminalCloseShortcut): neither rightPanel.close nor the same close again.
// An oversized write's refusal text and CRLF system lines are Swift (macos/tests/terminal, drawer.swift).
import { describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { initialShell, obj, type Obj } from '../../src/client/domain';
import type { Files, Native } from '../../src/client/protocol';
import { keyboardDispatch, keyboardDispatchSource, TERMINAL_CLOSE_CONFIRMS, type DispatchContext } from '../../src/shell/keyboard-dispatch';
import { keybindingDefaults } from '../../src/settings/keybinding-settings';
import { parseWhen } from '../../src/settings/keybinding-view';
import { recordTerminalFocus } from '../../src/terminal/terminal-focus';
import { terminalUiStore } from '../../src/terminal/terminal-ui-state';
import { restCommand } from '../../src/settings/settings-rest-commands';
import { terminalDrawerView, terminalOps } from '../../src/terminal/terminal-drawer-view';
import { terminalLinkAction } from '../../src/terminal/terminal-integrations';
import { welcomeLocal, welcomeView } from '../../src/pages/pages-welcome';
import { noPrimary, resetPrimary } from '../support/local-primary-fixture';
import { environmentKey, fleet, type FleetEntry } from '../../src/connections/settings-b-fleet';

const storage: Files = { fs: { async mkdir() {}, async atomicWriteFile() {}, async readFile() { return new ArrayBuffer(0); } } };
const out = () => ({ message: '', id: '', value: '' });

// ── The welcome wizard's setup terminal ───────────────────────────────────────────────────────────
describe('The welcome wizard setup terminal\'s links are the drawer\'s', () => {
  /** A focused environment with Cursor available; its native module records the ops a link sends. */
  function wizardClient() {
    const rpcs: { method: string; payload: Obj }[] = [], ops: Obj[] = [];
    const client = {
      local: { deviceSettings: { appearanceMode: 'system' } }, origin: 'http://127.0.0.1:16402', environmentId: 'setup-env', connection: 'connected', statusMessage: '', scopes: ['terminal:operate'],
      config: { cwd: '/Users/lane/work', availableEditors: ['cursor'], environment: { platform: { os: 'darwin' } }, settings: {}, providers: [{ driver: 'claudeAgent', instanceId: 'claudeAgent', installed: false, status: 'error', auth: { status: 'unknown' } }] },
      shell: { projects: [] as Obj[], threads: [] as Obj[], sequence: 0 }, threadId: '', projectId: '', ids: async () => ['fixture-id'],
      rpc: async (_native: Native, method: string, payload: Obj) => { rpcs.push({ method, payload }); return {}; },
      call: async (_native: Native, request: Obj) => { ops.push(request); return { opened: true }; },
      restAccess: () => ({ request: async (method: string, payload: Obj) => { rpcs.push({ method, payload }); return {}; } }),
    } as unknown as T3Client;
    const native = { available: true, watch: () => {}, later: async (request: Obj) => {
      if (request.op === 'environments') return { ok: true, value: { saved: [] }, generation: 1 };
      return { ok: true, value: {}, generation: 1 };
    } } as unknown as Native;
    return { client, native, rpcs, ops };
  }
  const link = (terminalId: string, text: string, metaKey = false) =>
    JSON.stringify({ type: 'link', text, metaKey, ctrlKey: false, shiftKey: false, altKey: false, environmentId: 'setup-env', threadId: 'onboarding-agent-setup', terminalId, surface: 'drawer' });

  test('an http link click and ⌘-click open the system browser once each; a path opens the editor once at the setup cwd (`<cwd>/./README.md:1`, as the reference\'s `cursor --goto`)', async () => {
    noPrimary();
    try {
      const { client, native, rpcs, ops } = wizardClient();
      const connect = await welcomeView(client, native, { step: 'connect', now: Date.now() });
      await welcomeLocal(client, native, 'setup', '', '');
      const agents = await welcomeView(client, native, { step: 'agents', now: 0 });
      const card = agents.machines[0]!.agents.find(agent => agent.driver === 'claudeAgent')!;
      await welcomeLocal(client, native, 'terminal-open', connect.computers[0]!.key, card.key);
      const terminalId = (await welcomeView(client, native, { step: 'agents', now: 0 })).machines[0]!.terminal.terminalId;
      expect(terminalId).toBe('onboarding-claudeAgent-fixture-id');
      expect(rpcs.map(call => call.method)).toEqual(['terminal.open', 'terminal.write']); // the command is typed, never run
      rpcs.length = 0;

      await welcomeLocal(client, native, 'terminal-exited', 'setup-env', link(terminalId, 'https://claude.ai/install.sh'));
      await welcomeLocal(client, native, 'terminal-exited', 'setup-env', link(terminalId, 'https://claude.ai/install.sh', true));
      expect(ops.filter(op => op.op === 'terminalOpenExternal').map(op => op.url)).toEqual(['https://claude.ai/install.sh', 'https://claude.ai/install.sh']);
      await welcomeLocal(client, native, 'terminal-exited', 'setup-env', link(terminalId, './README.md:1'));
      expect(rpcs).toEqual([{ method: 'shell.openInEditor', payload: { cwd: '/Users/lane/work/./README.md:1', editor: 'cursor' } }]);
      // Another session's link (a terminal the wizard replaced) is not this setup's.
      await welcomeLocal(client, native, 'terminal-exited', 'setup-env', link('old-session', 'https://example.com/'));
      expect(ops.filter(op => op.op === 'terminalOpenExternal')).toHaveLength(2);
      // The exit report still closes the setup terminal.
      await welcomeLocal(client, native, 'terminal-exited', 'setup-env', JSON.stringify({ type: 'exited', terminalId }));
      expect(rpcs.at(-1)).toEqual({ method: 'terminal.close', payload: { threadId: 'onboarding-agent-setup', terminalId, deleteHistory: true } });
    } finally { resetPrimary(); }
  });

  test('the drawer turns the same messages into the same actions', async () => {
    const { client, native, rpcs, ops } = wizardClient();
    for (const message of [{ text: 'https://claude.ai/install.sh' }, { text: 'https://claude.ai/install.sh', metaKey: true }, { text: './README.md:1', cwd: '/Users/lane/work' }])
      await terminalLinkAction(client, native, { ...message, environmentId: 'setup-env', threadId: 'thread-1', terminalId: 'term-1' });
    expect(ops.filter(op => op.op === 'terminalOpenExternal').map(op => op.url)).toEqual(['https://claude.ai/install.sh', 'https://claude.ai/install.sh']);
    expect(rpcs).toEqual([{ method: 'shell.openInEditor', payload: { cwd: '/Users/lane/work/./README.md:1', editor: 'cursor' } }]);
  });

  test('a setup terminal on another environment opens a path with that environment\'s editors and server', async () => {
    noPrimary();
    const origin = 'http://127.0.0.1:16403', key = environmentKey(origin, 'other-env');
    const saved = fleet.saved, previous = fleet.entries.get(key);
    const { client, native: focusedNative, rpcs } = wizardClient();
    const remote: Obj[] = [];
    const entry: FleetEntry = { key, origin, environmentId: 'other-env', phase: 'connected', message: '', traceId: '', generation: 7, synchronized: 7,
      lastEvent: 0, subscriptions: {}, config: { ...client.config, cwd: '/remote/work', availableEditors: ['zed'] }, shell: initialShell(), scopes: [], error: '', requested: true };
    const native: Native = { available: true, watch() {}, async later(request) {
      const call = obj(request);
      if (call.op === 'environments') return { ok: true, value: { saved: fleet.saved }, generation: 1 };
      if (call.fleet === key) { remote.push(call); return { ok: true, value: {}, generation: entry.generation }; }
      return focusedNative.later(request);
    } };
    try {
      fleet.saved = [];
      // Enter the wizard before pairing the second environment, as first-run setup does.
      expect((await welcomeView(client, native, { step: 'connect', now: Date.now() })).show).toBe(true);
      fleet.saved = [{ origin, environmentId: entry.environmentId, enabled: true }];
      fleet.entries.set(key, entry);
      const connect = await welcomeView(client, native, { step: 'connect', now: 0 });
      expect(connect.computers.find(computer => computer.key === key)?.selected).toBe(true);
      await welcomeLocal(client, native, 'setup', '', '');
      const agents = await welcomeView(client, native, { step: 'agents', now: 0 });
      const card = agents.machines.find(machine => machine.key === key)!.agents.find(agent => agent.driver === 'claudeAgent')!;
      await welcomeLocal(client, native, 'terminal-open', key, card.key);
      const terminalId = (await welcomeView(client, native, { step: 'agents', now: 0 })).machines.find(machine => machine.key === key)!.terminal.terminalId;
      expect(remote.filter(call => call.op === 'request').map(call => call.method)).toEqual(['terminal.open', 'terminal.write']);
      remote.length = 0;

      await welcomeLocal(client, native, 'terminal-exited', entry.environmentId, JSON.stringify({ type: 'link', text: 'src/app.ts:3', terminalId }));
      expect(remote).toEqual([{ op: 'request', method: 'shell.openInEditor', payload: { cwd: '/remote/work/src/app.ts:3', editor: 'zed' }, generation: 7, fleet: key }]);
      expect(rpcs).toEqual([]);
      await welcomeLocal(client, native, 'terminal-exited', entry.environmentId, JSON.stringify({ type: 'closed', terminalId }));
      expect(remote.at(-1)).toMatchObject({ method: 'terminal.close', fleet: key, generation: 7 });
    } finally {
      fleet.saved = saved;
      if (previous) fleet.entries.set(key, previous); else fleet.entries.delete(key);
      resetPrimary();
    }
  });
});

// ── Open links in, set at runtime ─────────────────────────────────────────────────────────────────
describe('"Open links in" = T3 Code, set at runtime, reaches a drawer link', () => {
  function drawerClient(options: { failOpen?: boolean } = {}) {
    const client = new T3Client(), rpcs: { method: string; payload: Obj }[] = [], ops: Obj[] = [];
    client.environmentId = 'local'; client.threadId = 'thread-1'; client.projectId = 'p';
    Object.assign(client, { loaded: true });
    client.shell = { ...client.shell, projects: [{ id: 'p', workspaceRoot: '/repo' }], threads: [{ id: 'thread-1', projectId: 'p' }] } as typeof client.shell;
    client.request = async (_native, method, payload) => {
      rpcs.push({ method, payload });
      if (method === 'preview.list') return { sessions: [], serverEpoch: 'epoch-1', revision: 1 };
      if (method === 'preview.open') {
        if (options.failOpen) throw new Error('preview.open refused');
        return { tabId: 'tab-1', threadId: 'thread-1', url: String(payload.url), navStatus: { _tag: 'Loading', url: String(payload.url), title: '' } };
      }
      return {};
    };
    const native: Native = { available: true, watch() {}, async later(request) { ops.push(request as Obj); return { ok: true, generation: client.generation, value: { opened: true } }; } };
    terminalUiStore(client).getState().ensureTerminal({ environmentId: 'local', threadId: 'thread-1' }, 'term-1', { open: true });
    return { client, native, rpcs, ops };
  }
  const click = (url: string, metaKey = false) => JSON.stringify({ type: 'link', text: url, metaKey, ctrlKey: false, environmentId: 'local', threadId: 'thread-1', terminalId: 'term-1', surface: 'drawer' });

  test('Settings › Integrations writes the preference, then the drawer\'s link opens a Browser tab and no system browser', async () => {
    const { client, native, rpcs, ops } = drawerClient();
    await terminalOps.call(client, 'terminallocal:message', 'local:thread-1|term-1', click('http://127.0.0.1:16405/lane-link?n=plain'), 0, native, storage, out());
    expect(ops.filter(op => op.op === 'terminalOpenExternal').map(op => op.url)).toEqual(['http://127.0.0.1:16405/lane-link?n=plain']);
    expect(rpcs.some(call => call.method === 'preview.open')).toBe(false);
    await restCommand(client, native, storage, 'browser-link-target', '', 'app'); // the row's `rest:browser-link-target`
    await terminalOps.call(client, 'terminallocal:message', 'local:thread-1|term-1', click('http://127.0.0.1:16405/lane-link?n=app'), 0, native, storage, out());
    expect(rpcs.find(call => call.method === 'preview.open')?.payload).toMatchObject({ threadId: 'thread-1', url: 'http://127.0.0.1:16405/lane-link?n=app' });
    expect(ops.filter(op => op.op === 'terminalOpenExternal')).toHaveLength(1);
    // ⌘-click is the system browser whatever the setting says.
    await terminalOps.call(client, 'terminallocal:message', 'local:thread-1|term-1', click('http://127.0.0.1:16405/lane-link?n=cmd', true), 0, native, storage, out());
    expect(ops.filter(op => op.op === 'terminalOpenExternal').map(op => op.url).at(-1)).toBe('http://127.0.0.1:16405/lane-link?n=cmd');
  });

  test('an in-app open that fails is kept as the drawer\'s failure, then the system browser opens the link once', async () => {
    const { client, native, ops } = drawerClient({ failOpen: true });
    await restCommand(client, native, storage, 'browser-link-target', '', 'app');
    await terminalOps.call(client, 'terminallocal:message', 'local:thread-1|term-1', click('http://127.0.0.1:16405/lane-link?n=app'), 0, native, storage, out());
    expect(ops.filter(op => op.op === 'terminalOpenExternal').map(op => op.url)).toEqual(['http://127.0.0.1:16405/lane-link?n=app']);
    expect((await terminalDrawerView(client, null, 840, 0)).failure)
      .toBe('terminal link: Failed to open terminal link http://127.0.0.1:16405 in preview for thread thread-1. preview.open refused');
  });
});

// ── A close confirm refuses the close chord ───────────────────────────────────────────────────────
describe('A terminal close confirm that is open refuses the close chord app-wide', () => {
  function resolved(rule: { key: string; command: string; when?: string }) {
    const tokens = rule.key.split('+'), key = tokens.pop() || '+';
    return { command: rule.command, shortcut: { key, modKey: tokens.includes('mod'), shiftKey: tokens.includes('shift'), altKey: tokens.includes('alt'), ctrlKey: tokens.includes('ctrl'), metaKey: false }, ...(rule.when ? { whenAst: parseWhen(rule.when) } : {}) };
  }
  function client(): T3Client {
    const c = new T3Client();
    Object.assign(c, { connection: 'connected', configLive: true, shellLive: true, threadLive: true, generation: 1, environmentId: 'env1', origin: 'http://127.0.0.1:1', scopes: [...AUTH_STANDARD_CLIENT_SCOPES] });
    c.config = { environment: { label: 'Local' }, keybindings: keybindingDefaults.map(resolved), settings: {} };
    c.shell = { sequence: 1, projects: [{ id: 'p1', title: 'Widgets', workspaceRoot: '/repo' }], threads: [{ id: 't1', projectId: 'p1', title: 'One', updatedAt: '2026-10-03T01:00:00.000Z' }] };
    c.projectId = 'p1'; c.threadId = 't1';
    terminalUiStore(c).getState().ensureTerminal({ environmentId: 'env1', threadId: 't1' }, 'term-1', { open: true });
    terminalUiStore(c).getState().newTerminal({ environmentId: 'env1', threadId: 't1' }, 'term-2');
    return c;
  }
  const thread: DispatchContext = { composerFocus: false, editableFocus: false, turnRunning: false, modelPickerOpen: false, draftThreadRoute: false, modalOpen: false, settingsOpen: false, diffOpen: true };
  const commandWRows = (rows: ReturnType<typeof keyboardDispatch>) => rows.filter(row => row.chord === 'Meta+w').map(row => [row.command, row.kind]);

  test('with the focus off the terminals ⌘W is rightPanel.close, and while a confirm is open it runs nothing', () => {
    const c = client();
    expect(commandWRows(keyboardDispatch(c, [], '', '', thread))).toEqual([['rightPanel.close', 'close-diff']]);
    expect(commandWRows(keyboardDispatch(c, [], '', '', { ...thread, terminalClosePending: true }))).toEqual([['terminal.close', 'refuse']]);
    // Nothing else moves: the other chords keep their rows.
    const pending = keyboardDispatch(c, [], '', '', { ...thread, terminalClosePending: true }).filter(row => row.chord !== 'Meta+w');
    expect(pending).toEqual(keyboardDispatch(c, [], '', '', thread).filter(row => row.chord !== 'Meta+w'));
  });

  test('with a terminal focused ⌘W asks to close it, and while a confirm is open a second ⌘W asks nothing', () => {
    const c = client();
    recordTerminalFocus(c, { environmentId: 'env1', threadId: 't1', terminalId: 'term-2', surface: 'drawer', focused: true });
    expect(commandWRows(keyboardDispatch(c, [], '', '', thread))).toEqual([['terminal.close', 'terminal-close']]);
    expect(commandWRows(keyboardDispatch(c, [], '', '', { ...thread, terminalClosePending: true }))).toEqual([['terminal.close', 'refuse']]);
  });

  test('the window\'s confirm names it: the drawer\'s, a panel terminal\'s and a Terminal surface\'s close, and no other confirm', () => {
    const c = client();
    const args = (confirmOp: string) => [[], '', '', false, false, false, false, false, false, true, 0, false, '', '', false, '', '', '', confirmOp];
    for (const op of TERMINAL_CLOSE_CONFIRMS) expect(commandWRows(keyboardDispatchSource(c, args(op)))).toEqual([['terminal.close', 'refuse']]);
    for (const op of ['', 'shell:archive', 'sidebar:delete']) expect(commandWRows(keyboardDispatchSource(c, args(op)))).toEqual([['rightPanel.close', 'close-diff']]);
  });
});

// ── The provider sign-in terminal ─────────────────────────────────────────────────────────────────
