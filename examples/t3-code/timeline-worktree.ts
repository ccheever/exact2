// The worktree setup card, adapted from T3 Code (MIT, see LICENSE-T3):
// packages/contracts/src/worktreeSetup.ts (the snapshot and stage labels),
// packages/client-runtime/src/worktreeSetup.ts (resolveVisibleWorktreeSetup,
// worktreeSetupAgentStarted), apps/web/src/components/ChatView.tsx and
// ChatView.logic.ts (the subscribeWorktreeSetup stream, resolveWorktreeSetupProgress,
// cancel), chat/MessagesTimeline.logic.ts (the card's slot under the working
// header, 3a7058d) and chat/WorktreeSetupCard.tsx (stages, tail, details, actions).
import { arr, obj, str, type Obj } from './domain';
import { ClientError, activeRun, type Native } from './protocol';
import type { T3Client } from './client';
import { setupTurnStarted } from './r12-threads-worktree';

export const WORKTREE_SETUP_KEY = 'worktree-setup';
const STAGE_LABELS: Record<string, string> = { fetch: 'Fetch base branch', checkout: 'Check out files', submodules: 'Init submodules',
  'setup-script': 'Run setup script', agent: 'Start agent' };
const TAIL_LINES = 4;

interface SetupState { id: string; threadId: string; latest: Obj | null; detailsOpen: boolean }
const states = new WeakMap<T3Client, SetupState>();
function stateOf(client: T3Client): SetupState {
  let state = states.get(client);
  if (!state) { state = { id: '', threadId: '', latest: null, detailsOpen: false }; states.set(client, state); }
  return state;
}

/** One event of the subscribeWorktreeSetup stream: null (nothing tracked) or the newest snapshot by sequence. */
export function worktreeSetupEvent(client: T3Client, entry: Obj): void {
  const state = states.get(client);
  if (!state || !state.id || str(entry.subscriptionId) !== state.id) return;
  const item = entry.value === null ? null : obj(entry.value);
  if (item && (item._transportError || item._streamEnded)) { state.id = ''; return; }
  if (!item || !item.threadId) return;
  if (str(item.threadId) !== state.threadId) return;
  if (state.latest && Number(state.latest.sequence) > Number(item.sequence)) return;
  state.latest = item;
}

/** ChatView's subscription rule: the open thread while its run prepares a workspace or it already has a worktree. */
export function wantsWorktreeSetup(client: T3Client): string {
  const thread = obj(client.projection.thread);
  if (!client.threadId || !client.thread || str(thread.id) !== client.threadId) return '';
  const preparing = str(activeRun(client.projection)?.status) === 'preparing';
  return preparing || str(thread.worktreePath) || stateOf(client).latest?.phase === 'running' ? client.threadId : '';
}

export async function syncWorktreeSetup(client: T3Client, native: Native | null | undefined): Promise<void> {
  if (!native?.available || !client.ready) return;
  const state = stateOf(client), wanted = wantsWorktreeSetup(client);
  if (state.id && state.threadId === wanted) return;
  if (state.id) { await client.restAccess(native).call({ op: 'unsubscribe', key: WORKTREE_SETUP_KEY }).catch(() => undefined); state.id = ''; }
  if (state.threadId !== wanted) { state.latest = null; state.detailsOpen = false; }
  state.threadId = wanted;
  if (!wanted) return;
  try { state.id = str((await client.restAccess(native).call({ op: 'subscribe', key: WORKTREE_SETUP_KEY, method: 'subscribeWorktreeSetup', payload: { threadId: wanted } })).id); }
  catch { state.id = ''; }
}

export const agentStarted = (snapshot: Obj) => arr(snapshot.stages).some(stage => stage.id === 'agent' && stage.status === 'done');

/** resolveVisibleWorktreeSetup without a recorded activity: a clean finish leaves once the turn is live; a follow-up retires it. */
export function visibleWorktreeSetup(snapshot: Obj | null, turnStarted: boolean, followUpSent: boolean): Obj | null {
  if (!snapshot) return null;
  if (snapshot.phase === 'running') return snapshot;
  if (followUpSent) return null;
  if (snapshot.phase !== 'done' || !turnStarted) return snapshot;
  return arr(snapshot.stages).some(stage => stage.status === 'failed') ? snapshot : null;
}

/** The open thread's setup as the timeline places it, with ChatView's isPreparingWorktree. */
export function threadWorktreeSetup(client: T3Client): { snapshot: Obj | null; preparing: boolean } {
  const state = stateOf(client);
  const live = state.latest && str(state.latest.threadId) === client.threadId ? state.latest : null;
  const run = activeRun(client.projection);
  const users = arr(client.projection.visibleTurnItems).filter(row => obj(row.item).type === 'user_message').length;
  const snapshot = visibleWorktreeSetup(live, setupTurnStarted(client.projection), users > 1); // r12-threads: the activity run, a settled one included
  const preparing = str(run?.status) === 'preparing' || (live?.phase === 'running' && !agentStarted(live));
  return { snapshot, preparing };
}

