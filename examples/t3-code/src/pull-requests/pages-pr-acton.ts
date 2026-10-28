// "Act on": on the Pull Requests page, when more than one connected server holds
// the pull request's repository, the More and Check out menus offer which server a hand-off acts on; the chosen
// server supplies the project, the checkout root and the thread (PullRequestDetailPanel.tsx pickableEnvironments,
// actingScope, ActOnEnvironmentPicker, startAsk/startHandoff with `actingEnvironmentId`; T3 Code MIT, see LICENSE-T3).
// A hand-off on a server other than the focused one runs its checkout there (its own transport), leaves the task in
// that server's draft for the project, and moves the window to it, as the Run on menu moves a draft (git-env.ts).
import type { T3Client } from '../client/client';
import { obj, str, type Obj } from '../client/domain';
import { ClientError, bridgeReply, type Native } from '../client/protocol';
import { pushToast } from '../shell/toast';
import { letGo } from '../client/let-go';
import { fleet } from '../connections/settings-b-fleet';
import { environmentRequest, prEnvironments, readsPullRequests } from './pages-pr-environments';
import { resolvePickableEnvironments, type PickableEnvironment } from './pages-pr-routing';
import { askedDescription, prState, writeTaskToDraft, type HandoffTask } from '../details/pr-row-actions';
import { checkoutFailureDescription, offHeadCheckoutToast, preparePayload } from '../details/pr-row-logic';
import { terminologyOf } from '../details/git-logic';
import { ensureDraftThreadId, withHandoffThread } from '../composer/draft-thread-id';

/** The reader's choice, for the pull request it was made on only (the panel shows another one every time it opens). */
const choices = new WeakMap<object, { key: string; environmentId: string }>();

export type ActOnItem = { key: string; label: string; machine: string; selected: boolean };
export const emptyActOn = (): { items: ActOnItem[] } => ({ items: [] });

/** The servers a pull request on the page could be acted on (resolvePickableEnvironments), the panel's own first. */
export function pickableFor(client: T3Client, environmentId: string, projectId: string): ReadonlyArray<PickableEnvironment> {
  const environments = prEnvironments(client).filter(environment => environment.connected && readsPullRequests(environment));
  const projects = environments.flatMap(environment => environment.projects.map(project => ({ id: str(project.id), environmentId: environment.id,
    repositoryIdentity: obj(project.repositoryIdentity) as { canonicalKey?: string }, workspaceRoot: str(project.workspaceRoot) })));
  return resolvePickableEnvironments({ environmentId: environmentId || client.environmentId, projectId }, projects,
    environments.map(environment => ({ environmentId: environment.id, label: environment.label, machine: environment.machine })));
}
/** The server a hand-off acts on: the chosen one while it is still offered, else none (the panel's own). */
export function actingOn(client: T3Client, key: string, pickable: ReadonlyArray<PickableEnvironment>): PickableEnvironment | null {
  const chosen = choices.get(client);
  return chosen?.key === key ? pickable.find(entry => entry.environmentId === chosen.environmentId) ?? null : null;
}
/**
 * Where a page hand-off acts: the chosen server while it is offered; else, for a row a background server listed, that
 * server and its own copy; null for the focused server's own (the hand-off runs as it always has).
 */
export function handoffServer(client: T3Client, key: string, environmentId: string, projectId: string): PickableEnvironment | null {
  const pickable = pickableFor(client, environmentId, projectId), chosen = actingOn(client, key, pickable);
  if (chosen) return chosen.environmentId === client.environmentId && !environmentId ? null : chosen;
  if (!environmentId || environmentId === client.environmentId) return null;
  const environment = prEnvironments(client).find(candidate => candidate.id === environmentId);
  const project = environment?.projects.find(candidate => str(candidate.id) === projectId);
  return environment && project ? { environmentId, projectId, workspaceRoot: str(project.workspaceRoot), label: environment.label, machine: environment.machine } : null;
}
/** The menus' radio group: nothing where there is no choice (one server, or beside a thread). */
export function presentActOn(client: T3Client, key: string, environmentId: string, projectId: string, page: boolean): { items: ActOnItem[] } {
  if (!page) return emptyActOn();
  const pickable = pickableFor(client, environmentId, projectId);
  if (pickable.length === 0) return emptyActOn();
  const acting = actingOn(client, key, pickable)?.environmentId ?? (environmentId || client.environmentId);
  return { items: pickable.map(entry => ({ key: entry.environmentId, label: entry.label, machine: entry.machine ?? 'server', selected: entry.environmentId === acting })) };
}
/** `chatlocal:pr-ui-act-on`: the radio's choice. */
export function chooseActOn(client: object, key: string, environmentId: string): void { choices.set(client, { key, environmentId }); }

