// A side chat's composer is the window's composer for its thread (T3 Code 2359643cff, MIT, see LICENSE-T3: SideChatPane
// embeds ChatView, so its ChatComposer is the whole editor). The window's editing engine (composer-members.contract)
// serves whichever field has the focus (`fieldTarget`); its ⌘ chords are the focused composer's (useChatShortcuts
// `embedded`, composerShortcutCommands).
import { describe, expect, test } from 'bun:test';
import { contractComponent, value, type Env } from '../support/contract-action-fixture';
import { appText } from '../support/app-files';

const root = contractComponent('app.contract', 'T3Code');
const source = appText('app.contract').split('\n');
/** The root's derives named, each over `env` and the ones before it. */
function derived(env: Env, ...names: string[]): Env {
  const out: Env = {}, scope: Env = { ...env };
  for (const name of names) {
    const line = source.find(entry => entry.startsWith(`  derive ${name} = `))!;
    scope[name] = out[name] = value(line.slice(line.indexOf(' = ') + 3), scope);
  }
  return out;
}
const key = (k: string, held: Env = {}) => ({ key: k, code: `Key${k.toUpperCase()}`, metaKey: false, shiftKey: false, ctrlKey: false, altKey: false, repeat: false, isComposing: false, ...held });

/** The window's composer and a side chat's, the side chat's field focused: the engine's state and the two fields. */
function sideFocused(text = 'side words', start = 0, end = text.length) {
  const calls: unknown[][] = [];
  const record = (name: string) => (...args: unknown[]) => { calls.push([name, ...args]); return {}; };
  const state: Env = { fieldTarget: 's1', fieldSide: true, composerFocused: true, composerComposing: false, composerShift: false, composerCommand: false,
    composerSelectionOwner: 'side-owner', composerSelectionValue: text, composerSelectionStart: start, composerSelectionEnd: end, composerFieldInputSerial: 3,
    composerIntentQueue: [], composerIntentPhase: '', fieldIntentNotice: '' };
  const menu = { open: false, count: 0, rows: [], searchKey: '' }, stash = { menuOpen: false, count: 0, rows: [] };
  const scope: Env = {
    sideChatThread: 's1', modal: false, shortcuts: [], settings: { sendShortcut: 'enter' },
    data: { snapshotOwner: 'snap', requestMode: '', composer: { subagent: false, sendChords: '', sendCommandChords: '', planVisible: true, planActive: false }, connected: true },
    sideChat: { snapshotOwner: 'snap', requestMode: '', composer: { subagent: false, sendChords: '', sendCommandChords: '', planVisible: true, planActive: false }, connected: true },
    richComposer: { owner: 'main-owner', value: 'main words', generation: 1, mode: 'rich', blocks: [], chips: [], historyPrevious: true, historyNext: false },
    richComposerOwner: 'main-owner', composerText: 'main words', editor: { menu, stash },
    sideRich: { owner: 'side-owner', value: text, generation: 1, mode: 'rich', blocks: [], chips: [], historyPrevious: true, historyNext: false },
    sideRichOwner: 'side-owner', sideText: text, sideEditor: { menu, stash },
    composerChord: (k: string, e: Env) => [e.metaKey ? 'Meta' : '', e.shiftKey ? 'Shift' : '', k].filter(Boolean).join('+'), composerChordIn: () => false,
    heldModifiers: () => '', max: Math.max, min: Math.min, first: (items: unknown[]) => items.length > 0 ? items[0] : undefined,
    rangeFrame: () => ({ unavailable: false, y: 10, height: 18 }), frame: () => ({ width: 300 }),
    preventDefault: record('preventDefault'), stopPropagation: record('stopPropagation'), format: record('format'),
    editorOp: record('editorOp'), sideEditorOp: record('sideEditorOp'), submit: record('submit'), sideSubmit: record('sideSubmit'),
    menuPick: record('menuPick'), sideInteraction: record('sideInteraction'), command: record('command'), sideCommand: record('sideCommand'),
    workspaceMove: record('workspaceMove'), composerFieldObserved: record('composerFieldObserved'), ComposerFieldObservation: (fields: Env) => fields,
    composerIntent: record('composerIntent'), ComposerIntentMetadata: (fields: Env) => fields,
  };
  return { state, scope, calls, run: (action: string, args: unknown[], extra: Env = {}) => root.call(action, state, { ...scope, ...extra }, args) };
}

