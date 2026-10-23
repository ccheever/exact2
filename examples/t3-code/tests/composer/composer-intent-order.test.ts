import {describe, expect, test} from 'bun:test';
import {readFileSync} from 'node:fs';
import {contractComponent, contractFns, value, type Env} from '../support/contract-action-fixture';
import {appPath} from '../support/app-files';
import {composerIntentAnswer} from '../../src/composer/composer-intent-source';
import {composerRichProjection} from '../../src/composer/composer-rich-projection';
import {composerSourceSession} from '../../src/composer/composer-rich-state';

const root = contractComponent('app.contract', 'T3Code');
const source = readFileSync(appPath('app.contract'), 'utf8').split('\n');
const record = (fields: Env) => fields;
const sum = (xs: number[]) => xs.reduce((a, x) => a + x, 0);
const chipFns = contractFns('src/composer/composer-rich.contract', {sum});
const keyFns = contractFns('src/composer/composer-keys.contract');
const chipsOf = (text: string) => composerRichProjection({value: text, owner: 'draft', styling: true, literal: false}).chips;
function fixture(initial = 'word', caret = initial.length) {
  const client = {snapshotOwner: 'snapshot'};
  composerSourceSession(client).observe('draft', initial);
  const state: Env = {
    draft: initial, draftOwner: 'draft', composerFieldInputSerial: 0, composerComposing: false, composerShift: false, composerCommand: false,
    composerSelectionOwner: 'draft', composerSelectionValue: initial, composerSelectionStart: caret,
    composerSelectionEnd: caret, composerInputType: '', composerFocused: true, composerTyping: false, typingSerial: 0,
  };
  for (const line of source) {
    const match = /^  state (composerIntent\w+)(?:\s*:[^=]+)? = (.+)$/.exec(line);
    if (match) state[match[1]!] = value(match[2]!, {ComposerFieldObservation: record});
  }
  const replies: ReturnType<typeof composerIntentAnswer>[] = [], commands: unknown[][] = [], selections: unknown[][] = [], requests: unknown[][] = [], formats: unknown[][] = [];
  let projection = {owner: 'draft', value: initial, generation: 1, mode: 'rich', chips: chipsOf(initial)};
  let sends = 0, suppressed = 0;
  const observations: unknown[][] = [];
  const scope = (): Env => {
    const env: Env = {
      data: {snapshotOwner: client.snapshotOwner, requestKey: '', requestMode: '', composer: {sendChords: 'Meta+Enter', sendCommandChords: ''}}, richComposerOwner: 'draft', composerOwner: 'draft', shortcuts: [], ...keyFns,
      composerText: state.draft, richComposer: projection, composerFieldLive: true, composerRequiresModifier: false, composerShift: false, composerCommand: false,
      editor: {menu: {open: false}, stash: {menuOpen: false}}, composerInCode: false, composerInList: false, composerCodeStart: false,
      first: (items: unknown[]) => items[0], ComposerIntentMetadata: record, ComposerFieldObservation: record, composerFieldObserved: (...args: unknown[]) => {observations.push(args); return {};}, command: () => '',
      composerIntent: (...args: unknown[]) => {requests.push(args); const reply = composerIntentAnswer(client, args); replies.push(reply); return reply;},
      setRangeText: (...args: unknown[]) => commands.push(args), setSelectionRange: (...args: unknown[]) => selections.push(args), send: () => sends++,
      format: (...args: unknown[]) => formats.push(args),
      preventDefault: () => suppressed++, stopPropagation: () => {}, releaseUndoClaim: () => {}, ...chipFns,
      ...state,
    };
    for (const name of ['composerRichCurrent', 'composerSelectionCurrent', 'composerFieldSnapshot',
      'composerIntentContextCurrent', 'composerIntentBaseCurrent', 'composerIntentReceiptCurrent']) {
      const line = source.find(line => line.startsWith(`  derive ${name} = `))!;
      env[name] = value(line.slice(line.indexOf(' = ') + 3), env);
    }
    for (const action of ['composerIntentClear', 'composerIntentRequest', 'composerIntentInput', 'composerMoved', 'composerSelectionObserved', 'composerGrammarKey']) {
      env[action] = (...args: unknown[]) => call(action, args);
    }
    return env;
  };
  let inherited: Env = {};
  function call(action: string, args: unknown[] = [], extra: Env = {}) {
    const previous = inherited; inherited = {...previous, ...extra};
    try {root.call(action, state, {...scope(), ...inherited}, args);} finally {inherited = previous;}
  }
  const input = (text: string, start: number, end = start, composing = false) => call('write', [text, {
    value: text, selectionStart: start, selectionEnd: end, isComposing: composing, inputType: 'insertText',
  }]);
  const move = (start: number, end = start, composing = false) => call('composerMoved', [{
    value: state.draft, selectionStart: start, selectionEnd: end, isComposing: composing,
  }]);
  const project = () => {composerSourceSession(client).observe('draft', String(state.draft)); projection = {...projection, value: String(state.draft), chips: chipsOf(String(state.draft))};};
  const press = () => call('composerGrammarKey', ['Enter', {shiftKey: true, metaKey: false, ctrlKey: false, altKey: false, repeat: false}, false]);
  const request = (kind: string, text = '', start = Number(state.composerSelectionStart), end = start) =>
    call('composerIntentRequest', [kind, text, kind === 'key' ? 'Enter' : '', true, false, start, end, Boolean(state.composerComposing)]);
  const reply = (index = replies.length - 1) => call('composerEditDone', [], {composerEdited: replies[index]});
  const accept = (index = replies.length - 1, projectNow = true) => {
    const result = replies[index]!;
    const next = String(state.draft).slice(0, result.start) + result.text + String(state.draft).slice(result.end);
    input(next, result.preserveSelection ? Number(state.composerSelectionStart) : result.selectionStart,
      result.preserveSelection ? Number(state.composerSelectionEnd) : result.selectionEnd);
    if (projectNow) project();
  };
  return {state, client, replies, commands, selections, requests, formats, observations, call, input, move, project, press, request, reply, accept,
    get sends() {return sends;}, get suppressed() {return suppressed;},
    set projection(next: typeof projection) {projection = next;}, get projection() {return projection;}};
}

