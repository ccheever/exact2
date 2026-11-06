import { describe, expect, test } from 'bun:test';
import { composerFieldReplacement } from '../../src/composer/composer-field-edit';
import { applyComposerEdit } from '../../src/composer/composer-markdown-edit';
import type { ComposerFieldSnapshot } from '../../src/composer/composer-field-commands';
import type { ComposerMode } from '../../src/composer/composer-source-session';

function context(value: string, start = value.length, end = start, mode: ComposerMode = 'rich') {
  const snapshot: ComposerFieldSnapshot = { generation: 3, snapshotOwner: 'snapshot', owner: 'rendered-owner', value, start, end, composing: false, live: true };
  return { snapshot, draftOwner: 'selected-draft', mode, chips: [] };
}
function apply(request: unknown, state: ReturnType<typeof context>) {
  const result = composerFieldReplacement(request, state);
  if (result.kind !== 'edit') throw new Error(result.reason);
  return { result, value: applyComposerEdit(state.snapshot.value, { ...result.edit, selection: { start: result.edit.selectionStart, end: result.edit.selectionEnd } }) };
}

describe('composer app field command normalization', () => {
  test('whole-prompt compare/replace, range padding and caret retain native caller semantics', () => {
    const state = context('😀 text');
    const replaced = apply({ op: 'editorEdit', all: true, expect: state.snapshot.value, text: 'short', caret: 999, focus: false }, state);
    expect(replaced.value).toBe('short');
    expect(replaced.result.edit).toEqual({ generation: 3, snapshotOwner: 'snapshot', owner: 'rendered-owner', value: '😀 text', start: 0, end: 7, text: 'short', selectionStart: 5, selectionEnd: 5 });
    expect(replaced.result.focus).toBe(false);
    expect(apply({ op: 'editorEdit', text: '@file ', pad: true }, context('word')).value).toBe('word @file ');
    expect(apply({ op: 'editorEdit', all: true, text: 'short', caret: -5 }, state).result.edit.selectionStart).toBe(0);
    expect(composerFieldReplacement({ op: 'editorEdit', all: true, expect: 'older', text: '' }, state)).toEqual({ kind: 'refused', reason: 'changed' });
  });
  test('insert uses the retained caret without deleting a selected chip and consumes one existing space', () => {
    const insertion = apply({ op: 'editorInsert', text: '[context](t3-context://v1/thread/thread_a)', owner: 'snapshot' }, context('word selected rest', 5, 13));
    expect(insertion.value).toBe('word [context](t3-context://v1/thread/thread_a) selected rest');
    expect(insertion.result.edit.start).toBe(5);
    expect(insertion.result.edit.end).toBe(5);
    expect(apply({ op: 'editorInsert', text: '@file ' }, context('word rest', 4, 9)).value).toBe('word @file rest');
    expect(apply({ op: 'editorInsert', text: '@file' }, context('😀', 2)).value).toBe('😀 @file ');
  });
  test('menu replacements validate current trigger and carry only admitted Contract effects', () => {
    const state = context('/model ' , 6);
    const result = apply({ op: 'editorEdit', start: 0, end: 6, kind: 'slash-command', text: '', then: 'models', focus: false }, state);
    expect(result.value).toBe(' ');
    expect(result.result).toMatchObject({ then: 'models', focus: false });
    expect(apply({ op: 'editorEdit', start: 0, end: 5, kind: 'path', text: '@new ', extendSpace: true }, context('@path rest', 5)).value).toBe('@new rest');
    expect(composerFieldReplacement({ op: 'editorEdit', start: 0, end: 2, kind: 'path', text: '' }, context('@new', 4))).toEqual({ kind: 'refused', reason: 'changed' });
    expect(composerFieldReplacement({ op: 'editorEdit', then: 'arbitrary-native-target' }, state)).toEqual({ kind: 'refused', reason: 'field action invalid' });
  });
  test('plain fields keep token menus inside literal fence text while rich/literal modes guard them', () => {
    const value = '```\n@path', request = { op: 'editorEdit', start: 4, end: 9, kind: 'path', text: '@new ' };
    expect(apply(request, context(value, 9, 9, 'plain')).value).toBe('```\n@new ');
    for (const mode of ['rich', 'literal'] satisfies ComposerMode[]) {
      expect(composerFieldReplacement(request, context(value, 9, 9, mode))).toEqual({ kind: 'refused', reason: 'changed' });
    }
  });
  test('invalid boundary values, changed owners, composition and detached fields refuse before editing', () => {
    const state = context('text');
    for (const request of [null, { op: 'other' }, { op: 'editorEdit', text: 2 }, { op: 'editorEdit', start: -1 }, { op: 'editorEdit', end: 5 }, { op: 'editorEdit', caret: NaN }, { op: 'editorEdit', focus: 'true' }]) {
      expect(composerFieldReplacement(request, state).kind).toBe('refused');
    }
    expect(composerFieldReplacement({ op: 'editorInsert', owner: 'old-draft' }, state)).toEqual({ kind: 'refused', reason: 'another composer' });
    expect(composerFieldReplacement({ op: 'editorInsert', owner: state.draftOwner }, state)).toEqual({ kind: 'refused', reason: 'another composer' });
    expect(composerFieldReplacement({ op: 'editorEdit' }, { ...state, snapshot: { ...state.snapshot, composing: true } })).toEqual({ kind: 'refused', reason: 'field composing' });
    expect(composerFieldReplacement({ op: 'editorEdit' }, { ...state, snapshot: { ...state.snapshot, live: false } })).toEqual({ kind: 'refused', reason: 'no composer' });
  });
});
