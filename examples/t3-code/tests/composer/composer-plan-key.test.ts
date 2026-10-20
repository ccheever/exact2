import { describe, expect, test } from 'bun:test';
import { contractComponent, type Env } from '../support/contract-action-fixture';

// app.contract's composer key action (composerKeyEvent → composerGrammarKey) and the command menu's
// highlight, run with Contract's action semantics. The reference is onComposerCommandKey (ChatComposer.tsx)
// behind ComposerPromptEditorTiptap's handleKeyDown.
const root = contractComponent('app.contract', 'T3Code');
const commandMenu = contractComponent('src/composer/composer-editor.contract', 'ComposerCommandMenu');

type Options = { planVisible?: boolean; planActive?: boolean; connected?: boolean; inCode?: boolean; inList?: boolean; menuOpen?: boolean };
function composer(options: Options = {}) {
  const calls: unknown[][] = [];
  const state: Env = { composerShift: false, composerCommand: false, composerComposing: false };
  const scope = (): Env => ({
    data: { composer: { planVisible: options.planVisible ?? true, planActive: options.planActive ?? false }, connected: options.connected ?? true },
    richComposer: { mode: 'rich', historyPrevious: true, historyNext: true },
    composerRichCurrent: true, composerSelectionCurrent: true, composerSelectionStart: 2, composerSelectionEnd: 2, composerText: '- a',
    composerInCode: options.inCode ?? false, composerInList: options.inList ?? false, composerCodeStart: false, composerRequiresModifier: false,
    editor: { menu: { open: options.menuOpen ?? false }, stash: { menuOpen: false } },
    preventDefault: () => calls.push(['preventDefault']), stopPropagation: () => {},
    menuPick: (...args: unknown[]) => calls.push(['menuPick', ...args]),
    composerIntentRequest: (kind: string, _text: string, key: string, shift: boolean) => calls.push(['intent', kind, key, shift]),
    composerGrammarKey: (...args: unknown[]) => root.call('composerGrammarKey', state, scope(), args),
    ...state,
  });
  const key = (k: string, modifiers: Partial<Record<'shiftKey' | 'ctrlKey' | 'altKey' | 'metaKey', boolean>> = {}) =>
    root.call('composerKeyEvent', state, scope(), [k, { key: k, code: '', repeat: false, shiftKey: false, ctrlKey: false, altKey: false, metaKey: false, ...modifiers }]);
  return { calls, key };
}

describe('⇧⇥ in the composer (onComposerCommandKey)', () => {
  test('toggles Build and Plan where plan mode is available, and keeps the key', () => {
    const build = composer();
    build.key('Tab', { shiftKey: true });
    expect(build.calls).toEqual([['preventDefault'], ['menuPick', 'interaction', '', 'plan']]);
    const plan = composer({ planActive: true });
    plan.key('Tab', { shiftKey: true });
    expect(plan.calls).toEqual([['preventDefault'], ['menuPick', 'interaction', '', 'default']]);
  });
  test('toggles in a list item and with a menu open; a code block outdents first', () => {
    for (const options of [{ inList: true }, { menuOpen: true }]) {
      const field = composer(options);
      field.key('Tab', { shiftKey: true });
      expect(field.calls).toEqual([['preventDefault'], ['menuPick', 'interaction', '', 'plan']]);
    }
    const code = composer({ inCode: true });
    code.key('Tab', { shiftKey: true });
    expect(code.calls).toEqual([['preventDefault'], ['intent', 'key', 'Tab', true]]);
  });
  test('without plan mode the focus goes back, from a list item too; a code block still outdents', () => {
    const list = composer({ planVisible: false, inList: true });
    list.key('Tab', { shiftKey: true });
    expect(list.calls).toEqual([]);
    const tab = composer({ planVisible: false, inList: true });
    tab.key('Tab');
    expect(tab.calls).toEqual([['preventDefault'], ['intent', 'key', 'Tab', false]]);
    const code = composer({ planVisible: false, inCode: true });
    code.key('Tab', { shiftKey: true });
    expect(code.calls).toEqual([['preventDefault'], ['intent', 'key', 'Tab', true]]);
    const text = composer({ planVisible: false });
    text.key('Tab', { shiftKey: true });
    expect(text.calls).toEqual([]);
  });
  test('plain Tab is not the toggle, and a disconnected composer keeps the key without a change', () => {
    const tab = composer();
    tab.key('Tab');
    expect(tab.calls).toEqual([]);
    const offline = composer({ connected: false });
    offline.key('Tab', { shiftKey: true });
    expect(offline.calls).toEqual([['preventDefault']]);
  });
});

describe('the open command menu keeps its keys for the text view (ComposerCommandMenu)', () => {
  test('⇧↓ and ↓ with a menu open reach the text view, whose key buttons move the highlight', () => {
    for (const modifiers of [{}, { shiftKey: true }, { altKey: true }]) {
      const field = composer({ menuOpen: true });
      field.key('ArrowDown', modifiers);
      expect(field.calls).toEqual([]);
    }
    const menu = { count: 3, searchKey: 'a' }, max = Math.max;
    const state = commandMenu.state();
    const move = (delta: number) => commandMenu.call('move', state, { menu, max, ...commandMenu.derives(state, { menu }) }, [delta]);
    move(1); move(1);
    expect(commandMenu.derives(state, { menu }).active).toBe(2);
    move(1);
    expect(commandMenu.derives(state, { menu }).active).toBe(0);
    move(-1);
    expect(commandMenu.derives(state, { menu }).active).toBe(2);
  });
});