describe('the window\'s editing engine serves the focused field', () => {
  test('a side chat field\'s focus takes it, the window\'s gives it back', () => {
    const f = sideFocused();
    f.state.fieldTarget = ''; f.state.fieldSide = false; f.state.composerFocused = false;
    f.run('sideComposerFocus', [], { sideRich: { mode: 'rich' }, modifierEditableFocus: () => {} });
    expect([f.state.fieldTarget, f.state.fieldSide, f.state.composerFocused, f.state.composerHeldMode]).toEqual(['s1', true, true, 'rich']);
    f.run('composerFocus', [], { modifierEditableFocus: () => {} });
    expect([f.state.fieldTarget, f.state.fieldSide, f.state.composerHeldMode]).toEqual(['', false, 'rich']);
  });
  test('the field\'s names are the side chat\'s while it serves it, the window\'s otherwise', () => {
    const f = sideFocused();
    const side = derived({ ...f.scope, ...f.state }, 'fieldText', 'fieldOwner', 'fieldId');
    expect([side.fieldText, side.fieldOwner, side.fieldId]).toEqual(['side words', 'side-owner', 'side-chat-composer']);
    const main = derived({ ...f.scope, ...f.state, fieldSide: false }, 'fieldText', 'fieldOwner', 'fieldId');
    expect([main.fieldText, main.fieldOwner, main.fieldId]).toEqual(['main words', 'main-owner', 'composer']);
  });
  test('⌘B and ⌘I format the side chat\'s field at its selection', () => {
    const f = sideFocused();
    f.run('composerKeyEvent', ['b', key('b', { metaKey: true })]);
    f.run('composerKeyEvent', ['i', key('i', { metaKey: true })]);
    expect(f.calls.filter(call => call[0] === 'format')).toEqual([['format', 'side-chat-composer', 'bold'], ['format', 'side-chat-composer', 'italic']]);
  });
  test('↑ on the first line recalls the side chat\'s history', () => {
    const f = sideFocused('', 0, 0);
    f.run('composerKeyEvent', ['ArrowUp', key('ArrowUp')]);
    expect(f.calls.filter(call => call[0] === 'sideEditorOp' || call[0] === 'editorOp')).toEqual([['sideEditorOp', 'history', 'previous', '']]);
  });
  test('⇧⇥ toggles the side chat\'s Plan mode, and the open stash menu takes its keys', () => {
    const f = sideFocused('words', 5, 5);
    f.run('composerKeyEvent', ['Tab', key('Tab', { shiftKey: true })]);
    expect(f.calls.filter(call => call[0] === 'sideInteraction' || call[0] === 'menuPick')).toEqual([['sideInteraction', 'plan']]);
    const stash = { menuOpen: true, count: 1, rows: [{ index: 0, id: 'p1' }] };
    f.run('composerKeyEvent', ['Enter', key('Enter')], { sideEditor: { menu: { open: false, count: 0, rows: [], searchKey: '' }, stash } });
    expect(f.calls.filter(call => call[0] === 'sideEditorOp')).toEqual([['sideEditorOp', 'stash-restore', 'p1', '']]);
  });
  test('its input is observed as the side chat\'s field, and ⌥↑ edits no window queue from it', () => {
    const f = sideFocused('ab', 2, 2);
    f.run('sideWrite', ['abc', { selectionStart: 3, selectionEnd: 3, isComposing: false, inputType: 'insertText' }], { releaseUndoClaim: () => {}, composerFieldLive: true });
    const observed = f.calls.find(call => call[0] === 'composerFieldObserved')!;
    expect(observed.slice(2)).toEqual([true, 's1']);
    expect([f.state.composerFieldInputSerial, f.state.sideDraft, f.state.composerSelectionOwner]).toEqual([4, 'abc', 'side-owner']);
    const queued = sideFocused('x', 0, 0);
    queued.run('composerKeyEvent', ['ArrowUp', key('ArrowUp', { altKey: true })], { shortcuts: [{ kind: 'composer-key', chord: 'ArrowUp', target: 'cclocal:queued-edit', extra: 'r1' }] });
    expect(queued.calls.filter(call => call[0] === 'command')).toEqual([]);
  });
});

describe('a composer chord acts on the composer that has the focus', () => {
  test('the side chat\'s controls declare the chords while its field has the focus', () => {
    const f = sideFocused();
    const chords = (extra: Env) => derived({ ...f.scope, ...f.state, ...extra }, 'sideKeys', 'composerChords');
    expect(chords({})).toEqual({ sideKeys: true, composerChords: false });
    expect(chords({ composerFocused: false })).toEqual({ sideKeys: false, composerChords: true });
    expect(chords({ fieldSide: false })).toEqual({ sideKeys: false, composerChords: true });
    const thread = contractComponent('side-chat.contract', 'SideChatThread');
    const pane = { sideRich: {}, sideRichOwner: '', sideQuestionMarkdown: { documents: [] }, sideTraceMarkdown: { documents: [] }, composerTarget: '', sideChatThread: 's1' };
    expect(thread.derives({}, { ...pane, sideKeys: true, fieldSide: true, fieldIntentNotice: 'n', fieldMenuActive: 2 })).toMatchObject({ composerChords: true, composerIntentNotice: 'n', composerMenuActive: 2 });
    expect(thread.derives({}, { ...pane, sideKeys: false, fieldSide: false, fieldIntentNotice: 'n', fieldMenuActive: 2 })).toMatchObject({ composerChords: false, composerIntentNotice: '', composerMenuActive: 0 });
  });
  test('a dispatched effort step goes to the focused side chat', () => {
    const f = sideFocused();
    f.run('dispatchCommand', ['effort-step', '1', '', 0]);
    f.run('dispatchCommand', ['diff', '', '', 0]);
    expect(f.calls.filter(call => call[0] === 'command' || call[0] === 'sideCommand')).toEqual([['sideCommand', 'effort-step', '1', '', 0], ['command', 'diff', '', '', 0]]);
  });
});
