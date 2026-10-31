// The welcome wizard's "Add a computer" fold and help, and where the focus goes in the wizard and the theme editor
// (T3 Code 9c161462c9, MIT, see LICENSE-T3: components/onboarding/WelcomeWizard.tsx ConnectionStep, PairingForm and
// AgentsStep; components/settings/ThemeEditorPanel.tsx). The actions run through the Contract action fixture; the
// focus a layout or a task moves is proven by the app drive.
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { contractComponent, value, type Env } from '../support/contract-action-fixture';
import { appPath, appText } from '../support/app-files';

const root = contractComponent('app.contract', 'T3Code');
const lineOf = (file: string, pattern: RegExp) => {
  const match = appText(file).split('\n').map(line => pattern.exec(line)).find(Boolean);
  if (!match) throw new Error(`${file}: no line matches ${pattern}`);
  return match[1]!;
};
const recorder = () => {
  const calls: string[] = [];
  const record = (name: string) => (...args: unknown[]) => { calls.push(`${name}:${args.join(',')}`); return name; };
  return { calls, record };
};

describe('"Add a computer" (ConnectionStep\'s expandPairingInitially)', () => {
  const open = (welcomePairFold: string, pairStartsOpen: boolean) =>
    value(lineOf('app.contract', /^  derive welcomePairOpen = (.+)$/), { welcomePairFold, welcome: { pairStartsOpen } }) as boolean;

  test('starts closed while the local server runs and open without one; a toggle then holds', () => {
    expect([open('', false), open('', true)]).toEqual([false, true]);
    const state: Env = { welcomePairFold: '' };
    root.call('welcomeTogglePair', state, { welcomePairOpen: open('', false) }, []);
    expect(state.welcomePairFold).toBe('open');
    root.call('welcomeTogglePair', state, { welcomePairOpen: open('open', false) }, []);
    expect(state.welcomePairFold).toBe('closed');
    expect(open('closed', true)).toBe(false);
  });

  test('the Connect step mounts again as it was first shown', () => {
    const state: Env = { welcomeStep: 'agents', welcomePairFold: 'open' };
    root.call('welcomeGo', state, {}, ['agents']);
    expect(state.welcomePairFold).toBe('open');
    root.call('welcomeGo', state, {}, ['connect']);
    expect(state).toEqual({ welcomeStep: 'connect', welcomePairFold: '' });
  });

  test('reopening the fold, or the Connect step, starts the pairing help closed', () => {
    const layer = contractComponent('app-overlays.contract', 'WelcomeLayer');
    const { calls, record } = recorder();
    const state: Env = { welcomeHelpOpen: true };
    layer.call('welcomeTogglePairing', state, { welcomeTogglePair: record('togglePair') }, []);
    expect([state.welcomeHelpOpen, calls]).toEqual([false, ['togglePair:']]);
    state.welcomeHelpOpen = true;
    layer.call('welcomeGoStep', state, { welcomeGo: record('go') }, ['connect']);
    expect([state.welcomeHelpOpen, calls.at(-1)]).toEqual([false, 'go:connect']);
  });
});

describe('the focus after a pairing (ConnectionStep\'s onPaired and its `ready` effect)', () => {
  test('a pairing closes the fold and counts, and Continue takes the focus once it is enabled', () => {
    const { calls, record } = recorder();
    const state: Env = { noticeExit: '', routeKeyboard: '', modalError: 'old', pendingModal: 'welcome-pair', commandPending: true, draftOwner: '', jumpRequest: 0,
      utilityPage: '', lineageStopping: '', renameAfterOp: '', renameAfterId: '', welcomeLink: 'http://127.0.0.1:16510/pair#token=A', welcomePairFold: 'open', welcomePaired: 0 };
    root.call('commandCompleted', state, { command: record('command'), focus: record('focus'), relaunch: record('relaunch'), openPage: record('openPage'),
      openSettings: record('openSettings'), prSelect: record('prSelect'), routeKeyboardComplete: () => '', routeKeyboardOwned: false,
      changed: { message: '', relaunch: false, routeReorder: 0 }, data: { sidebar: { renameId: '' } } }, []);
    expect(state).toMatchObject({ welcomeLink: '', welcomePairFold: 'closed', welcomePaired: 1, commandPending: false });
    const when = lineOf('app.contract', /^  task welcomePairedFocus when (.+) key=welcomePaired$/);
    const runs = (paired: number, ready: boolean, pending: boolean, step = 'connect') =>
      value(when, { welcomePaired: paired, welcomeStep: step, welcome: { ready }, commandPending: pending });
    expect([runs(1, true, false), runs(0, true, false), runs(1, false, false), runs(1, true, true), runs(1, true, false, 'agents')]).toEqual([true, false, false, false, false]);
    root.call('welcomeFocusContinue', {}, { focus: record('focus') }, []);
    expect(calls.at(-1)).toBe('focus:welcome-continue');
  });
});

