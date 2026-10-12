import { describe, test, expect } from 'bun:test';
import { T3Client } from './client';
import { obj, str, type Obj } from './domain';
import type { Files, Native } from './protocol';
import { parseChangeRequestUrl, parsePullRequestReference, resolveLinkInput, threadPullRequestKey, linkMode, linkPullRequestView, LINK_DESCRIPTION } from './palette-linkpr';
import { paletteCommand } from './palette-commands';
import { paletteView } from './palette-view';
import { keyboardDispatch } from './keyboard-dispatch';

const github = { id: 'p1', title: 'Widgets', workspaceRoot: '/repo', repositoryIdentity: { provider: 'github', canonicalKey: 'github.com/acme/widgets', owner: 'acme', name: 'widgets', locator: { remoteUrl: 'git@github.com:acme/widgets.git' } } };
function client(capabilities: Obj = { threadPullRequests: true, threadPinning: true }, project: Obj = github): T3Client {
  const c = new T3Client();
  Object.assign(c, { connection: 'connected', configLive: true, shellLive: true, threadLive: true, generation: 1, environmentId: 'env1', origin: 'http://127.0.0.1:1', scopes: ['orchestration:read', 'orchestration:operate'] });
  c.config = { environment: { label: 'Local', capabilities }, keybindings: [{ command: 'thread.pin', shortcut: { key: 'p', modKey: true, shiftKey: true } }], settings: {} };
  c.shell = { sequence: 1, projects: [project], threads: [{ id: 't1', projectId: 'p1', title: 'One', updatedAt: '2026-10-03T01:00:00.000Z' }] };
  c.projectId = 'p1'; c.threadId = 't1';
  return c;
}
class Fake implements Native {
  available = true; calls: Obj[] = [];
  watch() {}
  async later(input: unknown): Promise<unknown> {
    const request = obj(input); this.calls.push(request);
    if (request.op === 'ids') return { ok: true, generation: 1, value: Array.from({ length: Number(request.count) }, (_, i) => `id-${i}`) };
    return { ok: true, generation: 1, value: {} };
  }
}
const files = (): Files => ({ fs: { async mkdir() {}, async readFile() { throw new Error('none'); }, async atomicWriteFile() {} } });

describe('change request references', () => {
  test('hosts by path shape', () => {
    expect(parseChangeRequestUrl('https://github.com/Acme/Widgets/pull/7')).toEqual({ host: 'github.com', repository: 'acme/widgets', number: 7 });
    expect(parseChangeRequestUrl('https://gitlab.example.com/group/sub/repo/-/merge_requests/3')).toEqual({ host: 'gitlab.example.com', repository: 'group/sub/repo', number: 3 });
    expect(parseChangeRequestUrl('https://codeberg.org:8443/o/r/pulls/4')).toEqual({ host: 'codeberg.org', repository: 'o/r', number: 4, authority: 'codeberg.org:8443' });
    expect(parseChangeRequestUrl('https://dev.azure.com/org/proj/_git/repo/pullrequest/9')!.repository).toBe('org/proj/_git/repo');
    expect(parseChangeRequestUrl('https://github.com/acme/widgets/issues/7')).toBeNull();
    expect(parseChangeRequestUrl('mailto:x@y.z')).toBeNull();
  });
  test('typed references: numbers, #numbers, URLs and checkout commands', () => {
    expect(parsePullRequestReference('#42')).toBe('42');
    expect(parsePullRequestReference('gh pr checkout 12')).toBe('12');
    expect(parsePullRequestReference('https://github.com/a/b/pull/1')).toBe('https://github.com/a/b/pull/1');
    expect(parsePullRequestReference('feature/x')).toBeNull();
  });
  test('resolution against the thread project and the environment host', () => {
    expect(resolveLinkInput('#42', [github], 'p1', 'multiple')).toEqual({ link: { host: 'github.com', repository: 'acme/widgets', number: 42, url: 'https://github.com/acme/widgets/pull/42' } });
    expect(resolveLinkInput('https://github.com/other/repo/pull/7', [github], 'p1', 'multiple')).toMatchObject({ link: { repository: 'other/repo', number: 7 } });
    expect(resolveLinkInput('https://github.com/other/repo/pull/7', [github], 'p1', 'single')).toEqual({ error: 'No project in this environment can read github.com/other/repo.' });
    expect(resolveLinkInput('#42', [{ id: 'p1' }], 'p1', 'multiple')).toEqual({ error: 'Paste a full URL to link a pull request from another repository.' });
    expect(resolveLinkInput('nope', [github], 'p1', 'multiple')).toBeNull();
    expect(threadPullRequestKey({ host: 'codeberg.org', repository: 'O/R', number: 4, authority: 'codeberg.org:8443' })).toEqual({ host: 'codeberg.org:8443', repository: 'o/r', number: 4 });
  });
  test('link mode follows the environment capabilities', () => {
    expect(linkMode({ environment: { capabilities: { threadPullRequests: true } } })).toBe('multiple');
    expect(linkMode({ environment: { capabilities: { threadPullRequestLinking: true } } })).toBe('single');
    expect(linkMode({})).toBe('unsupported');
  });
});

