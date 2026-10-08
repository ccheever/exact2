// Tests for pages-pr-links-logic.ts (task pr-links-previews-and-routing). Ported with their original
// names, from T3 Code (MIT, see LICENSE-T3) at 1e2ecbd975: apps/web/src/components/CommandPalette.logic.test.ts
// ("linked pull request thread navigation"), apps/web/src/lib/openPullRequestLink.test.ts
// (changeRequestRepositoryUrl, pullRequestCandidateUrlFromReferenceAutolink, matchesLinkedPullRequestUrl) and
// packages/client-runtime/src/threadPullRequestCompatibility.test.ts ("thread pull request capability negotiation").
//
// The reference has no tests of remarkPullRequestAutolinks itself; the "pull request autolinks" cases
// below are this clone's, written from its rules (pullRequestMarkdown.logic.ts:150-192), including the
// task's acceptance fixture. Not ported: "keeps hover previews fresh after edits and turns"
// (client-runtime state/pullRequests.test.ts) tests the reference's preview atom over an AtomRegistry
// and the subscribeRefreshes stream; the clone has no atoms, and its preview read is a resource that
// re-reads on the same announcements as the detail (pages-pr-refresh.ts).
import { describe, test, expect } from 'bun:test';
import { type Obj } from './domain';
import {
  autolinkPullRequestMarkdown, changeRequestRepositoryUrl, pullRequestCandidateUrlFromReferenceAutolink, matchesLinkedPullRequestUrl, resolvePullRequestPreviewTarget,
  buildLinkedThreadActionItems, linkedThreadsLabel, isPullRequestLinked, planThreadPullRequestMutation, threadPickerCandidates, threadPullRequestKeysEqual, type LinkMode,
} from './pages-pr-links-logic';
import { filterGroups, row } from './palette';
import { linkMode } from './palette-linkpr';

const REPO = 'https://github.com/acme/web';
const links = (markdown: string) => autolinkPullRequestMarkdown(markdown, REPO);
const sha = '0123456789abcdef0123456789abcdef01234567';

