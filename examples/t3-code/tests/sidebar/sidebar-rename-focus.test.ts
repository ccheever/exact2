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
    const state: Env = { renameReturn: '' };
    root.call('sidebarRenameClosed', state, {}, ['t1']);
    expect(state.renameReturn).toBe('t1');
    // The task waits for the field to go: the snapshot's rename no longer names the row.
    const task = appText('app.contract').split('\n').find(line => line.includes('task renameReturnDue'))!;
    expect(task.trim()).toBe('task renameReturnDue when renameReturn != "" and data.sidebar.renameId != renameReturn key=renameReturn');
    root.call('sidebarRenameReturned', state, { focus: (id: string) => focused.push(id) }, []);
    expect([state.renameReturn, focused]).toEqual(['', ['thread-t1']]);
  });

  test('the row\'s treeitem is the button whose id the root focuses', () => {
    const rows = appText('src/sidebar/sidebar-row.contract').split('\n').filter(line => line.includes('button cursor="pointer" id=`thread-${t.id}`'));
    expect(rows.length).toBe(2); // the card row and the compact row
    for (const row of rows) expect(row).toContain('role="treeitem"');
  });
});
