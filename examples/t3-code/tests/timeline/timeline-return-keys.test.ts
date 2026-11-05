// Return presses a focused timeline or header button, as Chromium's `<button>` takes Enter in the reference. The Mac
// presses a focused button with Space only unless a container says `-exact-return-presses="descendants"` (LLP 1115.000
// D7.3), which the surfaces the reference draws as `<button>`s carry.
import { describe, expect, test } from 'bun:test';
import { readdirSync } from 'node:fs';
import { join } from 'node:path';
import { appText } from '../support/app-files';
import { contractComponent, type Env } from '../support/contract-action-fixture';

const files = ['timeline', 'media'].flatMap(dir =>
  readdirSync(join(import.meta.dir, '../../src', dir)).filter(name => name.endsWith('.contract')).map(name => `src/${dir}/${name}`));
const opted = (file: string, marker: string) => appText(file).split('\n')
  .some(line => line.includes(marker) && line.includes('-exact-return-presses="descendants"'));

describe('Return on a timeline button', () => {
  test('the chat column, the right panel, the side chat and the diagram preview ask for Chromium\'s Return', () => {
    expect(opted('src/shell/app-main.contract', 'id="frame-chat"')).toBe(true);
    expect(appText('src/shell/app-main.contract').split('\n')
      .filter(line => line.includes('id="workspace-panel"') && line.includes('-exact-return-presses="descendants"')).length).toBe(2);
    const window = appText('src/shell/app-window.contract').split('\n');
    expect(window[window.findIndex(line => line.includes('SideChatLayer(')) - 1]).toContain('-exact-return-presses="descendants"');
    expect(opted('src/timeline/timeline-mermaid.contract', 'role="dialog"')).toBe(true);
  });

  test('no timeline button presses itself on Return', () => {
    const twins = files.flatMap(file => appText(file).split('\n')
      .filter(line => /\bkmReturn\b|OnEnter\b/.test(line)).map(line => `${file}: ${line.trim().slice(0, 80)}`));
    expect(twins).toEqual([]);
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
