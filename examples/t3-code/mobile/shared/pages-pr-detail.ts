// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/pages-pr-detail.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// The pull request detail panel (lane "pages"): pullRequests.detail and
// .activity read for the selected row, presented as PullRequestDetailPanel's
// header and Summary/Timeline tabs, and its writes (runAction, update,
// requestReviewers, setLabels). Sources: T3 Code (MIT, see LICENSE-T3)
// apps/web/src/components/pullRequest/{PullRequestDetailPanel,
// PullRequestSummaryTab,PullRequestTimelineTab,pullRequestPresentation}.tsx.
import { diffSchemeOf, markdownEnv, type ChipView } from './r4-timeline-chips';
import { arr, num, obj, str, type Obj } from './domain';
import { ClientError, type Native } from './protocol';
import { pushToast } from './toast';
import { relativeLabel, stateKey, conflictLabel, labelChip } from './pages-prs';
import type { T3Client } from './client';
import { letGo } from './let-go';

export type PrSelection = { projectId: string; host: string; repository: string; number: number };
/** The row key the list wears, parsed back into the reference a read needs. */
export function parseSelection(value: string): PrSelection | null {
  try {
    const parsed = obj(JSON.parse(value));
    const number = num(parsed.number), repository = str(parsed.repository), projectId = str(parsed.projectId);
    return number > 0 && repository && projectId ? { projectId, host: str(parsed.host), repository, number } : null;
  } catch { return null; }
}
export const selectionRef = (selection: PrSelection): Obj => ({ projectId: selection.projectId, ...(selection.host ? { host: selection.host } : {}), repository: selection.repository, number: selection.number });

const STATE_LABELS: Record<string, string> = { open: 'Open', draft: 'Draft', closed: 'Closed', merged: 'Merged' };
const CHECK_LABELS: Record<string, string> = { pending: 'Running', 'action-required': 'Awaiting action', success: 'Passed', failure: 'Failed', cancelled: 'Cancelled', skipped: 'Skipped', neutral: 'Neutral' };
const ACTION_DONE: Record<string, string> = { merge: 'Pull request merged', ready: 'Marked ready for review', draft: 'Converted to draft', close: 'Pull request closed', reopen: 'Pull request reopened', 'update-branch': 'Branch updated with the base branch' };
const ACTION_FAILED: Record<string, string> = { merge: 'Could not merge this pull request', ready: 'Could not mark this ready for review', draft: 'Could not convert this to a draft', close: 'Could not close this pull request', reopen: 'Could not reopen this pull request', 'update-branch': 'Could not update this branch' };
const count = (value: number) => String(Math.round(value)).replace(/\B(?=(\d{3})+(?!\d))/g, ',');

