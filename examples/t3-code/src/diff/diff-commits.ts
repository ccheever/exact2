// The Diff panel's Commits source and code-host commit links, adapted from T3 Code 2359643cff (MIT; see LICENSE-T3):
// - DiffPanel.tsx: the scope menu's Commits submenu over review.getBranchCommits (the newest 100 local commits of the
//   branch comparison, read again after 5 s), the list the submenu opened with kept while it is open, its error,
//   file-access, loading, empty and "Showing latest N commits" rows, and the header's short SHA linking to the code host;
// - client-runtime changeRequestLinks.ts projectCommitUrl and each host's commitUrl and parseCommitUrl
//   (packages/source-control-*/src/shared/definition.ts, in the registry's claim order);
// - lib/openPullRequestLink.ts: a commit link of the thread's own repository opens that commit in the Diff pane when the
//   thread's workspace has it (a review.getDiffPreview that answers its commit source), else the pull request the link
//   names, else follows "Open links in". Shift and Alt keep their browser meaning; ⌘ and Ctrl open it outside.
// The selection itself is diff.ts's (selectCommit, diffPanelStore's `commit` kind).
import type { T3Client } from '../client/client';
import { arr, obj, str, type Obj } from '../client/domain';
import type { Native } from '../client/protocol';
import { letGo } from '../client/let-go';
import { can } from '../connections/command-permissions';
import { formatRelativeTimeLabel } from '../browser/browser-history';
import { findProjectForChangeRequest, canonicalRepositoryKey } from '../palette/palette-linkpr';
import type { DiffSource } from './diff-lazy';
import { branchBaseRef, currentSelection, diffNotGit, workspace } from './diff';
import { composerNow } from '../composer/composer-controls'; // the snapshot's wall time: data sources have no clock

export type BranchCommit = { sha: string; subject: string; committedAt: string };
export type BranchCommitsResult = { commits: BranchCommit[]; truncated: boolean };
/** The query's state: the cwd and target it read for, when, its answer or error, and the list an open submenu keeps. */
export type BranchCommits = { key: string; at: number; data: BranchCommitsResult | null; error: string; frozen: BranchCommitsResult | null; reads: number };
export const emptyBranchCommits = (): BranchCommits => ({ key: '', at: 0, data: null, error: '', frozen: null, reads: 0 });
const STALE_MS = 5_000;

/** ReviewBranchCommitsResult, with any malformed entry dropped. */
export function adoptBranchCommits(result: Obj): BranchCommitsResult {
  const commits = arr(result.commits).flatMap(commit => typeof commit.sha === 'string' && commit.sha
    ? [{ sha: commit.sha, subject: str(commit.subject), committedAt: str(commit.committedAt) }] : []);
  return { commits, truncated: result.truncated === true };
}

type CommitsHost = { cwd: string; baseRef: string | null; threadKey: string; canRead: boolean };
/**
 * Reads the branch's commits for the comparison the Diff keeps (its remembered target), unless the same read answered
 * within 5 s (by the window's wall time `now`; 0, unknown, always reads); `force` reads again anyway (the submenu
 * opening). Another cwd or target starts from nothing.
 */
export async function readBranchCommits(state: BranchCommits, host: CommitsHost, send: (method: string, payload: Obj) => Promise<Obj>, now: number, force = false): Promise<void> {
  if (!host.cwd || !host.canRead) return;
  const key = JSON.stringify([host.threadKey, host.cwd, host.baseRef]);
  if (key !== state.key) Object.assign(state, { key, at: 0, data: null, error: '' });
  else if (!force && now > 0 && now - state.at < STALE_MS) return;
  const reading = ++state.reads;
  state.at = now;
  try {
    const result = await send('review.getBranchCommits', { cwd: host.cwd, ...(host.baseRef ? { baseRef: host.baseRef } : {}) });
    if (state.reads === reading && state.key === key) Object.assign(state, { data: adoptBranchCommits(result), error: '' });
  } catch (error) {
    if (letGo(error)) throw error;
    if (state.reads === reading && state.key === key) state.error = error instanceof Error ? error.message : String(error);
  }
}
/** The submenu opened (it keeps the list it had, so rows do not move under the pointer) or closed. */
export function noteCommitsMenu(state: BranchCommits, open: boolean): void { state.frozen = open ? state.data : null; }

export type DiffCommitRow = { id: string; sha: string; short: string; subject: string; time: string; url: string; selected: boolean };
/** `note`: the submenu's disabled status row, '' for none. `sha` and `url`: the header's short SHA beside the trigger. */
export type DiffCommitsView = { rows: DiffCommitRow[]; note: string; sha: string; url: string };

