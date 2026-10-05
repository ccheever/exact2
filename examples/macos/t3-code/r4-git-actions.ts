// The workspace card's Git actions (lane r4-git), adapted from T3 Code (MIT; see
// LICENSE-T3): apps/web/src/components/GitActionsControl.tsx (panel display mode:
// the split quick action, its options menu, the commit, default-branch and publish
// dialogs, the inline progress and success rows) and
// packages/client-runtime/src/state/vcsAction.ts (the git.runStackedAction stream:
// a transport action id scoped to the environment and cwd, progress folding, the
// terminal result). The server runs every action; this module keeps the card's
// presentation state per client. The stream's events reach it through the
// client's event drain (client.ts) under GIT_ACTION_KEY.
import type { T3Client } from './client';
import { obj, str, type Obj } from './domain';
import { ClientError, type Native } from './protocol';
import { pushToast } from './toast';
import { draftContext } from './composer-controls-branch';
import { branchState } from './r4-git-branch';
import {
  defaultBranchCopy, formatElapsed, menuItems, menuNotes, menuReason, progressPresentation, providerMark, publishPathValid,
  quickAction, quickActionIcon, requiresDefaultBranchConfirmation, sortedPublishProviders, terminology, workingFiles,
} from './r4-git-logic';

export const GIT_ACTION_KEY = 'r4-git-action';
export const SUCCESS_VISIBLE_MS = 10_000;

type Run = { cwd: string; transportId: string; action: string; operation: string; running: boolean; label: string; phaseLabel: string; output: string;
  phaseAt: number; hookAt: number; subscriptionId: string; generation: number; threadId: string; draftKey: string };
type Pending = { action: string; branchName: string; includesCommit: boolean; commitMessage: string; filePaths: string[] };
type Publish = { step: number; provider: string; repository: string | null; visibility: string; remote: string; protocol: string; advanced: boolean;
  pending: boolean; error: string; result: Obj | null; discovered: Obj[]; loaded: boolean };
export interface GitState {
  cwd: string; status: Obj | null; run: Run | null; success: { cwd: string; title: string; description: string; at: number } | null;
  dialog: string; pending: Pending | null; editing: boolean; excluded: Set<string>; initPending: boolean;
  ends: Map<string, Obj>; branchSync: { threadId: string; draftKey: string; branch: string }[]; publish: Publish; editor: string; lastNow: number;
}
const freshPublish = (): Publish => ({ step: 0, provider: '', repository: null, visibility: 'private', remote: 'origin', protocol: 'ssh', advanced: false, pending: false, error: '', result: null, discovered: [], loaded: false });
const states = new WeakMap<T3Client, GitState>();
export function gitState(client: T3Client): GitState {
  let state = states.get(client);
  if (!state) {
    state = { cwd: '', status: null, run: null, success: null, dialog: '', pending: null, editing: false, excluded: new Set(), initPending: false, ends: new Map(), branchSync: [], publish: freshPublish(), editor: '', lastNow: 0 };
    states.set(client, state);
  }
  return state;
}
const message = (error: unknown) => error instanceof Error ? error.message : 'An error occurred.';
const failure = (client: T3Client, title: string, error: unknown) => pushToast(client, { kind: 'error', title, description: typeof error === 'string' ? error : message(error), timeoutMs: 0, stacked: true });

/** True while the card must keep its clock running: an action's elapsed time or an inline success. */
export function gitTicking(client: T3Client): boolean {
  const state = states.get(client);
  return !!state && (!!state.run?.running || !!state.success);
}

// ── The progress stream ──────────────────────────────────────────────────────

/** createVcsActionTransportId: the action id the server echoes, scoped to environment and cwd. */
export function transportActionId(environmentId: string, cwd: string, actionId: string): string {
  const key = JSON.stringify([environmentId, cwd]);
  return `${key.length}:${key}${actionId}`;
}

function finish(client: T3Client, state: GitState, run: Run, failed: string): void {
  if (state.run !== run) return;
  state.run = null;
  if (failed) failure(client, 'Action failed', failed);
}

