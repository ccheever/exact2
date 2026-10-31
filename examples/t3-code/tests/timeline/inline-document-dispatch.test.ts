import { expect, test } from 'bun:test';
import { contractComponent, value } from '../support/contract-action-fixture';
import { appText } from '../support/app-files';
import { documentShortcutCommand } from '../../src/timeline/inline-document-shortcuts';

const root = contractComponent('app.contract', 'T3Code');
test('a configured guest sidebar key invokes the existing button operation without a dispatch row', () => {
  const key = { key: 'ArrowDown', metaKey: false, ctrlKey: false, shiftKey: false, altKey: false, repeat: false };
  const name = documentShortcutCommand({ keybindings: [{ command: 'sidebar.toggle', shortcut: { key: 'arrowdown' } }] }, key);
  expect(name).toBe('sidebar.toggle');
  const calls: unknown[][] = [];
  const scope = { inlineDocumentKeysBlocked: false, modal: false, paletteOpen: false, settingsOpen: false, shortcuts: [], command: (...args: unknown[]) => calls.push(args) };
  root.call('inlineDocumentShortcut', {}, { ...scope, data: { sidebarOpen: true } }, [name]);
  root.call('inlineDocumentShortcut', {}, { ...scope, data: { sidebarOpen: false } }, [name]);
  expect(calls).toEqual([['sidebar', '', 'closed', 0], ['sidebar', '', 'open', 0]]);
  for (const blocked of [{ inlineDocumentKeysBlocked: true }, { modal: true }, { paletteOpen: true }, { settingsOpen: true }]) {
    root.call('inlineDocumentShortcut', {}, { ...scope, ...blocked, data: { sidebarOpen: true } }, [name]);
  }
  root.call('inlineDocumentShortcut', {}, scope, ['composer.send']);
  expect(calls).toHaveLength(2);
});

test('inert guest messages cannot bypass global confirmations or preview overlays', () => {
  const source = appText('app.contract');
  const expression = /^  derive inlineDocumentKeysBlocked = (.+)$/m.exec(source)![1]!;
  const base = { sshPrompt: { open: false }, confirmOp: '', data: { serverUpdate: { confirmTitle: '' }, updateEverything: { confirm: false }, composerImagePreview: [], messages: [{ inlineDocuments: [] }] }, shell: { panel: { deviceSetup: false }, composerFile: { open: false } } };
  expect(value(expression, base)).toBe(false);
  for (const scope of [
    { ...base, sshPrompt: { open: true } }, { ...base, confirmOp: 'close-terminal' },
    { ...base, data: { ...base.data, serverUpdate: { confirmTitle: 'Update?' } } },
    { ...base, data: { ...base.data, updateEverything: { confirm: true } } },
    { ...base, data: { ...base.data, composerImagePreview: [{}] } },
    { ...base, data: { ...base.data, messages: [{ inlineDocuments: [{ permission: 'Allow write?' }] }] } },
    { ...base, shell: { ...base.shell, panel: { deviceSetup: true } } },
    { ...base, shell: { ...base.shell, composerFile: { open: true } } },
  ]) expect(value(expression, scope)).toBe(true);
  const calls: unknown[][] = [];
  const scope = { inlineDocumentKeysBlocked: false, shortcuts: [{ command: 'commandPalette.toggle', target: 'command', extra: '' }], paletteShow: (...args: unknown[]) => calls.push(args) };
  root.call('inlineDocumentShortcut', {}, scope, ['commandPalette.toggle']);
  root.call('inlineDocumentShortcut', {}, { ...scope, inlineDocumentKeysBlocked: true }, ['commandPalette.toggle']);
  expect(calls).toEqual([['command', '']]);
});
