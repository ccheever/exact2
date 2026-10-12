// context-menu-gaps: the right-click hookup end to end, short of the host. The Contract nodes carry
// the `contextmenu` handlers (read from the sources, as timeline.test.ts reads shapes), the root
// actions turn them into the op strings, and those ops — sent through `T3Client.command` as the
// host sends them — reach the native `contextMenu` op with the reference's items.
import { describe, expect, test } from 'bun:test';
import { obj, type Obj } from './domain';
import type { Files, Native } from './protocol';
import { T3Client } from './client';
import { filesState } from './r4-surfaces-files';
import { resetRemoteEditorsForTests } from './remote-open';

const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();
/** The line of `file` whose node has this testId (the handler sits on the same node). */
async function nodeLine(file: string, testId: string): Promise<string> {
  const line = (await source(file)).split('\n').find(entry => entry.includes(testId));
  if (!line) throw new Error(`${file}: no node ${testId}`);
  return line;
}

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
  test('the Contract nodes carry the handlers and the root actions name the ops', async () => {
    expect(await nodeLine('pages-prs.contract', 'testId=`pr-number-${row.number}`')).toContain('contextmenu=control("link-menu", row.linkMenu)');
    expect(await source('pages-prs.contract')).toContain('PrRowButton(row=row, scheme=scheme, compact=compact, select=select, control=control,');
    expect(await nodeLine('pages-pr-detail.contract', 'testId="pull-request-open-host"')).toContain('contextmenu=local("pr-link-menu", "", detail.linkMenu)');
    expect(await nodeLine('r4-surfaces-files.contract', 'testId=`tree-${row.path}`')).toContain('contextmenu=local("surface-files-row-menu", row.path, "")');
    const app = await source('app.contract');
    // prControl (the list's `control`) and chatLocal (the panels' and the detail's `local`).
    expect(app).toMatch(/action prControl\(op: string, value: string\)[\s\S]*?send localChanged = command\(`pageslocal:pr-\$\{op\}`, "", value, 0\)/);
    expect(app).toMatch(/action chatLocal\(op: string, id: string, value: string\)[\s\S]*?send localChanged = command\(`chatlocal:\$\{op\}`, id,/);
    const main = await source('app-main.contract');
    expect(main).toContain('control=prControl');
    expect(main).toMatch(/PrPanel\([^\n]*local=chatLocal\)/);
    expect(main).toMatch(/SurfacePanel\([^\n]*local=chatLocal/);
  });

  test('the ops those handlers send open the reference menus', async () => {
    const { menus, command } = fixture();
    await command('pageslocal:pr-link-menu', '', 'github https://github.com/a/b/pull/7');
    await command('chatlocal:pr-link-menu', '', 'gitlab https://gitlab.com/a/b/-/merge_requests/7');
    await command('chatlocal:surface-files-row-menu', 'README.md', '');
    expect(menus()).toEqual([
      ['Copy link', 'Open on GitHub'],
      ['Copy link', 'Open on GitLab'],
      ['Open', 'Reveal in Finder', 'Open with', 'Copy mention', 'Add to chat'],
    ]);
  });
});
