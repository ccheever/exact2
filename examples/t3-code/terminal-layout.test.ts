// New tests for the decisions terminal-layout.ts extracts from T3 Code 1e2ecbd975 ThreadTerminalDrawer.tsx
// (:1212-1268 and the tab list :1588-1710); the reference has no unit test for them.
import { describe, expect, test } from 'bun:test';
import { terminalGroupLabel, terminalLayout, terminalSplitLabel, terminalTabs } from './terminal-layout';

const groups = [
  { id: 'group-term-1', terminalIds: ['term-1', 'term-3'], splitDirection: 'vertical' as const },
  { id: 'group-term-2', terminalIds: ['term-2'] },
];
const label = (id: string) => `Terminal ${id.slice(5)}`;

describe('terminalLayout', () => {
  test('one terminal: no tab list, no headers, the floating toolbar', () => {
    expect(terminalLayout(['term-1'], [{ id: 'group-term-1', terminalIds: ['term-1'] }], 'term-1', 'group-term-1'))
      .toEqual({ active: 'term-1', visible: ['term-1'], direction: 'row', showTabs: false, showHeaders: false, splitDisabled: false });
  });
  test('the active group decides the visible panes and their direction', () => {
    expect(terminalLayout(['term-1', 'term-3', 'term-2'], groups, 'term-3', 'group-term-1'))
      .toMatchObject({ visible: ['term-1', 'term-3'], direction: 'column', showTabs: true, showHeaders: true });
    expect(terminalLayout(['term-1', 'term-3', 'term-2'], groups, 'term-2', 'group-term-2')).toMatchObject({ visible: ['term-2'], direction: 'row' });
  });
  test('an unknown group falls back to the group holding the active terminal, then the first', () => {
    expect(terminalLayout(['term-1', 'term-3', 'term-2'], groups, 'term-2', 'gone').visible).toEqual(['term-2']);
    expect(terminalLayout(['term-1', 'term-3', 'term-2'], groups, 'gone', 'gone')).toMatchObject({ active: 'term-1', visible: ['term-1', 'term-3'] });
  });
  test('two groups of one show headers; four panes reach the split limit', () => {
    expect(terminalLayout(['a', 'b'], [{ id: 'a', terminalIds: ['a'] }, { id: 'b', terminalIds: ['b'] }], 'a', 'a').showHeaders).toBe(true);
    expect(terminalLayout(['a', 'b', 'c', 'd'], [{ id: 'a', terminalIds: ['a', 'b', 'c', 'd'] }], 'a', 'a').splitDisabled).toBe(true);
  });
});

describe('terminal action labels', () => {
  test('split labels carry the shortcut, or the limit at four', () => {
    expect(terminalSplitLabel(false, false, '⌘D')).toBe('Split Terminal Horizontally (⌘D)');
    expect(terminalSplitLabel(true, false, '')).toBe('Split Terminal Vertically');
    expect(terminalSplitLabel(false, true, '⌘D')).toBe('Split Terminal Horizontally (max 4 per group)');
    expect(terminalSplitLabel(true, true, '⇧⌘D')).toBe('Split Terminal Vertically (max 4 per group)');
  });
  test('group headers read Single, Stacked, Side by side', () => {
    expect(terminalGroupLabel({ terminalIds: ['a'], splitDirection: 'vertical' })).toEqual({ label: 'Single', icon: 'square' });
    expect(terminalGroupLabel({ terminalIds: ['a', 'b'], splitDirection: 'vertical' })).toEqual({ label: 'Stacked', icon: 'split-vertical' });
    expect(terminalGroupLabel({ terminalIds: ['a', 'b'] })).toEqual({ label: 'Side by side', icon: 'split-horizontal' });
  });
});

describe('terminalTabs', () => {
  test('one row per terminal, the header and count on each group\'s first row', () => {
    const tabs = terminalTabs(groups, 'term-3', true, 'env:thread', label, '⌘W');
    expect(tabs.map(tab => [tab.id, tab.heading, tab.count, tab.active, tab.groupActive])).toEqual([
      ['term-1', 'Stacked', 2, false, true], ['term-3', '', 2, true, true], ['term-2', 'Single', 1, false, false]]);
    expect(tabs[0]).toMatchObject({ groupIcon: 'split-vertical', groupTarget: 'env:thread|term-3', target: 'env:thread|term-1' });
    expect(tabs[2]).toMatchObject({ groupTarget: 'env:thread|term-2', closeTitle: 'Close terminal "Terminal 2"?' });
  });
  test('only the active row names the close shortcut; no headers when hidden', () => {
    const tabs = terminalTabs(groups, 'term-3', false, 'k', label, '⌘W');
    expect(tabs.map(tab => tab.closeLabel)).toEqual(['Close Terminal 1', 'Close Terminal 3 (⌘W)', 'Close Terminal 2']);
    expect(tabs.every(tab => tab.heading === '')).toBe(true);
    expect(terminalTabs(groups, 'term-3', true, 'k', label, '')[1]?.closeLabel).toBe('Close Terminal 3');
  });
});
