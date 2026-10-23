// RightPanelTabs.tsx handleTabAuxClick (T3 Code 1e2ecbd975, MIT; see LICENSE-T3): the middle button closes the tab
// it goes down and comes up on. panel.contract PanelTabChip runs it on the tab's own pointerdown and pointerup.
import { describe, expect, test } from 'bun:test';
import { contractComponent } from '../support/contract-action-fixture';

const chip = contractComponent('panel.contract', 'PanelTabChip');
const box = { x: 100, y: 10, width: 80, height: 24 };

function scope(closed: string[][]) {
  return {
    tab: { id: 'files' }, begin: () => {},
    frame: (id: string) => (id === 'panel-tab-row-files' ? box : { x: 0, y: 0, width: 0, height: 0 }),
    ui: (op: string, id: string) => { closed.push([op, id]); },
  };
}

describe('the panel tab middle click', () => {
  test('a middle press released on the tab closes it', () => {
    const closed: string[][] = [], state = chip.state();
    chip.call('tabPointerDown', state, scope(closed), [{ buttons: 4 }]);
    chip.call('tabPointerUp', state, scope(closed), [{ buttons: 0, clientX: 140, clientY: 20 }]);
    expect(closed).toEqual([['tab-close', 'files']]);
    expect(state.middleDown).toBe(false);
  });

  test('released off the tab, it closes nothing', () => {
    const closed: string[][] = [], state = chip.state();
    chip.call('tabPointerDown', state, scope(closed), [{ buttons: 4 }]);
    chip.call('tabPointerUp', state, scope(closed), [{ buttons: 0, clientX: 200, clientY: 20 }]);
    expect(closed).toEqual([]);
  });

  test('the primary and secondary buttons close nothing', () => {
    for (const buttons of [1, 2, 5]) {
      const closed: string[][] = [], state = chip.state();
      chip.call('tabPointerDown', state, scope(closed), [{ buttons }]);
      chip.call('tabPointerUp', state, scope(closed), [{ buttons: 0, clientX: 140, clientY: 20 }]);
      expect(closed).toEqual([]);
    }
  });
});