/** summarizePullRequestChecks: GitHub's own headline for the rollup. */
export function summarizeChecks(checks: Obj[]): string {
  if (!checks.length) return 'No checks reported';
  const failed = checks.filter(check => check.status === 'failure' || check.status === 'cancelled').length;
  const action = checks.filter(check => check.status === 'action-required').length;
  const pending = checks.filter(check => check.status === 'pending').length;
  const passed = checks.filter(check => check.status === 'success').length;
  if (failed) return `${failed} of ${checks.length} failing`;
  if (action) return `${action} ${action === 1 ? 'check' : 'checks'} awaiting action`;
  if (pending) return `${pending} of ${checks.length} running`;
  return passed === checks.length ? 'All checks passed' : `${passed} of ${checks.length} passing`;
}
export function checksTone(checks: Obj[]): string {
  if (!checks.length) return '';
  const statuses = new Set(checks.map(check => str(check.status)));
  if (statuses.has('failure') || statuses.has('cancelled')) return 'failing';
  if (statuses.has('pending') || statuses.has('action-required')) return 'pending';
  return statuses.has('success') ? 'passing' : '';
}
const person = (value: unknown) => { const actor = obj(value), login = str(actor.login, 'ghost'); return { key: login.toLowerCase(), login, avatar: str(actor.avatarUrl), initial: login.slice(0, 1).toUpperCase() }; };
const labelColor = (color: unknown) => { const hex = str(color).trim().replace(/^#/, ''); return /^[0-9a-fA-F]{6}$/.test(hex) ? `#${hex}` : ''; };

export function emptyDetail() {
  return {
    open: false, loading: false, error: '', errorTitle: '', githubUrl: '', copiedCheckout: 0, copiedBranch: 0, ref: '', number: 0, numberLabel: '', title: '', repository: '', url: '', state: 'open', stateLabel: 'Open', conflict: '',
    author: '', authorAvatar: '', authorInitial: '', updated: '', checkoutCommand: '', baseBranch: '', headBranch: '', files: '', additions: '', deletions: '',
    checksSummary: '', checksTone: '', reviewers: [] as { key: string; login: string; avatar: string; initial: string }[],
    labels: [] as { key: string; name: string; background: string; ink: string }[],
    bodies: [] as { id: string; kind: string; title: string; body: string }[], hasBody: false, bodyId: '',
    checks: [] as { key: string; name: string; status: string; statusLabel: string; description: string; url: string }[],
    comments: [] as { key: string; bodyId: string; author: string; avatar: string; initial: string; age: string; kind: string }[], commentCount: 0, commentsLabel: 'Comments (0)',
    timeline: [] as { key: string; kind: string; title: string; detail: string; age: string; author: string; avatar: string; initial: string }[],
    canEdit: false, canClose: false, canReopen: false, canDraft: false, canReady: false, canMerge: false, canUpdateBranch: false, canReview: false, canLabel: false,
    projectId: '', host: '', hostName: 'GitHub', linkMenu: '', code: [] as { id: string; code: string; icon: string; tokens: { id: string; text: string; cls: string }[] }[],
    // r4-timeline: Settings → Appearance code font, size and word wrap for the Markdown.
    md: { codeFont: 'ui-monospace', codeSize: 13, wrap: true, chips: [] as ChipView[], runCommands: [] as string[] }, diffScheme: 'red-green',
  };
}
export type PrDetailView = ReturnType<typeof emptyDetail>;

type DetailCache = { key: string; detail: Obj | null; activity: Obj | null; error: string; notFound: boolean };
const details = new WeakMap<object, DetailCache>();

export async function pullRequestDetail(client: T3Client, native: Native | null | undefined, input: { selected: string; refresh: number; now: number }): Promise<PrDetailView> {
  const view = emptyDetail();
  view.md = markdownEnv(client); view.diffScheme = diffSchemeOf(client);
  const selection = parseSelection(input.selected);
  if (!selection) return view;
  view.open = true; view.ref = input.selected;
  if (!native?.available || !client.ready) { view.loading = true; return view; }
  const key = JSON.stringify([client.environmentId, selection, input.refresh]);
  let cached = details.get(client);
  if (!cached || cached.key !== key) {
    const previous = cached;
    cached = { key, detail: null, activity: null, error: '', notFound: false };
    const ref = selectionRef(selection);
    try {
      if (previous && previous.key !== key && JSON.parse(previous.key)[1]?.number === selection.number && input.refresh > 0) await client.rpc(native, 'pullRequests.invalidate', { reference: ref }).catch(() => ({}));
      cached.detail = await client.rpc(native, 'pullRequests.detail', ref);
      cached.activity = await client.rpc(native, 'pullRequests.activity', ref).catch(() => null);
    } catch (error) {
      if (letGo(error)) throw error;
      cached.error = error instanceof Error && error.message.trim() ? error.message : 'The environment request failed.';
      // 7bc161f869: a link to an issue (or a PR this account cannot see) reads as not found.
      cached.notFound = isPullRequestNotFound(error);
    }
    details.set(client, cached);
  }
  if (!cached.detail) {
    Object.assign(view, { loading: !cached.error, number: selection.number, numberLabel: `#${selection.number}`, repository: selection.repository });
    if (cached.error) Object.assign(view, unavailable(selection, cached, client.shell.projects.find(project => project.id === selection.projectId)));
    return view;
  }
  presentDetail(view, cached.detail, cached.activity, input.now);
  view.copiedCheckout = copyNonce(client, view.checkoutCommand); view.copiedBranch = copyNonce(client, view.headBranch);
  return view;
}
/** The last in-place copy per client: the value and a nonce that restarts the "Copied" swap. */
const copies = new WeakMap<object, { value: string; nonce: number }>();
export function noteCopy(client: object, value: string): void { copies.set(client, { value, nonce: (copies.get(client)?.nonce ?? 0) + 1 }); }
export function copyNonce(client: object, value: string): number { const copy = copies.get(client); return copy && value && copy.value === value ? copy.nonce : 0; }

export function presentDetail(view: PrDetailView, detail: Obj, activity: Obj | null, now: number): PrDetailView {
  const author = person(activity?.author ?? detail.author), state = stateKey(detail), checks = arr(detail.checks);
  const permissions = obj(detail.viewerPermissions), capabilities = obj(detail.capabilities);
  const key = `${str(detail.repository)}#${num(detail.number)}`;
  Object.assign(view, {
    number: num(detail.number), numberLabel: `#${num(detail.number)}`, title: str(detail.title), repository: str(detail.repository),
    url: str(detail.url), state, stateLabel: STATE_LABELS[state] ?? 'Open', conflict: conflictLabel(detail), author: author.login, authorAvatar: author.avatar, authorInitial: author.initial,
    updated: `updated ${relativeLabel(detail.updatedAt, now)}`,
    checkoutCommand: str(detail.provider, 'github') === 'github' ? `gh pr checkout ${num(detail.number)}` : '', baseBranch: str(detail.baseBranch), headBranch: str(detail.headBranch),
    files: `${count(num(detail.changedFiles))} ${num(detail.changedFiles) === 1 ? 'file' : 'files'}`, additions: `+${count(num(detail.additions))}`, deletions: `-${count(num(detail.deletions))}`,
    checksSummary: summarizeChecks(checks), checksTone: checksTone(checks), projectId: str(detail.projectId), host: hostOf(str(detail.url)),
    hostName: HOST_NAMES[str(detail.provider)] ?? 'GitHub',
    linkMenu: `${str(detail.provider)} ${str(detail.url)}`, // context-menu-gaps: the number's right-click
  });
  view.reviewers = arr(activity?.reviewers ?? detail.reviewers).map(person).map(({ key, login, avatar, initial }) => ({ key, login, avatar, initial }));
  view.labels = arr(detail.labels).map(label => labelChip(str(label.name), labelColor(label.color)));
  view.hasBody = str(detail.body).trim().length > 0;
  view.bodyId = `pr-body:${key}`;
  const comments = arr(activity?.comments).filter(comment => comment.kind !== 'review-comment');
  view.bodies = [...(view.hasBody ? [{ id: view.bodyId, kind: 'assistant', title: '', body: str(detail.body) }] : []),
    ...comments.filter(comment => str(comment.body).trim()).map(comment => ({ id: `pr-comment:${str(comment.id)}`, kind: 'assistant', title: '', body: str(comment.body) }))];
  view.checks = checks.map((check, index) => ({ key: `${index}:${str(check.name)}`, name: str(check.name), status: str(check.status), statusLabel: CHECK_LABELS[str(check.status)] ?? str(check.status), description: str(check.description), url: str(check.url) }));
  view.commentCount = num(activity?.commentCount, comments.length);
  view.commentsLabel = `Comments (${view.commentCount})`;
  view.comments = comments.map(comment => { const by = person(comment.author);
    return { key: str(comment.id), bodyId: `pr-comment:${str(comment.id)}`, author: by.login, avatar: by.avatar, initial: by.initial, age: relativeLabel(comment.createdAt, now), kind: str(comment.kind) }; })
    .reverse();
  // PullRequestTimelineTab: commits and remarks in the order they happened.
  view.timeline = [
    ...arr(activity?.commits).map(commit => ({ at: Date.parse(str(commit.committedDate)) || 0, item: { key: `commit:${str(commit.oid)}`, kind: 'commit', title: str(commit.messageHeadline), detail: str(commit.oid).slice(0, 7),
      age: relativeLabel(commit.committedDate, now), ...authorFields(arr(commit.authors)[0]) } })),
    ...comments.map(comment => ({ at: Date.parse(str(comment.createdAt)) || 0, item: { key: `comment:${str(comment.id)}`, kind: comment.kind === 'review' ? 'review' : 'comment',
      title: comment.kind === 'review' ? reviewTitle(str(comment.reviewState)) : 'commented', detail: str(comment.body).split('\n')[0]!.slice(0, 200), age: relativeLabel(comment.createdAt, now), ...authorFields(comment.author) } })),
  ].sort((a, b) => a.at - b.at).map(event => event.item);
  // A control belongs on the page only where the host can do it and this viewer may ask (PullRequestViewerPermissions).
  const names = (value: unknown) => (Array.isArray(value) ? value : []).filter((name): name is string => typeof name === 'string');
  const offered = (action: string) => names(capabilities.actions).includes(action) && names(permissions.actions).includes(action);
  const open = detail.state === 'open';
  view.canEdit = obj(capabilities.edit).changeRequest !== false;
  view.canClose = open && offered('close');
  view.canReopen = detail.state === 'closed' && offered('reopen');
  view.canDraft = open && detail.isDraft !== true && offered('draft');
  view.canReady = open && detail.isDraft === true && offered('ready');
  view.canMerge = open && detail.isDraft !== true && detail.mergeability !== 'conflicting' && offered('merge');
  view.canUpdateBranch = open && detail.baseComparison === 'behind' && offered('update-branch');
  view.canReview = obj(capabilities.reviewers).request === true && permissions.requestReviewers === true;
  view.canLabel = capabilities.labels !== false && permissions.labels !== false;
  return view;
}
/** PullRequestOperationError{reason:"not-found"}: GitHub's not-found, Bitbucket's 404. */
export function isPullRequestNotFound(error: unknown): boolean {
  return error instanceof ClientError && error.kind === 'PullRequestOperationError' && error.reason === 'not-found';
}
/** PullRequestsUnavailableState for a detail that never loaded: its title, text, and GitHub link. */
export function unavailable(selection: PrSelection, cached: { error: string; notFound: boolean }, project: Obj | undefined) {
  return {
    errorTitle: cached.notFound ? `Pull request #${selection.number} not found` : 'Could not load pull requests',
    error: cached.notFound ? "It may be an issue rather than a pull request, or this account can't see it." : cached.error,
    githubUrl: gitHubPullRequestBrowserUrl(obj(project?.repositoryIdentity), selection.repository, selection.number),
  };
}
/** changeRequestUrl.gitHubPullRequestBrowserUrl: only a GitHub identity, an owner/name path and a positive number. */
export function gitHubPullRequestBrowserUrl(identity: Obj, repository: string, number: number): string {
  if (identity.provider !== 'github' || !Number.isSafeInteger(number) || number < 1) return '';
  const path = repository.split('/');
  if (path.length !== 2 || path.some(segment => !segment || segment === '.' || segment === '..')) return '';
  let origin = '';
  try { const remote = new URL(str(obj(identity.locator).remoteUrl).trim()); if (remote.protocol === 'http:' || remote.protocol === 'https:') origin = remote.origin; } catch { /* SCP-style remotes use the canonical key */ }
  const hostname = str(identity.canonicalKey).split('/')[0];
  if (!origin && !hostname) return '';
  try { const url = new URL(origin || `https://${hostname}`); url.pathname = `/${path.join('/')}/pull/${number}`; return url.toString(); } catch { return ''; }
}
const HOST_NAMES: Record<string, string> = { github: 'GitHub', gitlab: 'GitLab', bitbucket: 'Bitbucket', 'azure-devops': 'Azure DevOps', forgejo: 'Forgejo' };
const authorFields = (value: unknown) => { const by = person(value); return { author: by.login, avatar: by.avatar, initial: by.initial }; };
const reviewTitle = (state: string) => state === 'APPROVED' || state === 'approved' ? 'approved these changes' : state === 'CHANGES_REQUESTED' || state === 'changes-requested' ? 'requested changes' : 'reviewed';
const hostOf = (url: string) => { try { return new URL(url).host; } catch { return ''; } };

/** pages:pr-* writes, each against the selected pull request on its host. */
export async function prCommand(client: T3Client, native: Native, op: string, selected: string, value: string): Promise<string> {
  const selection = parseSelection(selected);
  if (!selection) throw new ClientError('Choose a pull request first.');
  const ref = selectionRef(selection);
  const label = `#${selection.number}`;
  try {
    if (op === 'action') {
      if (!ACTION_DONE[value]) throw new ClientError(`Unknown pull request action: ${value}`);
      await client.rpc(native, 'pullRequests.runAction', { ...ref, action: value }, true);
      pushToast(client, { kind: 'success', title: ACTION_DONE[value]!, description: label });
    } else if (op === 'title') {
      const title = value.trim();
      if (!title) throw new ClientError('A pull request needs a title.');
      await client.rpc(native, 'pullRequests.update', { ...ref, title: title.slice(0, 1024) }, true);
      pushToast(client, { kind: 'success', title: 'Title updated', description: label });
    } else if (op === 'label' || op === 'unlabel') {
      await client.rpc(native, 'pullRequests.setLabels', { ...ref, labels: [value], applied: op === 'label' }, true);
    } else if (op === 'request-review' || op === 'remove-review') {
      const reviewer = obj(JSON.parse(value));
      await client.rpc(native, 'pullRequests.requestReviewers', { ...ref, reviewers: [{ id: str(reviewer.id), kind: str(reviewer.kind, 'user') }], requested: op === 'request-review' }, true);
    } else if (op === 'comment') {
      if (!value.trim()) throw new ClientError('Write a comment first.');
      await client.rpc(native, 'pullRequests.comment', { ...ref, body: value.slice(0, 65_536) }, true);
      pushToast(client, { kind: 'success', title: 'Comment posted', description: label });
    } else throw new ClientError(`Unknown pull request action: ${op}`);
  } catch (error) {
    if (letGo(error)) throw error;
    const message = error instanceof Error ? error.message : 'The host refused it.';
    pushToast(client, { kind: 'error', title: op === 'action' ? ACTION_FAILED[value] ?? 'Could not update this pull request' : 'Could not update this pull request', description: message });
    return message;
  }
  details.delete(client);
  return '';
}

type Candidates = { key: string; reviewers: Obj[]; labels: Obj[]; error: string };
const candidates = new WeakMap<object, Candidates>();
/** Reviewer and label candidates, read when their menu opens (not with every detail). */
export async function prCandidates(client: T3Client, native: Native | null | undefined, selected: string, which: string) {
  const selection = parseSelection(selected);
  const empty = { reviewers: [] as { key: string; id: string; kind: string; login: string; avatar: string; initial: string; requested: boolean }[], labels: [] as { key: string; name: string; color: string; applied: boolean; description: string }[], loading: false, error: '' };
  if (!selection || !which || !native?.available || !client.ready) return empty;
  const key = JSON.stringify([selection, which]);
  let cached = candidates.get(client);
  if (!cached || cached.key !== key) {
    cached = { key, reviewers: [], labels: [], error: '' };
    try {
      if (which === 'reviewers') cached.reviewers = arr((await client.rpc(native, 'pullRequests.reviewerCandidates', selectionRef(selection))).candidates);
      else cached.labels = arr((await client.rpc(native, 'pullRequests.labelCandidates', selectionRef(selection))).candidates);
    } catch (error) { if (letGo(error)) throw error; cached.error = error instanceof Error ? error.message : 'Could not read the candidates.'; }
    candidates.set(client, cached);
  }
  empty.error = cached.error;
  empty.reviewers = cached.reviewers.map(candidate => { const by = person(candidate); return { key: str(candidate.id), id: str(candidate.id), kind: str(candidate.kind, 'user'), login: by.login, avatar: by.avatar, initial: by.initial, requested: candidate.isRequested === true }; });
  empty.labels = cached.labels.map(candidate => ({ key: str(candidate.name), name: str(candidate.name), color: labelColor(candidate.color), applied: candidate.isApplied === true, description: str(candidate.description) }));
  return empty;
}
export function forgetCandidates(client: object): void { candidates.delete(client); }
