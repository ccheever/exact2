// Return presses a focused timeline or header button, as Chromium's `<button>` takes Enter in the reference. The Mac
// presses a focused button with Space only (LLP 1115.000 D7.3), so each button hears Return in a `key` handler.
import { describe, expect, test } from 'bun:test';
import { readdirSync } from 'node:fs';
import { join } from 'node:path';
import { appText } from '../support/app-files';
import { contractComponent, contractFns, type Env } from '../support/contract-action-fixture';

const files = ['timeline', 'media'].flatMap(dir =>
  readdirSync(join(import.meta.dir, '../../src', dir)).filter(name => name.endsWith('.contract')).map(name => `src/${dir}/${name}`));
const keys = contractFns('src/shared/menu-keys.contract');
const key = (k: string, mods: Partial<Record<'metaKey' | 'shiftKey' | 'ctrlKey' | 'altKey' | 'isComposing', boolean>> = {}) =>
  ({ key: k, metaKey: false, shiftKey: false, ctrlKey: false, altKey: false, isComposing: false, repeat: false, code: '', ...mods });

describe('Return on a timeline button', () => {
  test('every Tab stop among the timeline\'s buttons hears keys', () => {
    const missing = files.flatMap(file => appText(file).split('\n')
      .filter(line => /^\s*button /.test(line) && line.includes('tabindex=0') && !line.includes('role="menuitem') && !/\bkey=/.test(line))
      .map(line => `${file}: ${line.trim().slice(0, 80)}`));
    expect(missing).toEqual([]);
  });

  test("Copy message: Return presses it once and goes no further; ⌘Return, ⇧Return and a candidate's Return do not", () => {
    const copy = contractComponent('timeline-work.contract', 'CopyButton');
    const calls: unknown[][] = [];
    const spy = (name: string) => (...args: unknown[]) => { calls.push([name, ...args]); };
    const scope: Env = { ...keys, local: spy('local'), preventDefault: spy('preventDefault'), stopPropagation: spy('stopPropagation') };
    for (const event of [key('Enter', { metaKey: true }), key('Enter', { shiftKey: true }), key('Enter', { isComposing: true }), key(' ')])
      copy.call('localOnEnter', {}, scope, ['copy', 'm1', '', event.key, event]);
    expect(calls).toEqual([]);
    copy.call('localOnEnter', {}, scope, ['copy', 'm1', '', 'Enter', key('Enter')]);
    expect(calls).toEqual([['preventDefault'], ['stopPropagation'], ['local', 'copy', 'm1', '']]);
  });

  test('a popover trigger chip opens its popover on Return', () => {
    const chip = contractComponent('skill-chip.contract', 'SkillChip');
    const calls: unknown[][] = [];
    const spy = (name: string) => (...args: unknown[]) => { calls.push([name, ...args]); };
    chip.call('invokerOnEnter', {}, { ...keys, id: 'r1', popupId: 'skill-chip-r1', preventDefault: spy('preventDefault'), stopPropagation: spy('stopPropagation'),
      togglePopover: spy('togglePopover') }, ['Enter', key('Enter')]);
    expect(calls.map(call => call[0])).toEqual(['preventDefault', 'stopPropagation', 'togglePopover']);
    expect(calls[2]![1]).toBe('skill-chip-r1');
  });
});

// ChatView's window focus listener and shouldRefocusComposerOnWindowFocus at 5d3d78ab93.
describe('the window gets key again', () => {
  const members = contractComponent('shell-members.contract', 'T3Code');
  const refocused = (state: Env, scope: Env = {}) => {
    const focused: unknown[] = [];
    members.call('composerOnWindowFocus', { focusField: false, focusMenu: false, focusByKey: false, workspaceArea: 'conversation', ...state }, {
      data: { threadId: 't1' }, terminalDrawer: { open: false }, modal: false, pageCover: false, mediaWidth: 1280,
      querySelectorAll: (selector: string) => (selector.includes('focus-keeps') ? ['browser-frame-1'] : []),
      focus: (id: string) => { focused.push(id); }, ...scope,
    }, []);
    return focused.length > 0;
  };

  test('a timeline button, tabbed to or clicked, gives the composer the focus', () => {
    expect(refocused({ focusTarget: 'copy-m1', focusByKey: true })).toBe(true);
    expect(refocused({ focusTarget: 'copy-m1' })).toBe(true);
    expect(refocused({ focusTarget: '' })).toBe(true);
  });

  test('a field, a frame, a menu, a dialog, the terminal, the side chat or a keyboard-focused sidebar row keeps it', () => {
    expect(refocused({ focusTarget: 'search-threads', focusField: true })).toBe(false);
    expect(refocused({ focusTarget: 'browser-frame-1', workspaceArea: 'panel' })).toBe(false);
    expect(refocused({ focusTarget: 'plan-menu-item-0', focusMenu: true })).toBe(false);
    expect(refocused({ focusTarget: 'x' }, { querySelectorAll: (selector: string) => (selector.includes('dialog-focus') ? ['d'] : []) })).toBe(false);
    expect(refocused({ focusTarget: 'terminal-page', workspaceArea: 'terminal' })).toBe(false);
    expect(refocused({ focusTarget: 'side-chat-bring-back', workspaceArea: 'panel' })).toBe(false);
    expect(refocused({ focusTarget: 'thread-t2', workspaceArea: 'sidebar', focusByKey: true })).toBe(false);
    expect(refocused({ focusTarget: 'thread-t2', workspaceArea: 'sidebar' })).toBe(true);
  });

  test('not without a thread, with the terminal drawer open, under a modal or in a phone-wide window', () => {
    expect(refocused({ focusTarget: 'copy-m1' }, { data: { threadId: '' } })).toBe(false);
    expect(refocused({ focusTarget: 'copy-m1' }, { terminalDrawer: { open: true } })).toBe(false);
    expect(refocused({ focusTarget: 'copy-m1' }, { modal: true })).toBe(false);
    expect(refocused({ focusTarget: 'copy-m1' }, { mediaWidth: 600 })).toBe(false);
  });

  test('a focus records its target, whether it is a field or a menu row, and whether a key moved it', () => {
    const state: Env = {};
    members.call('focusSeen', state, { keyboardLast: true }, ['copy-m1', false, false]);
    expect(state).toMatchObject({ focusTarget: 'copy-m1', focusField: false, focusMenu: false, focusByKey: true });
  });
});
