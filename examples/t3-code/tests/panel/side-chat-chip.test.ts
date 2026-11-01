// A pill pressed in a side chat's field opens its details as one pressed in the window's does (T3 Code 2359643cff, MIT,
// see LICENSE-T3: SideChatPane embeds ChatView, whose ChatComposer wraps each chip in its ContextChipPopover). The window
// draws the one details popover; `chipTarget` names the composer it serves (composer-members.contract).
import { describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { threadScope } from '../../src/client/thread-scope';
import { composerFieldOwner } from '../../src/composer/auto-balance-owner';
import { composerChipCurrent, composerChipOpen } from '../../src/composer/composer-chip-bridge';
import { composerFieldObserved } from '../../src/composer/composer-field-bridge';
import { composerSourceSession } from '../../src/composer/composer-rich-state';
import { chipScopedOp, noteChipTarget } from '../../src/panel/side-chat-thread';
import { contractComponent, type Env } from '../support/contract-action-fixture';

const root = contractComponent('app.contract', 'T3Code');

/** A field of `host` holding `value`, observed as its events report it, and a press on its chip `index`. */
function field(host: T3Client, value: string) {
  const owner = composerFieldOwner(host);
  const snapshot = { generation: host.generation, snapshotOwner: host.snapshotOwner, owner, value, start: 0, end: 0, composing: false, focused: true, live: true, inputSerial: 0 };
  composerFieldObserved(host, [snapshot, false], owner);
  const session = composerSourceSession(host);
  session.observe(owner, value);
  return (index = 0) => composerChipOpen(host, { ...snapshot, ...session.chips[index]!, surface: 'composer', comment: false, frame: { unavailable: false, x: 900, y: 600, width: 80, height: 18 } });
}

describe('the side chat\'s pills (composer-chip-bridge.ts on its scope)', () => {
  test('a press in the side chat\'s field opens on its own field, numbered after every press in either', () => {
    const client = new T3Client(), scope = threadScope(client, 's1');
    const window = field(client, '$review $other '), side = field(scope, '$skill ');
    const first = window(0)!, second = side()!, third = window(1)!;
    expect(second).toMatchObject({ kind: 'skill', name: 'skill', open: true });
    expect([second.seq > first.seq, third.seq > second.seq]).toEqual([true, true]);
    // Each field keeps its own press: the window's newest does not close the side chat's.
    expect(composerChipCurrent(scope)?.seq).toBe(second.seq);
    expect(composerChipCurrent(client)?.seq).toBe(third.seq);
  });
});

/** The root's state with a side chat's field focused, and the calls its actions make. */
function sideField() {
  const calls: unknown[][] = [];
  const record = (name: string) => (...args: unknown[]) => { calls.push([name, ...args]); return {}; };
  const chips = [{ start: 0, end: 6, source: '$skill', kind: 'skill' }];
  const state: Env = { chipTarget: '', composerFieldInputSerial: 4, composerComposing: false, commandPending: false };
  const scope: Env = {
    sideChatThread: 's1', modal: false,
    sideChat: { snapshotOwner: 'snap', requestMode: '', canSend: true, composer: { subagent: false } },
    sideRich: { owner: 'side-owner', value: '$skill ', generation: 2, chips }, sideRichOwner: 'side-owner', sideText: '$skill ',
    data: { snapshotOwner: 'snap', requestMode: '', canSend: true, composer: { subagent: false } },
    richComposer: { owner: 'main-owner', value: 'main', generation: 1, chips: [] }, richComposerOwner: 'main-owner', composerText: 'main',
    chip: { act: '', seq: 9 }, wallTime: { epochAtZero: 0 }, performanceNow: () => 5,
    rangeFrame: (id: string) => ({ unavailable: id !== 'side-chat-composer', x: 900, y: 600, width: 50, height: 18 }), frame: () => ({ width: 300, unavailable: false }),
    ComposerChipRequest: (fields: Env) => fields, composerChipPressed: record('composerChipPressed'), command: record('command'),
    focus: record('focus'), setSelectionRange: record('setSelectionRange'), sideCommand: record('sideCommand'),
    querySelectorAll: () => [],
  };
  return { state, scope, calls, run: (action: string, args: unknown[], extra: Env = {}) => root.call(action, state, { ...scope, ...extra }, args) };
}

describe('the window\'s details popover serves the composer whose pill was pressed (chipTarget)', () => {
  test('a press in the side chat\'s field asks the side chat\'s scope, anchored to the pill', () => {
    const f = sideField();
    f.run('sideChipPress', ['side-owner', '$skill ', 4, 0, 6, '$skill', false]);
    expect(f.state.chipTarget).toBe('s1');
    const [, request, target] = f.calls.find(call => call[0] === 'composerChipPressed')!;
    expect(target).toBe('s1');
    expect(request).toMatchObject({ generation: 2, owner: 'side-owner', value: '$skill ', start: 0, end: 6, surface: 'composer', frame: { x: 900, y: 600 } });
    // A stale press (the field changed since) asks nothing.
    const stale = sideField();
    stale.run('sideChipPress', ['side-owner', '$skill x', 4, 0, 6, '$skill', false]);
    expect([stale.state.chipTarget, stale.calls.filter(call => call[0] === 'composerChipPressed')]).toEqual(['', []]);
  });
  test('the popover\'s close, View instructions, comment and actions run on that composer; the window\'s overlays stay the window\'s', () => {
    const client = new T3Client();
    noteChipTarget(client, 's1'); // the root's chipTarget, as the composerChip resource asks
    expect(['chip-close', 'chip-instructions', 'chip-cite-save', 'chip-act', 'chip-image-close', 'chip-file', 'stash-restore'].map(op => chipScopedOp(client, `editorlocal:${op}`))).toEqual([
      'scope:s1|editorlocal:chip-close', 'scope:s1|editorlocal:chip-instructions', 'scope:s1|editorlocal:chip-cite-save', 'scope:s1|editorlocal:chip-act',
      'editorlocal:chip-image-close', 'editorlocal:chip-file', 'editorlocal:stash-restore']);
    noteChipTarget(threadScope(client, 's1'), '');
    expect(chipScopedOp(client, 'editorlocal:chip-close')).toBe('editorlocal:chip-close');
    // A press in the window's field gives the window's composer the details back.
    const f = sideField();
    f.state.chipTarget = 's1';
    f.run('composerChipPress', ['composer', 'main-owner', 'main', 4, 0, 1, 'm', false], { composerFieldLive: true });
    expect(f.state.chipTarget).toBe('');
  });
  test('the details\' finish gives the side chat\'s field its focus and selection back', () => {
    const f = sideField();
    f.state.chipTarget = 's1';
    const item = { seq: 9, surface: 'composer', generation: 2, snapshotOwner: 'snap', owner: 'side-owner', value: '$skill ', inputSerial: 4, focus: 'select', start: 0, end: 6, send: false };
    f.run('chipComplete', [item, ''], { chip: { finish: item, seq: 9 }, chipLive: true });
    expect(f.calls.filter(call => call[0] === 'focus' || call[0] === 'setSelectionRange')).toEqual([['focus', 'side-chat-composer'], ['setSelectionRange', 'side-chat-composer', 0, 6]]);
  });
});