/** One `r4-git-action` inbox entry: applyVcsActionProgressEvent, then the terminal result. */
export function gitActionEvent(client: T3Client, entry: Obj): void {
  const state = states.get(client), run = state?.run;
  if (!state || !run) return;
  const subscription = str(entry.subscriptionId), item = obj(entry.value);
  if (item._retryDue) return; // a one-shot action is never resubscribed
  if (item._transportError || item._streamEnded) {
    if (!run.subscriptionId) { state.ends.set(subscription, item); return; }
    if (subscription !== run.subscriptionId) return;
    finish(client, state, run, item._transportError ? str(obj(item._transportError).message, 'The connection ended.')
      : `Source control action '${run.action}' ended without a terminal result.`);
    return;
  }
  if (str(item.actionId) !== run.transportId || str(item.cwd) !== run.cwd) return;
  if (!run.subscriptionId) run.subscriptionId = subscription;
  switch (str(item.kind)) {
    case 'action_started': run.phaseAt = 0; run.hookAt = 0; run.output = ''; break;
    case 'phase_started': run.label = run.phaseLabel = str(item.label); run.phaseAt = 0; run.hookAt = 0; run.output = ''; break;
    case 'hook_started': run.label = `Running ${str(item.hookName)}...`; run.hookAt = -1; run.output = ''; break;
    case 'hook_output': run.output = str(item.text); break;
    case 'hook_finished': run.label = run.phaseLabel; run.hookAt = 0; run.output = ''; break;
    case 'action_finished': {
      const result = obj(item.result), toast = obj(result.toast), branch = obj(result.branch);
      state.success = { cwd: run.cwd, title: str(toast.title, 'Done'), description: str(toast.description), at: 0 };
      // resolveThreadBranchUpdate: a feature ref the action created becomes the thread's branch.
      if (branch.status === 'created' && str(branch.name)) state.branchSync.push({ threadId: run.threadId, draftKey: run.draftKey, branch: str(branch.name) });
      finish(client, state, run, '');
      break;
    }
    case 'action_failed': {
      const phase = str(item.phase) || 'execution';
      finish(client, state, run, `Source control action '${str(item.action, run.action)}' failed during ${phase}.`);
      break;
    }
  }
}

async function startAction(client: T3Client, native: Native, input: { action: string; commitMessage?: string; featureBranch?: boolean; filePaths?: string[] }): Promise<void> {
  const state = gitState(client);
  if (state.run?.running) throw new ClientError('Git action in progress.');
  const cwd = state.cwd;
  if (!cwd) throw new ClientError('Git status is unavailable.');
  const access = client.restAccess(native);
  const [actionId = ''] = await access.ids(1);
  const transportId = transportActionId(client.environmentId, cwd, actionId);
  const run: Run = { cwd, transportId, action: input.action, operation: 'run_change_request', running: true, label: 'Running source control action', phaseLabel: 'Running source control action',
    output: '', phaseAt: 0, hookAt: 0, subscriptionId: '', generation: client.generation, threadId: client.threadId, draftKey: client.draftKey };
  state.success = null; state.run = run;
  const payload: Obj = { actionId: transportId, cwd, action: input.action,
    ...(input.commitMessage ? { commitMessage: input.commitMessage } : {}), ...(input.featureBranch ? { featureBranch: true } : {}),
    ...(input.filePaths?.length ? { filePaths: input.filePaths } : {}),
    // A pull request the action opens is linked to the thread it ran beside; a draft has none yet.
    ...(client.threadId ? { threadId: client.threadId } : client.projectId ? { projectId: client.projectId } : {}) };
  try {
    const reply = await access.call({ op: 'subscribe', key: GIT_ACTION_KEY, method: 'git.runStackedAction', payload });
    if (state.run !== run) return;
    run.subscriptionId = run.subscriptionId || str(reply.id);
    const ended = state.ends.get(run.subscriptionId);
    state.ends.clear();
    if (ended) finish(client, state, run, ended._transportError ? str(obj(ended._transportError).message, 'The connection ended.') : `Source control action '${run.action}' ended without a terminal result.`);
  } catch (error) {
    finish(client, state, run, message(error));
  }
}

