// Lane composer-controls: the workspace / branch strip under the composer
// (BranchToolbar, BranchToolbarBranchSelector, BranchToolbar.logic).
import { describe, expect, test } from 'bun:test';
import { obj, str } from './domain';
import { composerBranches, sanitizeNewRefName, previousWorktree, stripShortcuts } from './composer-controls-branch';
import { keyboardDispatch } from './keyboard-dispatch';
import { connected, thread } from './composer-controls-fixture';
import { toasts } from './toast';

describe('workspace strip details', () => {
  test('a typed name is offered as a sanitized new ref, and Return picks the highlighted first item', async () => {
    const { client, native } = await connected();
    expect(sanitizeNewRefName('  my new\tbranch ')).toBe('my-new-branch');
    const typed = await composerBranches(client, native, true, 'brand new');
    expect(typed).toMatchObject({ creatable: 'brand-new', enterOp: 'cc:branch-create', enterValue: 'brand-new', refs: [] });
    const listed = await composerBranches(client, native, true, 'mai');
    expect(listed).toMatchObject({ enterOp: 'cc:branch', enterValue: 'main', creatable: 'mai' });
    expect(listed.refs.map(ref => [ref.name, ref.index])).toEqual([['main', 0], ['origin/main', 1]]);
  });
  test('Previous worktree points the draft at the newest worktree and launches into it', async () => {
    const { client, native, command } = await connected();
    client.shell.threads.push(thread('w1', { worktreePath: '/repo-wt/old', branch: 'old', updatedAt: '2026-10-01T00:00:00.000Z' }),
      thread('w2', { worktreePath: '/repo-wt/new', branch: 'feature/new', updatedAt: '2026-10-02T00:00:00.000Z' }),
      thread('w3', { worktreePath: '/repo-wt/gone', branch: 'gone', updatedAt: '2026-10-03T00:00:00.000Z', archivedAt: '2026-10-03T00:00:00.000Z' }));
    expect(previousWorktree(client)).toEqual({ branch: 'feature/new', worktreePath: '/repo-wt/new' });
    expect(await composerBranches(client, native, false, '')).toMatchObject({ previous: true, previousBranch: 'feature/new', envLabel: 'Current checkout', envIcon: 'folder' });
    await command('cclocal:env-mode', '', 'previous');
    const view = await composerBranches(client, native, false, '');
    expect(view).toMatchObject({ envMode: 'local', envLabel: 'Current worktree', envIcon: 'folder-git-1', previous: true, previousBranch: 'old' });
    await command('send', '', 'Continue there');
    expect(native.committed.at(-1)).toMatchObject({ method: 'orchestration.launchThread',
      workspaceStrategy: { type: 'existing_worktree', worktreePath: '/repo-wt/new', branch: 'feature/new' } });
  });
  test('a started thread shows the locked row: Local checkout, or Worktree', async () => {
    const { client, native, command } = await connected();
    client.local.clientSettings.persistComposerContextStrip = true;
    await command('select-thread', 't1');
    expect(await composerBranches(client, native, false, '')).toMatchObject({ show: true, envLocked: true, envLabel: 'Local checkout', envIcon: 'folder', previous: false });
    obj(client.thread!.projection.thread).worktreePath = '/repo-wt/t1';
    obj(client.thread!.projection.thread).branch = 't1-branch';
    expect(await composerBranches(client, native, false, '')).toMatchObject({ envLocked: true, envLabel: 'Worktree', envIcon: 'folder-git', envMode: 'worktree' });
  });
  test('choosing a ref checked out in another worktree reuses that worktree without switching', async () => {
    const { client, native, command } = await connected();
    native.extraRefs = [{ name: 'busy', current: false, isDefault: false, worktreePath: '/repo-wt/busy' }];
    await composerBranches(client, native, true, '');
    await command('cc:branch', '', 'busy');
    expect(native.committed.some(entry => entry.method === 'vcs.switchRef')).toBe(false);
    expect(await composerBranches(client, native, false, '')).toMatchObject({ envLabel: 'Current worktree', envIcon: 'folder-git-1' });
    await command('send', '', 'In busy');
    expect(str(obj(native.committed.at(-1)!.workspaceStrategy).type)).toBe('existing_worktree');
  });
  test('⇧⌘X, ⇧⌘G and ⇧⌘L dispatch while the strip shows', async () => {
    const { client, native, command } = await connected();
    client.config.keybindings = [
      { command: 'composer.workspace', shortcut: { key: 'x', modKey: true, shiftKey: true } },
      { command: 'composer.branch', shortcut: { key: 'g', modKey: true, shiftKey: true } },
      { command: 'composer.previousWorktree', shortcut: { key: 'l', modKey: true, shiftKey: true } }];
    const context = { composerFocus: true, editableFocus: true, turnRunning: false, modelPickerOpen: false, draftThreadRoute: true, modalOpen: false,
      settingsOpen: false, diffOpen: false, paletteOpen: false, paletteMode: '' };
    const strip = () => keyboardDispatch(client, [], '', '', context).filter(item => item.command.startsWith('composer.') && item.kind !== 'options' || ['workspace', 'branch'].includes(item.target)).map(item => [item.command, item.chord, item.kind, item.target]);
    expect(strip()).toEqual([]);
    await composerBranches(client, native, false, '');
    expect(stripShortcuts(client)).toEqual({ workspace: true, branch: true, previous: false });
    client.shell.threads.push(thread('w1', { worktreePath: '/repo-wt/one', branch: 'one', updatedAt: '2026-10-01T00:00:00.000Z' }));
    expect(strip()).toEqual([['composer.workspace', 'Meta+Shift+x', 'options', 'workspace'], ['composer.branch', 'Meta+Shift+g', 'options', 'branch'],
      ['composer.previousWorktree', 'Meta+Shift+l', 'command', 'cclocal:previous-worktree']]);
    await command('cclocal:previous-worktree');
    expect(await composerBranches(client, native, false, '')).toMatchObject({ envLabel: 'Current worktree' });
  });
  test('right-clicking the branch offers Copy branch name, copied with a toast', async () => {
    const { client, native, command } = await connected();
    await command('cclocal:branch-menu', '', 'main');
    expect(native.copied).toEqual([]);
    native.menuPick = 'copy-branch-name';
    await command('cclocal:branch-menu', '', 'main');
    expect(native.copied).toEqual(['main']);
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'success', title: 'Branch name copied', description: 'main' });
  });
});