describe('actual root composer intent ordering', () => {
  for (const count of [2, 3]) test(`${count} suppressed presses reach the real source one at a time at the resulting caret`, () => {
    const f = fixture();
    for (let i = 0; i < count; i++) f.press();
    expect(f.suppressed).toBe(count);
    expect(f.requests).toHaveLength(0);
    for (let i = 0; i < count; i++) {
      f.call('composerIntentPump');
      expect(f.requests).toHaveLength(i + 1);
      expect(f.requests[i]?.slice(2, 6)).toEqual(['word' + '\n'.repeat(i), i + 1, 4 + i, 4 + i]);
      f.reply();
      // One Markdown-field range edit is one native undo step, with no mode flag (LLP 1045 D6).
      expect(f.commands[i]).toEqual(['composer', '\n', 4 + i, 4 + i, 'preserve']);
      f.accept();
      f.call('composerIntentConfirmed');
    }
    expect(f.state.draft).toBe('word' + '\n'.repeat(count));
    expect(f.state.composerIntentQueue).toEqual([]);
    expect(f.commands).toHaveLength(count);
  });
  test('neither result nor projection alone releases the next key; actual input and final caret are required', () => {
    const f = fixture(); f.press(); f.press(); f.call('composerIntentPump'); f.reply();
    f.call('composerIntentPump'); f.call('composerIntentConfirmed');
    expect(f.requests).toHaveLength(1);
    f.accept(0, false); f.call('composerIntentConfirmed');
    expect(f.state.composerIntentPhase).toBe('projection');
    expect(f.requests).toHaveLength(1);
    f.project(); f.call('composerIntentConfirmed'); f.call('composerIntentPump');
    expect(f.requests).toHaveLength(2);
  });
  test('programmatic intermediate selection is admitted until the requested caret arrives', () => {
    const f = fixture(); f.press(); f.press(); f.call('composerIntentPump'); f.reply();
    f.input('word\n', 4); f.project(); f.move(4); f.call('composerIntentConfirmed');
    expect(f.state.composerIntentPhase).toBe('projection');
    f.move(5); f.call('composerIntentConfirmed'); f.call('composerIntentPump');
    expect(f.requests[1]?.slice(4, 6)).toEqual([5, 5]);
  });
  for (const stage of ['request', 'projection']) test(`manual input/selection and composition invalidate queued keys during ${stage}`, () => {
    for (const change of ['input', 'selection', 'composition']) {
      const f = fixture(); f.press(); f.press(); f.call('composerIntentPump');
      if (stage === 'projection') {f.reply(); f.accept();}
      if (change === 'input') f.input(String(f.state.draft) + 'x', String(f.state.draft).length + 1);
      if (change === 'selection') f.move(0);
      if (change === 'composition') f.move(Number(f.state.composerSelectionStart), undefined, true);
      expect(f.state.composerIntentQueue).toEqual([]);
      expect(f.state.composerIntentNotice).not.toBe('');
      const before = f.commands.length; f.reply(); expect(f.commands).toHaveLength(before);
    }
  });
  test('owner, generation, mode, snapshot and duplicate replies cannot apply a stale command', () => {
    for (const change of ['owner', 'generation', 'mode', 'snapshot']) {
      const f = fixture(); f.press(); f.press(); f.call('composerIntentPump');
      if (change === 'owner') f.projection = {...f.projection, owner: 'other'};
      if (change === 'generation') f.projection = {...f.projection, generation: 2};
      if (change === 'mode') f.projection = {...f.projection, mode: 'plain'};
      if (change === 'snapshot') f.client.snapshotOwner = 'next';
      f.reply(); expect(f.commands).toHaveLength(0); expect(f.state.composerIntentQueue).toEqual([]);
    }
    const f = fixture(); f.press(); f.press(); f.call('composerIntentPump'); f.reply(); f.reply();
    expect(f.commands).toHaveLength(1);
    f.accept(); f.call('composerIntentConfirmed'); f.call('composerIntentPump'); f.reply(0);
    expect(f.commands).toHaveLength(1);
  });
  test('pastes use the resulting selection; language edits retain explicit source ranges', () => {
    const paste = fixture('a'); paste.request('paste', 'x'); paste.request('paste', 'y');
    paste.call('composerIntentPump'); paste.reply(); paste.accept(); paste.call('composerIntentConfirmed'); paste.call('composerIntentPump'); paste.reply(); paste.accept();
    expect(paste.state.draft).toBe('axy');
    const f = fixture('```ts\nx\n```\n- [ ] task', 8);
    f.request('language', 'rust', 0, 5);
    f.call('composerIntentPump');
    expect(f.requests[0]?.slice(4, 6)).toEqual([0, 5]);
    f.reply(); expect(f.replies[0]?.preserveSelection).toBe(true); f.accept(); f.call('composerIntentConfirmed');
    expect(f.state.composerIntentQueue).toEqual([]);
    expect(f.state.draft).toBe('```rust\nx\n```\n- [ ] task');
    // The Markdown field draws task boxes; the root has no checkbox action.
    expect(source.some(line => line.includes('composerTaskToggle'))).toBe(false);
  });
  test('stale explicit ranges refuse visibly and a full bounded queue retains admitted keys', () => {
    const f = fixture('```ts\nx\n```'); f.press(); f.request('language', 'rust', 0, 5);
    f.call('composerIntentPump'); f.reply(); f.accept(); f.call('composerIntentConfirmed'); f.call('composerIntentPump');
    expect(f.requests).toHaveLength(1); expect(f.state.composerIntentQueue).toEqual([]); expect(f.state.composerIntentNotice).not.toBe('');
    const full = fixture(); for (let i = 0; i < 33; i++) full.press();
    expect(full.state.composerIntentQueue).toHaveLength(32); expect(full.state.composerIntentNotice).not.toBe('');
  });
  test('beforeinput routes rapid line breaks and pastes through the same FIFO', () => {
    for (const [inputType, data] of [['insertLineBreak', null], ['insertFromPaste', 'x']] as const) {
      const f = fixture();
      for (let i = 0; i < 2; i++) f.call('composerBeforeInput', [{value: 'word', selectionStart: 4, selectionEnd: 4,
        isComposing: false, cancelable: true, inputType, data}]);
      expect(f.suppressed).toBe(2);
      for (let i = 0; i < 2; i++) {f.call('composerIntentPump'); f.reply(); f.accept(); f.call('composerIntentConfirmed');}
      expect(f.state.draft).toBe(inputType === 'insertLineBreak' ? 'word\n\n' : 'wordxx');
    }
  });
  test('no-op outdent consumes only its metadata; send happens once and refuses queued repeats visibly', () => {
    const noOp = fixture('```ts\nx\n```', 6);
    noOp.call('composerIntentRequest', ['key', '', 'Tab', true, false, 6, 6, false]);
    noOp.press(); noOp.call('composerIntentPump'); noOp.reply();
    expect(noOp.replies[0]?.kind).toBe('handled');
    expect(noOp.commands).toHaveLength(0);
    expect(noOp.state.composerIntentNotice).toBe('');
    noOp.call('composerIntentPump'); expect(noOp.requests).toHaveLength(2);
    const send = fixture();
    for (let i = 0; i < 2; i++) send.call('composerIntentRequest', ['key', '', 'Enter', false, false, 4, 4, false]);
    send.call('composerIntentPump'); send.reply(); send.reply();
    expect(send.sends).toBe(1); expect(send.state.composerIntentQueue).toEqual([]);
    expect(send.state.composerIntentNotice).not.toBe('');
  });
  test('changed input serial refuses alias source, and a new admission clears refusal', () => {
    const f = fixture(); f.press(); f.call('composerIntentPump');
    f.state.composerFieldInputSerial = 1;
    f.reply(); expect(f.commands).toHaveLength(0); expect(f.state.composerIntentNotice).not.toBe('');
    f.press(); expect(f.state.composerIntentNotice).toBe('');
    f.state.composerIntentNotice = 'Previous edit was refused'; f.client.snapshotOwner = 'next';
    f.call('composerIntentOwnerChanged'); expect(f.state.composerIntentNotice).toBe('');
  });

  test('typing in a code body, an empty one included, is the field\'s own', () => {
    const projected = (f: ReturnType<typeof fixture>) => ({richComposer: {...f.projection, ...composerRichProjection({value: String(f.state.draft), owner: 'draft', styling: true, literal: false})}});
    const body = fixture('```ts\nx\n```', 7); body.state.composerComposing = true;
    body.call('composerBeforeInput', [{value: body.state.draft, selectionStart: 7, selectionEnd: 7,
      isComposing: false, cancelable: true, inputType: 'insertText', data: '字'}], projected(body));
    expect(body.suppressed).toBe(0); expect(body.state.composerComposing).toBe(false); expect(body.state.composerIntentQueue).toEqual([]);
    const empty = fixture('```ts\n\n```', 6);
    empty.call('composerBeforeInput', [{value: empty.state.draft, selectionStart: 6, selectionEnd: 6,
      isComposing: false, cancelable: true, inputType: 'insertText', data: 'c'}], projected(empty));
    expect(empty.suppressed).toBe(0); expect(empty.state.composerIntentQueue).toEqual([]);
  });
  test('text at a restored fence without a body line keeps its closing fence whole; Backspace unwraps it', () => {
    const draft = '```ts\n```', blocks = composerRichProjection({value: draft, owner: 'draft', styling: true, literal: false}).blocks;
    const typed = fixture(draft, 6); typed.projection = {...typed.projection, blocks} as typeof typed.projection;
    typed.call('composerBeforeInput', [{value: draft, selectionStart: 6, selectionEnd: 6, isComposing: false, cancelable: true, inputType: 'insertText', data: 'x'}]);
    expect(typed.suppressed).toBe(1);
    typed.call('composerIntentPump'); typed.reply(); typed.accept(); typed.call('composerIntentConfirmed');
    expect([typed.state.draft, typed.state.composerSelectionStart]).toEqual(['```ts\nx\n```', 7]);
    const line = source.find(line => line.startsWith('  derive composerCodeStart = '))!;
    const codeStart = value(line.slice(line.indexOf(' = ') + 3), {composerRichCurrent: true, composerSelectionStart: 6, composerSelectionEnd: 6, richComposer: {blocks}});
    expect(codeStart).toBe(true);
    const erased = fixture(draft, 6); erased.projection = {...erased.projection, blocks} as typeof erased.projection;
    erased.call('composerGrammarKey', ['Backspace', {shiftKey: false, metaKey: false, ctrlKey: false, altKey: false, repeat: false}, false], {composerCodeStart: codeStart});
    expect(erased.suppressed).toBe(1);
    erased.call('composerIntentPump'); erased.reply(); erased.accept(); erased.call('composerIntentConfirmed');
    expect(erased.commands).toEqual([['composer', '', 0, 9, 'preserve']]);
    expect([erased.state.draft, erased.state.composerSelectionStart, erased.state.composerIntentQueue]).toEqual(['', 0, []]);
  });
  test('composition start invalidates deferred keys while leaving composition input native-owned', () => {
    const f = fixture(); f.press();
    f.call('composerBeforeInput', [{value: 'word', selectionStart: 4, selectionEnd: 4,
      isComposing: true, cancelable: true, inputType: 'insertCompositionText', data: '字'}]);
    expect(f.suppressed).toBe(1); expect(f.state.composerComposing).toBe(true);
    expect(f.state.composerIntentQueue).toEqual([]);
  });
  test('paragraph fallback after composition follows the same send policy as the root key action', () => {
    const key = fixture(); key.call('composerGrammarKey', ['Enter', {shiftKey: false, metaKey: false, ctrlKey: false, altKey: false, repeat: false}, false]); key.call('composerIntentPump');
    const fallback = fixture(); fallback.state.composerComposing = true;
    fallback.call('composerBeforeInput', [{value: 'word', selectionStart: 4, selectionEnd: 4,
      isComposing: false, cancelable: true, inputType: 'insertParagraph', data: null}]); fallback.call('composerIntentPump');
    expect(fallback.replies[0]?.kind).toBe(key.replies[0]?.kind);
  });

  for (const [label, inputType, shift, command, requiresModifier, expected] of [
    ['normal', 'insertParagraph', false, false, false, 'send'],
    ['line break', 'insertLineBreak', false, false, false, 'edit'],
    ['Shift', 'insertParagraph', true, false, false, 'edit'],
    ['Command', 'insertParagraph', false, true, false, 'send'],
    ['modifier shortcut without modifier', 'insertParagraph', false, false, true, 'edit'],
    ['modifier shortcut with modifier', 'insertParagraph', false, true, true, 'send'],
  ] as const) test(`beforeinput ${label} preserves Enter policy after unchanged composition completion`, () => {
    const f = fixture(); Object.assign(f.state, {composerComposing: true, composerShift: shift, composerCommand: command});
    f.call('composerBeforeInput', [{value: 'word', selectionStart: 4, selectionEnd: 4,
      isComposing: false, cancelable: true, inputType, data: null}], {composerRequiresModifier: requiresModifier});
    expect(f.state.composerComposing).toBe(false); expect(f.suppressed).toBe(1);
    f.call('composerIntentPump'); expect(f.replies[0]?.kind).toBe(expected);
  });

  for (const [label, text, caret, key, meta, expected] of [
    ['Enter', 'word', 4, 'Enter', false, 'send'],
    ['Tab', '```ts\nx\n```', 7, 'Tab', false, 'edit'],
  ] as const) test(`Apple admitted ${label} after unchanged unmark uses the fresh key boundary`, () => {
    const f = fixture(text, caret); f.state.composerComposing = true;
    f.call('composerKeyEvent', [key, {shiftKey: false, metaKey: meta, ctrlKey: false, altKey: false, repeat: false, code: key === 'b' ? 'KeyB' : key}], {composerInCode: key === 'Tab'});
    expect(f.state.composerComposing).toBe(false); expect(f.suppressed).toBe(1);
    f.call('composerIntentPump'); expect(f.replies[0]?.kind).toBe(expected);
  });
  test('Command-B and Command-I are the Markdown field\'s format commands, even before the projection catches up', () => {
    for (const [key, command] of [['b', 'bold'], ['i', 'italic']] as const) {
      const f = fixture(); f.state.composerComposing = true;
      f.call('composerKeyEvent', [key, {shiftKey: false, metaKey: true, ctrlKey: false, altKey: false, repeat: false, code: key === 'b' ? 'KeyB' : 'KeyI'}],
        {composerRichCurrent: false, composerSelectionCurrent: false});
      expect(f.state.composerComposing).toBe(false); expect(f.suppressed).toBe(1);
      expect(f.formats).toEqual([['composer', command]]); expect(f.state.composerIntentQueue).toEqual([]);
    }
    const plain = fixture(); plain.projection = {...plain.projection, mode: 'plain'};
    plain.call('composerKeyEvent', ['b', {shiftKey: false, metaKey: true, ctrlKey: false, altKey: false, repeat: false, code: 'KeyB'}]);
    expect(plain.formats).toEqual([]); expect(plain.suppressed).toBe(0);
  });
  test('Command-B pressed while an edit waits takes its turn after that edit, at the resulting selection', () => {
    const bold = {shiftKey: false, metaKey: true, ctrlKey: false, altKey: false, repeat: false, code: 'KeyB'};
    const kinds = (f: ReturnType<typeof fixture>) => (f.state.composerIntentQueue as {kind: string}[]).map(item => item.kind);
    const f = fixture('- a', 3); f.press(); f.call('composerIntentPump');
    f.call('composerKeyEvent', ['b', bold]);
    expect(f.formats).toEqual([]); expect(kinds(f)).toEqual(['key', 'format']); expect(f.state.composerIntentNotice).toBe('');
    // Also while the list edit's projection still lags the field.
    f.reply(); f.accept(undefined, false);
    f.call('composerKeyEvent', ['i', {...bold, code: 'KeyI'}]);
    expect(kinds(f)).toEqual(['key', 'format', 'format']); expect(f.state.composerIntentNotice).toBe('');
    f.project(); f.call('composerIntentConfirmed'); f.call('composerIntentPump');
    expect(f.state.draft).toBe('- a\n- ');
    expect(f.formats).toEqual([['composer', 'bold']]); expect(f.state.composerIntentPhase).toBe('format');
    // The field reports the command as typing; its receipt releases the next one.
    f.input('- a\n- ****', 8); f.project(); f.call('composerIntentConfirmed'); f.call('composerIntentPump');
    expect(f.formats).toEqual([['composer', 'bold'], ['composer', 'italic']]); expect(kinds(f)).toEqual(['format']);
    // A command that changed nothing reports no input; its turn still ends.
    f.call('composerIntentFormatSettled');
    expect(f.state.composerIntentQueue).toEqual([]); expect(f.state.composerIntentPhase).toBe(''); expect(f.state.composerIntentNotice).toBe('');
    f.press(); f.call('composerIntentPump'); expect(f.requests).toHaveLength(2);
  });
  test('fresh composition completion reaches independent field metadata without a synthetic input', () => {
    const f = fixture(); f.state.composerComposing = true;
    f.call('composerBeforeInput', [{value: 'word', selectionStart: 4, selectionEnd: 4,
      isComposing: false, cancelable: true, inputType: 'insertParagraph', data: null}]);
    expect(f.observations.at(-1)).toEqual([expect.objectContaining({composing: false, value: 'word', inputSerial: 0}), false]);
    expect(f.state.draft).toBe('word'); expect(f.state.composerFieldInputSerial).toBe(0);
  });
  test('Apple nonediting key after unchanged unmark refreshes field metadata once without source/input changes', () => {
    const f = fixture(); f.state.composerComposing = true;
    const event = {shiftKey: false, metaKey: false, ctrlKey: false, altKey: false, repeat: false, code: 'ArrowRight'};
    f.call('composerKeyEvent', ['ArrowRight', event]);
    expect(f.observations).toEqual([[expect.objectContaining({composing: false, value: 'word', start: 4, end: 4, inputSerial: 0}), false]]);
    expect(f.state.composerComposing).toBe(false); expect(f.state.composerFieldInputSerial).toBe(0);
    expect(f.state.draft).toBe('word'); expect(f.commands).toHaveLength(0); expect(f.requests).toHaveLength(0);
    f.call('composerKeyEvent', ['ArrowRight', event]); expect(f.observations).toHaveLength(1);
  });
  test('modifier-only key admission retains the cached composition state', () => {
    const f = fixture(); f.state.composerComposing = true;
    f.call('composerKeyEvent', ['Shift', {shiftKey: true, metaKey: false, ctrlKey: false, altKey: false, repeat: false, code: 'ShiftLeft'}]);
    expect(f.state.composerComposing).toBe(true); expect(f.suppressed).toBe(0); expect(f.requests).toHaveLength(0);
  });

});

