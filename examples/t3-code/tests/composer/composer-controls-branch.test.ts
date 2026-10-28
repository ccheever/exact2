// Lane composer-controls: the workspace / branch strip under the composer
// (BranchToolbar, BranchToolbarBranchSelector, BranchToolbar.logic).
import { describe, expect, test } from 'bun:test';
import { obj, str, type Obj } from '../../src/client/domain';
import { composerBranches, patchDraftContext, sanitizeNewRefName, previousWorktree, stripShortcuts } from '../../src/composer/composer-controls-branch';
import { branchLocal, cardBranchView } from '../../src/details/git-branch';
import { keyboardDispatch } from '../../src/shell/keyboard-dispatch';
import { connected, thread } from '../support/composer-controls-fixture';
import { toasts } from '../../src/shell/toast';
import { appPath, appText } from '../support/app-files';

describe('workspace strip details', () => {
  // handleOpenChange (BranchToolbarBranchSelector): each picker that opens asks for the checkout's current branch once.
  const refreshes = (native: Awaited<ReturnType<typeof connected>>['native']) => {
    native.later = (later => async (input: unknown) => {
      const request = obj(input);
      if (request.op === 'request' && request.method === 'vcs.refreshStatus') native.committed.push({ method: request.method, ...obj(request.payload) });
      return later(input);
    })(native.later.bind(native));
    return () => native.committed.filter(entry => entry.method === 'vcs.refreshStatus' && entry.localOnly === true).map(entry => entry.cwd);
  };
  test('opening the strip\'s picker asks for the current branch once, without a remote fetch', async () => {
    const { client, native } = await connected();
    const asked = refreshes(native);
    await composerBranches(client, native, false, '');
    expect(asked()).toEqual([]);
    await composerBranches(client, native, true, '');
    await composerBranches(client, native, true, 'ma');
    expect(asked()).toEqual(['/repo']);
    await composerBranches(client, native, false, '');
    await composerBranches(client, native, true, '');
    expect(asked()).toEqual(['/repo', '/repo']);
  });
  test('the picker\'s own open state decides, whatever folder or read closed it', async () => {
    const { client, native, command } = await connected();
    const asked = refreshes(native);
    // Opened on the checkout, closed after the draft moved to a worktree (a ref checked out there): the next opening on
    // the checkout asks again.
    await composerBranches(client, native, true, '');
    patchDraftContext(client, { envMode: 'local', worktreePath: '/repo-wt/b' });
    await composerBranches(client, native, false, '');
    patchDraftContext(client, { worktreePath: '' });
    await composerBranches(client, native, true, '');
    expect(asked()).toEqual(['/repo', '/repo']);
    // Closed while a thread hides the strip (its read returns early): the next opening asks again.
    await command('select-thread', 't1');
    await composerBranches(client, native, false, '');
    await command('new-thread');
    await composerBranches(client, native, true, '');
    expect(asked()).toEqual(['/repo', '/repo', '/repo']);
  });
  test("the workspace card's reads never ask: its picker asks as its popover opens (git-branch.ts `opened`)", async () => {
    const { client, native } = await connected();
    const asked = refreshes(native);
    await cardBranchView(client, native, '/repo', '/repo', true);
    await cardBranchView(client, native, '/repo', '/repo', true);
    expect(asked()).toEqual([]);
    branchLocal(client, 'opened', '', '', native);
    await new Promise(resolve => setTimeout(resolve, 0));
    expect(asked()).toEqual(['/repo']);
  });
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
    // envLocked: a message (or a runtime) starts it; an unstarted server thread still picks (canOverrideServerThreadEnvMode).
    await command('select-thread', 't2');
    expect(await composerBranches(client, native, false, '')).toMatchObject({ show: true, envLocked: false, envLabel: 'Current checkout', envIcon: 'folder' });
    (native.details.t1!.projection as Obj).messages = [{ id: 'm1', role: 'user', text: 'Earlier' }];
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
  test('Copy branch name copies with a toast', async () => {
    const { client, native, command } = await connected();
    await command('cclocal:branch-copy', '', '');
    expect(native.copied).toEqual([]);
    await command('cclocal:branch-copy', '', 'main');
    expect(native.copied).toEqual(['main']);
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'success', title: 'Branch name copied', description: 'main' });
  });
  test('the trigger and each ref name the Copy branch name popover, which copies the right-clicked name', async () => {
    const source = await Bun.file(appPath('composer-controls.contract')).text();
    expect(source).toContain('contextmenu=refMenu(ref.name) contextPopover="branch-ref-menu"');
    // The root's command: the picker's own (menuCommand) closes it, and the reference's menu leaves it open.
    expect(source).toContain('BranchNameMenu(menuId="branch-ref-menu", name=menuRef, command=rootCommand)');
    expect(await Bun.file(appPath('app-window.contract')).text()).toContain('ComposerMenus(rootCommand=command,');
    expect(source).toContain('menu=(strip.branch == "" ? "" : "branch-name-menu")');
    expect(source).toContain('BranchNameMenu(menuId="branch-name-menu", name=strip.branch, command=command)');
    expect(source).toContain('button press=command("cclocal:branch-copy", "", name, 0) popovertarget=menuId popovertargetaction="hide" role="menuitem"');
    expect(source).not.toContain('branch-menu"');
  });
});

