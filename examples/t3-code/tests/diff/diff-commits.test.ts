// The Diff pane's Commits source and code-host commit links (T3 Code 2359643cff, MIT, see LICENSE-T3: DiffPanel.tsx,
// diffPanelStore.ts selectCommit, lib/openPullRequestLink.ts, changeRequestLinks.ts projectCommitUrl and each host's
// commitUrl / parseCommitUrl), over the clone's diff.ts, diff-commits.ts and diff-commit-links.ts.
import { describe, expect, test, beforeEach } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { obj, type Obj } from '../../src/client/domain';
import type { Native, Files } from '../../src/client/protocol';
import { diffRequest, diffSnapshot, reviewSection } from '../../src/diff/diff';
import { commitsView, emptyBranchCommits, noteCommitsMenu, parseCommitUrl, projectCommitUrl, readBranchCommits } from '../../src/diff/diff-commits';
import { openCommitLink } from '../../src/diff/diff-commit-links';
import { filePatchPayload } from '../../src/diff/diff-lazy';
import { panelState } from '../../src/panel/panel-surfaces';
import { externalOpensForTests, resetExternalOpensForTests } from '../../src/client/external-open';
import { noteNow } from '../../src/composer/composer-controls';

const SHA = 'a'.repeat(40), OTHER = 'b'.repeat(40);
const patch = 'diff --git a/a.ts b/a.ts\n--- a/a.ts\n+++ b/a.ts\n@@ -1,1 +1,1 @@\n-one\n+two\n';
const github = { provider: 'github', canonicalKey: 'github.com/acme/app', displayName: 'acme/app', owner: 'acme', name: 'app', locator: { remoteUrl: 'git@github.com:acme/app.git' } };

function harness(options: { commits?: Obj | Error; localCommits?: string[] } = {}) {
  const calls: Obj[] = [];
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    if (request.op === 'request' && request.method === 'review.getBranchCommits') {
      const answer = options.commits ?? { commits: [{ sha: SHA, subject: 'Add the thing', committedAt: '2026-11-01T10:00:00Z' }], truncated: false };
      return answer instanceof Error ? { ok: false, generation: 1, error: { kind: 'Server', message: answer.message, uncertain: false } } : { ok: true, generation: 1, value: answer };
    }
    if (request.op === 'request' && request.method === 'review.getDiffPreview') {
      const payload = obj(request.payload), sha = typeof payload.commitSha === 'string' ? payload.commitSha : '';
      if (sha) {
        if (!(options.localCommits ?? [SHA]).includes(sha)) return { ok: false, generation: 1, error: { kind: 'Server', message: 'Selected commit is invalid or no longer available.', uncertain: false } };
        return { ok: true, generation: 1, value: { cwd: '/repo', sources: [{ id: sha, kind: 'commit', title: `Commit ${sha.slice(0, 7)}`, commitSubject: 'Add the thing', baseRef: 'c'.repeat(40), headRef: sha, diff: patch, diffHash: 'h', truncated: false }] } };
      }
      return { ok: true, generation: 1, value: { cwd: '/repo', sources: [{ kind: 'working-tree', diff: '', truncated: false }, { kind: 'branch-range', diff: patch, truncated: false, baseRef: 'main', headRef: 'feature' }] } };
    }
    return { ok: true, generation: 1, value: {} };
  } };
  const storage: Files = { fs: { async mkdir() {}, async readFile() { throw new Error('missing'); }, async atomicWriteFile() {} } };
  const client = new T3Client();
  Object.assign(client, { available: true, generation: 1, connection: 'connected', environmentId: 'env', projectId: 'p1', threadId: 't1',
    loaded: true, configLive: true, shellLive: true, threadLive: true, scopes: ['orchestration:read', 'orchestration:operate', 'filesystem:read'], config: { environment: { capabilities: { pullRequests: true } } } });
  client.shell.projects = [{ id: 'p1', title: 'Fixture', workspaceRoot: '/repo', repositoryIdentity: github }];
  client.thread = { projection: { thread: { id: 't1' }, runtimeRequests: [], turnItems: [], runs: [], checkpoints: [] }, sequence: 0, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: null };
  const command = (op: string, id = '', value = '', n = 0) => client.command(op, id, value, n, native, storage);
  return { client, native, calls, command };
}
const requests = (calls: Obj[], method: string) => calls.filter(call => call.op === 'request' && call.method === method).map(call => obj(call.payload));

