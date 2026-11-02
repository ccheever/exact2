// App shortcuts wait while a confirmation dialog is open (T3 Code v0.1.6 shortcuts/dispatcher.ts runScope, #27157):
// a command chord pressed inside a role="alertdialog" runs nothing and its key is swallowed.
import { describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { keyboardDispatch, keyboardDispatchSource, type DispatchContext } from '../../src/shell/keyboard-dispatch';
import { keybindingDefaults } from '../../src/settings/keybinding-settings';
import { parseWhen } from '../../src/settings/keybinding-view';
import { appPath } from '../support/app-files';

function resolved(rule: { key: string; command: string; when?: string }) {
  const tokens = rule.key.split('+'), key = tokens.pop() || '+';
  return { command: rule.command, shortcut: { key, modKey: tokens.includes('mod'), shiftKey: tokens.includes('shift'), altKey: tokens.includes('alt'), ctrlKey: tokens.includes('ctrl'), metaKey: false }, ...(rule.when ? { whenAst: parseWhen(rule.when) } : {}) };
}
function client(): T3Client {
  const c = new T3Client();
  Object.assign(c, { connection: 'connected', configLive: true, shellLive: true, threadLive: true, generation: 1, environmentId: 'env1', origin: 'http://127.0.0.1:1' });
  c.config = { environment: { label: 'Local', capabilities: {} }, keybindings: keybindingDefaults.map(resolved), settings: {} };
  c.shell = { sequence: 1, projects: [{ id: 'p1', title: 'Widgets', workspaceRoot: '/repo' }], threads: [{ id: 't1', projectId: 'p1', title: 'One' }] };
  c.projectId = 'p1'; c.threadId = 't1';
  return c;
}
const thread: DispatchContext = { composerFocus: false, editableFocus: false, turnRunning: false, modelPickerOpen: false, draftThreadRoute: false, modalOpen: false, settingsOpen: false, diffOpen: true };

describe('a confirmation holds the app shortcuts', () => {
  test('every chord stays a button that runs nothing, once per chord; Settings… and the dialog\'s Escape stay', () => {
    const c = client();
    const live = keyboardDispatch(c, [], '', '', thread);
    const waiting = keyboardDispatch(c, [], '', '', { ...thread, confirmOpen: true });
    expect(live.some(row => row.command === 'chat.new' && row.chord === 'Meta+n')).toBe(true);
    expect(waiting.filter(row => row.command !== 'settings.open').every(row => row.kind === 'refuse' && row.command === 'confirmation.wait' && row.target === '')).toBe(true);
    expect(new Set(waiting.map(row => row.chord)).size).toBe(waiting.length);
    expect(new Set(waiting.filter(row => row.command !== 'settings.open').map(row => row.chord))).toEqual(new Set(live.filter(row => row.command !== 'settings.open').map(row => row.chord)));
    expect(waiting.find(row => row.command === 'settings.open')).toMatchObject({ kind: 'settings', chord: 'Meta+,' });
    // rightPanel.close's window capture finds no row to run.
    expect(waiting.some(row => row.command === 'rightPanel.close' || row.command === 'thread.undo')).toBe(false);
  });

  test('the contract names any open confirmation, the window\'s confirm and its other alert dialogs', async () => {
    const c = client();
    const args = (confirmOp: string) => [[], '', '', false, false, false, false, false, false, true, 0, false, '', '', false, '', '', '', confirmOp];
    expect(keyboardDispatchSource(c, args('shell:delete')).find(row => row.chord === 'Meta+n')).toMatchObject({ kind: 'refuse' });
    expect(keyboardDispatchSource(c, args('alertdialog')).find(row => row.chord === 'Meta+n')).toMatchObject({ kind: 'refuse' });
    expect(keyboardDispatchSource(c, args('')).find(row => row.chord === 'Meta+n')).toMatchObject({ command: 'chat.new' });
    const app = await Bun.file(appPath('app.contract')).text();
    expect(app).toContain('confirmOp != "" ? confirmOp : (data.composer.resetConfirm != "" or data.revertPending != "" or data.serverUpdate.confirmTitle != "" or data.updateEverything.confirm ? "alertdialog" : "")');
  });
});
