// ui/tooltip.tsx TooltipScrollDismissArea (T3 Code 1e2ecbd975, MIT; see LICENSE-T3): a transcript tooltip closes on the
// transcript's scroll, unless its trigger holds the focus, until the pointer moves again. shell-tip.contract TimelineTip
// and row-tips.contract RowTip keep the epoch they were hovered at; app-main.contract ChatColumn moves it on a scroll.
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { contractComponent } from '../support/contract-action-fixture';
import { appPath } from '../support/app-files';

const tip = contractComponent('shell-tip.contract', 'TimelineTip');
const row = contractComponent('row-tips.contract', 'RowTip');

const column = contractComponent('app-main.contract', 'ChatColumn');

/** The chat column's half: its epoch, its `tipArm`, and a scroll report from the transcript. */
function transcript() {
  const root: Record<string, unknown> = { tipEpoch: 0, tipArmed: false, firstKey: '', anchor: { owner: '' }, turnsSeen: [] };
  const scope = { citeScrolled: () => {}, restScrolled: () => {}, restGesture: () => {}, frame: () => ({ unavailable: true }), anchorKey: (key?: string) => key ?? '', composerOwner: 'o', commandPending: false, turnIds: [] };
  return {
    root: root as { tipEpoch: number; tipArmed: boolean },
    tipArm: () => column.call('tipArm', root, scope, []),
    scroll: () => column.call('transcriptScrolled', root, scope, [0, 120, { firstVisibleKey: undefined, firstVisibleOffset: 0, scrollTop: 120, scrollHeight: 2000, clientHeight: 600 }]),
  };
}

describe('a transcript tooltip and the transcript\'s scroll', () => {
  test('a scroll closes the hovered tip; the pointer moving reopens it', () => {
    const t = transcript(), state = tip.state({ tipEpoch: t.root.tipEpoch });
    const scope = () => ({ tipEpoch: t.root.tipEpoch, tipArm: t.tipArm });
    tip.call('setOver', state, scope(), [true]);
    expect(state.seen).toBe(t.root.tipEpoch);
    t.scroll();
    expect(state.seen === t.root.tipEpoch).toBe(false);
    t.scroll(); // nothing armed: the epoch stays
    expect(t.root.tipEpoch).toBe(1);
    tip.call('moved', state, scope(), []);
    expect(state.seen).toBe(1);
    expect(t.root.tipArmed).toBe(true);
  });

  test('a focused trigger keeps its tip through a scroll, and leaving it does not close it', () => {
    const t = transcript(), state = tip.state({ tipEpoch: 0 });
    const scope = () => ({ tipEpoch: t.root.tipEpoch, tipArm: t.tipArm });
    tip.call('setOver', state, scope(), [true]);
    tip.call('focusEntered', state, scope(), [{ target: 'x' }]);
    t.scroll();
    expect(state.focused).toBe(true);
    tip.call('focusLeft', state, scope(), [{ target: '' }]);
    expect(state.seen).toBe(t.root.tipEpoch);
  });

  test('a row tip shown without a hover (its row focused) starts open at the current epoch', () => {
    const state = row.state({ tipEpoch: 3 });
    expect(state.seen).toBe(3);
  });

  test('each tip shows only while its epoch is the transcript\'s, or its trigger has the focus', () => {
    const source = readFileSync(appPath('shell-tip.contract'), 'utf8') + readFileSync(appPath('row-tips.contract'), 'utf8');
    expect(source).toContain('shown=(over and (focused or seen == tipEpoch))');
    expect(source).toContain('shown=(shown and (focused or seen == tipEpoch))');
    expect(source).toContain('shown=(over and seen == tipEpoch)');
    expect(source).not.toContain('hatch="t3-timeline-tip"');
  });
});
