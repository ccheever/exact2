// Approvals a subagent or side chat below the open thread waits on, answered from above the parent's composer:
// chat/ChildThreadApprovals.tsx and client-runtime state/threadRequests.ts (createChildThreadApprovalsAtom) at T3 Code
// 5d3d78ab93 (MIT; see LICENSE-T3). Only the children asking for approval are read, each once per change of its
// shell row; a card per pending approval names the child and answers its own request.
import { arr, str, type Obj } from '../client/domain';
import { ClientError, type Files, type Native } from '../client/protocol';
import type { T3Client } from '../client/client';
import { childThreadInputs } from '../sidebar/subagent-requests';
import { fullAccessOverrideExplanation, pendingRequests, type PendingApproval } from './requests';

type Child = { signature: string; title: string; approvals: PendingApproval[] };
type State = { scope: string; children: Map<string, Child>; responding: string; errors: Map<string, string> };
const states = new WeakMap<T3Client, State>();
const state = (client: T3Client): State => {
  let found = states.get(client);
  if (!found) states.set(client, found = { scope: '', children: new Map(), responding: '', errors: new Map() });
  return found;
};
const cardKey = (threadId: string, requestId: string) => `${threadId}\t${requestId}`;

/** What the parent's shell says about a child: a change reads its requests again. */
function signature(thread: Obj | undefined): string {
  return JSON.stringify([thread?.updatedAt, thread?.latestRunId, thread?.pendingRuntimeRequest, thread?.hasPendingApprovals, thread?.pendingSubagentRequest]);
}

/** Before a snapshot: reads the pending requests of each child that asks for approval and changed since its last read. */
export async function childApprovalsPrepare(client: T3Client, native: Native | null | undefined): Promise<void> {
  const own = state(client), scope = `${client.environmentId}:${client.generation}:${client.threadId}`;
  if (own.scope !== scope) { own.children.clear(); own.errors.clear(); own.responding = ''; own.scope = scope; }
  if (!native?.available || !client.ready || client.connection !== 'connected' || !client.threadId) { own.children.clear(); return; }
  const asking = childThreadInputs(client.shell.threads, client.threadId).filter(child => child.approval);
  for (const id of [...own.children.keys()]) if (!asking.some(child => child.id === id)) own.children.delete(id);
  for (const child of asking) {
    const sig = signature(client.shell.threads.find(thread => thread.id === child.id));
    const known = own.children.get(child.id);
    if (known?.signature === sig) { known.title = child.title; continue; }
    let approvals: PendingApproval[] = known?.approvals ?? [];
    try { approvals = pendingRequests(await client.restAccess(native).request('orchestration.getThreadProjection', { threadId: child.id })).approvals; }
    catch { continue; } // Keep the last cards and retry on the next snapshot.
    if (own.scope !== scope || `${client.environmentId}:${client.generation}:${client.threadId}` !== scope) return;
    own.children.set(child.id, { signature: sig, title: child.title, approvals });
  }
}

export type ChildApprovalCard = { key: string; threadId: string; requestId: string; title: string; detail: string; appName: string; reason: string;
  whyAsked: string; live: boolean; operable: boolean; responding: boolean; error: string;
  options: Array<{ id: string; label: string; warning: string; primary: boolean }>; more: Array<{ id: string; label: string; warning: string; primary: boolean }> };

/** The cards, in the order the children are listed. */
export function childApprovalCards(client: T3Client): ChildApprovalCard[] {
  const own = states.get(client);
  if (!own?.children.size || !client.threadId) return [];
  const operable = client.scopes.includes('orchestration:operate');
  return childThreadInputs(client.shell.threads, client.threadId).filter(child => child.approval).flatMap(child =>
    (own.children.get(child.id)?.approvals ?? []).map(approval => {
      const choices = approval.options.map(option => ({ id: str(option.decision), label: str(option.label), warning: str(option.warning),
        primary: option.decision === 'accept' || option.decision === 'decline' }));
      const key = cardKey(child.id, approval.requestId);
      return { key, threadId: child.id, requestId: approval.requestId, title: own.children.get(child.id)!.title, detail: approval.detail || approval.kind,
        appName: approval.appName, reason: approval.reason, whyAsked: approval.fullAccessOverride ? fullAccessOverrideExplanation(approval.fullAccessOverride) : '',
        live: approval.live, operable, responding: own.responding === key, error: own.errors.get(key) ?? '',
        options: choices.filter(choice => choice.primary), more: choices.filter(choice => !choice.primary) };
    }));
}

/** `cc:child-approval`: answers a child's request with one of its decisions; a failure shows under its card, not as a toast. */
export async function respondToChildApproval(client: T3Client, native: Native, storage: Files, key: string, decision: string): Promise<void> {
  const own = state(client), [threadId = '', requestId = ''] = key.split('\t');
  const approval = own.children.get(threadId)?.approvals.find(entry => entry.requestId === requestId);
  if (!approval || own.responding) return;
  if (!client.scopes.includes('orchestration:operate')) throw new ClientError('This session cannot answer approvals.');
  if (!approval.live || !arr(approval.options).some(option => option.decision === decision)) return;
  own.responding = key; own.errors.delete(key);
  try {
    const access = client.restAccess(native), [commandId] = await access.ids(1);
    await access.dispatch(storage, { type: 'runtime-request.respond', commandId, threadId, requestId, decision }, 'Approval');
    // The child's next read finds its request answered.
    own.children.get(threadId)!.signature = '';
  } catch (error) {
    own.errors.set(key, error instanceof Error ? error.message : 'Failed to submit approval decision.');
  } finally { own.responding = ''; }
}
