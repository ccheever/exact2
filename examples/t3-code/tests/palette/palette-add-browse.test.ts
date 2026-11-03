// T3 Code 5d3d78ab93 (MIT; see LICENSE-T3): the add-project folder browser's Tab (CommandPalette.tsx handleKeyDown,
// client-runtime state/filesystem.ts getFilesystemBrowseTabCompletion, its test ported as is), its listing, kept only
// while the browser shows it (BROWSE_IDLE_TTL_MS), and the GitHub repository step's owner-only refusal
// (operations/projects.ts githubIncompleteOwnerRepositoryMessage, its test ported as is).
import { afterEach, describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { obj, str, type Obj } from '../../src/client/domain';
import type { Files, Native } from '../../src/client/protocol';
import { browseTabCompletion, githubIncompleteOwnerRepositoryMessage } from '../../src/palette/palette-add';
import { paletteCommand } from '../../src/palette/palette-commands';
import { toasts } from '../../src/shell/toast';
import { paletteView } from '../../src/palette/palette-view';
import { fleet } from '../../src/connections/settings-b-fleet';
import { AUTH_STANDARD_CLIENT_SCOPES } from '../../src/connections/remote-scopes';

afterEach(() => { fleet.entries.clear(); fleet.saved = []; });

class Fake implements Native {
  available = true; calls: Obj[] = []; entries: Obj[] = [];
  watch() {}
  async later(input: unknown): Promise<unknown> {
    const request = obj(input); this.calls.push(request);
    const generation = Number(request.generation) || 1;
    if (request.op === 'ids') return { ok: true, generation, value: Array.from({ length: Number(request.count) }, (_, i) => `id-${i}`) };
    return { ok: true, generation, value: request.method === 'filesystem.browse' ? { parentPath: '/work/', entries: this.entries } : {} };
  }
}
function client(): T3Client {
  const c = new T3Client();
  Object.assign(c, { connection: 'connected', configLive: true, shellLive: true, threadLive: true, generation: 1, environmentId: 'env-one', origin: 'http://127.0.0.1:16580', scopes: [...AUTH_STANDARD_CLIENT_SCOPES] });
  c.config = { environment: { label: 'Laptop' }, settings: {}, keybindings: [] };
  c.shell = { sequence: 1, projects: [], threads: [] };
  return c;
}
const folders = (...names: string[]) => names.map(name => ({ name, fullPath: `/work/${name}` }));
const browse = (c: T3Client, native: Fake, query: string, open = true) => paletteView(c, native, [open, 'command', 'add-project/browse', query, '', false, 0, 'dark']);
const reads = (native: Fake) => native.calls.filter(call => str(call.method) === 'filesystem.browse').length;

describe('Tab in the folder browser', () => {
  test('completes Tab like a shell', () => {
    expect(browseTabCompletion(['Creations'], 'Creation')).toEqual({ name: 'Creations', isComplete: true });
    expect(browseTabCompletion(['OpenSourced', 'OpenAI'], 'op')).toEqual({ name: 'Open', isComplete: false });
    expect(browseTabCompletion(['Open', 'Opal'], 'Op')).toBeNull();
    expect(browseTabCompletion(['openAI', 'OpenSource'], 'o')).toEqual({ name: 'open', isComplete: false });
    expect(browseTabCompletion(['😀one', '😁two'], '')).toBeNull();
    expect(browseTabCompletion(['ΟΣα', 'οσβ'], 'ο')).toBeNull();
    expect(browseTabCompletion(['İfoo', 'İfar'], 'i̇')).toEqual({ name: 'İf', isComplete: false });
    expect(browseTabCompletion(['😀one', '😀other'], '😀')).toEqual({ name: '😀o', isComplete: false });
    expect(browseTabCompletion([], 'x')).toBeNull();
  });

  test('a sole match enters the folder; several extend the query to their common prefix', async () => {
    const c = client(), native = new Fake();
    native.entries = folders('Creations', 'OpenAI', 'OpenSourced');
    expect(await browse(c, native, '/work/Cre')).toMatchObject({ tabOp: 'browse-to', tabArg: '/work/Creations/' });
    expect(await browse(c, native, '/work/op')).toMatchObject({ tabOp: 'browse-to', tabArg: '/work/Open' });
    // Nothing to add: Tab is taken and does nothing.
    expect(await browse(c, native, '/work/')).toMatchObject({ tabOp: 'stay', tabArg: '' });
    expect(await browse(c, native, '/work/zzz')).toMatchObject({ tabOp: 'stay', tabArg: '' });
  });

  test('outside the browser Tab stays the dialog\'s', async () => {
    const c = client(), native = new Fake();
    expect((await paletteView(c, native, [true, 'command', '', 'new', '', false, 0, 'dark'])).tabOp).toBe('');
  });
});

test('the field reads a pasted path wherever it could browse one', async () => {
  const c = client(), native = new Fake();
  const pastes = async (page: string) => (await paletteView(c, native, [true, 'command', page, '', '', false, 0, 'dark'])).pastePaths;
  expect(await pastes('')).toBe(true);
  expect(await pastes('add-project/browse')).toBe(true);
  expect(await pastes('add-project/clone:github/confirm')).toBe(true);
  expect(await pastes('add-project/clone:github')).toBe(false);
  expect(await pastes('add-project/new-project')).toBe(false);
  expect((await paletteView(c, native, [true, 'files', '', '', '', false, 0, 'dark'])).pastePaths).toBe(false);
});

describe('the folder listing', () => {
  test('is read again when the browser opens again, so new folders appear', async () => {
    const c = client(), native = new Fake();
    native.entries = folders('alpha');
    expect((await browse(c, native, '/work/')).rows.map(row => row.title)).toContain('alpha');
    // Typing within the folder reuses its listing.
    await browse(c, native, '/work/a');
    expect(reads(native)).toBe(1);
    native.entries = folders('alpha', 'beta');
    await browse(c, native, '/work/', false);
    expect((await browse(c, native, '/work/')).rows.map(row => row.title)).toContain('beta');
    expect(reads(native)).toBe(2);
  });
});

describe('the GitHub repository step', () => {
  test('asks for a repository name when GitHub input is only an owner path', () => {
    for (const input of ['pingdotgg/', '  pingdotgg//  ', 'mona-cat_octo/']) expect(githubIncompleteOwnerRepositoryMessage('github', input)).toBe('Enter the full owner/repository name.');
    for (const input of ['pingdotgg/t3code', 'mona-cat_octo/repo', 'pingdotgg', 'https://github.com/pingdotgg/t3code.git', 'git@github.com:pingdotgg/t3code.git']) {
      expect(githubIncompleteOwnerRepositoryMessage('github', input)).toBeNull();
    }
    expect(githubIncompleteOwnerRepositoryMessage('gitlab', 'group/')).toBeNull();
    expect(githubIncompleteOwnerRepositoryMessage('url', 'owner/')).toBeNull();
    expect(githubIncompleteOwnerRepositoryMessage('forgejo', 'owner/')).toBeNull();
  });

  test('an owner-only input stays on the step with a toast, and looks nothing up', async () => {
    const c = client(), native = new Fake();
    const files: Files = { fs: { async mkdir() {}, async readFile() { throw new Error('none'); }, async atomicWriteFile() {} } };
    const result = await paletteCommand(c, native, files, 'clone-repository:github', 'pingdotgg/', 'add-project/clone:github');
    expect(result).toMatchObject({ close: false, page: '-' });
    expect(toasts(c).map(toast => [toast.title, toast.description])).toEqual([['Repository name incomplete', 'Enter the full owner/repository name.']]);
    expect(native.calls.some(call => str(call.method) === 'sourceControl.lookupRepository')).toBe(false);
  });
});
