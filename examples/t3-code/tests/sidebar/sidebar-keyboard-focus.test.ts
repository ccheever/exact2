// The sidebar under the keyboard, as the reference's: its header and footer buttons are Tab stops, and a visible focus
// opens the tooltip or the row's hover card that a hover would, without the row's hover actions.
import { describe, expect, test } from 'bun:test';
import { contractComponent, value } from '../support/contract-action-fixture';
import { appPath } from '../support/app-files';

const source = (file: string) => Bun.file(appPath(file)).text();

describe('the sidebar under the keyboard', () => {
  test('the header and footer buttons are Tab stops with a focus ring', async () => {
    const sidebar = await source('sidebar.contract');
    for (const id of ['filter-project', 'add-project', 'new-thread', 'connection-settings', 'open-pull-requests', 'open-usage']) {
      const line = sidebar.split('\n').find(row => row.includes(`testId="${id}"`) && row.trimStart().startsWith('button '))!;
      expect(line).toEndWith('tabindex=0');
      const next = sidebar.split('\n')[sidebar.split('\n').indexOf(line) + 1]!;
      expect(next.trim()).toBe(':focus-visible');
    }
    expect(await source('sidebar-icons-kit.contract')).toContain('justify-content="center" title="" tabindex=0\n      :focus-visible');
  });

  test('a focus after a key opens the row card, keeps its actions hidden, and closes it on blur', async () => {
    const pointerOver = (await source('sidebar-row.contract')).match(/^  derive pointerOver = (.+)$/m)![1]!;
    const row = contractComponent('sidebar-row.contract', 'ThreadRow'), state = row.state();
    const calls: unknown[][] = [];
    const scope = { t: { hoverKey: 'env:t1' }, hoverCard: (...args: unknown[]) => calls.push(args), keyboardLast: true };
    row.call('keyCard', state, scope, [true]);
    expect(calls).toEqual([['env:t1', true]]);
    expect(value(pointerOver, { ...state, hovering: true })).toBe(false);
    row.call('keyCard', state, scope, [false]);
    expect(calls).toEqual([['env:t1', true], ['env:t1', false]]);
    // The pointer arriving takes the card over: its actions show.
    row.call('keyCard', state, scope, [true]);
    row.call('rowHover', state, scope, [true]);
    expect(value(pointerOver, { ...state, hovering: true })).toBe(true);
  });

  test('a focus after a press opens nothing', () => {
    const row = contractComponent('sidebar-row.contract', 'ThreadRow'), state = row.state();
    const calls: unknown[][] = [];
    row.call('keyCard', state, { t: { hoverKey: 'env:t1' }, hoverCard: (...args: unknown[]) => calls.push(args), keyboardLast: false }, [true]);
    expect(calls).toEqual([]);
    const tip = contractComponent('sidebar-row.contract', 'SidebarTip'), tipState = tip.state();
    tip.call('keyTip', tipState, { tip: 'sidebar-filter', hoverCard: (...args: unknown[]) => calls.push(args), keyboardLast: false }, [true]);
    expect(calls).toEqual([]);
    tip.call('keyTip', tipState, { tip: 'sidebar-filter', hoverCard: (...args: unknown[]) => calls.push(args), keyboardLast: true }, [true]);
    expect(calls).toEqual([['sidebar-filter', true]]);
  });
});
