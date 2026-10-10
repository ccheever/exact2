// Pinned365aa87982 use-vcs-action-state / GitActionProgressOverlay (MIT).
// @ref llp/1109.011-responsive-workspace.decision.md#navigation-and-data-ownership
// Presentation around the shared fold, never a second Git event reducer.
import type { T3Client } from './shared/client';
import { arr, num, obj, str } from './shared/domain';
import { bridgeReply, type Native } from './shared/protocol';
import { gitState, GIT_ACTION_KEY, type GitState } from './shared/r4-git-actions';
import { workspaceOf } from './shared/r4-surfaces-panel';
import { toasts } from './shared/toast';

type Run = NonNullable<GitState['run']>;
type Result = { id: string; phase: string; label: string; description: string; prUrl: string; deadline: number };
interface State {
  owner: string; serial: number; run: Run | null; toastFloor: number;
  success: GitState['success']; result: Result | null;
  link: { run: Run; title: string; description: string; url: string } | null;
  pull: { id: string; cwd: string; startedAt: number } | null;
  displayed: Result | null; retireAt: number;
}
const states = new WeakMap<T3Client, State>();
const scope = (client: T3Client) => JSON.stringify([client.origin, client.environmentId, client.generation]);
function stateOf(client: T3Client): State {
  const owner = scope(client); let state = states.get(client);
  if (!state || state.owner !== owner) {
    state = { owner, serial: 0, run: null, toastFloor: 0, success: gitState(client).success, result: null, link: null, pull: null, displayed: null, retireAt: 0 };
    states.set(client, state);
  }
  return state;
}
function currentRun(client: T3Client) {
  const run = gitState(client).run;
  return run?.running && run.generation === client.generation ? run : null;
}
function captureRun(client: T3Client, state: State) {
  const run = currentRun(client);
  if (run && run !== state.run) {
    state.run = run; state.link = null; state.pull = null; state.result = null;
    state.toastFloor = Math.max(0, ...toasts(client).map(toast => toast.id));
  }
}
function httpUrl(input: string) {
  try {
    const url = new URL(input);
    return ['http:', 'https:'].includes(url.protocol) && !url.username && !url.password ? url.href : '';
  } catch { return ''; }
}
function publish(state: State, now: number, phase: string, label: string, description = '', prUrl = '') {
  // Completion may arrive in a long-lived invocation with its older input
  // clock. Stamp the five seconds at the first fresh presentation instead.
  state.result = { id: `${state.owner}:${++state.serial}`, phase, label, description, prUrl, deadline: 0 };
  state.run = null; state.pull = null; state.link = null;
}

/** Before the shared client acknowledges a batch, retain only the CTA metadata
 * its success projection omits. A later shared success must agree before use.
 * No phase, terminal outcome, toast or branch metadata is folded here. */
export function mobileGitFeedbackMetadata(entries: unknown, after: number, client: T3Client) {
  const state = stateOf(client); captureRun(client, state); const run = state.run;
  if (!run) return;
  for (const entry of arr(entries)) {
    if (entry.key !== GIT_ACTION_KEY || num(entry.generation, -1) !== client.generation || num(entry.seq) <= after) continue;
    const item = obj(entry.value);
    if (item.kind !== 'action_finished' || item.actionId !== run.transportId || item.cwd !== run.cwd
      || run.subscriptionId && entry.subscriptionId !== run.subscriptionId || state.link) continue;
    const toast = obj(obj(item.result).toast), cta = obj(toast.cta);
    state.link = { run, title: str(toast.title, 'Done'), description: str(toast.description),
      url: cta.kind === 'open_pr' ? httpUrl(str(cta.url)) : '' };
  }
}

/** Called after the actual shared fold and after an action. Only that fold's
 * success object or newly emitted Git failure toast authorizes a notification.
 * The result is app-wide within its connection, like the source result atom.
 * Reconnect/environment changes retire all callbacks and never replay success. */