describe('the Commits submenu', () => {
  test('the open Diff reads the branch commits for its comparison; the submenu keeps the list it opened with', async () => {
    const { client, calls, command } = harness();
    noteNow(client, Date.parse('2026-11-01T12:00:00Z'));
    await command('diff');
    expect(requests(calls, 'review.getBranchCommits')).toEqual([{ cwd: '/repo' }]);
    let view = diffSnapshot(client, Date.parse('2026-11-01T12:00:00Z')).diffCommits;
    expect(view.rows).toEqual([{ id: `commit:${SHA}`, sha: SHA, short: 'aaaaaaa', subject: 'Add the thing', time: '2h ago', url: `https://github.com/acme/app/commit/${SHA}`, selected: false }]);
    expect(view.note).toBe('');
    // A read within 5 s is not repeated; opening the submenu reads again but shows the list it opened with.
    await command('diff-refresh');
    expect(requests(calls, 'review.getBranchCommits')).toHaveLength(1);
    await command('diffreview', 'commits-open');
    expect(requests(calls, 'review.getBranchCommits')).toHaveLength(2);
    expect(client.diffState.commits.frozen).not.toBeNull();
    await command('diffreview', 'commits-close');
    expect(client.diffState.commits.frozen).toBeNull();
    // The comparison target goes with the read.
    await command('diff-base', '', 'origin/main');
    expect(requests(calls, 'review.getBranchCommits').at(-1)).toEqual({ cwd: '/repo', baseRef: 'origin/main' });
    view = diffSnapshot(client, 0).diffCommits;
    expect(view.rows).toHaveLength(1);
  });

  test('status rows: error, file access, loading, empty, truncated', () => {
    const base = { canRead: true, selectedSha: null, identity: null, now: 0 };
    const state = emptyBranchCommits();
    expect(commitsView(state, base).note).toBe('Loading commits...');
    expect(commitsView(state, { ...base, canRead: false }).note).toBe('This connection cannot read local diffs.');
    state.data = { commits: [], truncated: false };
    expect(commitsView(state, base).note).toBe('No commits on this branch.');
    state.data = { commits: Array.from({ length: 100 }, (_, index) => ({ sha: index.toString(16).padStart(40, '0'), subject: `c${index}`, committedAt: '' })), truncated: true };
    expect(commitsView(state, base).note).toBe('Showing latest 100 commits');
    expect(commitsView(state, base).rows[0]!.url).toBe('');
    state.error = 'Could not read branch commit metadata.';
    expect(commitsView(state, base).note).toBe('Could not read branch commit metadata.');
    // The open submenu keeps its list while a newer answer lands.
    const kept = emptyBranchCommits();
    kept.data = { commits: [{ sha: SHA, subject: 'old', committedAt: '' }], truncated: false };
    noteCommitsMenu(kept, true);
    kept.data = { commits: [{ sha: OTHER, subject: 'new', committedAt: '' }], truncated: false };
    expect(commitsView(kept, base).rows.map(row => row.subject)).toEqual(['old']);
  });

  test('a failed read shows its message; another cwd starts from nothing', async () => {
    const state = emptyBranchCommits();
    const send = async () => { throw new Error('Review commit list cwd must stay within the configured workspace root.'); };
    await readBranchCommits(state, { cwd: '/repo', baseRef: null, threadKey: 'env:t1', canRead: true }, send, 0);
    expect(state.error).toBe('Review commit list cwd must stay within the configured workspace root.');
    await readBranchCommits(state, { cwd: '/other', baseRef: null, threadKey: 'env:t1', canRead: true }, async () => ({ commits: [], truncated: false }), 1);
    expect([state.error, state.data]).toEqual(['', { commits: [], truncated: false }]);
  });
});