/** runGitActionWithToast's gate: an action that pushes from the default ref asks first. */
async function runWithConfirmation(client: T3Client, native: Native, action: string, extra: { commitMessage?: string; filePaths?: string[] } = {}): Promise<void> {
  const state = gitState(client), status = state.status;
  const branch = str(status?.refName);
  if (requiresDefaultBranchConfirmation(action, status?.isDefaultRef === true) && branch) {
    const includesCommit = ['commit', 'commit_push', 'commit_push_pr'].includes(action) && (action === 'commit' || status?.hasWorkingTreeChanges === true);
    state.pending = { action, branchName: branch, includesCommit, commitMessage: extra.commitMessage ?? '', filePaths: extra.filePaths ?? [] };
    state.dialog = 'confirm';
    return;
  }
  await startAction(client, native, { action, ...extra });
}

/** `shell:git-*` from the card and its dialogs (server writes). */
export async function gitCommand(client: T3Client, native: Native, op: string, id: string, value: string): Promise<string> {
  const state = gitState(client);
  if (id && state.cwd && id !== state.cwd) throw new ClientError('The workspace changed. Try again.');
  if (op === 'quick') {
    const status = state.status, action = quickAction(status, !!state.run?.running, status?.isDefaultRef === true, status?.hasPrimaryRemote === true);
    if (action.kind === 'open_publish') { openPublish(state); return ''; }
    if (action.kind === 'run_pull') return pull(client, native, state);
    if (action.kind === 'show_hint') { pushToast(client, { kind: 'info', title: action.label, description: action.hint }); return ''; }
    await runWithConfirmation(client, native, str(action.action));
    return '';
  }
  if (op === 'menu') {
    const item = menuItems(state.status, !!state.run?.running, state.status?.hasPrimaryRemote === true).find(entry => entry.dialogAction === value || entry.id === value);
    if (value === 'publish') { openPublish(state); return ''; }
    if (!item || item.disabled) throw new ClientError(item ? menuReason(item, state.status, !!state.run?.running, state.status?.hasPrimaryRemote === true) : 'That action is unavailable.');
    if (item.dialogAction === 'commit') { state.dialog = 'commit'; state.editing = false; state.excluded = new Set(); return ''; }
    await runWithConfirmation(client, native, item.dialogAction);
    return '';
  }
  if (op === 'commit' || op === 'commit-branch') {
    const files = workingFiles(state.status), selected = files.filter(file => !state.excluded.has(file.path));
    if (!selected.length) throw new ClientError('Select at least one file to commit.');
    const all = selected.length === files.length, commitMessage = value.trim();
    state.dialog = ''; state.editing = false; state.excluded = new Set();
    await startAction(client, native, { action: 'commit', ...(commitMessage ? { commitMessage } : {}), ...(all ? {} : { filePaths: selected.map(file => file.path) }),
      ...(op === 'commit-branch' ? { featureBranch: true } : {}) });
    return '';
  }
  if (op === 'confirm') {
    const pending = state.pending;
    state.pending = null; state.dialog = '';
    if (!pending || (value !== 'continue' && value !== 'feature')) return '';
    await startAction(client, native, { action: pending.action, ...(pending.commitMessage ? { commitMessage: pending.commitMessage } : {}),
      ...(pending.filePaths.length ? { filePaths: pending.filePaths } : {}), ...(value === 'feature' ? { featureBranch: true } : {}) });
    return '';
  }
  if (op === 'init') {
    if (!state.cwd) throw new ClientError('Git status is unavailable.');
    state.initPending = true;
    try { await client.restAccess(native).request('vcs.init', { cwd: state.cwd }, true); }
    catch (error) { failure(client, 'Git initialization failed', error); }
    finally { state.initPending = false; }
    return '';
  }
  if (op === 'publish') return publish(client, native, state, value);
  throw new ClientError(`Unknown git action: ${op}`);
}