/** The scope trigger's label and the review section's title for a commit: its subject once its source answered. */
export function commitScopeLabels(source: DiffSource | null, sha: string): { trigger: string; section: string } {
  const subject = source?.kind === 'commit' && source.headRef === sha ? source.commitSubject : undefined;
  return { trigger: subject ?? 'Commit', section: subject ?? `Commit ${sha.slice(0, 7)}` };
}

/** The submenu's rows and status row, and the header's SHA, as DiffPanel draws them. */
export function commitsView(state: BranchCommits, input: { canRead: boolean; selectedSha: string | null; identity: Obj | null; now: number }): DiffCommitsView {
  const list = state.frozen ?? state.data;
  const rows = (list?.commits ?? []).map(commit => ({ id: `commit:${commit.sha}`, sha: commit.sha, short: commit.sha.slice(0, 7), subject: commit.subject,
    time: formatRelativeTimeLabel(Date.parse(commit.committedAt), input.now), url: projectCommitUrl(input.identity, commit.sha) ?? '', selected: commit.sha === input.selectedSha }));
  const note = state.error ? state.error : !input.canRead ? 'This connection cannot read local diffs.'
    : list === null ? 'Loading commits...' : list.commits.length === 0 ? 'No commits on this branch.' : list.truncated ? `Showing latest ${list.commits.length} commits` : '';
  const sha = input.selectedSha ?? '';
  return { rows, note, sha: sha.slice(0, 7), url: sha ? projectCommitUrl(input.identity, sha) ?? '' : '' };
}

// ── Commit URLs, each host's ────────────────────────────────────────────────────────────────────────────────────────
const FULL_COMMIT_SHA = /^(?:[0-9a-f]{40}|[0-9a-f]{64})$/iu;
const GITCAFE_HOSTS = new Set(['git.cafe', 'staging.git.cafe']);
const isGitHubHost = (hostname: string) => hostname === 'github.com' || hostname.endsWith('.ghe.com') || hostname.split('.').includes('github');
const isBitbucketHost = (hostname: string) => hostname === 'bitbucket.org' || hostname.endsWith('.bitbucket.org') || hostname.split('.').includes('bitbucket');
type CommitHost = { kind: string; route: RegExp; claims?: (hostname: string, known: boolean) => boolean };
/** The registry's claim order: GitCafe and Azure DevOps first, then GitHub, GitLab, Forgejo, Bitbucket. */
const COMMIT_HOSTS: CommitHost[] = [
  { kind: 'gitcafe', route: /^\/([^/]+\/[^/]+)\/commit\/([0-9a-f]+)\/?$/iu, claims: hostname => GITCAFE_HOSTS.has(hostname) },
  { kind: 'azure-devops', route: /^\/((?:[^/]+\/)*_git\/[^/]+)\/commit\/([0-9a-f]+)\/?$/iu },
  // A known project's host can be any GitHub Enterprise Server name.
  { kind: 'github', route: /^\/([^/]+\/[^/]+)\/(?:commit|pull\/\d+\/commits)\/([0-9a-f]+)\/?$/iu, claims: (hostname, known) => known || isGitHubHost(hostname) },
  { kind: 'gitlab', route: /^\/([^/]+(?:\/[^/]+)+)\/-\/commit\/([0-9a-f]+)\/?$/iu },
  { kind: 'forgejo', route: /^\/([^/]+(?:\/[^/]+)+)\/commit\/([0-9a-f]+)\/?$/iu },
  { kind: 'bitbucket', route: /^\/([^/]+\/[^/]+)\/commits\/([0-9a-f]+)\/?$/iu, claims: isBitbucketHost },
];
export type CommitLink = { host: string; repository: string; sha: string; authority?: string };

/** sourceControlClients.parseCommitUrl: with the project's host `kind`, only that host reads the URL. */
export function parseCommitUrl(value: string, kind?: string): { kind: string; link: CommitLink } | null {
  let url: URL;
  try { url = new URL(value); } catch { return null; }
  if (url.protocol !== 'https:' && url.protocol !== 'http:') return null;
  const hostname = url.hostname.toLowerCase(), known = kind !== undefined;
  for (const host of known ? COMMIT_HOSTS.filter(entry => entry.kind === kind) : COMMIT_HOSTS) {
    if (host.claims && !host.claims(hostname, known)) continue;
    const match = host.route.exec(url.pathname), repository = match?.[1], sha = match?.[2];
    if (!repository || !sha || !FULL_COMMIT_SHA.test(sha)) continue;
    return { kind: host.kind, link: { host: hostname, repository: repository.toLowerCase(), sha: sha.toLowerCase(), ...(url.port ? { authority: url.host.toLowerCase() } : {}) } };
  }
  return null;
}

