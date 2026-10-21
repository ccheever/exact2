// ⌘Z undoes a thread action from the composer right after it (T3 Code 74541bab17 AppSidebarLayout.tsx
// ThreadUndoShortcut, hooks/showThreadUndoNotice.ts threadUndoClaimsShortcut / releaseThreadUndoShortcut): the
// dispatch row the claim adds, and the window's actions that release it and cancel a held chord's repeats.
import { describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { chordCode, keyboardDispatch, keyboardDispatchSource, type DispatchContext } from '../../src/shell/keyboard-dispatch';
import { keybindingDefaults } from '../../src/settings/keybinding-settings';
import { parseWhen } from '../../src/settings/keybinding-view';
import { sidebarSession } from '../../src/sidebar/sidebar-state';
import { contractComponent, type Env } from '../support/contract-action-fixture';

const undoBinding = (() => {
  const entry = keybindingDefaults.find(binding => binding.command === 'thread.undo')!;
  return { command: entry.command, shortcut: { key: 'z', modKey: true }, whenAst: parseWhen(entry.when!) };
})();
function client(bindings: unknown[] = [undoBinding]): T3Client {
  const c = new T3Client();
  Object.assign(c, { connection: 'connected', configLive: true, shellLive: true, threadLive: true, generation: 1, environmentId: 'env1', origin: 'http://127.0.0.1:1' });
  c.config = { environment: { label: 'Local', capabilities: {} }, keybindings: bindings, settings: {} };
  c.shell = { sequence: 1, projects: [{ id: 'p1', title: 'Widgets', workspaceRoot: '/repo' }], threads: [{ id: 't1', projectId: 'p1', title: 'One' }] };
  c.projectId = 'p1'; c.threadId = 't1';
  sidebarSession(c).undo = { action: 'Settled', threadIds: ['t0'], at: Date.now() };
  return c;
}
const composer: DispatchContext = { composerFocus: true, editableFocus: true, turnRunning: false, modelPickerOpen: false, draftThreadRoute: false, modalOpen: false, settingsOpen: false, diffOpen: false, undoShown: true };
const undoRows = (c: T3Client, context: DispatchContext) => keyboardDispatch(c, [], '', '', context).filter(row => row.command === 'thread.undo');

describe('the notice claims the undo chord in the composer', () => {
  test('without the claim the composer keeps ⌘Z; with it the chord undoes and names its physical key', () => {
    const c = client();
    expect(undoRows(c, composer)).toEqual([]);
    expect(undoRows(c, { ...composer, undoClaimed: true }).map(row => [row.chord, row.target, row.extra])).toEqual([['Meta+z', 'sidebar:undo', 'KeyZ']]);
    // Outside a field the chord undoes with or without the claim, as before.
    expect(undoRows(c, { ...composer, composerFocus: false, editableFocus: false }).map(row => row.chord)).toEqual(['Meta+z']);
  });
  test('the model picker keeps the chord', () => {
    expect(undoRows(client(), { ...composer, undoClaimed: true, modelPickerOpen: true })).toEqual([]);
  });
  test('a claimed chord is the notice\'s alone', () => {
    // A field's own binding of the chord wins while the field has the focus; the claim takes the chord from it.
    const field = { command: 'chat.new', shortcut: { key: 'z', modKey: true }, whenAst: parseWhen('editableFocus') };
    const c = client([undoBinding, field]);
    const chord = (context: DispatchContext) => keyboardDispatch(c, [], '', '', context).filter(row => row.chord === 'Meta+z').map(row => row.command);
    expect(chord(composer)).toEqual(['chat.new']);
    expect(chord({ ...composer, undoClaimed: true })).toEqual(['thread.undo']);
  });
  test('the contract\'s argument carries the claim', () => {
    const c = client();
    const args = (claimed: boolean) => [[], '', '', true, false, false, false, false, false, false, 0, false, 'command', '', true, '', '', '', '', '', false, claimed];
    expect(keyboardDispatchSource(c, args(false)).some(row => row.command === 'thread.undo')).toBe(false);
    expect(keyboardDispatchSource(c, args(true)).some(row => row.command === 'thread.undo')).toBe(true);
  });
  test('a chord\'s physical key', () => {
    expect(chordCode('Meta+z')).toBe('KeyZ');
    expect(chordCode('Meta+Shift+7')).toBe('Digit7');
    expect(chordCode('Meta+Plus')).toBe('');
  });
});

describe('the window releases the claim and swallows a held chord\'s repeats', () => {
  const root = contractComponent('app.contract', 'T3Code');
  const emptyModifiers = () => ({ metaKey: false, ctrlKey: false, altKey: false, shiftKey: false });
  const key = (init: Partial<{ key: string; code: string; repeat: boolean; metaKey: boolean }>) => ({ ...emptyModifiers(), key: '', code: '', repeat: false, ...init });
  function window(undoUntil = 9_000) {
    const calls: string[] = [];
    const state: Env = { shortcutModifiers: emptyModifiers(), speedModifierState: emptyModifiers(), undoHeldCode: '', undoReleasedUntil: 0 };
    const data = { sidebarOpen: true, sidebar: { undoUntil } };
    const scope: Env = {
      data,
      shortcutModifierStateAfterKeyboardEvent: (current: unknown) => current, areShortcutModifierStatesEqual: () => true, emptyShortcutModifierState: emptyModifiers,
      preventDefault: () => calls.push('preventDefault'), stopPropagation: () => calls.push('stopPropagation'),
      releaseUndoClaim: () => { calls.push('release'); root.call('releaseUndoClaim', state, { data }, []); },
    };
    return { calls, state, scope, data };
  }

  test('a modifier keeps the claim; any other key releases it', () => {
    const w = window();
    root.call('modifierKey', w.state, w.scope, [true, key({ key: 'Meta', code: 'MetaLeft', metaKey: true }), true]);
    expect(w.calls).toEqual([]);
    root.call('modifierKey', w.state, w.scope, [true, key({ key: 'a', code: 'KeyA' }), true]);
    expect(w.calls).toEqual(['release']);
    expect(w.state.undoReleasedUntil).toBe(9_000);
  });
  test('the chord\'s repeats after an undo are cancelled until another key', () => {
    const w = window();
    root.call('command', w.state, { ...w.scope, commandPending: true, wallTime: { epochAtZero: 0 }, performanceNow: () => 0 }, ['sidebar:undo', 'KeyZ', '', 0]);
    expect(w.state.undoHeldCode).toBe('KeyZ');
    root.call('modifierKey', w.state, w.scope, [true, key({ key: 'z', code: 'KeyZ', repeat: true, metaKey: true }), true]);
    expect(w.calls).toEqual(['preventDefault', 'stopPropagation']);
    // Another key's repeat is not the chord's.
    w.calls.length = 0;
    root.call('modifierKey', w.state, w.scope, [true, key({ key: 'x', code: 'KeyX', repeat: true }), true]);
    expect(w.calls).toEqual([]);
    root.call('modifierKey', w.state, w.scope, [true, key({ key: 'z', code: 'KeyZ', metaKey: true }), true]);
    expect(w.state.undoHeldCode).toBe('');
    w.calls.length = 0;
    root.call('modifierKey', w.state, w.scope, [true, key({ key: 'z', code: 'KeyZ', repeat: true, metaKey: true }), true]);
    expect(w.calls).toEqual([]);
  });
  test('the inline Undo, a press with no chord, holds no key', () => {
    const w = window();
    root.call('command', w.state, { ...w.scope, commandPending: true, wallTime: { epochAtZero: 0 }, performanceNow: () => 0 }, ['sidebar:undo', '', '', 0]);
    expect(w.state.undoHeldCode).toBe('');
  });
});