describe('a commit selected', () => {
  test('asks for that commit, reads by its subject once answered, names its SHA, and goes back to Changes when chosen again', async () => {
    const { client, calls, command } = harness();
    await command('diff');
    await command('diff-base', '', 'origin/main');
    await command('diff-scope', '', `commit:${SHA}`);
    expect(requests(calls, 'review.getDiffPreview').at(-1)).toEqual({ cwd: '/repo', baseRef: 'origin/main', commitSha: SHA, ignoreWhitespace: true });
    const view = diffSnapshot(client, 0);
    expect(view).toMatchObject({ diffScope: `commit:${SHA}`, diffScopeLabel: 'Add the thing', diffCanRefresh: true, diffAdditions: 1, diffDeletions: 1, diffCompare: false });
    expect(view.diffCommits).toMatchObject({ sha: 'aaaaaaa', url: `https://github.com/acme/app/commit/${SHA}` });
    expect(view.diffCommits.rows[0]!.selected).toBe(true);
    expect(reviewSection(client)).toEqual({ id: `commit:${SHA}`, title: 'Add the thing' });
    // Its file patches and contents name the commit.
    expect(filePatchPayload(client.diffState.source!, { path: 'a.ts', previousPath: null, additions: 1, deletions: 1 }, true))
      .toEqual({ cwd: '/repo', commitSha: SHA, ignoreWhitespace: true, file: { path: 'a.ts', previousPath: null, sourceKind: 'commit' } });
    // The same commit again is Changes, on the comparison target the commit kept.
    await command('diff-scope', '', `commit:${SHA}`);
    expect(diffSnapshot(client, 0)).toMatchObject({ diffScope: 'branch', diffScopeLabel: 'Changes' });
    expect(diffRequest(client).payload).toEqual({ cwd: '/repo', baseRef: 'origin/main', ignoreWhitespace: true });
  });

  test('before its answer the trigger reads Commit and the section its short SHA; loading text', () => {
    const { client } = harness();
    client.diffState.selections['env:t1'] = { kind: 'commit', runId: '', filePath: '', baseRef: null, sha: OTHER };
    expect(diffSnapshot(client, 0)).toMatchObject({ diffScopeLabel: 'Commit', diffLoadingLabel: 'Loading commit...' });
    expect(reviewSection(client).title).toBe('Commit bbbbbbb');
  });
});

describe('commit URLs', () => {
  test('each host builds its commit page below the repository', () => {
    const identity = (provider: string, canonicalKey: string, extra: Obj = {}) => ({ provider, canonicalKey, locator: { remoteUrl: '' }, ...extra });
    expect(projectCommitUrl(identity('github', 'github.com/acme/app'), SHA)).toBe(`https://github.com/acme/app/commit/${SHA}`);
    expect(projectCommitUrl(identity('github', 'github.com/acme/app/extra'), SHA)).toBeNull();
    expect(projectCommitUrl(identity('gitlab', 'gitlab.com/group/sub/app'), SHA)).toBe(`https://gitlab.com/group/sub/app/-/commit/${SHA}`);
    expect(projectCommitUrl(identity('forgejo', 'git.example.com/acme/app', { locator: { remoteUrl: 'https://git.example.com:3000/acme/app.git' } }), SHA))
      .toBe(`https://git.example.com:3000/acme/app/commit/${SHA}`);
    expect(projectCommitUrl(identity('bitbucket', 'bitbucket.org/acme/app'), SHA)).toBe(`https://bitbucket.org/acme/app/commits/${SHA}`);
    expect(projectCommitUrl(identity('azure-devops', 'ssh.dev.azure.com/v3/org/proj/repo'), SHA)).toBe(`https://dev.azure.com/org/proj/_git/repo/commit/${SHA}`);
    expect(projectCommitUrl(identity('github', 'github.com/acme/app', { webUrl: 'https://ghe.example.com/acme/app/' }), SHA)).toBe(`https://ghe.example.com/acme/app/commit/${SHA}`);
    expect(projectCommitUrl(identity('github', 'github.com/acme/app'), 'abc123')).toBeNull();
    expect(projectCommitUrl(null, SHA)).toBeNull();
  });

  test('each host reads its commit links; a known host reads any hostname, an unknown GitHub host does not', () => {
    expect(parseCommitUrl(`https://github.com/Acme/App/commit/${SHA.toUpperCase()}`)).toEqual({ kind: 'github', link: { host: 'github.com', repository: 'acme/app', sha: SHA } });
    expect(parseCommitUrl(`https://github.com/acme/app/pull/12/commits/${SHA}`)?.kind).toBe('github');
    expect(parseCommitUrl(`https://code.corp.example/acme/app/commit/${SHA}`, 'github')?.kind).toBe('github');
    expect(parseCommitUrl(`https://code.corp.example/acme/app/commit/${SHA}`)?.kind).toBe('forgejo');
    expect(parseCommitUrl(`https://gitlab.com/group/app/-/commit/${SHA}`, 'gitlab')?.link.repository).toBe('group/app');
    expect(parseCommitUrl(`https://bitbucket.org/acme/app/commits/${SHA}`)?.kind).toBe('bitbucket');
    expect(parseCommitUrl(`https://dev.azure.com/org/proj/_git/repo/commit/${SHA}`)?.kind).toBe('azure-devops');
    expect(parseCommitUrl(`https://git.cafe/acme/app/commit/${SHA}`)?.kind).toBe('gitcafe');
    expect(parseCommitUrl(`https://forgejo.example:3000/acme/app/commit/${SHA}`)?.link.authority).toBe('forgejo.example:3000');
    expect(parseCommitUrl('https://github.com/acme/app/commit/abc1234')).toBeNull();
    expect(parseCommitUrl(`ftp://github.com/acme/app/commit/${SHA}`)).toBeNull();
  });
});