async function pull(client: T3Client, native: Native, state: GitState): Promise<string> {
  const cwd = state.cwd;
  const run: Run = { cwd, transportId: '', action: '', operation: 'pull', running: true, label: '', phaseLabel: '', output: '', phaseAt: 0, hookAt: 0,
    subscriptionId: '', generation: client.generation, threadId: client.threadId, draftKey: client.draftKey };
  state.success = null; state.run = run;
  try {
    const result = await client.restAccess(native).request('vcs.pull', { cwd }, true);
    const pulled = result.status === 'pulled', ref = str(result.refName);
    state.success = { cwd, title: pulled ? 'Pulled' : 'Already up to date', at: 0,
      description: pulled ? `Updated ${ref} from ${str(result.upstreamRef) || 'upstream'}` : `${ref} is already synchronized.` };
  } catch (error) { failure(client, 'Pull failed', error); }
  finally { if (state.run === run) state.run = null; }
  return '';
}

// ── Publish repository ───────────────────────────────────────────────────────
function openPublish(state: GitState): void { state.publish = freshPublish(); state.dialog = 'publish'; }
async function publish(client: T3Client, native: Native, state: GitState, value: string): Promise<string> {
  const form = state.publish, chosen = sortedPublishProviders(form.discovered).find(option => option.value === form.provider) ?? sortedPublishProviders(form.discovered).find(option => option.ready);
  const repository = (form.repository ?? (chosen?.account ? `${chosen.account}/` : '')).trim();
  if (!chosen?.ready || !publishPathValid(repository) || form.pending) throw new ClientError('Choose a ready provider and enter owner/name.');
  void value;
  form.pending = true; form.error = '';
  try {
    form.result = await client.restAccess(native).request('sourceControl.publishRepository', { cwd: state.cwd, provider: chosen.value, repository,
      visibility: form.visibility, remoteName: form.remote.trim() || 'origin', protocol: form.protocol }, true);
    form.step = 2;
  } catch (error) { form.error = message(error); }
  finally { form.pending = false; }
  return '';
}

// ── Card-local presentation (`chatlocal:git-*`) ──────────────────────────────
export async function gitLocal(client: T3Client, native: Native, op: string, id: string, value: string): Promise<string> {
  const state = gitState(client), form = state.publish;
  if (op === 'dialog-close') { state.dialog = ''; state.pending = null; state.editing = false; state.excluded = new Set(); return ''; }
  if (op === 'files-edit') { state.editing = !state.editing; return ''; }
  if (op === 'file') { if (state.excluded.has(id)) state.excluded.delete(id); else state.excluded.add(id); return ''; }
  if (op === 'files-all') { const files = workingFiles(state.status); state.excluded = state.excluded.size === 0 ? new Set(files.map(file => file.path)) : new Set(); return ''; }
  if (op === 'open-file') {
    if (!state.cwd) { pushToast(client, { kind: 'error', title: 'Editor opening is unavailable.' }); return ''; }
    // useOpenInPreferredEditor: the card's chosen editor (shell-details.ts hands it over).
    try { await client.restAccess(native).request('shell.openInEditor', { cwd: `${state.cwd.replace(/\/+$/, '')}/${id}`, editor: state.editor || 'file-manager' }); }
    catch (error) { failure(client, 'Unable to open file', error); }
    return '';
  }
  if (op === 'publish-provider') { form.provider = id; form.repository = null; return ''; }
  if (op === 'publish-back') { if (form.step === 0) state.dialog = ''; else form.step = Math.max(0, form.step - 1); return ''; }
  if (op === 'publish-step') { form.step = Math.max(0, Math.min(2, Number(value) || 0)); return ''; }
  if (op === 'publish-repository') { form.repository = value; return ''; }
  if (op === 'publish-visibility') { form.visibility = value === 'public' ? 'public' : 'private'; return ''; }
  if (op === 'publish-remote') { form.remote = value; return ''; }
  if (op === 'publish-protocol') { form.protocol = value === 'https' ? 'https' : 'ssh'; return ''; }
  if (op === 'publish-advanced') { form.advanced = !form.advanced; return ''; }
  throw new ClientError(`Unknown git view action: ${op}`);
}

