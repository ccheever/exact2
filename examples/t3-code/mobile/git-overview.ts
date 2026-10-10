import { resolveQuickAction } from './thread-header-model';
// Pinned365aa87982 GitOverviewSheet/GitCommitSheet/GitConfirmSheet (MIT, LICENSE-T3).
// Shared status reducer/action stream remain the sole transport and mutation owners.
// @ref llp/1109.011-responsive-workspace.decision.md#navigation-and-data-ownership
import { mobileClient, mobileNative } from './client';
import { mobileGitFeedbackObserve, mobileGitFeedbackError, mobileGitFeedbackPull, mobileGitFeedbackCancelPull } from './git-feedback';
import type { T3Client } from './shared/client';
import { arr, num, obj, str, type Obj } from './shared/domain';
import { ClientError, nativeFiles, bridgeReply, type Native } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { gitCardView, gitCommand, gitLocal, gitState } from './shared/r4-git-actions';
import { workingFiles } from './shared/r4-git-logic';
import { peekVcsStatus, watchVcsStatus, VCS_STATUS_KEY, subscriptionSerial } from './shared/shell-vcs';
import { resolveChains } from './shared/r4-surfaces-prs';
import { workspaceOf, capabilities } from './shared/r4-surfaces-panel';
import { assertReviewOwner, reviewNative, reviewOwner } from './review-owner';
import { mobileSessionGrants } from './environment-detail';
import { buildMenuItems, getGitActionDisabledReason, mobileGitStatus, mobileGitSummary,
  resolveDefaultBranchActionDialogCopy, requiresDefaultBranchConfirmation, mobileAutoBranch } from './git-overview-model';

export interface MobileGitRow { id: string; label: string; subtitle: string; symbol: string; disabled: boolean }
export interface MobileGitLink { id: string; title: string; subtitle: string; url: string; separator: boolean }
export interface MobileGitGroup { id: string; label: string; rows: MobileGitLink[] }
interface Access { owner: string; allowed: boolean; write: boolean; operate: boolean; serial: number; authCompleted: number; revocation: number;
  reading: boolean; readSerial: number; acting: boolean; error: string; status: Obj | null; stream: Obj | null; refreshed: boolean; confirmation: string; retry: boolean; invalidation: number; subscription: number; lastSeq: number }
const accesses = new WeakMap<T3Client, Access>();
// Shared branchSync has no environment tag. Only this mobile action's immutable
// stream admission may authorize its later metadata write, even after navigation.
const streamOwners = new WeakMap<T3Client, string>();
const streamOwner = (client: T3Client) => JSON.stringify([client.origin, client.environmentId,
  client.projectId, client.threadId, client.generation, workspaceOf(client).cwd]);
function canReconcile(client: T3Client) {
  const queued = gitState(client).branchSync;
  return streamOwners.get(client) === streamOwner(client) && queued.length > 0 && queued.every(item => item.threadId === client.threadId);
}
export function mobileGitAccess(client: T3Client) {
  const owner = reviewOwner(client); let state = accesses.get(client);
  if (!state || state.owner !== owner) {
    const dialog = gitState(client); dialog.dialog = ''; dialog.pending = null; dialog.excluded.clear(); dialog.editing = false;
    state = { owner, allowed: false, write: false, operate: false, serial: 0, authCompleted: 0, revocation: 0,
      reading: false, readSerial: 0, acting: false, error: '', status: null, stream: peekVcsStatus(client, workspaceOf(client).cwd), refreshed: false, confirmation: '', retry: false, invalidation: 0, subscription: 0, lastSeq: 0 };
    accesses.set(client, state);
  }
  return state;
}
/** Observe before the shared event batch is acknowledged. The shared status
 * reducer still folds every snapshot; this only honors transport retry markers. */