describe('pull request autolinks', () => {
  test('links only #N and a commit hash, never code or a word-attached reference (acceptance fixture)', () => {
    const result = links(`See #101 and ${sha}, not \`#2\` and not a#3`);
    expect(result.links).toEqual([
      { href: `${REPO}/issues/101`, text: '#101', kind: 'reference' },
      { href: `${REPO}/commit/${sha}`, text: '0123456', kind: 'commit' },
    ]);
    expect(result.text).toBe(`See [#101](${REPO}/issues/101) and [0123456](${REPO}/commit/${sha}), not \`#2\` and not a#3`);
  });
  test('needs a non-word character before #N and none after either', () => {
    expect(links('x#1 #2a #3_ (#4) #5.').links.map(link => link.text)).toEqual(['#4', '#5']);
    expect(links('#0 #07').links).toEqual([]);
  });
  test('needs a start, a space or an opening bracket before a hash', () => {
    expect(links(`(${sha}) [${sha}] {${sha}} x${sha} -${sha}`).links.map(link => link.kind)).toEqual(['commit']);
    expect(links(`a ${sha}0`).links).toEqual([]);
    expect(links(`${sha.toUpperCase()}`).links.map(link => link.href)).toEqual([`${REPO}/commit/${sha.toUpperCase()}`]);
  });
  test('leaves fenced and indented code alone', () => {
    const fenced = links('```\nsee #1\n```\n\n~~~ts\n#2\n~~~\n\n#3');
    expect(fenced.links.map(link => link.text)).toEqual(['#3']);
    expect(fenced.text).toContain('```\nsee #1\n```');
    expect(links('text\n\n    #4 in code\n\n#5').links.map(link => link.text)).toEqual(['#5']);
  });
  test('leaves an inline code span alone, across a line break too', () => {
    expect(links('`a #1\nb` #2 ``x ` #3`` #4').links.map(link => link.text)).toEqual(['#2', '#4']);
  });
  test('leaves authored links, link references, images and their text alone', () => {
    expect(links('[fixes #1](https://example.com/#2) ![#3](i.png) [#4][ref] [#5]\n\n[ref]: https://example.com\n[#5]: https://example.com/5\n\n#6').links.map(link => link.text)).toEqual(['#6']);
    // An undefined shortcut reference is just bracketed text, so its content still links.
    expect(links('[#7]').links.map(link => link.text)).toEqual(['#7']);
  });
  test('leaves bare URLs, autolinks and raw HTML alone', () => {
    expect(links('https://example.com/page#8 <https://example.com/#9> <a href="#10">#11</a> www.example.com/#12 #13').links.map(link => link.text)).toEqual(['#11', '#13']);
    expect(links('<!-- template\nfixes #14\n-->\n\n#15').links.map(link => link.text)).toEqual(['#15']);
    expect(links('Text <!-- #16 --> #17').links.map(link => link.text)).toEqual(['#17']);
  });
  test('reads a text node edge at an emphasis delimiter as no neighbour', () => {
    expect(links('**#1** _#2_ ~~#3~~ snake_case#4').links.map(link => link.text)).toEqual(['#1', '#2', '#3']);
    expect(links(`**${sha}**`).links.map(link => link.kind)).toEqual(['commit']);
  });
  test('links each reference once in the list, every occurrence in the text', () => {
    const result = links('#1 and #1');
    expect(result.links).toHaveLength(1);
    expect(result.text).toBe(`[#1](${REPO}/issues/1) and [#1](${REPO}/issues/1)`);
  });
  test('headings, lists, quotes and tables still link', () => {
    expect(links('# Fix #1\n\n- #2\n\n> #3\n\n| a |\n| - |\n| #4 |').links.map(link => link.text)).toEqual(['#1', '#2', '#3', '#4']);
  });
  test('keeps a backslash-escaped mark, and links nothing without a repository', () => {
    expect(links('\\#1 #2').links.map(link => link.text)).toEqual(['#2']);
    expect(autolinkPullRequestMarkdown('#1', '')).toEqual({ text: '#1', links: [] });
    expect(autolinkPullRequestMarkdown('#1', `${REPO}/`).links[0]?.href).toBe(`${REPO}/issues/1`);
  });
});

describe('changeRequestRepositoryUrl', () => {
  test('preserves repository path casing', () => {
    expect(changeRequestRepositoryUrl('https://gitlab.example.test/Team/Platform/Repo/-/merge_requests/42/diffs#note_1')).toBe('https://gitlab.example.test/Team/Platform/Repo');
  });
  test('keeps pull-like segments inside nested GitLab repository paths', () => {
    expect(changeRequestRepositoryUrl('https://gitlab.example.test/group/pull/123/repo/-/merge_requests/42')).toBe('https://gitlab.example.test/group/pull/123/repo');
  });
});

describe('pullRequestCandidateUrlFromReferenceAutolink', () => {
  test("turns GitHub's shared issue route into a pull request candidate", () => {
    expect(pullRequestCandidateUrlFromReferenceAutolink('https://github.com/pingdotgg/t3code/issues/8600#issuecomment-1')).toBe('https://github.com/pingdotgg/t3code/pull/8600#issuecomment-1');
  });
  test('does not reinterpret other issue hosts or malformed references', () => {
    expect(pullRequestCandidateUrlFromReferenceAutolink('https://gitlab.com/pingdotgg/t3code/-/issues/8600')).toBeNull();
    expect(pullRequestCandidateUrlFromReferenceAutolink('https://github.com/pingdotgg/t3code/issues/not-a-number')).toBeNull();
  });
});