describe('steps and panels that focus a control as they mount (autoFocus)', () => {
  test('DialogOpenFocus focuses its target at its first layout', () => {
    const opener = contractComponent('dialog-trap.contract', 'DialogOpenFocus');
    const { calls, record } = recorder();
    const state = opener.state();
    opener.call('laidOut', state, { target: 'theme-editor-name', focus: record('focus') }, [1, 1]);
    opener.call('laidOut', state, { target: 'theme-editor-name', focus: record('focus') }, [1, 1]);
    expect(calls).toEqual(['focus:theme-editor-name']);
  });

  test('the Agents step focuses its Continue, unless a command is running', () => {
    const target = lineOf('pages-welcome.contract', /^      DialogOpenFocus\(target=(\(busy \? .+\))\)$/);
    expect([value(target, { busy: false }), value(target, { busy: true })]).toEqual(['welcome-agents-continue', '']);
    const agents = contractComponent('pages-welcome.contract', 'WelcomeAgents');
    expect(agents.drawn('DialogOpenFocus(', {}, { busy: false, view: { machines: [] } })).toBe(true);
  });

  test('the theme editor focuses Theme name whenever its body mounts', () => {
    const panel = contractComponent('settings-appearance-editor.contract', 'ThemeEditorPanel');
    const scope = { ...panel.state(), inspecting: false, usage: [], vw: 1280, vh: 840, max: Math.max, min: Math.min };
    expect(panel.drawn('DialogOpenFocus(target="theme-editor-name")', { minimized: false }, scope)).toBe(true);
    expect(panel.drawn('DialogOpenFocus(target="theme-editor-name")', { minimized: true }, scope)).toBe(false);
    expect(readFileSync(appPath('settings-appearance-editor.contract'), 'utf8')).toContain('input id="theme-editor-name" ');
  });

  test('Return on a focused wizard button presses it, as a button does in Chrome', () => {
    const primary = contractComponent('pages-welcome.contract', 'WelcomePrimary');
    const { calls, record } = recorder();
    const key = (k: string, disabled: boolean, held: Partial<Record<'metaKey' | 'shiftKey' | 'repeat', boolean>> = {}) =>
      primary.call('buttonKey', primary.state(), { disabled, press: record('press'), preventDefault: record('prevent'), stopPropagation: record('stop') },
        [k, { metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, repeat: false, ...held }]);
    key('Enter', false);
    expect(calls).toEqual(['prevent:', 'stop:', 'press:']);
    calls.length = 0;
    key('Enter', true);
    key('Enter', false, { repeat: true });
    key('Enter', false, { metaKey: true });
    key('a', false);
    expect(calls.filter(call => call === 'press:')).toEqual([]);
  });
});

// The reference binds its chords on the window: none adds an element to the accessibility tree.
test('the chord buttons are hidden from accessibility and keep their chords', () => {
  const source = readFileSync(appPath('settings-shortcuts.contract'), 'utf8');
  const box = source.split('\n').find(line => line.includes('testId="keyboard-dispatch"'))!;
  expect(box).toContain('aria-hidden=true');
  const button = source.split('\n').find(line => line.includes('button -exact-menu="none" press=pressed'))!;
  expect(button).toContain('aria-keyshortcuts=(item.command == "rightPanel.close" or (held and editingChord(item.chord)) ? "" : item.chord)'); // rightPanel.close is the window capture's (app-window.contract modifierDown); the device tab's rename field holds some (window-interaction.test.ts)
});