/** remoteWebOrigin: an HTTP remote on `host` names the server's web origin, port included; SSH remotes do not. */
function remoteWebOrigin(host: string, remoteUrl: string): string {
  try {
    const remote = new URL(remoteUrl);
    if ((remote.protocol === 'http:' || remote.protocol === 'https:') && (remote.hostname.toLowerCase() === host.toLowerCase() || remote.host.toLowerCase() === host.toLowerCase())) return remote.origin;
  } catch { /* SSH remotes do not name the web origin. */ }
  return `https://${host}`;
}
/** buildCommitUrl: `<route>/<sha>` below the repository's browser URL, without credentials, query or fragment. */
function buildCommitUrl(input: { host: string; repository: string; sha: string; remoteUrl: string; webUrl: string }, route: string): string | null {
  if (!input.repository || !FULL_COMMIT_SHA.test(input.sha)) return null;
  try {
    const url = new URL(input.webUrl || `${remoteWebOrigin(input.host, input.remoteUrl)}/${input.repository}`);
    if (url.protocol !== 'http:' && url.protocol !== 'https:') return null;
    url.pathname = `${url.pathname.replace(/\/+$/u, '')}/${route}/${input.sha.toLowerCase()}`;
    url.username = ''; url.password = ''; url.search = ''; url.hash = '';
    return url.toString();
  } catch { return null; }
}
/** projectCommitUrl: the commit's page on the project's code host, or null when the host has none or the identity is unknown. */
export function projectCommitUrl(identity: Obj | null | undefined, sha: string): string | null {
  const provider = str(identity?.provider);
  if (!identity || !provider) return null;
  const [host, ...path] = str(identity.canonicalKey).split('/');
  if (!host) return null;
  const input = { host, repository: path.join('/'), sha, remoteUrl: str(obj(identity.locator).remoteUrl).trim(), webUrl: str(identity.webUrl) };
  switch (provider) {
    case 'github': {
      const segments = input.repository.split('/');
      return segments.length === 2 && segments.every(segment => /^[\w.-]+$/u.test(segment) && segment !== '.' && segment !== '..') ? buildCommitUrl(input, 'commit') : null;
    }
    case 'gitlab': return buildCommitUrl(input, '-/commit');
    case 'forgejo': case 'gitcafe': return buildCommitUrl(input, 'commit');
    case 'bitbucket': return buildCommitUrl(input, 'commits');
    case 'azure-devops': {
      const key = `${input.host}/${input.repository}`.toLowerCase();
      const [canonicalHost = input.host, ...rest] = canonicalRepositoryKey(key).split('/');
      return buildCommitUrl({ ...input, host: canonicalHost, repository: rest.join('/') }, 'commit');
    }
    default: return null;
  }
}

/** A commit link of the project's own repository (definition.isChangeRequestInRepository), with its host's kind. */
export function ownCommitLink(project: Obj | undefined, url: string): CommitLink | null {
  const identity = obj(project?.repositoryIdentity), provider = str(identity.provider);
  if (!project || !provider) return null;
  const parsed = parseCommitUrl(url, provider);
  if (!parsed || parsed.kind !== provider) return null;
  return findProjectForChangeRequest([project], { host: parsed.link.host, repository: parsed.link.repository, number: 0, ...(parsed.link.authority ? { authority: parsed.link.authority } : {}) })
    ? parsed.link : null;
}

/** Whether `client` may read the branch's commits (DiffPanel canReadFiles). */
export const canReadCommits = (client: T3Client): boolean => can(client, 'filesystem:read');

/** The thread's project's repository identity, or null. */
export function threadIdentity(client: T3Client): Obj | null {
  const project = client.shell.projects.find(entry => entry.id === client.projectId);
  const identity = obj(project?.repositoryIdentity);
  return Object.keys(identity).length ? identity : null;
}
/** readBranchCommits for the thread the Diff shows, at its workspace and remembered comparison target. */
export async function refreshBranchCommits(client: T3Client, native: Native, force = false): Promise<void> {
  if (!client.threadId || diffNotGit(client)) return;
  await readBranchCommits(client.diffState.commits, { cwd: workspace(client), baseRef: branchBaseRef(client), threadKey: `${client.environmentId}:${client.threadId}`, canRead: canReadCommits(client) },
    (method, payload) => client.rpc(native, method, payload), composerNow(client), force);
}
/** The snapshot's `diffCommits`. */
export function branchCommitsView(client: T3Client, now: number): DiffCommitsView {
  const selection = currentSelection(client);
  return commitsView(client.diffState.commits, { canRead: canReadCommits(client), selectedSha: selection.kind === 'commit' ? selection.sha ?? null : null, identity: threadIdentity(client), now });
}