describe('matchesLinkedPullRequestUrl', () => {
  const linkedPullRequest = { projectId: 'project-1', repository: 'pingdotgg/t3code', number: 42, url: 'https://github.com/pingdotgg/t3code/pull/42' };
  test('matches the same pull request without looking up its project', () => {
    expect(matchesLinkedPullRequestUrl(linkedPullRequest, 'https://github.com/PingDotGG/T3Code/pull/42/files')).toBe(true);
  });
  for (const [url, expected] of [['http://forge.example:3000/git/team/repo/pulls/42/files', true], ['http://forge.example:4000/git/team/repo/pulls/42', false],
    ['http://forge.example/git/team/repo/pulls/42', false]] as const) {
    test(`matches Forgejo links by web authority: ${url}`, () => {
      expect(matchesLinkedPullRequestUrl({ ...linkedPullRequest, url: 'http://forge.example:3000/git/team/repo/pulls/42' }, url)).toBe(expected);
    });
  }
  test("keeps other providers' existing port normalization", () => {
    expect(matchesLinkedPullRequestUrl(linkedPullRequest, 'https://github.com:8443/pingdotgg/t3code/pull/42')).toBe(true);
  });
  test('keeps Forgejo and GitHub path shapes distinct on a GitHub-named host', () => {
    expect(matchesLinkedPullRequestUrl({ ...linkedPullRequest, url: 'https://github.internal/team/repo/pulls/42' }, 'https://github.internal/team/repo/pull/42')).toBe(false);
  });
  test('rejects a different pull request or host', () => {
    expect(matchesLinkedPullRequestUrl(linkedPullRequest, 'https://github.com/pingdotgg/t3code/pull/43')).toBe(false);
    expect(matchesLinkedPullRequestUrl(linkedPullRequest, 'https://github.example.com/pingdotgg/t3code/pull/42')).toBe(false);
  });
});

describe('linked pull request thread navigation', () => {
  test('keeps archived relations searchable and routes them through the PR environment', () => {
    const environmentId = 'remote', id = 'archived-thread', query = 'https://github.com/acme/web/pull/42';
    const linkedThreads = { environmentId, threads: [{ id, projectId: 'project', title: 'Completed work', archivedAt: '2026-09-01T00:00:00.000Z' }] };
    const items = buildLinkedThreadActionItems({ ...linkedThreads, query });
    // The palette's own filter (palette.ts filterGroups) with these as the thread results, as CommandPalette.tsx passes them.
    const paletteItems = items.map(item => ({ terms: item.searchTerms, row: row({ key: item.value, op: 'thread', arg: item.threadId, title: item.title, description: item.description }) }));
    const groups = filterGroups([], query, false, { projects: [], settings: [], threads: paletteItems });
    expect(groups.flatMap(group => group.items)).toEqual(paletteItems);
    expect(items[0]?.description).toBe('Archived thread');
    expect({ environmentId: items[0]?.environmentId, id: items[0]?.threadId }).toEqual({ environmentId, id });
    expect(items[0]?.value).toBe(`thread:${environmentId}:${id}`);
  });
  test('names a live relation "Linked thread" and an untitled one "Untitled thread"', () => {
    expect(buildLinkedThreadActionItems({ environmentId: 'e', threads: [{ id: 't', title: '', archivedAt: null }], query: 'q' })[0]).toMatchObject({ title: 'Untitled thread', description: 'Linked thread' });
  });
  test('the count button names how many threads link here', () => {
    expect([linkedThreadsLabel(0), linkedThreadsLabel(1), linkedThreadsLabel(2)]).toEqual(['Linked threads', 'Linked from 1 thread', 'Linked from 2 threads']);
  });
});

const reference = { host: 'github.example', repository: 'team/repo', number: 7, url: 'https://github.example/team/repo/pull/7' };
const input = { threadId: 'thread', reference, legacyProjectId: 'exact-checkout' as string | null, linked: true };
const modeOf = (capabilities: Obj | undefined): LinkMode => linkMode({ environment: { capabilities } });

