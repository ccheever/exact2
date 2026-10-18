// The right-click hookup end to end, short of the host. The context popovers' rows send their
// choices as op strings, and those ops — sent through `T3Client.command` as the host sends them —
// act with no native `contextMenu` op.
import { describe, expect, test } from 'bun:test';
import { obj, type Obj } from '../../src/client/domain';
import type { Files, Native } from '../../src/client/protocol';
import { T3Client } from '../../src/client/client';
import { filesState } from '../../src/panel/files-surface';
import { resetRemoteEditorsForTests } from '../../src/connections/remote-open';

const files: Files = { fs: { mkdir: async () => undefined, readFile: async () => { throw new Error('none'); }, atomicWriteFile: async () => undefined } } as unknown as Files;
function fixture() {
  resetRemoteEditorsForTests();
  const owner = new T3Client();
  owner.origin = 'http://127.0.0.1:41857'; owner.generation = 0; owner.environmentId = 'env'; owner.projectId = 'p'; owner.threadId = 't';
  owner.shell.projects = [{ id: 'p', title: 'project', workspaceRoot: '/srv/project' }];
  owner.config = { availableEditors: ['cursor', 'file-manager'], shellRevealInFileManager: true, environment: { platform: { os: 'darwin' } } };
  const calls: Obj[] = [];
  const native: Native = { available: true, watch() {}, later: async (input: unknown) => {
    const request = obj(input); calls.push(request);
    return { ok: true, generation: 0, value: request.op === 'contextMenu' ? { clicked: null } : request.op === 'sshHosts' ? { targets: {} } : {} };
  } };
  filesState(owner).dirs.set('', []);
  const menus = () => calls.filter(call => call.op === 'contextMenu').map(call => (call.items as Obj[]).map(item => item.label));
  return { owner, native, calls, menus, command: (op: string, id: string, value: string) => owner.command(op, id, value, 0, native, files) };
}

describe('right-click hookup', () => {
  test('a pull request number\'s and a Files tree row\'s menu choices act and ask for no native menu', async () => {
    const { calls, menus, command } = fixture();
    await command('pageslocal:pr-link-pick', '', 'copy-link\ngithub https://github.com/a/b/pull/7');
    await command('chatlocal:pr-link-pick', '', 'open-external\ngitlab https://gitlab.com/a/b/-/merge_requests/7');
    await command('chatlocal:surface-files-row-pick', 'README.md', 'copy-mention');
    expect(calls.filter(call => call.op === 'copyText').map(call => call.text)).toEqual(['https://github.com/a/b/pull/7', '[README.md](README.md)']);
    expect(menus()).toEqual([]);
  });
});