/**
 * A hand-off on another server (startAsk / startHandoff with the acting environment): a question goes into that server's
 * draft for the project; a checkout runs there first (`git.preparePullRequestThread` with the draft's thread id), the draft
 * pointed at it; then the window moves to that server, whose draft it shows (`sidebar:new-thread`).
 */
export async function actOnHandoff(client: T3Client, native: Native, acting: PickableEnvironment, detail: Obj, kind: string, task: HandoffTask | null, mode: 'worktree' | 'local'): Promise<string> {
  const state = prState(client);
  if (state.handoff) return '';
  state.handoff = kind;
  const draftKey = `${acting.environmentId}:new:${acting.projectId}`;
  // The panel's host words (PullRequestDetailPanel terminology).
  const term = terminologyOf(str(detail.provider) || undefined);
  try {
    if (kind === 'ask' || kind === 'explain') {
      if (task) writeTaskToDraft(client, draftKey, task);
      await moveTo(client, native, acting);
      pushToast(client, { kind: 'success', title: 'Asked in a thread', description: askedDescription(task ?? { prompt: '' }, term) });
      return 'sidebar:new-thread';
    }
    if (!acting.workspaceRoot) return '';
    const loading = pushToast(client, { kind: 'loading', title: `Preparing the ${term.singular} checkout...`, key: 'pr-handoff' });
    let prepared: Obj;
    try {
      const threadId = await ensureDraftThreadId(client, native, draftKey);
      prepared = await environmentRequest(client, native, acting.environmentId, 'git.preparePullRequestThread',
        withHandoffThread(preparePayload({ ...detail, workspaceRoot: acting.workspaceRoot }, mode), threadId), { write: true });
    } catch (error) {
      if (letGo(error)) throw error;
      pushToast(client, { kind: 'error', title: `Could not prepare the ${term.singular} checkout`, description: checkoutFailureDescription(error), key: 'pr-handoff' });
      return '';
    }
    void loading;
    const branch = str(prepared.branch), worktreePath = str(prepared.worktreePath);
    (client.local.composerControls.contexts ??= {})[draftKey] = { envMode: worktreePath ? 'worktree' : 'local', branch, worktreePath };
    if (task) writeTaskToDraft(client, draftKey, task);
    await moveTo(client, native, acting);
    pushToast(client, prepared.isOnPullRequestHead === false ? { ...offHeadCheckoutToast(term), key: 'pr-handoff' }
      : task ? { kind: 'success', title: 'Checkout ready', description: 'The task is in the composer. Read it over, then send.', key: 'pr-handoff' }
      : { kind: 'success', title: mode === 'local' ? 'Checked out here' : 'Checked out', key: 'pr-handoff',
        description: mode === 'local' ? `This repository is on the ${term.singular}'s branch, with a thread open on it.` : `The ${term.singular} is in its own worktree, with a thread open on it.` });
    return 'sidebar:new-thread';
  } finally { state.handoff = ''; }
}

/** The window moves to the acting server, on its draft for the project (runOnEnvironment's focus change). */
async function moveTo(client: T3Client, native: Native, acting: PickableEnvironment): Promise<void> {
  if (acting.environmentId === client.environmentId) { await client.openProjectDraft(native, acting.projectId); return; }
  const entry = [...fleet.entries.values()].find(candidate => candidate.environmentId === acting.environmentId);
  if (!entry) throw new ClientError('That environment is no longer connected.');
  client.local.selections[acting.environmentId] = { projectId: acting.projectId, threadId: '' };
  fleet.forget(entry.key);
  await native.later({ op: 'fleetStop', fleet: entry.key }).catch(() => {});
  const reply = await bridgeReply(native, { op: 'connect', origin: entry.origin, ...(entry.primary ? { primary: true } : { credential: '' }) });
  if (!reply.ok) throw new ClientError(reply.error!.message);
  client.adoptStatus(obj(reply.value), reply.generation);
}