describe('thread pull request capability negotiation', () => {
  for (const linked of [true, false]) {
    test(`preserves Forgejo ports in a multi-link mutation: linked=${linked}`, () => {
      const mutation = planThreadPullRequestMutation({ ...input, linked, mode: modeOf({ threadPullRequests: true }),
        reference: { host: 'forge.example', repository: 'team/repo', number: 7, url: 'http://forge.example:3000/team/repo/pulls/7' } });
      expect(mutation).toMatchObject({ host: 'forge.example:3000', number: 7 });
    });
  }
  for (const capabilities of [undefined, {}, { threadPullRequests: false, threadPullRequestLinking: false }]) {
    test(`does not dispatch when linking is unadvertised: ${JSON.stringify(capabilities)}`, () => {
      expect(modeOf(capabilities)).toBe('unsupported');
      expect(planThreadPullRequestMutation({ ...input, mode: modeOf(capabilities) })).toBeNull();
    });
  }
  test('uses metadata updates for old single-link servers, including unlink', () => {
    const mode = modeOf({ threadPullRequestLinking: true });
    expect(planThreadPullRequestMutation({ ...input, mode })).toEqual({ type: 'thread.metadata.update', threadId: 'thread',
      linkedPullRequest: { projectId: 'exact-checkout', repository: reference.repository, number: 7, url: reference.url } });
    expect(planThreadPullRequestMutation({ ...input, mode, linked: false, legacyProjectId: null })).toEqual({ type: 'thread.metadata.update', threadId: 'thread', linkedPullRequest: null });
  });
  test("uses the checkout's Azure selector only for legacy metadata commands", () => {
    const azure = { ...input, reference: { host: 'dev.azure.com', repository: 'org/project/_git/web', number: 42, url: 'https://dev.azure.com/org/project/_git/web/pullrequest/42' }, legacyRepository: 'web' };
    expect(planThreadPullRequestMutation({ ...azure, mode: modeOf({ threadPullRequestLinking: true }) })).toMatchObject({ type: 'thread.metadata.update', linkedPullRequest: { repository: 'web', number: 42 } });
    expect(planThreadPullRequestMutation({ ...azure, mode: modeOf({ threadPullRequests: true }) })).toMatchObject({ type: 'thread.pull-request.link', repository: 'org/project/_git/web', number: 42 });
  });
  test('never sends a same-host route as an exact repository to an old server', () => {
    expect(planThreadPullRequestMutation({ ...input, mode: modeOf({ threadPullRequestLinking: true }), legacyProjectId: null })).toBeNull();
  });
  for (const capabilities of [{ threadPullRequests: true }, { threadPullRequests: true, threadPullRequestLinking: true }]) {
    test(`prefers multi-link commands when available: ${JSON.stringify(capabilities)}`, () => {
      expect(planThreadPullRequestMutation({ ...input, mode: modeOf(capabilities), legacyProjectId: null })).toEqual({ type: 'thread.pull-request.link', threadId: 'thread', ...reference, source: 'manual' });
      expect(planThreadPullRequestMutation({ ...input, mode: modeOf(capabilities), linked: false })).toEqual({ type: 'thread.pull-request.unlink', threadId: 'thread',
        host: reference.host, repository: reference.repository, number: reference.number });
    });
  }
});

describe('whether a thread links a pull request (usePullRequestLinking.isLinked)', () => {
  const url = 'https://github.com/acme/web/pull/101';
  test('reads the visible links of a multi-link thread, host and repository case-insensitive', () => {
    expect(isPullRequestLinked({ pullRequests: [{ host: 'GitHub.com', repository: 'Acme/Web', number: 101, url }] }, url, 'multiple')).toBe(true);
    expect(isPullRequestLinked({ pullRequests: [{ host: 'github.com', repository: 'acme/web', number: 101, url, source: 'stack-dismissed' }] }, url, 'multiple')).toBe(false);
    expect(isPullRequestLinked({ pullRequests: [{ host: 'github.com', repository: 'acme/web', number: 102 }] }, url, 'multiple')).toBe(false);
  });
  test('reads the one legacy link of a single-link thread, and nothing when linking is unsupported', () => {
    const thread = { linkedPullRequest: { projectId: 'p', repository: 'acme/web', number: 101, url } };
    expect(isPullRequestLinked(thread, url, 'single')).toBe(true);
    expect(isPullRequestLinked(thread, url, 'unsupported')).toBe(false);
    expect(isPullRequestLinked(null, url, 'multiple')).toBe(false);
  });
  test('compares keys the way stored links are normalized', () => {
    expect(threadPullRequestKeysEqual({ host: 'forge.example', repository: 'team/repo', number: 7, url: 'http://forge.example:3000/team/repo/pulls/7' },
      { host: 'forge.example:3000', repository: 'Team/Repo', number: 7 })).toBe(true);
  });
});