export function mobileGitEvents(entries: unknown, client: T3Client = mobileClient) {
  const state = accesses.get(client); if (!state || state.owner !== reviewOwner(client)) return;
  for (const entry of arr(entries)) {
    if (entry.key !== VCS_STATUS_KEY || num(entry.generation, -1) !== client.generation) continue;
    const serial = subscriptionSerial(str(entry.subscriptionId)), seq = num(entry.seq);
    if (serial < state.subscription || seq > 0 && seq <= state.lastSeq) continue;
    state.subscription = Math.max(state.subscription, serial); state.lastSeq = Math.max(state.lastSeq, seq);
    const value = obj(entry.value);
    if (value._retryDue) { state.retry = true; state.invalidation++; }
    else if (value._transportError) state.error = str(obj(value._transportError).message, 'Git status is unavailable.');
  }
}
const message = (error: unknown) => error instanceof Error ? error.message : 'Git action failed.';
export function mobileGitThread(client: T3Client) { return client.shell.threads.find(thread => thread.id === client.threadId); }
export async function mobileGitAuthorize(owner: string, nativeInput: Native | null | undefined, client: T3Client, write = false, change = false) {
  assertReviewOwner(client, owner);
  const state = mobileGitAccess(client), serial = ++state.serial; let revocation = state.revocation;
  if (!nativeInput?.available || !client.ready || !mobileGitThread(client) || !workspaceOf(client).cwd)
    throw new ClientError('Connect and select a thread workspace to use Git.');
  const base = reviewNative(client, letGoAware(mobileNative(nativeInput)), owner);
  let failure: unknown;
  const assertCurrent = () => {
    if (failure) throw failure;
    assertReviewOwner(client, owner);
    if (state.revocation !== revocation) throw new ClientError('Git access changed. Open this screen again.', 'superseded');
  };
  const native: Native = { ...base, later: async input => {
    assertCurrent();
    try { const result = await bridgeReply(base, input); assertCurrent();
      if (!result.ok) throw new ClientError(str(obj(result.error).message, 'Git request failed.'));
      return result;
    } catch (error) { failure = error; throw error; }
  } };
  const session = await client.http(native, '/api/auth/session');
  assertCurrent();
  if (serial > state.authCompleted) {
    const allowed = mobileSessionGrants(session, 'orchestration:read');
    const canWrite = mobileSessionGrants(session, 'source-control:write');
    const operate = mobileSessionGrants(session, 'orchestration:operate');
    const downgraded = state.allowed && !allowed || state.write && !canWrite || state.operate && !operate;
    state.allowed = allowed; state.write = canWrite; state.operate = operate; state.authCompleted = serial;
    if (downgraded) state.revocation++;
    revocation = state.revocation;
  }
  if (!state.allowed || write && !state.write || change && !state.operate) {
    state.revocation++; state.error = !state.allowed ? 'This connection cannot read source control.'
      : !state.write ? 'This connection cannot change source control.' : "This connection cannot update the thread's branch.";
    throw new ClientError(state.error);
  }
  return { native, assertCurrent };
}
export function mobileGitCurrentStatus(client: T3Client, state = mobileGitAccess(client)) {
  if (!state.allowed) return null;
  const stream = peekVcsStatus(client, workspaceOf(client).cwd);
  if (stream && stream !== state.stream) { state.stream = stream; state.status = stream; }
  return state.status;
}
function confirmationIdentity(status: Obj | null) {
  return JSON.stringify([status?.refName ?? null, status?.isDefaultRef === true]);
}
export function mobileGitSnapshot(now = 0, client: T3Client = mobileClient) {
  const state = mobileGitAccess(client), status = mobileGitCurrentStatus(client, state), cwd = workspaceOf(client).cwd;
  const card = gitCardView(client, status, state.error, cwd, now), git = gitState(client);
  mobileGitFeedbackObserve(client, now);
  const busy = state.acting || card.progress, typed = mobileGitStatus(status), repo = status?.isRepo !== false;
  const files = state.allowed ? workingFiles(status).map(file => ({ ...file, included: !git.excluded.has(file.path) })) : [];
  const selected = files.filter(file => file.included);
  const rows: MobileGitRow[] = repo ? buildMenuItems(typed, busy, status?.hasPrimaryRemote === true).map(item => {
    const denied = !state.write && item.kind !== 'open_pr';
    const reason = denied ? 'This connection cannot change source control.' : getGitActionDisabledReason({ item, gitStatus: typed, isBusy: busy, hasOriginRemote: status?.hasPrimaryRemote === true });
    const subtitle = reason || (item.dialogAction === 'commit' && status?.hasWorkingTreeChanges ? `${files.length} file${files.length === 1 ? '' : 's'} changed`
      : item.dialogAction === 'push' && num(status?.aheadCount) > 0 ? `${status?.aheadCount} commit${status?.aheadCount === 1 ? '' : 's'} ahead`
      : item.kind === 'open_pr' ? `PR #${obj(status?.pr).number} ${str(obj(status?.pr).state, 'open')}` : '');
    return { id: item.id, label: item.label, subtitle, disabled: denied || item.disabled,
      symbol: item.icon === 'commit' ? 'checkmark.circle' : item.icon === 'push' ? 'arrow.up.circle' : 'arrow.up.right.circle' };
  }) : [];
  if (num(status?.behindCount) > 0) rows.push({ id: 'pull', label: 'Pull latest', symbol: 'arrow.down.circle',
    subtitle: state.write ? `${status?.behindCount} commit${status?.behindCount === 1 ? '' : 's'} behind upstream` : 'This connection cannot change source control.', disabled: !state.write || busy || !repo });
  rows.push({ id: 'review', label: 'Review changes', subtitle: 'Inspect changes, uncommitted edits, and turn diffs', symbol: 'text.bubble', disabled: busy || !repo || !state.allowed },
    { id: 'branches', label: 'Branches & worktrees', subtitle: state.write && state.operate ? 'Switch branch, create branch, or move to a worktree' : 'View branches and worktrees', symbol: 'point.topleft.down.curvedto.point.bottomright.up', disabled: busy || !repo || !state.allowed });
  const groups: MobileGitGroup[] = client.ready && capabilities(client).threadPullRequests === true ? resolveChains(arr(mobileGitThread(client)?.pullRequests)).map((chain, index) => ({
    id: String(index), label: chain.layers.length > 1 ? `${chain.kind === 'native' ? 'Stack' : 'Branch stack'} · ${chain.layers.length} PRs · bottom to top` : '',
    rows: chain.layers.map((link, index) => { const snapshot = link.snapshot ? obj(link.snapshot) : null;
      return { id: `${str(link.host).toLowerCase()}/${str(link.repository).toLowerCase()}#${num(link.number)}`, title: `#${link.number} ${str(snapshot?.title, 'Pull request')}`,
        subtitle: `${str(link.repository)} · ${snapshot ? snapshot.state === 'open' && snapshot.isDraft ? 'Draft' : str(snapshot.state) : 'Status pending'}${link.watch === undefined ? '' : ' · Watching'}`, url: str(link.url), separator: index > 0 }; }) })) : [];
  const pending = git.pending;
  const copy = pending && ['push', 'create_pr', 'commit_push', 'commit_push_pr'].includes(pending.action)
    ? resolveDefaultBranchActionDialogCopy({ action: pending.action as 'push' | 'create_pr' | 'commit_push' | 'commit_push_pr', branchName: pending.branchName, includesCommit: pending.includesCommit })
    : { title: '', description: '', continueLabel: '' };
  return { owner: state.owner, revision: client.revision, ready: state.allowed, branchLabel: str(status?.refName) || str(mobileGitThread(client)?.branch) || 'Detached HEAD',
    statusSummary: mobileGitSummary(status), worktree: str(mobileGitThread(client)?.worktreePath), error: state.error,
    loading: state.reading || state.allowed && !status, busy, canWrite: state.write, canChangeBranch: state.write && state.operate,
    rows, groups, dialog: state.allowed ? git.dialog : '', defaultBranch: status?.isDefaultRef === true, editingFiles: git.editing,
    files, previewFiles: selected.slice(0, 3), extraFiles: Math.max(0, selected.length - 3), selectedCount: selected.length,
    selectedInsertions: selected.reduce((n, file) => n + file.insertions, 0), selectedDeletions: selected.reduce((n, file) => n + file.deletions, 0),
    allSelected: git.excluded.size === 0, commitDisabled: !state.write || !selected.length || busy,
    featureDisabled: !state.write || !state.operate || !selected.length || busy,
    confirmTitle: copy.title, confirmDescription: copy.description, confirmContinue: copy.continueLabel,
    confirmDisabled: !state.write || busy || !pending, confirmFeatureDisabled: !state.write || !state.operate || busy || !pending,
    needsReconcile: state.write && state.operate && canReconcile(client), needsRefresh: state.retry && state.allowed,
    progress: card.progressStatus, success: card.successTitle, successDescription: card.successDescription };
}
export type MobileGitSnapshot = ReturnType<typeof mobileGitSnapshot>;
/** No selected-client data: safe only as an empty retained-inspector fallback. */
export const EMPTY_MOBILE_GIT: MobileGitSnapshot = {
  owner: '', revision: 0, ready: false, branchLabel: '', statusSummary: '', worktree: '', error: '', loading: false,
  busy: false, canWrite: false, canChangeBranch: false, rows: [], groups: [], dialog: '', defaultBranch: false,
  editingFiles: false, files: [], previewFiles: [], extraFiles: 0, selectedCount: 0, selectedInsertions: 0,
  selectedDeletions: 0, allSelected: true, commitDisabled: true, featureDisabled: true, confirmTitle: '',
  confirmDescription: '', confirmContinue: '', confirmDisabled: true, confirmFeatureDisabled: true,
  needsReconcile: false, needsRefresh: false, progress: '', success: '', successDescription: '',
};
export async function mobileGitRead(owner: string, now: number, nativeInput: Native | null | undefined, client: T3Client = mobileClient, refresh = false) {
  const state = mobileGitAccess(client), serial = ++state.readSerial, invalidation = state.invalidation; state.reading = true;
  try {
    const { native, assertCurrent } = await mobileGitAuthorize(owner, nativeInput, client);
    state.error = ''; const cwd = workspaceOf(client).cwd;
    if (state.retry) {
      const reply = await client.restAccess(native).call({ op: 'subscribe', key: VCS_STATUS_KEY, method: 'subscribeVcsStatus', payload: { cwd } });
      assertCurrent(); state.subscription = Math.max(state.subscription, subscriptionSerial(str(reply.id)));
    }
    const live = await watchVcsStatus(client, native, cwd, now); assertCurrent();
    if (live.status && (state.refreshed || live.status !== state.stream)) { state.stream = live.status; state.status = live.status; }
    if (live.error) state.error = live.error;
    if (!state.refreshed || refresh) {
      const result = await client.restAccess(native).request('vcs.refreshStatus', { cwd }); assertCurrent();
      // Refresh RPC is authoritative; a later shared stream object supersedes it.
      if (typeof result.isRepo === 'boolean') state.status = result;
      state.refreshed = true;
    }
    assertCurrent();
  } catch (error) { if (letGo(error)) throw error; if (mobileGitAccess(client) === state) state.error = message(error); }
  finally { if (serial === state.readSerial) state.reading = false; if (invalidation === state.invalidation) state.retry = false; client.revision++; }
  return mobileGitSnapshot(now, client);
}