// ── The card's view ──────────────────────────────────────────────────────────
async function flushBranchSync(client: T3Client, native: Native, state: GitState): Promise<void> {
  const queue = state.branchSync.splice(0);
  for (const entry of queue) {
    if (entry.threadId) {
      if (!client.writable) continue;
      const shell = client.shell.threads.find(thread => thread.id === entry.threadId);
      if (shell && str(shell.branch) === entry.branch) continue;
      try {
        const access = client.restAccess(native), [commandId] = await access.ids(1);
        await access.request('orchestration.dispatchCommand', { type: 'thread.metadata.update', commandId, threadId: entry.threadId, branch: entry.branch }, true);
      } catch { /* reportFailure: thread branch metadata update */ }
    } else {
      const contexts = ((client.local as { composerControls: { contexts?: Record<string, Obj> } }).composerControls.contexts ??= {});
      contexts[entry.draftKey] = { ...draftContext(client, entry.draftKey), branch: entry.branch };
    }
  }
}

async function loadDiscovery(client: T3Client, native: Native, state: GitState): Promise<void> {
  if (state.dialog !== 'publish' || state.publish.loaded) return;
  state.publish.loaded = true;
  try {
    const found = (await client.restAccess(native).request('server.discoverSourceControl', {})).sourceControlProviders;
    state.publish.discovered = Array.isArray(found) ? found.filter((entry): entry is Obj => !!entry && typeof entry === 'object' && !Array.isArray(entry)) : [];
  } catch { state.publish.discovered = []; }
}

