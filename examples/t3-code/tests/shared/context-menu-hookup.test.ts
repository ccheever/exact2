// The right-click hookup end to end, short of the host. The Contract nodes carry
// the `contextmenu` handlers (read from the sources, as timeline.test.ts reads shapes), the root
// actions turn them into the op strings, and those ops — sent through `T3Client.command` as the
// host sends them — reach the native `contextMenu` op with the reference's items.
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
  return { owner, native, menus, command: (op: string, id: string, value: string) => owner.command(op, id, value, 0, native, files) };
}

describe('right-click hookup', () => {
  test('the ops those handlers send open the reference menus; a number\'s menu choice asks for no native menu', async () => {
    const { menus, command } = fixture();
    await command('pageslocal:pr-link-pick', '', 'copy-link\ngithub https://github.com/a/b/pull/7');
    await command('chatlocal:pr-link-pick', '', 'open-external\ngitlab https://gitlab.com/a/b/-/merge_requests/7');
    await command('chatlocal:surface-files-row-menu', 'README.md', '');
    expect(menus()).toEqual([
      ['Open', 'Reveal in Finder', 'Open with', 'Copy mention', 'Add to chat'],
    ]);
  });
});
