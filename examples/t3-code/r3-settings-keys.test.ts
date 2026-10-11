// Lane r3-settings: Keybindings follow T3 f90b77d809's defaults (upstream 5c0429b157
// mod+alt+enter sends and opens a new thread; 263097a8da mod+enter starts in the background).
import { describe, expect, test } from 'bun:test';
import { keybindingCommands, keybindingDefaults } from './keybinding-settings';
import { buildRows, commandLabel, commandOptions } from './keybinding-view';
import { commandLabel as searchLabel } from './settings-search';

// The server's resolved rule: shortcut flags and the when-expression as an AST.
const binding = (command: string, key: string) => ({ command, shortcut: Object.fromEntries(key.split('+').map(part => part === 'mod' ? ['modKey', true] : part === 'alt' ? ['altKey', true] : ['key', part])),
  whenAst: { type: 'and', left: { type: 'identifier', name: 'composerFocus' }, right: { type: 'identifier', name: 'draftThreadRoute' } } });

describe('composer send shortcuts', () => {
  test('the defaults carry both new rules in the reference order', () => {
    const composer = keybindingDefaults.filter(rule => rule.command.startsWith('composer.send'));
    expect(composer).toEqual([
      { key: 'mod+enter', command: 'composer.sendAlternate', when: 'composerFocus && turnRunning' },
      { key: 'mod+enter', command: 'composer.sendBackground', when: 'composerFocus && draftThreadRoute' },
      { key: 'mod+alt+enter', command: 'composer.sendBackground', when: 'composerFocus && draftThreadRoute' },
      { key: 'mod+alt+enter', command: 'composer.sendAndNewThread', when: 'composerFocus && !draftThreadRoute' },
    ]);
    expect(keybindingCommands.slice(keybindingCommands.indexOf('composer.sendAlternate'), keybindingCommands.indexOf('composer.host')))
      .toEqual(['composer.sendAlternate', 'composer.sendBackground', 'composer.sendAndNewThread']);
  });
  test('the new command is labelled everywhere it is listed', () => {
    expect(commandLabel('composer.sendAndNewThread')).toBe('Composer: Send and Start New Thread');
    expect(searchLabel('composer.sendAndNewThread')).toBe('Composer: Send and Start New Thread');
    expect(commandOptions([]).some(option => option.value === 'composer.sendAndNewThread' && option.label === 'Composer: Send and Start New Thread')).toBe(true);
  });
  test('the rules a HEAD server migrates into configs read as defaults, not customizations', () => {
    const rows = buildRows([binding('composer.sendBackground', 'mod+enter')], '');
    expect(rows.map(row => [row.label, row.source])).toEqual([['Composer: Start in Background', 'Default']]);
  });
});