// The reference's composer chips are atoms (ComposerPromptEditorTiptap: atom, contentEditable={false}): Backspace or
// Delete beside one removes it whole, the caret never rests inside it and typing never edits it.
describe('a composer reference is one unit', () => {
  const cite = '[Assistant quote](t3-citation://v1/e/t/m?text=the&start=0&end=3&prefix=&suffix=)';
  const draft = `see ${cite} now`, start = 4, end = 4 + cite.length, shownEnd = start + '[Assistant quote'.length;
  const key = (k: string, held: Partial<Record<'altKey' | 'metaKey' | 'shiftKey' | 'ctrlKey', boolean>> = {}) =>
    [k, {shiftKey: false, metaKey: false, ctrlKey: false, altKey: false, repeat: false, ...held}, false];
  const settle = (f: ReturnType<typeof fixture>) => {f.call('composerIntentPump'); f.reply(); f.accept(); f.call('composerIntentConfirmed');};
  const input = (f: ReturnType<typeof fixture>, inputType: string, at: number, data: string | null = null) =>
    f.call('composerBeforeInput', [{value: f.state.draft, selectionStart: at, selectionEnd: at, isComposing: false, cancelable: true, inputType, data}], {richComposer: {...f.projection, blocks: []}});
  test('Backspace after it, Delete before it, either inside it, removes its whole source in one range edit', () => {
    for (const [k, caret] of [['Backspace', end], ['Delete', start], ['Backspace', 10], ['Delete', 10]] as const) {
      const f = fixture(draft, caret);
      f.call('composerGrammarKey', key(k));
      expect(f.suppressed).toBe(1);
      settle(f);
      expect(f.commands).toEqual([['composer', '', start, end, 'preserve']]);
      expect([f.state.draft, f.state.composerSelectionStart, f.state.composerIntentQueue]).toEqual(['see  now', start, []]);
    }
  });
  test('Option-Backspace takes the reference and the spaces between, as word deletion takes an atom', () => {
    const f = fixture(draft.replace(' now', '  now'), end + 1);
    f.call('composerGrammarKey', key('Backspace', {altKey: true}));
    expect(f.suppressed).toBe(1);
    settle(f);
    expect(f.commands).toEqual([['composer', '', start, end + 1, 'preserve']]);
  });
  test('keys that do not reach it, and ⌘⌫, stay the field\'s own', () => {
    for (const [k, caret, held] of [['Backspace', start, {}], ['Delete', end, {}], ['Backspace', end + 1, {}], ['Backspace', end, {metaKey: true}], ['Backspace', end + 4, {altKey: true}]] as const) {
      const f = fixture(draft, caret);
      f.call('composerGrammarKey', key(k, held));
      expect([f.suppressed, f.requests.length]).toEqual([0, 0]);
    }
  });
  test('a second Backspace pressed before the first removal lands deletes a character, not nothing', () => {
    const f = fixture(draft, end);
    f.call('composerGrammarKey', key('Backspace'));
    f.call('composerGrammarKey', key('Backspace'));
    expect(f.suppressed).toBe(2);
    settle(f); settle(f);
    expect([f.state.draft, f.state.composerSelectionStart, f.state.composerIntentNotice, f.state.composerIntentQueue]).toEqual(['see now', 3, '', []]);
  });
  test('cite, Backspace twice, type: the citation goes whole, so the text never enters its label', () => {
    const f = fixture(`${cite} `, cite.length + 1);
    f.call('composerGrammarKey', key('Backspace'));
    expect(f.suppressed).toBe(0);
    f.input(cite, cite.length); f.project();
    f.call('composerGrammarKey', key('Backspace'));
    expect(f.suppressed).toBe(1);
    settle(f);
    expect([f.state.draft, f.state.composerSelectionStart]).toEqual(['', 0]);
  });
  test('⌃H, ⌃D and word deletion at it remove it whole through beforeinput', () => {
    for (const [inputType, caret, removed] of [['deleteContentBackward', end, [start, end]], ['deleteContentForward', start, [start, end]], ['deleteWordBackward', end + 1, [start, end + 1]], ['deleteWordForward', start - 1, [start - 1, end]]] as const) {
      const f = fixture(draft, caret);
      input(f, inputType, caret);
      expect(f.suppressed).toBe(1);
      settle(f);
      expect(f.commands).toEqual([['composer', '', removed[0], removed[1], 'preserve']]);
    }
    const away = fixture(draft, end + 3);
    input(away, 'deleteWordBackward', end + 3);
    expect(away.suppressed).toBe(0);
  });
  test('text typed or dropped inside it goes after it', () => {
    for (const inputType of ['insertText', 'insertFromDrop']) {
      const f = fixture(draft, 10);
      input(f, inputType, 10, 'x');
      expect(f.suppressed).toBe(1);
      settle(f);
      expect(f.commands).toEqual([['composer', 'x', end, end, 'preserve']]);
      expect([f.state.draft, f.state.composerSelectionStart]).toEqual([`see ${cite}x now`, end + 1]);
    }
    const outside = fixture(draft, end);
    input(outside, 'insertText', end, 'x');
    expect(outside.suppressed).toBe(0);
  });
  test('text put over a selection that holds it replaces the whole selection, its source included', () => {
    for (const [inputType, from, to, expected] of [
      ['insertText', 0, draft.length, 'z'],
      ['insertText', 2, start + 3, 'sez now'],
      ['insertText', start - 1, end + 1, 'seeznow'],
      ['insertFromPaste', 0, draft.length, 'z'],
      ['insertReplacementText', start, end, 'see z now'],
      ['insertFromDrop', start, end + 4, 'see z'],
    ] as const) {
      const f = fixture(draft, from);
      f.state.composerSelectionEnd = to;
      f.call('composerBeforeInput', [{value: draft, selectionStart: from, selectionEnd: to, isComposing: false, cancelable: true, inputType, data: 'z'}], {richComposer: {...f.projection, blocks: []}});
      expect(f.suppressed).toBe(1);
      settle(f);
      expect(f.commands).toHaveLength(1);
      expect([f.state.draft, f.state.composerSelectionStart]).toEqual([expected, expected.indexOf('z') + 1]);
    }
    const plain = fixture(draft, 0);
    plain.state.composerSelectionEnd = 3;
    plain.call('composerBeforeInput', [{value: draft, selectionStart: 0, selectionEnd: 3, isComposing: false, cancelable: true, inputType: 'insertText', data: 'z'}], {richComposer: {...plain.projection, blocks: []}});
    expect(plain.suppressed).toBe(0);
  });
  test('a quote typed over a selection wraps it; composing or a collapsed caret leaves the field\'s own typing', () => {
    const f = fixture('plain words', 0);
    f.state.composerSelectionEnd = 5;
    f.call('composerBeforeInput', [{value: 'plain words', selectionStart: 0, selectionEnd: 5, isComposing: false, cancelable: true, inputType: 'insertText', data: '"'}], {richComposer: {...f.projection, blocks: []}});
    expect(f.suppressed).toBe(1);
    settle(f);
    expect([f.state.draft, f.state.composerSelectionStart, f.state.composerSelectionEnd]).toEqual(['"plain" words', 1, 6]);
    for (const [from, to, composing] of [[0, 5, true], [5, 5, false]] as const) {
      const g = fixture('plain words', from);
      g.state.composerSelectionEnd = to;
      g.call('composerBeforeInput', [{value: 'plain words', selectionStart: from, selectionEnd: to, isComposing: composing, cancelable: true, inputType: 'insertText', data: '"'}], {richComposer: {...g.projection, blocks: []}});
      expect(g.suppressed).toBe(0);
    }
  });
  test('a caret inside it goes to the nearer edge of its shown label; a range takes it whole', () => {
    const moved = (from: number, to: number, toEnd = to) => {
      const f = fixture(draft, from);
      f.move(to, toEnd);
      return f.selections.map(args => args.slice(1, 3));
    };
    expect(moved(0, start + 2)).toEqual([[start, start]]);
    expect(moved(0, shownEnd - 2)).toEqual([[end, end]]);
    expect(moved(end, end - 2)).toEqual([[end, end]]);
    expect(moved(start, start + 1)).toEqual([[start, start]]);
    expect(moved(0, 1, start + 2)).toEqual([[1, end]]);
    expect(moved(end, start + 2, end)).toEqual([[start, end]]);
    expect(moved(0, end)).toEqual([]);
    expect(moved(0, 2)).toEqual([]);
  });
  test('an unmodified ← or → at its edge steps over it; ⇧, ⌥ and ↑/↓ stay the field\'s own', () => {
    const step = (k: string, caret: number, held = {}) => {
      const f = fixture(draft, caret);
      f.call('composerGrammarKey', key(k, held));
      return [f.suppressed, f.selections.map(args => args.slice(1, 3))];
    };
    expect(step('ArrowLeft', end)).toEqual([1, [[start, start]]]);
    expect(step('ArrowRight', start)).toEqual([1, [[end, end]]]);
    for (const [k, caret, held] of [['ArrowLeft', end, {shiftKey: true}], ['ArrowLeft', end, {altKey: true}], ['ArrowRight', end, {}], ['ArrowLeft', start, {}], ['ArrowUp', end, {}]] as const) {
      expect(step(k, caret, held)).toEqual([0, []]);
    }
  });
});