describe('Link pull request dialog', () => {
  test('validation appears once the field is touched', async () => {
    const c = client(), native = new Fake();
    let view = await paletteView(c, native, [true, 'command', 'link-pr', '', '', false, 0, 'dark']);
    expect(view).toMatchObject({ panel: 'dialog', label: 'Link pull request', placeholder: 'Pull request URL or #42', contextDescription: LINK_DESCRIPTION, empty: '', accessory: 'Link', accessoryEnabled: false });
    view = linkPullRequestView(c, 'https://github.com/acme/widgets/pull/7');
    expect(view).toMatchObject({ contextTitle: 'github.com/acme/widgets #7', accessoryEnabled: true, empty: '' });
    expect(linkPullRequestView(c, '').empty).toBe('Paste a pull request URL or enter 123 / #123.');
    expect(linkPullRequestView(c, 'zz').empty).toBe('Use a pull request URL, 123, or #123.');
    await paletteView(c, native, [true, 'command', '', '', '', false, 0, 'dark']);
    expect(linkPullRequestView(c, '').empty).toBe('');
  });
  test('submit dispatches thread.pull-request.link and closes', async () => {
    const c = client(), native = new Fake();
    const result = await paletteCommand(c, native, files(), 'link-pr', '#42', 'link-pr');
    expect(result.close).toBe(true);
    const dispatched = native.calls.map(call => obj(obj(call.payload).command)).find(command => command.type === 'thread.pull-request.link')
      ?? native.calls.map(call => obj(call.payload)).find(payload => payload.type === 'thread.pull-request.link');
    expect(dispatched).toMatchObject({ threadId: 't1', host: 'github.com', repository: 'acme/widgets', number: 42, url: 'https://github.com/acme/widgets/pull/42', source: 'manual' });
  });
  test('an invalid submit stays open and shows the validation', async () => {
    const c = client(), native = new Fake();
    expect((await paletteCommand(c, native, files(), 'link-pr', '', 'link-pr')).close).toBe(false);
    expect(linkPullRequestView(c, '').empty).toBe('Paste a pull request URL or enter 123 / #123.');
  });
});

describe('thread shortcuts', () => {
  test('thread.pin pins, or unpins a pinned thread', () => {
    const context = { composerFocus: false, editableFocus: false, turnRunning: false, modelPickerOpen: false, draftThreadRoute: false, modalOpen: false, settingsOpen: false, diffOpen: false };
    const c = client();
    expect(keyboardDispatch(c, [], '', '', context).find(item => item.command === 'thread.pin')).toMatchObject({ kind: 'command', target: 'sidebar:pin', extra: 't1', chord: 'Meta+Shift+p' });
    c.shell.threads[0]!.pinnedAt = '2026-10-03T00:00:00.000Z';
    expect(keyboardDispatch(c, [], '', '', context).find(item => item.command === 'thread.pin')).toMatchObject({ target: 'sidebar:unpin', label: 'Unpin Thread' });
    expect(keyboardDispatch(client({ threadPullRequests: true }), [], '', '', context).find(item => item.command === 'thread.pin')).toBeUndefined();
    expect(str(keyboardDispatch(c, [], '', '', context).find(item => item.command === 'thread.pin')?.chord)).toBe('Meta+Shift+p');
  });
});