describe('one Run on and Workspace keyboard order', () => {
  const source = Bun.file(appPath('composer-controls.contract')).text();
  const item = (id: string, label: string) => ({ id, label });
  const primary = item('primary', 'This Mac'), remote = item('remote', 'Studio'), auto = item('auto', 'Auto balance');
  const base = { envOptions: [primary, remote], envAdd: [] as { id: string; label: string }[], gitless: false, forceWorktree: false, previous: true, envIcon: 'folder' };
  const derive = async (name: string, scope: Record<string, unknown>) => {
    const { value } = await import('../support/contract-action-fixture');
    const expression = new RegExp(`^  derive ${name} = (.+)$`, 'm').exec(await source)?.[1];
    if (!expression) throw new Error(`ComposerMenus is missing ${name}`);
    return value(expression, scope);
  };
  const items = (branches: typeof base) => derive('workspaceItems', { branches, KmItem: (fields: { id: string; label: string }) => fields });

  test('order includes Run on first, omits disabled checkout, and covers previous, auto-balance and gitless variants', async () => {
    expect(await items(base)).toEqual([item('run-on-primary', 'This Mac'), item('run-on-remote', 'Studio'),
      item('workspace-local', 'Current checkout'), item('workspace-worktree', 'New worktree'), item('workspace-previous', 'Previous worktree')]);
    expect(await items({ ...base, forceWorktree: true })).toEqual([item('run-on-primary', 'This Mac'), item('run-on-remote', 'Studio'),
      item('workspace-worktree', 'New worktree'), item('workspace-previous', 'Previous worktree')]);
    expect(await items({ ...base, envOptions: [], previous: false, envIcon: 'folder-git-1' })).toEqual([
      item('workspace-local', 'Current worktree'), item('workspace-worktree', 'New worktree')]);
    expect(await items({ ...base, envOptions: [auto, primary, remote], gitless: true })).toEqual([
      item('run-on-auto', 'Auto balance'), item('run-on-primary', 'This Mac'), item('run-on-remote', 'Studio')]);
    expect(await items({ ...base, envOptions: [], gitless: true })).toEqual([]);
    // "Add to another machine" follows the Run on rows (74541bab17).
    expect(await items({ ...base, envOptions: [primary], envAdd: [remote], previous: false })).toEqual([item('run-on-primary', 'This Mac'),
      item('run-on-add', 'Add to another machine'), item('workspace-local', 'Current checkout'), item('workspace-worktree', 'New worktree')]);
  });

  test('arrows wrap both ends, Home/End and typeahead follow hover, and a new session forgets the old row', async () => {
    const { contractComponent } = await import('../support/contract-action-fixture');
    const { contractFn } = await import('../support/contract-fn-fixture');
    const kmIndex = await contractFn('menu-keys.contract', 'kmIndex', ['items', 'current']);
    const kmAfter = await contractFn('menu-keys.contract', 'kmAfter', ['items', 'current'], { kmIndex });
    const kmLetter = await contractFn('menu-keys.contract', 'kmLetter', ['k']);
    const kmTarget = await contractFn('menu-keys.contract', 'kmTarget', ['items', 'current', 'k'], { kmIndex, kmAfter, kmLetter, first: (rows: unknown[]) => rows[0], none: null });
    const kmReturn = await contractFn('menu-keys.contract', 'kmReturn', ['k', 'e']);
    const component = contractComponent('composer-controls.contract', 'ComposerMenus'), state = component.state();
    let workspaceItems = await items(base);
    const calls: unknown[][] = [];
    let focused = 'composer-menu';
    const scope = async () => ({ which: 'workspace', session: 7, workspaceItems, kmTarget, kmReturn, branches: base,
      workspaceCurrent: await derive('workspaceCurrent', { ...state, session: 7 }),
      focus: (id: string) => { calls.push(['focus', id]); focused = id; },
      preventDefault: () => calls.push(['prevent']), stopPropagation: () => calls.push(['stop']) });
    const key = async (name: string, event = {}) => {
      component.call('move', state, await scope(), [-1, name, event]);
      component.call('menuEntered', state, await scope(), [{ target: focused }]);
      return focused;
    };
    const order = ['run-on-primary', 'run-on-remote', 'workspace-local', 'workspace-worktree', 'workspace-previous'];
    for (const id of [...order, order[0]]) expect(await key('ArrowDown')).toBe(id);
    expect(await key('ArrowUp')).toBe('workspace-previous');
    expect(await key('Home')).toBe('run-on-primary');
    expect(await key('End')).toBe('workspace-previous');
    component.call('workspaceHover', state, await scope(), ['run-on-remote', true]);
    component.call('menuEntered', state, await scope(), [{ target: focused }]);
    expect(await key('ArrowDown')).toBe('workspace-local');
    expect(await key('s')).toBe('run-on-remote');
    expect(calls.filter(call => call[0] === 'prevent')).toHaveLength(11);
    expect(calls.filter(call => call[0] === 'stop')).toHaveLength(11);
    for (const modifier of ['metaKey', 'ctrlKey', 'altKey']) {
      const before = calls.length;
      await key('ArrowDown', { [modifier]: true });
      expect(calls).toHaveLength(before);
    }
    component.call('menuEntered', state, await scope(), [{ target: 'unrelated-descendant' }]);
    expect(await derive('workspaceCurrent', { ...state, session: 7 })).toBe('run-on-remote');
    expect(await derive('workspaceCurrent', { ...state, session: 8 })).toBe('');
    const variants = [
      { branches: { ...base, forceWorktree: true }, order: ['run-on-primary', 'run-on-remote', 'workspace-worktree', 'workspace-previous'] },
      { branches: { ...base, envOptions: [], previous: false }, order: ['workspace-local', 'workspace-worktree'] },
      { branches: { ...base, envOptions: [auto, primary, remote], gitless: true }, order: ['run-on-auto', 'run-on-primary', 'run-on-remote'] },
      { branches: { ...base, envOptions: [], gitless: true }, order: [] },
    ];
    for (const variant of variants) {
      workspaceItems = await items(variant.branches);
      focused = 'composer-menu';
      component.call('menuEntered', state, await scope(), [{ target: focused }]);
      for (const id of variant.order) expect(await key('ArrowDown')).toBe(id);
      expect(await key('ArrowDown')).toBe(variant.order[0] ?? 'composer-menu');
      expect(await key('ArrowUp')).toBe(variant.order.at(-1) ?? 'composer-menu');
    }
  });

  test('disabled and absent rows cannot be hovered or keyed, and branch focus still reaches its dialog owner', async () => {
    const { contractComponent } = await import('../support/contract-action-fixture');
    const component = contractComponent('composer-controls.contract', 'ComposerMenus'), state = component.state(), calls: unknown[][] = [];
    component.call('workspaceHover', state, { workspaceItems: await items({ ...base, forceWorktree: true }), workspaceCurrent: '', branches: base,
      focus: (id: string) => calls.push(['focus', id]) }, ['workspace-local', true]);
    expect(calls).toEqual([]);
    // A sibling row taking the pointer closes "Add to another machine"'s submenu (74541bab17).
    const adding = { ...base, envOptions: [primary], envAdd: [remote] };
    component.call('workspaceHover', state, { workspaceItems: await items(adding), workspaceCurrent: '', branches: adding,
      focus: (id: string) => calls.push(['focus', id]), hidePopover: (id: string) => calls.push(['hide', id]) }, ['workspace-local', true]);
    expect(calls).toEqual([['hide', 'run-on-add-menu'], ['focus', 'workspace-local']]);
    calls.length = 0;
    const event = { target: 'branch-search' };
    component.call('menuEntered', state, { which: 'branch', dialogMenuEntered: (...args: unknown[]) => calls.push(args) }, [event]);
    expect(calls).toEqual([['composer-menu', 'branch-selector', event]]);
    const text = await source;
    expect(text).toContain('data-dialog-return=(which == "branch" ? "menu" : "") focusin=menuEntered');
    const workspace = text.split('          if which == "workspace"\n')[1]!.split('          if which == "branch"\n')[0]!;
    expect(workspace).not.toMatch(/key=move|focus=mark|hover=hover|composer-menu-item-/);
    for (const button of workspace.split('\n').filter(line => line.trimStart().startsWith('button '))) {
      expect(button).toContain('role=(wideWorkspace ? "option" : "menuitemradio")');
      expect(button).toContain('aria-selected=');
    }
  });

  test('each row keeps one command binding and delegates selection, close and Message focus to its root owner', async () => {
    const { value } = await import('../support/contract-action-fixture');
    const text = await source;
    const workspace = text.split('          if which == "workspace"\n')[1]!.split('          if which == "branch"\n')[0]!;
    const buttons = workspace.split('\n').filter(line => line.trimStart().startsWith('button '));
    expect(buttons).toHaveLength(4);
    const calls: unknown[][] = [];
    for (const line of buttons) {
      const binding = /press=(command\([^)]*\))/.exec(line)?.[1];
      if (!binding) throw new Error('Workspace row lost its command');
      value(binding, { option: primary, command: (...args: unknown[]) => calls.push(args) });
    }
    expect(calls).toEqual([['environment-run-on', '', 'primary', 0], ['cclocal:env-mode', '', 'local', 0],
      ['cclocal:env-mode', '', 'worktree', 0], ['cclocal:env-mode', '', 'previous', 0]]);
    expect(workspace).not.toContain('focus(');
    expect(workspace).not.toContain('close(');
    const app = await appText('app.contract');
    expect(app).toContain('action menuCommand(op: string, id: string, value: string, n: number)\n    optionsOpen = ""\n    focus("composer")\n    if not commandPending\n      commandPending = true\n      send changed = command(op, id, value, n)');
  });
});
