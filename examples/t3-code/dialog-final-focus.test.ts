// The data a dialog's DialogFinalFocus reads to name the control that
// opened it, where that is not in the Contract view already. The git card's dialogs remember whether the quick action or
// the options menu opened them, and the Save plan dialog the plan row whose menu opened it. dialog-trap.test.ts checks
// where each DialogFinalFocus stands and what it names.
import { describe, expect, test } from 'bun:test';
import { gitCardView, gitCommand, gitLocal, gitState } from './r4-git-actions';
import { planAction, planSaveView } from './timeline-plan';
import type { T3Client } from './client';
import type { Obj } from './domain';
import type { Native } from './protocol';

const native = { available: true, watch() {}, later: async () => ({}) } as unknown as Native;
const status = (extra: Obj = {}): Obj => ({ isRepo: true, hasPrimaryRemote: true, isDefaultRef: false, refName: 'feature', hasWorkingTreeChanges: true,
  workingTree: { files: [{ path: 'a.ts', insertions: 1, deletions: 0 }], insertions: 1, deletions: 0 }, hasUpstream: true, aheadCount: 0, behindCount: 0, pr: null, ...extra });
const gitClient = () => ({ environmentId: 'env', generation: 3, threadId: 't1', projectId: 'p1', draftKey: 'env:t1', writable: true, local: { composerControls: {} },
  shell: { threads: [], projects: [] }, config: {}, projection: {},
  restAccess: () => ({ ids: async () => ['a1'], call: async () => ({ id: '3-7' }), request: async () => ({}) }) }) as unknown as T3Client;

describe('the git card\'s dialogs give the focus back to the control that opened them', () => {
  test('the options menu\'s Commit: the menu\'s trigger; once it closes, the quick action\'s confirm: the quick action', async () => {
    const client = gitClient();
    gitCardView(client, status({ isDefaultRef: true, refName: 'main' }), '', '/repo', 100);
    await gitCommand(client, native, 'menu', '/repo', 'commit');
    expect(gitCardView(client, gitState(client).status, '', '/repo', 100)).toMatchObject({ dialog: 'commit', dialogFrom: 'details-git-menu' });
    await gitLocal(client, native, 'dialog-close', '', '');
    await gitCommand(client, native, 'quick', '/repo', '');
    expect(gitCardView(client, gitState(client).status, '', '/repo', 100)).toMatchObject({ dialog: 'confirm', dialogFrom: 'details-git-action' });
  });
});

describe('Save plan gives the focus back to the plan\'s menu', () => {
  test('the dialog keeps the row whose "Save to workspace" opened it', async () => {
    const rowId = JSON.stringify(['t1', 'i1']);
    const client = { environmentId: 'env', threadId: 't1', projectId: 'p1', shell: { projects: [{ id: 'p1', workspaceRoot: '/work' }] },
      projection: { thread: { projectId: 'p1' }, visibleTurnItems: [{ sourceThreadId: 't1', sourceItemId: 'i1', item: { type: 'proposed_plan', markdown: '# Ship it\n' } }] } } as unknown as T3Client;
    expect(planSaveView(client)).toMatchObject({ planSaveOpen: false, planSaveRow: '' });
    await planAction(client, native, 'plan-save-open', rowId, '');
    expect(planSaveView(client)).toMatchObject({ planSaveOpen: true, planSavePath: 'ship-it.md', planSaveRow: rowId });
    await planAction(client, native, 'plan-save-cancel', '', '');
    expect(planSaveView(client)).toMatchObject({ planSaveOpen: false, planSaveRow: '' });
  });
});