export type GitView = ReturnType<typeof gitCardView>;
/** The card's Version Control rows, menu, notes and dialogs for one status. */
export function gitCardView(client: T3Client, status: Obj | null, error: string, cwd: string, now: number, editor = '') {
  const state = gitState(client);
  state.editor = editor;
  if (state.cwd !== cwd) { state.cwd = cwd; state.dialog = ''; state.pending = null; if (state.success?.cwd !== cwd) state.success = null; }
  state.status = status;
  const run = state.run && state.run.cwd === cwd && state.run.generation === client.generation ? state.run : null;
  if (state.run && !run && state.run.generation !== client.generation) state.run = null; // a reconnect interrupts the stream
  // Event times are stamped from the card's clock once it moves past the answer that saw the event:
  // a clock that idled before the action started would otherwise count its idle time.
  const moved = now !== state.lastNow; state.lastNow = now;
  if (run && run.phaseAt === 0 && moved) run.phaseAt = now;
  if (run && run.hookAt === -1 && moved) run.hookAt = now;
  if (state.success && state.success.at === 0 && moved) state.success.at = now;
  // An unstamped success has not been shown yet, so it cannot have expired.
  if (state.success && (state.success.cwd !== cwd || (state.success.at !== 0 && now - state.success.at >= SUCCESS_VISIBLE_MS))) state.success = null;
  const busy = !!run?.running, hasRemote = status?.hasPrimaryRemote === true;
  const quick = quickAction(status, busy, status?.isDefaultRef === true, hasRemote);
  const progress = run ? progressPresentation(run) : null;
  const items = menuItems(status, busy, hasRemote);
  const files = workingFiles(status), selected = files.filter(file => !state.excluded.has(file.path));
  const term = terminology(status), pending = state.pending;
  const copy = pending ? defaultBranchCopy(pending.action, pending.branchName, pending.includesCommit, term) : { title: 'Run action on default branch?', description: '', continueLabel: 'Continue' };
  const form = state.publish, providers = sortedPublishProviders(form.discovered);
  const chosen = providers.find(option => option.value === form.provider && option.ready) ?? providers.find(option => option.ready) ?? providers.find(option => option.value === (form.provider || 'github'))!;
  const repository = form.repository ?? (chosen.account ? `${chosen.account}/` : '');
  return {
    repo: status?.isRepo !== false, initLabel: state.initPending ? 'Initializing...' : 'Initialize Git', initPending: state.initPending,
    quickLabel: quick.label, quickIcon: quickActionIcon(quick, status), quickDisabled: quick.disabled, quickHint: quick.disabled ? (quick.hint ?? 'This action is currently unavailable.') : '',
    progress: !!progress, progressStatus: progress?.status ?? '', progressOutput: progress?.output ?? '', progressElapsed: progress ? (progress.startedAt > 0 ? formatElapsed(progress.startedAt, now) : '0s') : '',
    success: !progress && !!state.success, successTitle: state.success?.title ?? '', successDescription: state.success?.description ?? '',
    menuDisabled: busy, menuOpen: !busy && branchState(client).open === 'menu',
    menu: [...items.map(item => ({ id: item.id as string, label: item.label, icon: item.icon, disabled: item.disabled, reason: menuReason(item, status, busy, hasRemote) })),
      ...(status?.isRepo === true && !hasRemote ? [{ id: 'publish', label: 'Publish repository...', icon: 'cloud-upload', disabled: busy, reason: '' }] : [])],
    menuPublish: status?.isRepo === true && !hasRemote, notes: menuNotes(status, error),
    dialog: state.dialog, sourceMark: providerMark(status),
    commitBranch: str(status?.refName) || '(detached HEAD)', commitDefault: status?.isDefaultRef === true, editing: state.editing,
    files: files.map(file => ({ ...file, excluded: state.excluded.has(file.path) })), fileCount: files.length, selectedCount: selected.length,
    allSelected: state.excluded.size === 0, noneSelected: selected.length === 0, someSelected: state.excluded.size > 0 && selected.length > 0,
    selectedInsertions: selected.reduce((sum, file) => sum + file.insertions, 0), selectedDeletions: selected.reduce((sum, file) => sum + file.deletions, 0),
    confirmTitle: copy.title, confirmDescription: copy.description, confirmContinue: copy.continueLabel,
    publishStep: form.step, publishProviders: providers.map(option => ({ value: option.value, label: option.label, ready: option.ready, hint: option.hint || 'Open Settings -> Source Control to configure this provider.',
      selected: option.value === chosen.value && option.ready })),
    publishRows: pairs(providers.map(option => ({ value: option.value, label: option.label, ready: option.ready, hint: option.hint || 'Open Settings -> Source Control to configure this provider.',
      selected: option.value === chosen.value && option.ready }))),
    publishProvider: chosen.value, publishProviderLabel: chosen.label, publishHost: chosen.value === 'forgejo' && chosen.host ? chosen.host : chosen.host || PROVIDER_HOSTS[chosen.value] || '',
    publishPlaceholder: chosen.placeholder, publishRepository: repository, publishVisibility: form.visibility, publishRemote: form.remote, publishProtocol: form.protocol,
    publishAdvanced: form.advanced, publishPending: form.pending, publishError: form.error, publishAnyReady: providers.some(option => option.ready),
    publishCanNext: chosen.ready, publishCanSubmit: chosen.ready && !form.pending && publishPathValid(repository),
    publishPushed: str(form.result?.status) === 'pushed', publishName: str(obj(form.result?.repository).nameWithOwner),
    publishUrl: str(obj(form.result?.repository).url),
    publishSummary: str(form.result?.status) === 'pushed' ? `${str(form.result?.branch)} is now live on ${chosen.label}.` : `Remote "${str(form.result?.remoteName, 'origin')}" is set up. Make a commit and push it to share your code.`,
  };
}
/** The provider grid's rows of two (an odd last card keeps its half). */
function pairs<T>(items: T[]): { key: string; first: T; second: T; pair: boolean }[] {
  const rows: { key: string; first: T; second: T; pair: boolean }[] = [];
  for (let index = 0; index < items.length; index += 2) rows.push({ key: String(index), first: items[index]!, second: items[index + 1] ?? items[index]!, pair: index + 1 < items.length });
  return rows;
}
const PROVIDER_HOSTS: Record<string, string> = { forgejo: 'your server', github: 'github.com', gitlab: 'gitlab.com', bitbucket: 'bitbucket.org', 'azure-devops': 'dev.azure.com' };

/** The data source's side work for the card: queued branch metadata and the publish dialog's discovery. */
export async function gitCardPrepare(client: T3Client, native: Native): Promise<void> {
  const state = gitState(client);
  if (state.branchSync.length) await flushBranchSync(client, native, state);
  await loadDiscovery(client, native, state);
}
