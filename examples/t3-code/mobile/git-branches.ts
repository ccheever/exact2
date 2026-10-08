// Pinned365aa87982 GitBranchesSheet/use-selected-thread-git-actions (MIT).
// @ref llp/1109.011-responsive-workspace.decision.md#navigation-and-data-ownership
import { mobileClient, mobileNative } from './client';
import type { T3Client } from './shared/client';
import { arr, obj, str, type Obj } from './shared/domain';
import { ClientError, nativeFiles, type Native } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { workspaceOf } from './shared/r4-surfaces-panel';
import { gitState } from './shared/r4-git-actions';
import { assertReviewOwner, reviewOwner } from './review-owner';
import { mobileGitAccess, mobileGitAuthorize, mobileGitSnapshot, mobileGitThread, mobileGitRefreshAfterMutation } from './git-overview';
import { mobileFeatureBranch } from './git-overview-model';
interface Branches { owner: string; serial: number; readRevision: number; loading: boolean; loaded: boolean; refs: Obj[]; error: string }
const states = new WeakMap<T3Client, Branches>();
function stateOf(client: T3Client) {
  const owner = reviewOwner(client); let state = states.get(client);
  if (!state || state.owner !== owner) { state = { owner, serial: 0, readRevision: 0, loading: false, loaded: false, refs: [], error: '' }; states.set(client, state); }
  return state;
}
export function mobileGitBranchesSnapshot(now = 0, client: T3Client = mobileClient) {
  const state = stateOf(client), git = mobileGitSnapshot(now, client), currentPath = str(mobileGitThread(client)?.worktreePath);
  return { owner: state.owner, revision: client.revision, readRevision: state.readRevision, branchLabel: git.branchLabel, worktree: currentPath,
    initialBase: git.branchLabel === 'Detached HEAD' ? 'main' : git.branchLabel,
    loading: state.loading || !state.loaded && !state.error, busy: git.busy, canChange: git.canChangeBranch,
    error: state.error || git.error,
    rows: git.ready ? state.refs.filter(ref => !ref.isRemote).map(ref => {
      const path = str(ref.worktreePath), unavailable = !!path && path !== currentPath;
      return { name: str(ref.name), current: ref.current === true,
        subtitle: path ? path === currentPath ? 'Checked out in this thread' : 'Checked out in another worktree' : ref.isDefault ? 'Default branch' : 'Local branch',
        disabled: !git.canChangeBranch || git.busy || unavailable };
    }) : [] };
}
export async function mobileGitBranchesRead(owner: string, now: number, nativeInput: Native | null | undefined, client: T3Client = mobileClient) {
  assertReviewOwner(client, owner); const state = stateOf(client), serial = ++state.serial; state.loading = true;
  try {
    const { native, assertCurrent } = await mobileGitAuthorize(owner, nativeInput, client);
    const result = await client.restAccess(native).request('vcs.listRefs', { cwd: str(client.shell.projects.find(project => project.id === client.projectId)?.workspaceRoot), limit: 100 });
    assertCurrent(); if (serial !== state.serial) throw new ClientError('A newer branch read replaced this one.', 'superseded');
    // Source useBranches loads the unfiltered first100 refs, then only local refs.
    const seen = new Set<string>(); state.refs = arr(result.refs).filter(ref => !ref.isRemote && !seen.has(str(ref.name)) && !!seen.add(str(ref.name)));
    state.loaded = true; state.error = '';
  } catch (error) { if (letGo(error)) throw error; if (serial === state.serial) state.error = error instanceof Error ? error.message : 'Branches unavailable.'; }
  finally { if (serial === state.serial) state.loading = false; client.revision++; }
  return mobileGitBranchesSnapshot(now, client);
}
export async function mobileGitBranchAction(owner: string, op: string, value: string, baseBranch: string, now: number,
  nativeInput: Native | null | undefined, client: T3Client = mobileClient) {
  assertReviewOwner(client, owner); const state = stateOf(client), admission = mobileGitAccess(client);
  let error = '', changed = false;
  if (admission.acting || gitState(client).run?.running) return { message: 'Git action in progress.', changed, data: mobileGitBranchesSnapshot(now, client) };
  const picked = mobileGitBranchesSnapshot(now, client).rows.find(ref => ref.name === value);
  if (op === 'checkout' && (!picked || picked.disabled)) return { message: picked?.subtitle || 'That branch is no longer available.', changed, data: mobileGitBranchesSnapshot(now, client) };
  admission.acting = true;
  try {
    await mobileGitAuthorize(owner, nativeInput, client, true, true);
    const cwd = workspaceOf(client).cwd, root = str(client.shell.projects.find(project => project.id === client.projectId)?.workspaceRoot);
    const threadId = client.threadId, worktreePath = str(mobileGitThread(client)?.worktreePath);
    const identity = () => JSON.stringify([client.origin, client.environmentId, client.projectId, client.threadId, client.generation, client.threadEpoch]);
    const captured = identity(), revocation = admission.revocation;
    let nextCwd = cwd;
    const check = () => { if (identity() !== captured || admission.revocation !== revocation || ![cwd, nextCwd].includes(workspaceOf(client).cwd))
      throw new ClientError('The workspace changed. Open this screen again.', 'superseded'); };
    const raw = letGoAware(mobileNative(nativeInput!));
    const native: Native = { ...raw, later: async input => { check(); const result = await raw.later(input); check(); return result; } };
    const access = client.restAccess(native); let branch = '', path = worktreePath;
    if (op === 'checkout') {
      // Re-read branch occupation immediately before checkout; a cached row is not authority.
      const refs = await access.request('vcs.listRefs', { cwd: root, limit: 100 });
      const ref = arr(refs.refs).find(ref => !ref.isRemote && ref.name === value);
      if (!ref || str(ref.worktreePath) && ref.worktreePath !== worktreePath) throw new ClientError('That branch is checked out in another worktree or is no longer available.');
      const result = await access.request('vcs.switchRef', { cwd, refName: value }, true);
      branch = str(result.refName, str(mobileGitThread(client)?.branch));
    } else if (op === 'create') {
      if (!value.trim()) throw new ClientError('Enter a branch name.');
      const result = await access.request('vcs.createRef', { cwd, refName: mobileFeatureBranch(value), switchRef: true }, true);
      branch = str(result.refName, str(mobileGitThread(client)?.branch));
    } else if (op === 'worktree') {
      if (!value.trim() || !baseBranch.trim()) throw new ClientError('Enter a base branch and new branch name.');
      const result = await access.request('vcs.createWorktree', { cwd: root, refName: baseBranch.trim(), newRefName: mobileFeatureBranch(value), path: null }, true);
      branch = str(obj(result.worktree).refName); path = str(obj(result.worktree).path);
      if (!path || !branch) throw new ClientError('The server did not return the new worktree.');
      nextCwd = path;
    } else throw new ClientError('Unsupported branch action.');
    check();
    let metadataError: unknown;
    try {
      const [commandId] = await access.ids(1);
      await access.dispatch(nativeFiles(native), { type: 'thread.metadata.update', commandId, threadId, branch: branch || null, worktreePath: path || null }, 'Change branch');
    } catch (error) { if (letGo(error)) throw error; metadataError = error; }
    finally { state.loaded = false; state.readRevision++; }
    // Creating the ref/worktree already landed. Refresh on IDs/metadata failure,
    // retaining that error and refusing to report successful thread selection.
    check();
    try { await mobileGitRefreshAfterMutation(reviewOwner(client), path || root, nativeInput, client); }
    catch (error) { if (letGo(error) || !metadataError) throw error; }
    if (metadataError) throw metadataError;
    check(); changed = true; state.error = '';
  } catch (cause) { if (letGo(cause)) throw cause; error = cause instanceof Error ? cause.message : 'Git action failed.'; if (stateOf(client) === state) state.error = error; }
  finally { admission.acting = false; client.revision++; }
  return { message: error, changed, data: mobileGitBranchesSnapshot(now, client) };
}
