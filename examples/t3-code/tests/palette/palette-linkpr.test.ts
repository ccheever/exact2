import { describe, test, expect } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { obj, str, type Obj } from '../../src/client/domain';
import type { Files, Native } from '../../src/client/protocol';
import { changeRequestTermsForUrl, claimChangeRequestUrl, parseChangeRequestUrl, parsePullRequestReference, resolveLinkInput, threadPullRequestKey, linkMode, linkPullRequestView, LINK_DESCRIPTION } from '../../src/palette/palette-linkpr';
import { paletteCommand } from '../../src/palette/palette-commands';
import { paletteView } from '../../src/palette/palette-view';
import { keyboardDispatch } from '../../src/shell/keyboard-dispatch';

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
  // registry.ts claimChangeRequestUrlsFirst: GitCafe, then Azure DevOps, before the hosts that take routes on any hostname.
  test('Azure DevOps claims its change requests before Forgejo', () => {
    const claimed = claimChangeRequestUrl('https://tfs.example.test/org/_git/repo/pullrequest/9/pulls/2');
    expect(claimed).toEqual({ kind: 'azure-devops', link: { host: 'tfs.example.test', repository: 'org/_git/repo', number: 9 } });
  });
  test('typed references: numbers, #numbers, URLs and checkout commands', () => {
    expect(parsePullRequestReference('#42')).toBe('42');
    expect(parsePullRequestReference('gh pr checkout 12')).toBe('12');
    expect(parsePullRequestReference('https://github.com/a/b/pull/1')).toBe('https://github.com/a/b/pull/1');
    expect(parsePullRequestReference('feature/x')).toBeNull();
  });
  // gitlab and bitbucket definition.test.ts, pullRequestLinkInput.test.ts at 2359643cff.
  test('self-hosted GitLab merge request URLs, http included, and only those it can parse', () => {
    for (const url of ['https://code.example.com/group/project/-/merge_requests/42', 'http://git.internal/group/sub/project/-/merge_requests/7',
      'https://code.acme.test/a/b/c/-/merge_requests/7?tab=diffs', 'http://git.internal/a/b/c/-/merge_requests/7#note_1', 'https://code.acme.test/a/b/c/-/merge_requests/7/diffs']) {
      expect(parsePullRequestReference(url)).toBe(url);
    }
    for (const url of ['https://gitlab.com/g/p?x=/-/merge_requests/5', 'https://gitlab.com/g/p#/-/merge_requests/5', 'https://gitlab.com?x=/g/p/-/merge_requests/5',
      'https://gitlab.com#/g/p/-/merge_requests/5', 'https://gitlab.com/t3code/-/merge_requests/12', 'https://gitlab.com/g//-/merge_requests/5']) {
      expect(parsePullRequestReference(url)).toBeNull();
    }
  });
  test('Bitbucket pull request URLs on Bitbucket hosts', () => {
    for (const url of ['https://bitbucket.org/workspace/repo/pull-requests/5/diff', 'https://git.bitbucket.acme.test/workspace/repo/pull-requests/5']) {
      expect(parsePullRequestReference(url)).toBe(url);
    }
    for (const url of ['https://scm.example.test/workspace/repo/pull-requests/5', 'https://bitbucket.org/workspace/repo/src/main']) {
      expect(parsePullRequestReference(url)).toBeNull();
    }
  });
  test('resolution against the thread project and the environment host', () => {
    expect(resolveLinkInput('#42', [github], 'p1', 'multiple')).toEqual({ link: { host: 'github.com', repository: 'acme/widgets', number: 42, url: 'https://github.com/acme/widgets/pull/42' } });
    expect(resolveLinkInput('https://github.com/other/repo/pull/7', [github], 'p1', 'multiple')).toMatchObject({ link: { repository: 'other/repo', number: 7 } });
    expect(resolveLinkInput('https://github.com/other/repo/pull/7', [github], 'p1', 'single')).toEqual({ error: 'No project in this environment can read github.com/other/repo.' });
    // names the missing repository when resolving a bare number (pullRequestLinkInput.test.ts)
    for (const reference of ['12', '#12']) expect(resolveLinkInput(reference, [{ id: 'p1' }], 'p1', 'multiple')).toEqual({ error: 'Paste the full pull request URL; this thread has no repository to resolve the number against.' });
    expect(resolveLinkInput('nope', [github], 'p1', 'multiple')).toBeNull();
    expect(threadPullRequestKey({ host: 'codeberg.org', repository: 'O/R', number: 4, authority: 'codeberg.org:8443' })).toEqual({ host: 'codeberg.org:8443', repository: 'o/r', number: 4 });
  });
  test('GitCafe: both routes on its own hosts, stored at the canonical /pulls/<n> (changeRequestUrl.test.ts, pullRequestLinkInput.test.ts)', () => {
    for (const path of ['pull', 'pulls']) expect(parseChangeRequestUrl(`https://git.cafe/PingDotGG/T3Code/${path}/23897`)).toEqual({ host: 'git.cafe', repository: 'pingdotgg/t3code', number: 23897 });
    expect(parseChangeRequestUrl('https://staging.git.cafe/team/repo/pull/7/files')).toEqual({ host: 'staging.git.cafe', repository: 'team/repo', number: 7 });
    for (const link of ['https://git.cafe/pingdotgg/t3code/issues/23897', 'https://git.cafe/pingdotgg/t3code/pull/abc', 'https://git.cafe/pingdotgg/t3code']) expect(parseChangeRequestUrl(link)).toBeNull();
    const gitcafe = { id: 'p1', repositoryIdentity: { provider: 'gitcafe', canonicalKey: 'git.cafe/acme/web', owner: 'acme', name: 'web', locator: { remoteUrl: 'git@git.cafe:acme/web.git' } } };
    for (const host of ['git.cafe', 'staging.git.cafe']) {
      for (const [repository, route] of [['web', 'pull'], ['web', 'pulls'], ['pull', 'pull'], ['pulls', 'pulls']]) {
        const url = `https://${host}/acme/${repository}/${route}/42`;
        expect(parsePullRequestReference(url)).toBe(url);
        expect(resolveLinkInput(url, [gitcafe], 'p1', 'multiple')).toEqual({ link: { host, repository: `acme/${repository}`, number: 42, url: `https://${host}/acme/${repository}/pulls/42` } });
      }
    }
    // GitHub Enterprise Server runs on any hostname: the route alone names a GitHub pull request.
    expect(parseChangeRequestUrl('https://code.acme.dev/Team/Repo/pull/5')).toEqual({ host: 'code.acme.dev', repository: 'team/repo', number: 5 });
  });
  test("a number names the project host's own term when the host has no URL for it", () => {
    const gitlab = { id: 'p1', repositoryIdentity: { provider: 'gitlab', canonicalKey: 'gitlab.com/web', displayName: 'web', locator: { remoteUrl: 'git@gitlab.com:web.git' } } };
    expect(resolveLinkInput('#42', [gitlab], 'p1', 'multiple')).toEqual({ error: "Paste a full URL; this project's host has no known merge request URL." });
    const unknown = { id: 'p1', repositoryIdentity: { canonicalKey: 'git.example/acme/web', owner: 'acme', name: 'web', locator: { remoteUrl: 'git@git.example:acme/web.git' } } };
    expect(resolveLinkInput('42', [unknown], 'p1', 'multiple')).toEqual({ error: "Paste a full URL; this project's host has no known change request URL." });
    // A project that names its browser URL resolves below it.
    const mounted = { id: 'p1', repositoryIdentity: { provider: 'gitlab', canonicalKey: 'git.example/group/web', displayName: 'group/web', webUrl: 'https://git.example/code/group/web/', locator: { remoteUrl: 'git@git.example:group/web.git' } } };
    expect(resolveLinkInput('#7', [mounted], 'p1', 'multiple')).toMatchObject({ link: { url: 'https://git.example/code/group/web/-/merge_requests/7', number: 7 } });
    // A link's own host names it; a URL no host claims reads as GitHub's.
    expect(changeRequestTermsForUrl('https://gitlab.example.com/group/web/-/merge_requests/3')).toMatchObject({ shortLabel: 'MR', plural: 'merge requests' });
    expect(changeRequestTermsForUrl('https://example.com/nothing')).toMatchObject({ shortLabel: 'PR', singular: 'pull request' });
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
  test('a GitCafe /pull/<n> link is stored at its canonical /pulls/<n>', async () => {
    const gitcafe = { id: 'p1', title: 'Web', workspaceRoot: '/repo', repositoryIdentity: { provider: 'gitcafe', canonicalKey: 'git.cafe/acme/web', owner: 'acme', name: 'web', locator: { remoteUrl: 'git@git.cafe:acme/web.git' } } };
    const c = client({ threadPullRequests: true }, gitcafe), native = new Fake();
    expect(linkPullRequestView(c, 'https://git.cafe/Acme/Web/pull/12').contextTitle).toBe('git.cafe/acme/web #12');
    expect((await paletteCommand(c, native, files(), 'link-pr', 'https://git.cafe/Acme/Web/pull/12', 'link-pr')).close).toBe(true);
    const dispatched = native.calls.map(call => obj(obj(call.payload).command)).find(command => command.type === 'thread.pull-request.link')
      ?? native.calls.map(call => obj(call.payload)).find(payload => payload.type === 'thread.pull-request.link');
    expect(dispatched).toMatchObject({ host: 'git.cafe', repository: 'acme/web', number: 12, url: 'https://git.cafe/acme/web/pulls/12' });
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
