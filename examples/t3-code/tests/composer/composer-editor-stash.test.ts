import { describe, expect, test } from 'bun:test';
import { adoptStash, deleteStashEntry, MAX_STASH_ENTRIES, relativeTime, stashEntries, stashPrompt, stashSnippet, stashView, takeStashEntry } from '../../src/composer/composer-editor-stash';

const at = new Date('2026-10-04T10:00:00.000Z');
describe('prompt stash', () => {
  test('⌘S saves newest first and clears; an empty composer restores the only entry or toggles the menu', () => {
    const local = {};
    expect(stashPrompt({ local, draft: '', environmentId: 'e' }, 'a', at)).toEqual({ clear: false, restore: null, toggleMenu: true, evicted: false });
    expect(stashPrompt({ local, draft: '- one\n- two', environmentId: 'e' }, 'a', at).clear).toBe(true);
    expect(stashEntries(local).map(entry => entry.prompt)).toEqual(['- one\n- two']);
    expect(stashPrompt({ local, draft: '  ', environmentId: 'e' }, 'b', at).restore?.id).toBe('a');
    stashPrompt({ local, draft: 'second', environmentId: 'e' }, 'b', at);
    expect(stashEntries(local).map(entry => entry.id)).toEqual(['b', 'a']);
    expect(stashPrompt({ local, draft: '', environmentId: 'e' }, 'c', at).toggleMenu).toBe(true);
  });
  test('holds 20 entries and reports the eviction', () => {
    const local = {};
    for (let index = 0; index < MAX_STASH_ENTRIES; index += 1) stashPrompt({ local, draft: `p${index}`, environmentId: 'e' }, `id${index}`, at);
    const result = stashPrompt({ local, draft: 'newest', environmentId: 'e' }, 'new', at);
    expect(result.evicted).toBe(true);
    expect(stashEntries(local)).toHaveLength(20);
    expect(stashEntries(local)[0]!.prompt).toBe('newest');
    expect(stashEntries(local).some(entry => entry.id === 'id0')).toBe(false);
  });
  test('restore appends after a blank line and removes the entry; delete removes', () => {
    const local = {};
    stashPrompt({ local, draft: 'stashed', environmentId: 'e' }, 'a', at);
    expect(takeStashEntry({ local, draft: 'current  ', environmentId: 'e' }, 'a')).toEqual({ prompt: 'current\n\nstashed' });
    expect(stashEntries(local)).toEqual([]);
    stashPrompt({ local, draft: 'x', environmentId: 'e' }, 'b', at);
    expect(takeStashEntry({ local, draft: '', environmentId: 'e' }, 'b')).toEqual({ prompt: 'x' });
    stashPrompt({ local, draft: 'y', environmentId: 'e' }, 'c', at);
    expect(deleteStashEntry(local, 'c')).toBe(true);
    expect(takeStashEntry({ local, draft: '', environmentId: 'e' }, 'c')).toBeNull();
  });
  test('menu rows: one-line snippet, relative time, restore label; persisted shape decodes', () => {
    const local = {};
    stashPrompt({ local, draft: '- one\n- two\n  - nested', environmentId: 'e' }, 'a', at);
    const view = stashView(local, at.getTime() + 5_000);
    expect(view.rows).toEqual([{ id: 'a', index: 0, snippet: '- one - two - nested', age: 'just now', label: 'Restore stashed prompt: - one - two - nested' }]);
    expect(view.pulse).toBe(1);
    expect(stashSnippet('x'.repeat(95))).toBe(`${'x'.repeat(90)}…`);
    expect(stashSnippet('   ')).toBe('(empty)');
    expect(relativeTime(at.toISOString(), at.getTime() + 3 * 60_000)).toBe('3m ago');
    const next = {};
    adoptStash(next, { promptStash: [{ id: 'a', createdAt: at.toISOString(), prompt: 'p', environmentId: 'e' }, { id: '', prompt: 'bad' }] });
    expect(stashEntries(next)).toEqual([{ id: 'a', createdAt: at.toISOString(), prompt: 'p', environmentId: 'e' }]);
  });
});

describe('stash drawer keys (ComposerStashMenu at T3 Code 74541bab17)', () => {
  const { contractComponent } = require('../support/contract-action-fixture') as typeof import('../support/contract-action-fixture');
  const { readFileSync } = require('node:fs') as typeof import('node:fs');
  const { appPath } = require('../support/app-files') as typeof import('../support/app-files');
  const menu = contractComponent('src/composer/composer-editor.contract', 'StashMenu');
  test('Escape inside the drawer closes it; no window-wide Escape chord takes it from a terminal or a diff comment', () => {
    for (const [k, held, closes] of [['Escape', {}, true], ['Escape', { shiftKey: true }, false], ['ArrowDown', {}, false]] as const) {
      const ops: unknown[][] = [];
      let prevented = 0;
      menu.call('drawerKey', menu.state(), { op: (...args: unknown[]) => ops.push(args), preventDefault: () => prevented++, stopPropagation: () => {} },
        [k, { metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, ...held }]);
      expect(ops).toEqual(closes ? [['stash-menu', '', 'close']] : []);
      expect(prevented).toBe(closes ? 1 : 0);
    }
    const source = readFileSync(appPath('src/composer/composer-editor.contract'), 'utf8');
    const drawer = source.slice(source.indexOf('component StashMenu'), source.indexOf('component ComposerEditorDock'));
    expect(drawer).not.toContain('aria-keyshortcuts="Escape"');
    // From the composer, its key action closes it (app.contract composerStashKey; composer-plan-key.test.ts).
  });
});
