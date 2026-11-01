// Enter and Escape in a sidebar row's rename field give the focus back to the row (T3 Code v0.1.5
// SidebarThreadRow.tsx ThreadRenameInput, whose field unmounts to the body, and lib/sidebarTree.ts refresh, which
// refocuses the tree's stop then), so F6 from the composer reaches the sidebar again. Run with the Contract action
// semantics of contract-action-fixture.ts.
import { describe, expect, test } from 'bun:test';
import { contractComponent, type Env } from '../support/contract-action-fixture';
import { appText } from '../support/app-files';

const field = contractComponent('src/sidebar/sidebar-row.contract', 'SidebarRenameField');
const root = contractComponent('app.contract', 'T3Code');

/** The field of row `t1` titled "Old", its calls in order. */
function renaming() {
  const calls: unknown[][] = [];
  const state: Env = { title: 'Old', settled: false };
  const scope: Env = {
    t: { id: 't1', title: 'Old' }, releaseUndoClaim: () => {},
    local: (...args: unknown[]) => calls.push(['local', ...args]), run: (...args: unknown[]) => calls.push(['run', ...args]),
    sidebarRenameClosed: (id: string) => calls.push(['closed', id]),
  };
  const act = (name: string, ...args: unknown[]) => field.call(name, state, scope, args);
  return { calls, state, act };
}

describe('the rename field closing', () => {
  test('Escape cancels, hands the row the focus back, and the closing blur commits nothing', () => {
    const f = renaming();
    f.act('edit', 'New');
    f.calls.length = 0;
    f.act('renameKey', 'Escape', { isComposing: false });
    f.act('blurred');
    expect(f.calls).toEqual([['closed', 't1'], ['local', 'rename-key', 't1', 'Escape']]);
  });

  test('Enter commits once and hands the row the focus back', () => {
    const f = renaming();
    f.act('edit', 'New');
    f.calls.length = 0;
    f.act('commit');
    f.act('blurred');
    expect(f.calls).toEqual([['run', 'rename', 't1', 'New'], ['closed', 't1']]);
  });

  test('a blur elsewhere commits and leaves the focus where it went; an input method\'s Escape closes nothing', () => {
    const f = renaming();
    f.act('edit', 'New');
    f.calls.length = 0;
    f.act('renameKey', 'Escape', { isComposing: true });
    f.act('blurred');
    expect(f.calls).toEqual([['run', 'rename', 't1', 'New']]);
  });

  test('the field binds Enter, blur and keys to these actions', () => {
    const line = appText('src/sidebar/sidebar-row.contract').split('\n').find(entry => entry.includes('id=`sidebar-rename-${t.id}`'))!;
    expect(line).toContain('submit=commit blur=blurred key=renameKey');
  });
});

describe('the row takes the focus back once the field has gone', () => {
  test('the root holds the row until the sidebar\'s rename ends, then focuses its treeitem', () => {
    const focused: string[] = [];
    const state: Env = { renameReturn: '', workspaceArea: 'sidebar' };
    root.call('sidebarRenameClosed', state, {}, ['t1']);
    expect(state.renameReturn).toBe('t1');
    // The task waits for the field to go: the snapshot's rename no longer names the row.
    const task = appText('app.contract').split('\n').find(line => line.includes('task renameReturnDue'))!;
    expect(task.trim()).toBe('task renameReturnDue when renameReturn != "" and data.sidebar.renameId != renameReturn key=renameReturn');
    root.call('sidebarRenameReturned', state, { focus: (id: string) => focused.push(id) }, []);
    expect([state.renameReturn, focused]).toEqual(['', ['thread-t1']]);
  });

  test('a focus moved out of the sidebar while the rename\'s answer was awaited stays where it went', () => {
    const focused: string[] = [];
    const state: Env = { renameReturn: 't1', workspaceArea: 'composer' };
    root.call('sidebarRenameReturned', state, { focus: (id: string) => focused.push(id) }, []);
    expect([state.renameReturn, focused]).toEqual(['', []]);
  });

  test('the row\'s treeitem is the button whose id the root focuses', () => {
    const rows = appText('src/sidebar/sidebar-row.contract').split('\n').filter(line => line.includes('button cursor="pointer" id=`thread-${t.id}`'));
    expect(rows.length).toBe(2); // the card row and the compact row
    for (const row of rows) expect(row).toContain('role="treeitem"');
  });
});

describe('a rename committed while another command runs', () => {
  const scope = (sent: unknown[][]) => ({
    composerFocused: false, blur: () => {}, command: (...args: unknown[]) => args,
    wallTime: { epochAtZero: 0 }, performanceNow: () => 5, sent: (_: string, command: unknown[]) => sent.push(command),
  });

  test('waits for that command\'s answer, then runs once', () => {
    const sent: unknown[][] = [];
    const state: Env = { commandPending: true, renameQueuedId: '', renameQueuedTitle: '' };
    root.call('sidebarRun', state, scope(sent), ['rename', 't1', 'New']);
    expect([sent, state.renameQueuedId, state.renameQueuedTitle]).toEqual([[], 't1', 'New']);
    const task = appText('app.contract').split('\n').find(line => line.includes('task renameQueuedDue'))!;
    expect(task.trim()).toBe('task renameQueuedDue when renameQueuedId != "" and not commandPending key=renameQueuedId');
    const runs: unknown[][] = [];
    root.call('renameQueuedRun', state, { sidebarRun: (...args: unknown[]) => runs.push(args) }, []);
    expect([runs, state.renameQueuedId]).toEqual([[['rename', 't1', 'New']], '']);
    state.commandPending = false;
    root.call('sidebarRun', state, scope(sent), ['rename', 't1', 'New']);
    expect([sent, state.commandPending]).toEqual([[['sidebar:rename', 't1', 'New', 5]], true]);
  });

  test('other commands are still dropped', () => {
    const state: Env = { commandPending: true, renameQueuedId: '', renameQueuedTitle: '' };
    root.call('sidebarRun', state, scope([]), ['settle', 't1', '']);
    expect(state.renameQueuedId).toBe('');
  });
});
