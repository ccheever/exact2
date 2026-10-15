// The font picker's search (FontFamilyPicker.tsx: a Base UI Combobox with `autoHighlight`, its ComboboxSearchInput in the
// popup): the field has the focus as the popup opens, typing lights the first match, ↓/↑ walk the matches and past either
// end back to none (loopFocus), and Return picks the lit family and closes the popup with the focus on its trigger.
import { describe, expect, test } from 'bun:test';
import { contractComponent, type Env } from './contract-action-fixture';
import { contractFns } from './contract-fn-fixture';

const picker = contractComponent('settings-a-fonts.contract', 'FontFamilyPicker');
const { dbStep } = contractFns('diff.contract');
const families = [
  { id: 'default', label: 'SF Pro', value: '', detail: '', selected: true },
  { id: 'avenir', label: 'Avenir', value: 'Avenir', detail: 'Avenir', selected: false },
  { id: 'georgia', label: 'Georgia', value: 'Georgia', detail: 'Georgia', selected: false },
  { id: 'geneva', label: 'Geneva', value: 'Geneva', detail: 'Geneva', selected: false },
];

function harness() {
  const calls: string[] = [];
  const scope: Env = { rowId: 'interface', families, dbStep,
    preventDefault: () => calls.push('preventDefault'), scrollIntoView: (id: string) => calls.push(`scroll ${id}`),
    hidePopover: (id: string) => calls.push(`hide ${id}`), focus: (id: string) => calls.push(`focus ${id}`), choose: (value: string) => calls.push(`choose ${value}`) };
  const state = picker.state();
  const lit = () => (picker.derives(state, scope).highlight as string);
  const act = (name: string, ...args: unknown[]) => picker.call(name, state, { ...scope, ...picker.derives(state, scope) }, args);
  return { calls, state, lit, act };
}

describe('the font picker\'s search', () => {
  test('has the focus as the popup opens (autofocus inside the popover) and owns the list keys', async () => {
    const source = await Bun.file(new URL('./settings-a-fonts.contract', import.meta.url)).text();
    const field = source.split('\n').find(line => line.includes('testId=`font-picker-${rowId}-search`'))!;
    expect(field).toContain('key=searchKey submit=pick autofocus=true role="combobox"');
  });

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
});