/** A landed Git mutation refreshes independently of failed metadata delivery.
 * Fresh read authorization deliberately does not reuse the failed action wrapper.
 */
export async function mobileGitRefreshAfterMutation(owner: string, cwd: string, nativeInput: Native | null | undefined, client: T3Client) {
  const { native, assertCurrent } = await mobileGitAuthorize(owner, nativeInput, client);
  const result = await client.restAccess(native).request('vcs.refreshStatus', { cwd }); assertCurrent();
  if (workspaceOf(client).cwd === cwd && typeof result.isRepo === 'boolean') mobileGitAccess(client).status = result;
}

/** Actions use only shared transport. No action-scoped Native survives this call. */
export async function mobileGitAction(owner: string, op: string, id: string, value: string, now: number,
  nativeInput: Native | null | undefined, client: T3Client = mobileClient) {
  assertReviewOwner(client, owner);
  const state = mobileGitAccess(client); let error = '', destination = '', dismiss = false, pull = '';
  if (state.acting) return { message: 'Git action in progress.', destination, dismiss, data: mobileGitSnapshot(now, client) };
  // Local selection carries the captured owner but does not need a new network permission read.
  if (['close', 'files-edit', 'file', 'files-reset'].includes(op)) {
    if (op === 'file' && !workingFiles(mobileGitCurrentStatus(client, state)).some(file => file.path === id)) throw new ClientError('That changed file is no longer available.');
    if (op === 'files-reset') gitState(client).excluded.clear();
    else if (nativeInput) await gitLocal(client, reviewNative(client, nativeInput, owner), op === 'close' ? 'dialog-close' : op, id, value);
    client.revision++; return { message: '', destination, dismiss, data: mobileGitSnapshot(now, client) };
  }
  const before = mobileGitSnapshot(now, client);
  if (op === 'select') {
    const row = before.rows.find(row => row.id === id);
    if (!row || row.disabled) {
      const error = row?.subtitle || 'That action is unavailable.'; mobileGitFeedbackError(client, now, error);
      return { message: error, destination, dismiss, data: before };
    }
    if (id === 'review' || id === 'branches') return { message: '', destination: id, dismiss, data: before };
  }
  state.acting = true;
  try {
    const writes = op === 'quick' || op === 'reconcile' || op === 'commit' || op === 'confirm' || op === 'select' && ['push', 'pull'].includes(id)
      || op === 'select' && id === 'pr' && obj(mobileGitCurrentStatus(client, state)?.pr).state !== 'open';
    const feature = op === 'reconcile' || (op === 'commit' || op === 'confirm') && id === 'feature';
    // Linked PRs are orchestration shell metadata, not host Git reads. Opening
    // a currently linked HTTP(S) URL needs no source-control mutation/read grant.
    if (op === 'link' && !nativeInput?.available) throw new ClientError('Open in the native app to follow this link.');
    const { native, assertCurrent } = op === 'link'
      ? { native: reviewNative(client, letGoAware(mobileNative(nativeInput!)), owner), assertCurrent: () => assertReviewOwner(client, owner) }
      : await mobileGitAuthorize(owner, nativeInput, client, writes, feature);
    if (op !== 'link') state.error = ''; const status = mobileGitCurrentStatus(client, state), git = gitState(client), cwd = workspaceOf(client).cwd;
    gitCardView(client, status, '', cwd, now);
    if (op === 'reconcile') {
      if (!canReconcile(client)) throw new ClientError('That completed Git action belongs to another workspace.');
      // Shared completion owns this queue; mobile drains its metadata effect in
      // an explicit action using the shared durable command path. No background
      // resource writes, cross-environment queue flush, or automatic write retry.
      const queue = git.branchSync.splice(0);
      for (const entry of queue) {
        if (mobileGitThread(client)?.branch === entry.branch) continue;
        const access = client.restAccess(native), [commandId] = await access.ids(1);
        await access.dispatch(nativeFiles(native), { type: 'thread.metadata.update', commandId,
          threadId: entry.threadId, branch: entry.branch }, 'Change branch');
      }
      streamOwners.delete(client);
    } else if (op === 'refresh') {
      const result = await client.restAccess(native).request('vcs.refreshStatus', { cwd }); assertCurrent();
      if (typeof result.isRepo === 'boolean') state.status = result;
    } else if (op === 'link' || op === 'select' && id === 'pr' && obj(status?.pr).state === 'open') {
      const url = op === 'link' ? mobileGitSnapshot(now, client).groups.flatMap(group => group.rows).find(row => row.id === id)?.url : str(obj(status?.pr).url);
      if (!url) throw new ClientError('This pull request is no longer available.');
      const parsed = new URL(url); if (!['https:', 'http:'].includes(parsed.protocol) || parsed.username || parsed.password) throw new ClientError('This pull request URL cannot be opened.');
      const reply = await client.restAccess(native).call({ op: 'mobileOpenURL', url: parsed.href });
      if (reply.opened !== true) throw new ClientError('The pull request could not be opened.');
    } else if (op === 'quick') {
      const action = resolveQuickAction(mobileGitStatus(status), !!git.run?.running, status?.isDefaultRef === true, status?.hasPrimaryRemote === true);
      if (!state.write || status?.isRepo === false || action.disabled || action.kind !== 'run_action' || !action.action || action.action !== id)
        throw new ClientError('That Git action changed. Open the menu again.');
      const includesCommit = ['commit', 'commit_push', 'commit_push_pr'].includes(action.action);
      git.pending = { action: action.action, branchName: str(status?.refName), includesCommit, commitMessage: '', filePaths: [] };
      if (requiresDefaultBranchConfirmation(action.action, status?.isDefaultRef === true)) {
        state.confirmation = confirmationIdentity(status); git.dialog = 'confirm'; destination = 'confirm';
      } else { streamOwners.set(client, streamOwner(client)); await gitCommand(client, native, 'confirm', cwd, 'continue'); }
    } else if (op === 'select' && id === 'commit') {
      if (!state.write || !status?.hasWorkingTreeChanges) throw new ClientError('This commit is unavailable.');
      git.dialog = 'commit'; git.editing = false; git.excluded.clear(); destination = 'commit';
    } else if (op === 'select' && id === 'pull') {
      if (!state.write || num(status?.behindCount) <= 0) throw new ClientError('Pull is currently unavailable.');
      pull = mobileGitFeedbackPull(client, now);
      const result = await client.restAccess(native).request('vcs.pull', { cwd }, true); assertCurrent();
      git.success = { cwd, title: result.status === 'skipped_up_to_date' ? 'Already up to date' : `Pulled latest on ${str(result.refName)}`, description: '', at: now };
      const refreshed = await client.restAccess(native).request('vcs.refreshStatus', { cwd }); if (typeof refreshed.isRepo === 'boolean') state.status = refreshed;
    } else if (op === 'select' && ['push', 'pr'].includes(id)) {
      const item = buildMenuItems(mobileGitStatus(status), false, status?.hasPrimaryRemote === true).find(item => item.id === id);
      if (!item || item.disabled || !item.dialogAction) throw new ClientError('That Git action is no longer available.');
      git.pending = { action: item.dialogAction, branchName: str(status?.refName), includesCommit: false, commitMessage: '', filePaths: [] };
      if (requiresDefaultBranchConfirmation(item.dialogAction, status?.isDefaultRef === true)) {
        state.confirmation = confirmationIdentity(status); git.dialog = 'confirm'; destination = 'confirm';
      } else { streamOwners.set(client, streamOwner(client)); await gitCommand(client, native, 'confirm', cwd, 'continue'); dismiss = true; }
    } else if (op === 'commit') {
      if (!state.write || git.dialog !== 'commit' || !workingFiles(status).some(file => !git.excluded.has(file.path))) throw new ClientError('This commit is no longer available.');
      streamOwners.set(client, streamOwner(client));
      await gitCommand(client, native, feature ? 'commit-branch' : 'commit', cwd, value); dismiss = true;
    } else if (op === 'confirm') {
      if (!['continue', 'feature'].includes(id) || !git.pending || git.dialog !== 'confirm' || state.confirmation !== confirmationIdentity(status)) throw new ClientError('The branch changed. Choose the Git action again.');
      streamOwners.set(client, streamOwner(client));
      if (feature && !git.pending.includesCommit) {
        const refs = await client.restAccess(native).request('vcs.listRefs', { cwd: str(client.shell.projects.find(p => p.id === client.projectId)?.workspaceRoot), limit: 100 });
        const name = mobileAutoBranch(arr(refs.refs).filter(ref => !ref.isRemote).map(ref => str(ref.name)));
        const result = await client.restAccess(native).request('vcs.createRef', { cwd, refName: name, switchRef: true }, true);
        let metadataError: unknown;
        try {
          const [commandId] = await client.restAccess(native).ids(1);
          await client.restAccess(native).dispatch(nativeFiles(native), { type: 'thread.metadata.update', commandId, threadId: client.threadId,
            branch: str(result.refName, str(mobileGitThread(client)?.branch)), worktreePath: str(mobileGitThread(client)?.worktreePath) || null }, 'Change branch');
        } catch (error) { if (letGo(error)) throw error; metadataError = error; }
        try { await mobileGitRefreshAfterMutation(owner, cwd, nativeInput, client); }
        catch (error) { if (letGo(error) || !metadataError) throw error; }
        if (metadataError) throw metadataError;
        await gitCommand(client, native, 'confirm', cwd, 'continue');
      } else await gitCommand(client, native, 'confirm', cwd, id);
      dismiss = true;
    } else throw new ClientError('Unsupported Git action.');
    assertCurrent();
  } catch (cause) { if (letGo(cause)) throw cause; error = message(cause); if (mobileGitAccess(client) === state) { state.error = error; mobileGitFeedbackError(client, now, error); } }
  finally { state.acting = false; mobileGitFeedbackObserve(client, now); if (pull) mobileGitFeedbackCancelPull(client, pull); client.revision++; }
  return { message: error, destination, dismiss: dismiss && !error, data: mobileGitSnapshot(now, client) };
}
