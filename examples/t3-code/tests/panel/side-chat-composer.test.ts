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
    composerIntentQueue: [], composerIntentPhase: '', fieldIntentNotice: '', fieldSwitchWaiting: false, fieldSwitchTo: '' };
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
    composerFieldAcknowledged: record('composerFieldAcknowledged'),
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
  test('each field keeps its own delivery cursors: a switch parks one and takes the other back', () => {
    const f = sideFocused();
    const cursors = () => [f.state.composerFieldAfter, f.state.composerFieldAcked, f.state.composerFieldEffects, f.state.composerFieldFocusDone];
    Object.assign(f.state, { fieldTarget: '', fieldSide: false, composerFieldAfter: 5, composerFieldAcked: 5, composerFieldEffects: 4, composerFieldFocusDone: 3 });
    f.run('fieldSwitch', ['s1']);
    expect([f.state.fieldTarget, f.state.fieldSide, ...cursors()]).toEqual(['s1', true, 0, 0, 0, 0]);
    Object.assign(f.state, { composerFieldAfter: 2, composerFieldAcked: 2, composerFieldEffects: 2, composerFieldFocusDone: 1 });
    f.run('fieldSwitch', ['']);
    expect([f.state.fieldTarget, f.state.fieldSide, ...cursors()]).toEqual(['', false, 5, 5, 4, 3]);
    f.run('fieldSwitch', ['s1']);
    expect(cursors()).toEqual([2, 2, 2, 1]);
    f.run('fieldSwitch', ['']);
    f.run('fieldSwitch', ['s2']);
    expect(cursors()).toEqual([0, 0, 0, 0]);
    // An edit queued for the field left lapses, with no notice in the next one.
    Object.assign(f.state, { composerIntentQueue: [{ serial: 1 }], composerIntentPhase: 'request', composerIntentActive: 1, fieldIntentNotice: 'x' });
    f.run('fieldSwitch', ['']);
    expect([f.state.composerIntentQueue, f.state.composerIntentPhase, f.state.composerIntentActive, f.state.fieldIntentNotice]).toEqual([[], '', 0, '']);
  });
  test('a switch in the middle of a delivery waits until the field it went to confirms it', () => {
    // Before: the confirm ran after the switch with the other field's cursors and the left field's target, so the left
    // field's edit was never acknowledged and its queue stayed blocked.
    const f = sideFocused();
    Object.assign(f.state, { fieldTarget: '', fieldSide: false, composerFocused: false, composerSelectionOwner: 'main-owner', composerSelectionValue: 'main words',
      composerFieldAfter: 6, composerFieldAcked: 5, composerFieldEffects: 5, composerFieldFocusDone: 5, composerFieldTarget: { seq: 6 }, composerFieldAdmitted: true });
    f.run('fieldSwitch', ['s1']);
    expect([f.state.fieldTarget, f.state.fieldSide, f.state.fieldSwitchWaiting, f.state.fieldSwitchTo]).toEqual(['', false, true, 's1']);
    // The confirm is the window field's, for its own target; then the switch goes ahead.
    f.run('composerFieldConfirmRun', [], { composerFieldLive: true });
    const acks = f.calls.filter(call => call[0] === 'composerFieldAcknowledged');
    expect(acks.map(call => [(call[1] as Env).owner, (call[1] as Env).value, ...call.slice(2)])).toEqual([['main-owner', 'main words', 6, true, '']]);
    f.run('fieldServeRun', [], { composerFieldAcked: 6, composerFieldOther: 0, composerFieldNext: { ready: false, seq: 0 }, pending: () => false });
    expect([f.state.fieldTarget, f.state.fieldSide, f.state.fieldSwitchWaiting, f.state.windowFieldAfter, f.state.windowFieldAcked, f.state.composerFieldAfter]).toEqual(['s1', true, false, 6, 6, 0]);
    // A focus to restore that the window's field asked for lapses, and leaves the side chat's cursor alone.
    f.run('composerFieldFocusRun', [], { composerFieldFocus: { seq: 9, owner: 'main-owner', focus: 'composer' } });
    expect(f.state.composerFieldFocusDone).toBe(0);
  });
  test('with neither field focused, an edit waiting for the other field moves the engine to it', () => {
    const settled = (other: number, extra: Env = {}) => ({ composerFieldOther: other, composerFieldNext: { ready: false, seq: 0 }, pending: () => false, ...extra });
    const f = sideFocused();
    Object.assign(f.state, { fieldTarget: '', fieldSide: false, composerFocused: false, composerFieldAfter: 3, composerFieldAcked: 3 });
    expect(derived({ ...f.scope, ...f.state, ...settled(2) }, 'fieldServeNext').fieldServeNext).toBe('s1');
    f.run('fieldServeRun', [], settled(2));
    expect([f.state.fieldTarget, f.state.fieldSide]).toEqual(['s1', true]);
    f.run('fieldServeRun', [], settled(4));
    expect([f.state.fieldTarget, f.state.fieldSide, f.state.composerFieldAfter]).toEqual(['', false, 3]);
    // A focused field keeps the engine, and so do a delivery not yet confirmed, an edit of its own waiting and an answer
    // still on its way (another field's, before the switch).
    const next = (state: Env, extra: Env) => derived({ ...f.scope, fieldSwitchWaiting: false, fieldSwitchTo: '', fieldTarget: '', fieldSide: false, composerFocused: false, composerFieldAfter: 3, composerFieldAcked: 3, ...state, ...extra }, 'fieldServeNext').fieldServeNext;
    expect(next({ composerFocused: true }, settled(2))).toBe('-');
    expect(next({ composerFieldAfter: 4 }, settled(2))).toBe('-');
    expect(next({}, settled(2, { composerFieldNext: { ready: true, seq: 4 } }))).toBe('-');
    expect(next({}, settled(2, { pending: (resource: unknown) => resource === 2 }))).toBe('-');
    // The side chat it served closed: the window's again.
    expect(next({ fieldTarget: 's2', fieldSide: true }, settled(0))).toBe('');
  });
  test('leaving the side chat\'s field hands the engine back to the window\'s, except a remount\'s blur', () => {
    const f = sideFocused();
    f.run('sideComposerBlur', [], { sideRich: { mode: 'rich' } });
    expect([f.state.fieldTarget, f.state.fieldSide, f.state.composerFocused]).toEqual(['', false, false]);
    const remount = sideFocused();
    remount.state.composerHeldMode = 'rich';
    remount.run('sideComposerBlur', [], { sideRich: { mode: 'literal' } });
    expect([remount.state.fieldTarget, remount.state.fieldSide]).toEqual(['s1', true]);
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
  test('the side chat\'s controls declare the chords while the focus is in its composer, the window\'s outside its pane', () => {
    const f = sideFocused();
    const chords = (extra: Env) => derived({ ...f.scope, ...f.state, ...extra }, 'sideKeys', 'composerChords');
    // Its field, or one of its composer's buttons (not the field: the engine serves the window's then).
    expect(chords({ sidePaneFocused: true, sideComposerFocused: true })).toEqual({ sideKeys: true, composerChords: false });
    expect(chords({ sidePaneFocused: true, sideComposerFocused: true, fieldSide: false, composerFocused: false })).toEqual({ sideKeys: true, composerChords: false });
    // A timeline row or the header in the pane: neither composer's (isInSideChatPane).
    expect(chords({ sidePaneFocused: true, sideComposerFocused: false })).toEqual({ sideKeys: false, composerChords: false });
    expect(chords({ sidePaneFocused: false, sideComposerFocused: false })).toEqual({ sideKeys: false, composerChords: true });
    expect(chords({ sidePaneFocused: true, sideComposerFocused: true, sideChatThread: '' })).toEqual({ sideKeys: false, composerChords: true });
    // The pane's and the composer's focusin and focusout say where the focus is; a move inside sends focusout, then focusin.
    f.run('sidePaneFocus', [true]); f.run('sideComposerFocusWithin', [true]);
    expect([f.state.sidePaneFocused, f.state.sideComposerFocused]).toEqual([true, true]);
    f.run('sideComposerFocusWithin', [false]);
    expect([f.state.sidePaneFocused, f.state.sideComposerFocused]).toEqual([true, false]);
    const view = appText('src/panel/side-chat.contract').split('\n'), line = (id: string) => view.find(entry => entry.includes(`testId="${id}"`))!;
    for (const id of ['side-chat-pane', 'side-chat-layer']) expect(line(id)).toContain('focusin=sidePaneFocus(true) focusout=sidePaneFocus(false)');
    expect(line('side-chat-composer-overlay')).toContain('focusin=sideComposerFocusWithin(true) focusout=sideComposerFocusWithin(false)');
    const thread = contractComponent('side-chat.contract', 'SideChatThread');
    const pane = { sideRich: {}, sideRichOwner: '', sideQuestionMarkdown: { documents: [] }, sideTraceMarkdown: { documents: [] }, composerTarget: '', sideChatThread: 's1' };
    expect(thread.derives({}, { ...pane, sideKeys: true, fieldSide: true, fieldIntentNotice: 'n', fieldMenuActive: 2 })).toMatchObject({ composerChords: true, composerIntentNotice: 'n', composerMenuActive: 2 });
    expect(thread.derives({}, { ...pane, sideKeys: false, fieldSide: false, fieldIntentNotice: 'n', fieldMenuActive: 2 })).toMatchObject({ composerChords: false, composerIntentNotice: '', composerMenuActive: 0 });
  });
  test('a dispatched effort step goes to the focused side chat, and to neither composer from elsewhere in its pane', () => {
    const f = sideFocused();
    Object.assign(f.state, { sidePaneFocused: true, sideComposerFocused: true });
    f.run('dispatchCommand', ['effort-step', '1', '', 0]);
    f.run('dispatchCommand', ['diff', '', '', 0]);
    f.state.sideComposerFocused = false;
    f.run('dispatchCommand', ['effort-step', '1', '', 0]);
    f.state.sidePaneFocused = false;
    f.run('dispatchCommand', ['effort-step', '-1', '', 0]);
    expect(f.calls.filter(call => call[0] === 'command' || call[0] === 'sideCommand')).toEqual([['sideCommand', 'effort-step', '1', '', 0], ['command', 'diff', '', '', 0], ['command', 'effort-step', '-1', '', 0]]);
  });
});