export function mobileGitFeedbackObserve(client: T3Client, now: number) {
  const state = stateOf(client); captureRun(client, state);
  const git = gitState(client), success = git.success;
  if (success !== state.success) {
    state.success = success;
    const cwd = state.run?.cwd || state.pull?.cwd;
    if (success && cwd === success.cwd) {
      const link = state.link;
      const url = link?.run === state.run && link.title === success.title && link.description === success.description ? link.url : '';
      publish(state, now, 'success', success.title, success.description, url);
    }
  }
  if (state.run && !currentRun(client)) {
    const failure = toasts(client).findLast(toast => toast.id > state.toastFloor && toast.kind === 'error' && toast.title === 'Action failed');
    if (failure) publish(state, now, 'error', 'Git action failed', failure.description);
    else { state.run = null; state.link = null; }
  }
}
export function mobileGitFeedbackPull(client: T3Client, now: number) {
  const state = stateOf(client);
  state.result = null; state.pull = { id: `${state.owner}:pull:${++state.serial}`, cwd: workspaceOf(client).cwd, startedAt: now };
  return state.pull.id;
}
export function mobileGitFeedbackCancelPull(client: T3Client, id: string) {
  const state = stateOf(client); if (state.pull?.id === id) state.pull = null;
}
export function mobileGitFeedbackError(client: T3Client, now: number, description: string) {
  const state = stateOf(client); state.success = gitState(client).success;
  publish(state, now, 'error', 'Git action failed', description);
}
export function mobileGitFeedbackSnapshot(now: number, client: T3Client, active = true) {
  mobileGitFeedbackObserve(client, now);
  const state = stateOf(client), run = currentRun(client), cwd = workspaceOf(client).cwd;
  const empty = { owner: state.owner, id: '', phase: 'idle', label: '', description: '', prUrl: '', deadline: 0, visible: false };
  if (state.result?.deadline === 0) state.result.deadline = now + 5000;
  if (state.result && now >= state.result.deadline) state.result = null;
  let current: Result | null = state.result;
  if (run && run.cwd === cwd) {
    const startedAt = run.hookAt > 0 ? run.hookAt : run.phaseAt;
    const seconds = startedAt > 0 ? Math.max(0, Math.floor((now - startedAt) / 1000)) : 0;
    const output = run.output.trim().split(/\r?\n/).filter(Boolean).at(-1) || '';
    current = { id: run.transportId, phase: 'running', label: run.label, prUrl: '', deadline: 0,
      description: output || (seconds >= 2 ? `Running for ${seconds}s` : '') };
  } else if (state.pull && state.pull.cwd === cwd) {
    const seconds = Math.max(0, Math.floor((now - state.pull.startedAt) / 1000));
    current = { id: state.pull.id, phase: 'running', label: 'Pulling latest changes', prUrl: '', deadline: 0, description: seconds >= 2 ? `Running for ${seconds}s` : '' };
  }
  if (!active) { state.displayed = null; state.retireAt = 0; return empty; }
  if (current) {
    state.displayed = current; state.retireAt = 0;
    return { owner: state.owner, ...current, visible: true };
  }
  if (state.displayed) {
    if (!state.retireAt) state.retireAt = now + 150;
    if (now < state.retireAt) return { owner: state.owner, ...state.displayed, deadline: state.retireAt, visible: false };
    state.displayed = null; state.retireAt = 0;
  }
  return empty;
}
export type MobileGitFeedback = ReturnType<typeof mobileGitFeedbackSnapshot>;

/** The displayed immutable result is the sole URL authority. Running presses
 * do nothing; expired, replaced or reconnected callbacks cannot dismiss/open.
 * This local operation never dispatches a Git/orchestration mutation. */
export async function mobileGitFeedbackAction(owner: string, id: string, op: string, now: number, native: Native | null | undefined, client: T3Client): Promise<{ opened: boolean }> {
  const shown = mobileGitFeedbackSnapshot(now, client), state = stateOf(client);
  if (shown.owner !== owner || shown.id !== id || !id || !shown.visible || shown.phase === 'running' || shown.phase === 'idle') return { opened: false };
  if (op === 'press' && shown.prUrl) {
    if (!native?.available || !httpUrl(shown.prUrl)) return { opened: false };
    const reply = await bridgeReply(native, { op: 'mobileOpenURL', url: shown.prUrl });
    if (stateOf(client) !== state || state.result?.id !== id) return { opened: false };
    return { opened: reply.ok && obj(reply.value).opened === true };
  }
  if (op === 'press' || op === 'dismiss') state.result = null;
  return { opened: false };
}