describe('the thread picker (PullRequestThreadLinks ThreadPicker)', () => {
  const url = 'https://github.com/acme/web/pull/101';
  const threads = [
    { id: 'a', projectId: 'p1', title: 'Older', updatedAt: '2026-10-01T00:00:00Z', archivedAt: null },
    { id: 'b', projectId: 'p2', title: 'Newer', updatedAt: '2026-10-02T00:00:00Z', archivedAt: null, pullRequests: [{ host: 'github.com', repository: 'acme/web', number: 101 }] },
    { id: 'c', projectId: 'p1', title: 'Archived', updatedAt: '2026-10-03T00:00:00Z', archivedAt: '2026-10-04T00:00:00Z' },
    { id: 'd', projectId: 'p1', title: '', updatedAt: '2026-09-01T00:00:00Z', archivedAt: null },
  ];
  const projects = [{ id: 'p1', title: 'Web' }, { id: 'p2', title: 'Docs' }];
  test('lists the unarchived threads newest first, each saying whether it already links the pull request', () => {
    expect(threadPickerCandidates({ threads, projects, query: '', url, mode: 'multiple' })).toEqual([
      { id: 'b', title: 'Newer', project: 'Docs', linked: true },
      { id: 'a', title: 'Older', project: 'Web', linked: false },
      { id: 'd', title: 'Untitled thread', project: 'Web', linked: false },
    ]);
  });
  test('searches the title and the project name together', () => {
    expect(threadPickerCandidates({ threads, projects, query: '  docs ', url, mode: 'multiple' }).map(thread => thread.id)).toEqual(['b']);
    expect(threadPickerCandidates({ threads, projects, query: 'older web', url, mode: 'multiple' }).map(thread => thread.id)).toEqual(['a']);
    expect(threadPickerCandidates({ threads, projects, query: 'nothing', url, mode: 'multiple' })).toEqual([]);
  });
});

describe('resolvePullRequestPreviewTarget', () => {
  const projects = [
    { id: 'p1', environmentId: 'env-1', repositoryIdentity: { provider: 'github', canonicalKey: 'github.com/acme/web', owner: 'acme', name: 'web', displayName: 'acme/web' } },
    { id: 'p2', environmentId: 'env-2', repositoryIdentity: { provider: 'github', canonicalKey: 'github.com/acme/api', owner: 'acme', name: 'api', displayName: 'acme/api' } },
  ];
  test("reads a link through this environment's own project for the repository", () => {
    expect(resolvePullRequestPreviewTarget({ environmentId: 'env-1', projects, pullRequestsEnabled: true, url: 'https://github.com/acme/web/pull/7' }))
      .toEqual({ environmentId: 'env-1', input: { projectId: 'p1', host: 'github.com', repository: 'acme/web', number: 7 } });
  });
  test("leaves a link ordinary on another environment's project, without pull requests, or for no environment", () => {
    expect(resolvePullRequestPreviewTarget({ environmentId: 'env-1', projects, pullRequestsEnabled: true, url: 'https://github.com/acme/api/pull/7' })).toBeNull();
    expect(resolvePullRequestPreviewTarget({ environmentId: 'env-1', projects, pullRequestsEnabled: false, url: 'https://github.com/acme/web/pull/7' })).toBeNull();
    expect(resolvePullRequestPreviewTarget({ environmentId: null, projects, pullRequestsEnabled: true, url: 'https://github.com/acme/web/pull/7' })).toBeNull();
    expect(resolvePullRequestPreviewTarget({ environmentId: 'env-1', projects, pullRequestsEnabled: true, url: 'https://github.com/acme/web/issues/7' })).toBeNull();
  });
  test("a #N autolink's candidate URL is read as the pull request it may be", () => {
    const candidate = pullRequestCandidateUrlFromReferenceAutolink(`${REPO}/issues/12`)!;
    expect(resolvePullRequestPreviewTarget({ environmentId: 'env-1', projects, pullRequestsEnabled: true, url: candidate })?.input).toEqual({ projectId: 'p1', host: 'github.com', repository: 'acme/web', number: 12 });
  });
});