describe('a commit link beside the thread', () => {
  beforeEach(() => resetExternalOpensForTests());
  const none = { meta: false, ctrl: false, shift: false, alt: false };

  test('a commit the workspace has opens in the Diff pane, selected', async () => {
    const { client, native } = harness();
    expect(await openCommitLink(client, native, `https://github.com/acme/app/commit/${SHA}`, none)).toBe(true);
    const state = panelState(client);
    expect([state.visible, state.active, client.diffOpen]).toEqual([true, 'diff', true]);
    expect(diffSnapshot(client, 0)).toMatchObject({ diffScope: `commit:${SHA}`, diffScopeLabel: 'Add the thing' });
  });

  test('one it lacks opens its pull request, else the browser', async () => {
    const { client, native } = harness({ localCommits: [] });
    expect(await openCommitLink(client, native, `https://github.com/acme/app/commit/${OTHER}`, none)).toBe(true);
    expect(externalOpensForTests()).toEqual([`https://github.com/acme/app/commit/${OTHER}`]);
    expect(client.diffOpen).toBe(false);
    expect(await openCommitLink(client, native, `https://github.com/acme/app/pull/7/commits/${OTHER}`, none)).toBe(true);
    expect(panelState(client).surfaces.map(surface => surface.kind)).toEqual(['pull-request']);
  });

  test('another repository, modifiers, or the setting off leave it an ordinary link', async () => {
    const { client, native, calls } = harness();
    expect(await openCommitLink(client, native, `https://github.com/other/repo/commit/${SHA}`, none)).toBe(false);
    for (const held of [{ ...none, meta: true }, { ...none, ctrl: true }, { ...none, shift: true }, { ...none, alt: true }])
      expect(await openCommitLink(client, native, `https://github.com/acme/app/commit/${SHA}`, held)).toBe(false);
    client.local.clientSettings = { ...client.local.clientSettings, pullRequestLinksOpenInApp: false };
    expect(await openCommitLink(client, native, `https://github.com/acme/app/commit/${SHA}`, none)).toBe(false);
    expect(requests(calls, 'review.getDiffPreview')).toHaveLength(0);
  });

  test('a panel choice made while the lookup runs wins', async () => {
    const { client, native } = harness();
    const opening = openCommitLink(client, native, `https://github.com/acme/app/commit/${SHA}`, none);
    panelState(client).userRevision++;
    expect(await opening).toBe(true);
    expect(client.diffOpen).toBe(false);
    expect(diffSnapshot(client, 0).diffScope).toBe('branch');
  });
});
