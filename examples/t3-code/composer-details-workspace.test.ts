// composer-provider-state-and-details: the thread details card's Workspace row
// follows the composer (BranchToolbar layout="panel" with ChatView's forceNewWorktree,
// envLocked and canOverrideServerThreadEnvMode). CO-8: several models force New
// worktree; PA-9: an unstarted server thread picks its workspace and its first
// message starts there (startThreadTurn's bootstrap.prepareWorktree).
import { describe, expect, test } from 'bun:test';
import { shellDetails } from './shell-details';
import { setFanout } from './r3-composer-controls-fanout';
import { canOverrideServerEnv, serverEnvMode } from './composer-controls-branch';
import { branchLocal, startFromOrigin } from './r4-git-branch';
import { type Obj } from './domain';
import { Fake, storage, connected, opened } from './composer-controls-fixture';
import { T3Client } from './client';

const image = (): Obj => ({ id: 'img1', name: 'Capture.png', mimeType: 'image/png', sizeBytes: 68, source: {}, uploadId: 'ready.png' });

describe('the details Workspace row', () => {
  test('CO-8: several models force New worktree in the row, and a pick there changes nothing', async () => {
    const { client, native, command } = await connected();
    expect(client.threadId).toBe('');
    const details = () => shellDetails(client, native, true, '', false);
    expect(await details()).toMatchObject({ envModeSelect: true, envMode: 'local', folderName: 'repo', folderLabel: '', envIcon: 'folder' });
    setFanout(client, [{ instanceId: 'codex', model: 'model-a', options: [] }, { instanceId: 'claude', model: 'model-b', options: [] }]);
    expect(await details()).toMatchObject({ envModeSelect: true, envMode: 'worktree', folderName: 'New worktree', folderLabel: 'Create', envIcon: 'folder-git-2', previousLabel: '' });
    // onEnvModeChange returns while several models are chosen.
    await command('cclocal:env-mode', '', 'local');
    expect((await details()).envMode).toBe('worktree');
    setFanout(client, null);
    expect(await details()).toMatchObject({ envMode: 'local', folderName: 'repo', folderLabel: '' });
  });

  test('PA-9: an unstarted server thread is a select; New worktree starts its first message in one', async () => {
    const native = new Fake(), disk = storage(), client = new T3Client();
    const record = (native.details.t1!.projection as Obj).thread as Obj;
    record.branch = 'feature'; // activeThreadBranch: the new worktree's base
    await client.refresh(native, disk);
    const command = (op: string, id = '', value = '') => client.command(op, id, value, 0, native, disk);
    await command('select-thread', 't1');
    await client.refresh(native, disk);
    expect(canOverrideServerEnv(client)).toBe(true);
    const details = () => shellDetails(client, native, true, 't1', false);
    expect(await details()).toMatchObject({ envModeSelect: true, envMode: 'local', folderName: 'repo', folderLabel: '' });
    await command('cclocal:env-mode', '', 'worktree');
    expect(await details()).toMatchObject({ envModeSelect: true, envMode: 'worktree', folderName: 'New worktree', folderLabel: 'Create', envIcon: 'folder-git-2' });
    // A pasted image already uploaded: the first message carries it, and the thread's composer lets it go (finishPending).
    client.local.snapshotDrafts[client.draftKey] = [image()];
    await command('send', '', 'Start in a worktree');
    expect(native.committed.at(-1)).toMatchObject({ method: 'orchestration.launchThread', threadId: 't1', reuseExistingThread: true,
      workspaceStrategy: { type: 'worktree', baseRef: 'feature', startFromOrigin: true },
      initialMessage: { text: 'Start in a worktree', attachments: [{ type: 'image', id: 'ready.png' }] } });
    expect(client.snapshotDrafts).toEqual([]);
    expect(client.local.snapshotDrafts[client.draftKey]).toEqual([]);
    expect(client.draft).toBe('');
  });

  test('PA-9: a pick is the thread\'s own; Current checkout sends an ordinary message', async () => {
    const { client, native, command } = await opened();
    await command('cclocal:env-mode', '', 'worktree');
    await command('select-thread', 't2');
    await client.refresh(native, storage());
    expect((await shellDetails(client, native, true, 't2', false)).envMode).toBe('local');
    client.local.snapshotDrafts[client.draftKey] = [image()];
    await command('send', '', 'Plain follow-up');
    expect(native.committed.at(-1)).toMatchObject({ type: 'message.dispatch', threadId: 't2', attachments: [{ type: 'image', id: 'ready.png' }] });
    expect(client.snapshotDrafts).toEqual([]);
  });

  test('PA-9: Start from origin is the thread\'s own and outlives a workspace switch and a thread change', async () => {
    const { client, native, command } = await opened();
    await command('cclocal:env-mode', '', 'worktree');
    expect(startFromOrigin(client)).toBe(true); // newWorktreesStartFromOrigin
    branchLocal(client, 'origin', '', ''); // onStartFromOriginChange
    expect(startFromOrigin(client)).toBe(false);
    // onEnvModeChange sets only the mode.
    await command('cclocal:env-mode', '', 'local');
    await command('cclocal:env-mode', '', 'worktree');
    expect(startFromOrigin(client)).toBe(false);
    // A thread change resets the mode and the base, not the origin choice.
    await command('select-thread', 't2');
    await client.refresh(native, storage());
    await command('cclocal:env-mode', '', 'worktree');
    expect(startFromOrigin(client)).toBe(true);
    await command('select-thread', 't1');
    await client.refresh(native, storage());
    expect(serverEnvMode(client)).toBe('local');
    await command('cclocal:env-mode', '', 'worktree');
    expect(startFromOrigin(client)).toBe(false);
  });

  test('PA-9: a started thread keeps its workspace', async () => {
    const native = new Fake();
    (native.details.t1!.projection as Obj).messages = [{ id: 'm1', role: 'user', text: 'Earlier' }];
    const client = new T3Client(), disk = storage();
    await client.refresh(native, disk);
    await client.command('select-thread', 't1', '', 0, native, disk);
    await client.refresh(native, disk);
    expect(canOverrideServerEnv(client)).toBe(false);
    expect(await shellDetails(client, native, true, 't1', false)).toMatchObject({ envModeSelect: false, envMode: 'local' });
    const result = await client.command('cclocal:env-mode', '', 'worktree', 0, native, disk);
    expect(result.message).toBe('A started thread keeps its workspace.');
  });
});

