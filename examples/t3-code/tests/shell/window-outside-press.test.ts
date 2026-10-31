// The window root's outside press (app-window.contract `outsidePressDown`). On macOS a press in a text area, and on a
// composer pill, reaches its ancestors' `pointerdown` as a web `<textarea>`'s does (exact2 #1105), so the root hears
// the presses Base UI's outside press hears.
import { describe, expect, test } from 'bun:test';
import { contractComponent, type Env } from '../support/contract-action-fixture';

const pill = { x: 100, y: 500, width: 60, height: 20 };
const popover = { x: 90, y: 380, width: 280, height: 110 };

/** One primary press at (x, y) with a skill chip's details open; what it closed. */
function press(x: number, y: number, kind = 'skill'): string[] {
  const window = contractComponent('app-window.contract', 'T3Window');
  const closed: string[] = [];
  const scope: Env = {
    interacted: () => {},
    chip: { open: true, seq: 3, kind, ...pill },
    chipCovered: false,
    frame: (id: string) => (id === 'chip-popover' ? popover : { x: 0, y: 0, width: 0, height: 0 }),
    prDetail: { actions: { armedAction: '', armedControl: '' } },
    detailsOpen: false,
    chipClose: (instructions: boolean) => closed.push(`chipClose ${instructions}`),
    citeSettle: () => closed.push('citeSettle'),
  };
  const state: Env = { outsideDown: false, keybindingField: '', citeFor: '', citeParked: '', chipClosed: 0 };
  window.call('outsidePressDown', state, scope, [{ buttons: 1, clientX: x, clientY: y }]);
  return closed;
}

describe("the window's outside press", () => {
  test("a press on the open chip's pill is its trigger's: the pill's atom press toggles the details", () => {
    expect(press(pill.x + 10, pill.y + 10)).toEqual([]);
    expect(press(pill.x + 10, pill.y + 10, 'citation')).toEqual([]);
  });

  test("a press elsewhere in the composer's text, or anywhere outside the popover, closes the details", () => {
    expect(press(pill.x + pill.width + 40, pill.y + 10)).toEqual(['chipClose false']);
    expect(press(pill.x + pill.width + 40, pill.y + 10, 'citation')).toEqual(['citeSettle']);
    expect(press(popover.x + 20, popover.y + 20)).toEqual([]);
  });
});
