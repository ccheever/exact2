// The font picker's search (FontFamilyPicker.tsx: a Base UI Combobox with `autoHighlight`, its ComboboxSearchInput in the
// popup): the field has the focus as the popup opens, typing lights the first match, ↓/↑ walk the matches and past either
// end back to none (loopFocus), and Return picks the lit family and closes the popup with the focus on its trigger. The
// popup opens on the family in use (Base UI's AriaCombobox syncs selectedIndex while closed, useListNavigation makes it
// the activeIndex on open and scrolls it into view).
import { describe, expect, test } from 'bun:test';
import { contractComponent, type Env } from '../support/contract-action-fixture';
import { contractFns } from '../support/contract-fn-fixture';

const picker = contractComponent('settings-a-fonts.contract', 'FontFamilyPicker');
const { dbStep } = contractFns('diff.contract');
const families = [
  { id: 'default', label: 'SF Pro', value: '', detail: '', selected: true },
  { id: 'avenir', label: 'Avenir', value: 'Avenir', detail: 'Avenir', selected: false },
  { id: 'georgia', label: 'Georgia', value: 'Georgia', detail: 'Georgia', selected: false },
  { id: 'geneva', label: 'Geneva', value: 'Geneva', detail: 'Geneva', selected: false },
];

function harness(list = families) {
  const calls: string[] = [];
  const scope: Env = { rowId: 'interface', families: list, dbStep,
    preventDefault: () => calls.push('preventDefault'), scrollIntoView: (id: string) => calls.push(`scroll ${id}`),
    hidePopover: (id: string) => calls.push(`hide ${id}`), focus: (id: string) => calls.push(`focus ${id}`), choose: (value: string) => calls.push(`choose ${value}`) };
  const state = picker.state();
  const lit = () => (picker.derives(state, scope).highlight as string);
  const act = (name: string, ...args: unknown[]): unknown => picker.call(name, state, { ...scope, ...picker.derives(state, scope) }, args);
  scope.pickRow = (id: string) => act('pickRow', id); // the component's own action, as `pick` calls it
  return { calls, state, lit, act };
}

describe('the font picker\'s search', () => {
  test('typing lights the first match; ↓/↑ walk the matches and loop through none', () => {
    const { lit, act, calls } = harness();
    expect(lit()).toBe('-'); // no query: nothing lit until a key or the pointer
    act('search', 'ge');
    expect(lit()).toBe('georgia');
    act('searchKey', 'ArrowDown');
    expect(lit()).toBe('geneva');
    expect(calls).toContain('scroll font-picker-interface-option-geneva');
    act('searchKey', 'ArrowDown');
    expect(lit()).toBe('-');
    act('searchKey', 'ArrowUp');
    expect(lit()).toBe('geneva');
    act('search', 'av');
    expect(lit()).toBe('avenir'); // an edit drops what the keys lit
  });

  test('Return picks the lit family and gives the focus back to the trigger; with none lit it does nothing', () => {
    const empty = harness();
    empty.act('pick');
    expect(empty.calls).toEqual([]);
    const { act, calls } = harness();
    act('search', 'geor');
    act('pick');
    expect(calls).toEqual(['hide font-picker-interface', 'focus setting-interface-family', 'choose Georgia']);
  });

  test('the pointer lights the row under it', () => {
    const { lit, act } = harness();
    act('hoverRow', 'avenir', true);
    expect(lit()).toBe('avenir');
    act('hoverRow', 'avenir', false);
    expect(lit()).toBe('-');
  });

  test('opens on the family in use, scrolled into view; clearing the search keeps the lit place', () => {
    const inUse = families.map(family => ({ ...family, selected: family.id === 'georgia' }));
    const { lit, act, calls } = harness(inUse);
    act('open');
    expect(lit()).toBe('georgia');
    act('toggled', { newState: 'open' });
    expect(calls).toEqual(['scroll font-picker-interface-option-georgia']);
    act('searchKey', 'ArrowDown');
    expect(lit()).toBe('geneva');
    act('search', 'a');
    expect(lit()).toBe('avenir');
    act('search', '');
    expect(lit()).toBe('default'); // Avenir was the first match: the first row of the full list
  });

  test('a pointer pick closes the popup and gives the focus back to the trigger', () => {
    const { act, calls } = harness();
    act('pickRow', 'avenir');
    expect(calls).toEqual(['hide font-picker-interface', 'focus setting-interface-family', 'choose Avenir']);
  });
});