// CO-5: ProjectScriptsControl's "Add project script" opens the Add Action dialog over the thread
// (openAddDialog), not Settings › Project. The hookup is Contract, so the sources are read as
// context-menu-hookup.test.ts reads them; sb:action-add itself is settings-b.test.ts's.
describe('the details card\'s Add project script', () => {
  const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();
  const nodeLine = async (file: string, testId: string) => {
    const line = (await source(file)).split('\n').find(entry => entry.includes(`testId="${testId}"`));
    if (!line) throw new Error(`${file}: no node ${testId}`);
    return line;
  };
  test('the row and the scripts menu item open the editor in place', async () => {
    // The card's row (no script yet) and the run-script menu's last item.
    for (const [file, testId] of [['shell-details.contract', 'details-add-script'], ['r6-polish.contract', 'details-scripts-add']] as const) {
      const line = await nodeLine(file, testId);
      expect(line).toContain('press=act("ui:add-script", "", "")');
      expect(line).not.toContain('ui:project-settings');
    }
    const app = await source('app.contract');
    // The root opens the card editor for the thread's project, and the op never reaches the client as a command.
    expect(app).toMatch(/if op == "ui:add-script"[^\n]*\n\s+restEditor = "card-action"\n\s+restSubject = target == "" \? data\.projectId : target\n/);
    expect(app).toMatch(/and op != "ui:add-script" and[^\n]*not commandPending\n\s+commandPending = true\n\s+send changed = command\(/);
    // ProjectDialogs mounts with the window, over the thread, and shows the settings' Add Action editor for that project.
    expect(await source('app-window.contract')).toMatch(/\n\s+ProjectDialogs\(restEditor=restEditor, restSubject=restSubject,[^\n]*projectsSettings=projectsSettings/);
    expect(await source('app-settings.contract')).toMatch(/when restEditor == "card-action"\n\s+each blank in projectsSettings\.blankActions key=blank\.id\n\s+ProjectActionEditor\(item=blank, scope=restSubject, editing=false, [^\n]*raw=restRaw, close=restClose\)/);
    // A new action's Save and Return send sb:action-add for that scope; Cancel closes.
    const editor = await source('settings-b-actions.contract');
    expect(editor.match(/raw\(\(editing \? "sb:action-save" : "sb:action-add"\), scope, payload\)/g)).toHaveLength(2);
    expect(await nodeLine('settings-b-actions.contract', 'action-cancel')).toContain('press=close');
  });
});