export function headerLabel(snapshot: Obj): string {
  switch (snapshot.phase) {
    case 'running': return 'Setting up worktree…';
    case 'done': return arr(snapshot.stages).some(stage => stage.status === 'failed') ? 'Worktree ready, setup script failed' : 'Worktree ready';
    case 'failed': return 'Worktree setup failed';
    default: return 'Worktree setup cancelled';
  }
}
const ms = (value: unknown) => { const parsed = Date.parse(str(value)); return Number.isFinite(parsed) ? parsed : 0; };

export interface SetupStage { id: string; label: string; status: string; trailing: string; startedMs: number; endedMs: number; tail: { id: string; text: string }[] }
export interface SetupView {
  id: string; phase: string; header: string; tone: string; showHeader: boolean; collapsed: boolean; summary: string; startedMs: number; endedMs: number;
  error: string; stages: SetupStage[]; details: { id: string; text: string }[]; detailsOpen: boolean; terminal: boolean; cancel: boolean;
}
/** WorktreeSetupCard's presentation of one snapshot; `embedded` once the agent's turn is live. */
export function setupView(snapshot: Obj, embedded: boolean, detailsOpen: boolean): SetupView {
  const running = snapshot.phase === 'running', stages = arr(snapshot.stages), script = obj(snapshot.setupScript);
  const setupStage = stages.find(stage => stage.id === 'setup-script');
  // The tail mounts with the first output line (or after a failure), so an empty box never flashes.
  const showTail = !!setupStage && (setupStage.status === 'failed' || setupStage.status === 'running' && Array.isArray(setupStage.tail) && setupStage.tail.length > 0);
  const failedScript = stages.some(stage => stage.id === 'setup-script' && stage.status === 'failed');
  const summary = snapshot.phase === 'failed' || snapshot.phase === 'cancelled' || failedScript ? 'failed' : 'done';
  const finishedWithFailedStage = snapshot.phase === 'done' && stages.some(stage => stage.status === 'failed');
  return {
    id: 'worktree-setup', phase: str(snapshot.phase), header: headerLabel(snapshot),
    tone: snapshot.phase === 'failed' ? 'failed' : finishedWithFailedStage ? 'warning' : '',
    showHeader: !embedded && !running && snapshot.phase !== 'done', collapsed: embedded && !running, summary,
    startedMs: ms(snapshot.startedAt), endedMs: ms(snapshot.endedAt),
    error: snapshot.phase === 'failed' ? str(snapshot.error) : '',
    stages: stages.map(stage => {
      const status = str(stage.status), tail = (Array.isArray(stage.tail) ? stage.tail : []).map(line => str(line));
      const trailing = status === 'pending' ? '' : status === 'skipped' ? str(stage.detail, 'skipped')
        : stage.id === 'checkout' && status === 'running' && typeof stage.percent === 'number' ? `${stage.percent}%` : str(stage.detail);
      const slots = stage.id === 'setup-script' && showTail
        ? Array.from({ length: TAIL_LINES }, (_, slot) => ({ id: String(slot), text: tail[tail.length - TAIL_LINES + slot] ?? '' })) : [];
      return { id: str(stage.id), label: stage.id === 'setup-script' && str(script.name) ? str(script.name) : STAGE_LABELS[str(stage.id)] ?? str(stage.id),
        status, trailing, startedMs: ms(stage.startedAt), endedMs: ms(stage.endedAt), tail: slots };
    }),
    details: [['Branch', str(snapshot.branch)], ['Base', str(snapshot.baseRef)], ['Path', str(snapshot.worktreePath)], ['Setup', str(script.command)]]
      .filter(([, value]) => value).map(([name, value]) => ({ id: name!, text: value! })),
    detailsOpen, terminal: false, cancel: !embedded && running,
  };
}

/** The card's Details toggle and Cancel (worktreeSetup.cancel interrupts the server-side bootstrap). */
export async function worktreeSetupAction(client: T3Client, native: Native, op: string): Promise<string> {
  const state = stateOf(client);
  if (op === 'setup-details') { state.detailsOpen = !state.detailsOpen; return ''; }
  if (op === 'setup-cancel') {
    const snapshot = state.latest;
    if (!snapshot || snapshot.phase !== 'running' || str(snapshot.threadId) !== client.threadId) throw new ClientError('The worktree setup has already finished.');
    await client.restAccess(native).request('worktreeSetup.cancel', { threadId: client.threadId }, true);
    return '';
  }
  throw new ClientError(`Unknown worktree setup action: ${op}`);
}
export const worktreeDetailsOpen = (client: T3Client) => stateOf(client).detailsOpen;
/** Test seam: adopt a snapshot as if the stream delivered it. */
export function adoptWorktreeSetup(client: T3Client, snapshot: Obj | null): void { const state = stateOf(client); state.threadId = str(snapshot?.threadId); state.latest = snapshot; }
