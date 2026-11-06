import {describe, expect, test} from 'bun:test';
import { appText } from '../support/app-files';

import {contractComponent, value, type Env} from '../support/contract-action-fixture';
import { windowField } from '../support/window-field';

const root = contractComponent('app.contract', 'T3Code');
const lines = appText('app.contract').split('\n');
const derived = lines.find(line => line.startsWith('  derive composerFieldFocusCurrent = '))!.split(' = ')[1]!;
const task = lines.find(line => line.startsWith('  task composerFieldRestoreFocus when '))!;
const admission = task.slice(task.indexOf(' when ') + 6, task.lastIndexOf(' key='));
function fixture() {
  const state: Env = {
    composerFieldFocusDone: 0, composerFieldEffects: 1,
    composerFieldFocus: {seq: 1, generation: 3, snapshotOwner: 'snapshot', owner: 'draft', value: 'after', start: 2, end: 2, inputSerial: 5, focus: 'next-image', removedImage: 'removed'},
  };
  const focus: string[] = [];
  const env: Env = {
    data: {snapshotOwner: 'snapshot', snapshotDrafts: [{id: 'removed'}, {id: 'next'}]},
    richComposer: {generation: 3}, richComposerOwner: 'draft', composerText: 'after', composerComposing: false,
    composerFieldInputSerial: 5, composerSelectionCurrent: true, composerSelectionStart: 2, composerSelectionEnd: 2,
    composerFieldLive: true, frame: () => ({width: 20}), focus: (id: string) => focus.push(id),
  };
  const scope = () => {
    const current: Env = {...env, ...state};
    Object.assign(current, windowField(current));
    current.composerFieldFocusCurrent = value(derived, current);
    return current;
  };
  return {state, env, focus, turn() {
    const current = scope();
    if (!value(admission, current)) return false;
    root.call('composerFieldFocusRun', state, current);
    return true;
  }, ack(result: Env) {root.call('composerFieldAckDone', state, {...scope(), composerFieldAck: result});}};
}

describe('confirmed image removal focus through the actual root actions', () => {
  test('waits for removed image to leave current data, then focuses once', () => {
    const f = fixture();
    expect(f.turn()).toBe(false);
    expect(f.focus).toEqual([]);
    f.env.data.snapshotDrafts = [{id: 'next'}];
    expect(f.turn()).toBe(true);
    expect(f.focus).toEqual(['next-image']);
    expect(f.turn()).toBe(false);
  });
  for (const [name, change] of [
    ['source', (env: Env) => {env.composerText = 'new manual input';}],
    ['owner', (env: Env) => {env.richComposerOwner = 'another-draft';}],
    ['snapshot', (env: Env) => {env.data.snapshotOwner = 'another-snapshot';}],
    ['generation', (env: Env) => {env.richComposer.generation = 4;}],
    ['composition', (env: Env) => {env.composerComposing = true;}],
    ['cover', (env: Env) => {env.composerFieldLive = false;}],
  ] as const) test(`a changed ${name} retires focus without moving it`, () => {
    const f = fixture(); change(f.env);
    expect(f.turn()).toBe(true);
    expect(f.state.composerFieldFocusDone).toBe(1);
    expect(f.focus).toEqual([]);
    f.env.data.snapshotDrafts = [];
    expect(f.turn()).toBe(false);
  });
  test('same-value input ABA cannot restore late focus', () => {
    const f = fixture(); f.env.data.snapshotDrafts = []; f.env.composerFieldInputSerial = 6;
    expect(f.turn()).toBe(true);
    expect(f.focus).toEqual([]);
  });
  test('a newer ACK retires older pending focus', () => {
    const f = fixture(); f.env.data.snapshotDrafts = []; f.state.composerFieldEffects = 2;
    expect(f.turn()).toBe(true);
    expect(f.focus).toEqual([]);
  });
  test('moving the caret while waiting preserves the new selection', () => {
    const f = fixture(); f.env.data.snapshotDrafts = []; f.env.composerSelectionStart = 4; f.env.composerSelectionEnd = 4;
    expect(f.turn()).toBe(true);
    expect(f.focus).toEqual([]);
  });
  test('an unmounted target is refused', () => {
    const f = fixture(); f.env.data.snapshotDrafts = []; f.env.frame = () => ({width: 0});
    expect(f.turn()).toBe(true);
    expect(f.focus).toEqual([]);
  });
  test('only an applied current ACK stores the deferred focus request', () => {
    const f = fixture();
    f.ack({applied: true, seq: 2, generation: 3, snapshotOwner: 'snapshot', owner: 'draft', value: 'after', start: 2, end: 2, inputSerial: 5, focus: 'next-image', removedImage: 'removed', send: false, then: ''});
    expect(f.state.composerFieldEffects).toBe(2);
    expect(f.state.composerFieldFocus.seq).toBe(2);
    expect(f.turn()).toBe(false);
    f.env.data.snapshotDrafts = [];
    expect(f.turn()).toBe(true);
    expect(f.focus).toEqual(['next-image']);
  });
  for (const [name, change] of [
    ['source', (env: Env) => {env.composerText = 'new input';}],
    ['serial', (env: Env) => {env.composerFieldInputSerial = 6;}],
    ['selection', (env: Env) => {env.composerSelectionStart = 3; env.composerSelectionEnd = 3;}],
  ] as const) test(`returned ACK cannot open controls after a ${name} change`, () => {
    const f = fixture(); change(f.env); f.env.openModels = () => f.focus.push('models');
    f.ack({applied: true, seq: 2, generation: 3, snapshotOwner: 'snapshot', owner: 'draft', value: 'after', start: 2, end: 2, inputSerial: 5, focus: '', removedImage: '', send: false, then: 'models'});
    expect(f.state.composerFieldEffects).toBe(1);
    expect(f.focus).toEqual([]);
  });
  test('queued admission cannot publish focus through the ACK handler', () => {
    const f = fixture(), previous = f.state.composerFieldFocus;
    f.ack({applied: false, seq: 2, generation: 3, snapshotOwner: 'snapshot', owner: 'draft', value: 'after', focus: 'queued-target', removedImage: 'removed'});
    expect(f.state.composerFieldEffects).toBe(1);
    expect(f.state.composerFieldFocus).toBe(previous);
  });
});
