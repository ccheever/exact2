// F6 between workspace areas (T3 Code 6d450a4284 lib/workspaceFocus.ts, AppSidebarLayout.tsx, SettingsSidebarNav.tsx
// useSettingsSectionNavigation, KeybindingsSettings.logic.ts) and ⌘K's Triage an issue (CommandPalette.tsx,
// hooks/useStartTriage.ts, lib/triageTarget.ts). The root's actions run here with Contract's action semantics.
import { afterEach, describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import { keybindingCommands, keybindingDefaults } from '../../src/settings/keybinding-settings';
import { commandLabel, parseWhen } from '../../src/settings/keybinding-view';
import { keyboardDispatch, type DispatchContext } from '../../src/shell/keyboard-dispatch';
import { rootActions } from '../../src/palette/palette';
import { paletteCommand } from '../../src/palette/palette-commands';
import { pickTriageEnvironment, startTriage, triageEnvironment } from '../../src/palette/palette-triage';
import { isTriagePrompt } from '../../src/pages/pages-home';
import { toasts } from '../../src/shell/toast';
import { fleet } from '../../src/connections/settings-b-fleet';
import { AUTH_STANDARD_CLIENT_SCOPES } from '../../src/connections/remote-scopes';
import type { Files } from '../../src/client/protocol';
import { contractComponent, type Env } from '../support/contract-action-fixture';
import { contractFns } from '../support/contract-fn-fixture';
import { primaryAt, resetPrimary } from '../support/local-primary-fixture';

afterEach(() => { fleet.entries.clear(); resetPrimary(); });

function resolved(rule: { key: string; command: string; when?: string }) {
  const tokens = rule.key.split('+');
  const key = tokens.pop() || '+';
  return { command: rule.command, shortcut: { key, modKey: tokens.includes('mod'), shiftKey: tokens.includes('shift'), altKey: tokens.includes('alt'), ctrlKey: tokens.includes('ctrl'), metaKey: false },
    ...(rule.when ? { whenAst: parseWhen(rule.when) } : {}) };
}
function client(capabilities: Record<string, boolean> = {}): T3Client {
  const c = new T3Client();
  Object.assign(c, { connection: 'connected', configLive: true, shellLive: true, generation: 1, environmentId: 'env1', origin: 'http://127.0.0.1:16801', threadLive: true, scopes: AUTH_STANDARD_CLIENT_SCOPES });
  c.config = { environment: { label: 'Studio', serverVersion: '0.1.5', capabilities }, keybindings: keybindingDefaults.map(resolved), settings: {}, scratchWorkspaceRoot: '/scratch' };
  c.shell = { sequence: 1, projects: [{ id: 'p1', title: 'Widgets', workspaceRoot: '/repo' }, { id: 'scratch', title: 'Scratch', workspaceRoot: '/scratch' }],
    threads: [{ id: 't1', projectId: 'p1', title: 'One', updatedAt: '2026-10-03T01:00:00.000Z' }] };
  c.projectId = 'p1'; c.threadId = 't1';
  return c;
}
const context: DispatchContext = { composerFocus: true, editableFocus: true, turnRunning: false, modelPickerOpen: false, draftThreadRoute: false, modalOpen: false, settingsOpen: false, diffOpen: false };

describe('F6 between workspace areas', () => {
  test('F6, ⇧F6, ⌘F6 and ⇧⌘F6 are dispatch buttons in a thread and in Settings, and none under a modal', () => {
    const rows = (over: Partial<DispatchContext>) => keyboardDispatch(client(), [], '', '', { ...context, ...over })
      .filter(row => row.kind === 'workspace-focus').map(row => [row.chord, row.target]);
    const all = [['F6', 'next'], ['Meta+F6', 'next'], ['Shift+F6', 'previous'], ['Meta+Shift+F6', 'previous']];
    expect(rows({}).sort()).toEqual([...all].sort());
    expect(rows({ settingsOpen: true }).sort()).toEqual([...all].sort());
    expect(rows({ modalOpen: true })).toEqual([]);
    expect(keybindingCommands.slice(0, 2)).toEqual(['workspace.focusNext', 'workspace.focusPrevious']);
    expect([commandLabel('workspace.focusNext'), commandLabel('workspace.focusPrevious'), commandLabel('thread.copyReference')])
      .toEqual(['Focus Next Workspace Area', 'Focus Previous Workspace Area', 'Pull Request: Copy Link or Thread']);
  });

  const root = contractComponent('app.contract', 'T3Code');
  /** The root's workspace state over a window whose shown areas have the frames `shown` gives. */
  function window(shown: string[], over: Env = {}) {
    const state: Env = { workspaceArea: '', workspaceTargets: [], terminalWorkspaceMove: 0, terminalWorkspaceFrom: '', terminalWorkspaceSerial: 0 };
    const focused: string[] = [], sent: unknown[][] = [];
    const container = (area: string) => area === 'composer' ? 'frame-overlay' : `workspace-${area}`;
    const scope: Env = {
      settingsOpen: false, settingsRoute: 'general', data: { threadId: 't1' }, details: { inline: true, cwd: '/repo', editorId: 'vscode', editorUnavailable: '' }, canvas: { cardShown: shown.includes('details') }, terminalDrawer: { open: shown.includes('terminal') },
      frame: (id: string) => {
        const visible = shown.some(area => container(area) === id) || ['row-t1', 'held-button', ...(over.controls as string[] ?? [])].includes(id) || (id === 'settings-navigation' && shown.includes('settings-nav'));
        return { width: visible ? 100 : 0, height: visible ? 100 : 0, x: 0, y: 0, unavailable: !visible };
      },
      focus: (id: string) => focused.push(id), command: (...args: unknown[]) => sent.push(args), navActiveRoute: (route: string) => route,
      WorkspaceTarget: (entry: Env) => entry, ...contractFns('src/shell/shell-shapes.contract'), ...over,
    };
    for (const name of ['workspaceFocusArea']) scope[name] = (...args: unknown[]) => root.call(name, state, scope, args);
    for (const name of ['workspaceMove', 'terminalWorkspaceQueue']) scope[name] = (...args: unknown[]) => root.call(name, state, scope, args);
    return { state, focused, sent, scopeOf: () => scope, move: (direction: number, from = '') => root.call('workspaceMove', state, scope, [direction, from]),
      enter: (area: string, target: string) => root.call('workspaceEntered', state, scope, [area, { target }]) };
  }

  test('F6 from the composer reaches the right panel, the terminal and the sidebar; ⇧F6 goes back', () => {
    const w = window(['sidebar', 'conversation', 'composer', 'panel', 'terminal']);
    w.enter('composer', 'composer');
    w.move(1);
    expect([w.state.workspaceArea, w.focused.at(-1)]).toEqual(['panel', 'workspace-panel']);
    w.move(1);
    expect(w.state.workspaceArea).toBe('terminal');
    expect(w.sent.at(-1)).toEqual(['terminallocal:focus', '', '', 0]); // the drawer focuses its page
    w.move(1);
    expect([w.state.workspaceArea, w.focused.at(-1)]).toEqual(['sidebar', 'row-t1']); // the open thread's row
    w.move(-1);
    w.move(-1);
    expect([w.state.workspaceArea, w.focused.at(-1)]).toEqual(['panel', 'workspace-panel']);
    // A move from inside the terminal names it, as its page holds the key.
    w.move(-1, 'terminal');
    expect(w.state.workspaceArea).toBe('panel');
  });

  test('F6 and ⇧F6 typed in a terminal page move on from its area: the drawer\'s is the terminal, the right panel\'s the panel', () => {
    const w = window(['sidebar', 'conversation', 'composer', 'panel', 'terminal']);
    // The answer queues the move (it cannot send the terminal's own mutation); the task runs it on the next turn.
    const completed = (message: string) => {
      root.call('terminalCompleted', w.state, { ...w.scopeOf(), terminalChanged: { message } }, []);
      if (w.state.terminalWorkspaceMove !== 0) root.call('terminalWorkspaceRun', w.state, w.scopeOf(), []);
    };
    completed('terminal:workspace-focus:next:terminal');
    expect([w.state.workspaceArea, w.focused.at(-1)]).toEqual(['sidebar', 'row-t1']);
    completed('terminal:workspace-focus:previous:terminal');
    expect([w.state.workspaceArea, w.focused.at(-1)]).toEqual(['panel', 'workspace-panel']);
    completed('terminal:workspace-focus:next:panel');
    expect(w.state.workspaceArea).toBe('terminal');
    completed('terminal:focus-composer');
    expect(w.focused.at(-1)).toBe('composer');
  });

  test('an area takes back the control it last held while that control is still shown, and only shown areas count', () => {
    const w = window(['sidebar', 'composer', 'details']);
    w.enter('details', 'held-button');
    w.enter('composer', 'composer');
    w.move(1);
    expect([w.state.workspaceArea, w.focused.at(-1)]).toEqual(['details', 'held-button']);
    w.enter('details', 'gone-button');
    w.move(-1);
    w.move(1);
    expect(w.focused.at(-1)).toBe('workspace-details'); // the held control is gone and the card shows no control
    // Else the card's first enabled control, as upstream's FOCUSABLE_SELECTOR fallback: the wrapper is out of flow.
    const card = window(['composer', 'details'], { controls: ['details-open-editor', 'details-choose-editor', 'details-add-script'] });
    card.enter('composer', 'composer');
    card.move(1);
    expect([card.state.workspaceArea, card.focused.at(-1)]).toEqual(['details', 'details-open-editor']);
    const noEditor = window(['composer', 'details'], { controls: ['details-open-editor', 'details-choose-editor', 'details-add-script'],
      details: { inline: true, cwd: '/repo', editorId: 'vscode', editorUnavailable: 'No SSH route to Studio' } });
    noEditor.enter('composer', 'composer');
    noEditor.move(1);
    expect(noEditor.focused.at(-1)).toBe('details-choose-editor'); // a disabled Open is passed over
    const settings = window(['settings-nav', 'settings'], { settingsOpen: true });
    settings.move(1);
    expect([settings.state.workspaceArea, settings.focused.at(-1)]).toEqual(['settings-nav', 'settings-general']);
  });

  test('Settings sections are one Tab stop that the arrows move without opening a section', () => {
    const nav = contractComponent('settings-core.contract', 'SettingsNav');
    const focused: string[] = [];
    const state: Env = { hovered: '', navFocused: '', navFocusedRoute: '' };
    const items = ['general', 'chat', 'diffs'].map(id => ({ id, route: id }));
    const scope: Env = { nav: { searching: false, items }, route: 'chat', navActiveRoute: (route: string) => route, focus: (id: string) => focused.push(id), min: Math.min, max: Math.max,
      preventDefault: () => focused.push('prevented'), navIds: items.map(item => item.id) };
    scope.navEntered = (...args: unknown[]) => nav.call('navEntered', state, scope, args);
    const key = (id: string, k: string) => nav.call('navKey', state, scope, [id, k, { key: k, metaKey: false, ctrlKey: false, altKey: false, shiftKey: false }]);
    key('chat', 'ArrowDown');
    expect([focused.at(-1), state.navFocused]).toEqual(['settings-diffs', 'diffs']);
    key('diffs', 'ArrowDown');
    expect(focused.at(-1)).toBe('settings-diffs');
    key('diffs', 'Home');
    expect(focused.at(-1)).toBe('settings-general');
    focused.length = 0;
    key('general', 'ArrowRight');
    expect(focused).toEqual(['prevented']);
  });
});

describe('Triage an issue', () => {
  test('the triage target rule: on screen, else the primary if it supports triage, else the first connected that does', () => {
    const supports = new Set(['b', 'c']);
    const pick = (onScreen: string | null, primary: string | null) => pickTriageEnvironment({ onScreen, primary, connected: ['a', 'b', 'c'], supportsTriage: id => supports.has(id) });
    expect([pick('a', 'b'), pick(null, 'b'), pick(null, 'a'), pick(null, null)]).toEqual(['a', 'b', 'b', 'b']);
    expect(pickTriageEnvironment({ onScreen: null, primary: null, connected: ['a'], supportsTriage: () => false })).toBeNull();
  });

  test('⌘K lists it where the environment on screen reports triage and the connection may operate it', () => {
    const titles = (c: T3Client) => rootActions(c).map(item => item.row.title);
    expect(titles(client())).not.toContain('Triage an issue');
    const ready = client({ triage: true });
    expect(triageEnvironment(ready)).toBe('env1');
    const row = rootActions(ready).find(item => item.row.title === 'Triage an issue')!;
    expect(row.row).toMatchObject({ op: 'flow', arg: 'triage', icon: 'bug', description: 'An agent investigates a T3 Code problem and helps file an issue' });
    expect(row.terms).toEqual(['triage', 'bug', 'issue', 'report', 'broken', 'debug', 'help', 'support']);
    // A machine on screen that does not report it offers none, even with a primary that does.
    primaryAt('http://127.0.0.1:16802', 'primary-env');
    expect(triageEnvironment(client())).toBeNull();
    expect(titles(ready).filter(title => title.startsWith('Focus'))).toEqual(['Focus Next Workspace Area', 'Focus Previous Workspace Area']);
  });

  test('a start opens the No project draft holding the prepared prompt; a draft with text keeps it', async () => {
    const c = client({ triage: true });
    const prompt = 'Investigate with [triage-playbook.md](/tmp/t3/triage-playbook.md).';
    const calls: string[] = [];
    Object.assign(c, {
      async prepareTriage() { calls.push('prepare'); return prompt; },
      rpc: async (_native: Native, method: string) => { calls.push(method); return { projectId: 'scratch' }; },
    });
    const native = { available: true } as unknown as Native;
    expect(await startTriage(c, native)).toBe('scratch');
    expect(calls).toEqual(['prepare', 'projects.ensureScratch']);
    expect(c.local.drafts['env1:new:scratch']).toBe(prompt);
    expect(isTriagePrompt(prompt)).toBe(true);
    c.local.drafts['env1:new:scratch'] = 'My own words';
    await startTriage(c, native);
    expect(c.local.drafts['env1:new:scratch']).toBe('My own words');
  });

  test('a failure says to run the command in a terminal, with a button that copies it', async () => {
    const c = client({ triage: true });
    Object.assign(c, { async prepareTriage() { throw new Error('The server could not prepare triage.'); } });
    const files: Files = { fs: { async mkdir() {}, async readFile() { throw new Error('none'); }, async atomicWriteFile() {} } };
    const result = await paletteCommand(c, { available: true } as unknown as Native, files, 'triage', '', '');
    expect(result).toMatchObject({ close: true, project: '' });
    expect(toasts(c).at(-1)).toMatchObject({ kind: 'error', title: 'Could not start triage', timeoutMs: 0,
      description: 'The server could not prepare triage. Run npx t3 triage in a terminal on that machine instead.',
      action: { label: 'Copy command', op: 'pageslocal:copy', value: 'npx t3 triage' } });
    c.config = { ...c.config, scratchWorkspaceRoot: '' };
    expect(await startTriage(c, { available: true } as unknown as Native)).toBe('');
    expect(toasts(c).at(-1)!.description).toBe('This machine cannot start threads without a project. Run npx t3 triage in a terminal on that machine instead.');
  });
});
