// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/thread-reference.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// thread.copyReference (task thread-commands-and-keys, A14), adapted from
// T3 Code 1e2ecbd975 (MIT); see LICENSE-T3. Sources:
// packages/shared/src/threadReference.ts (resolveThreadReferenceCopyTarget,
// unchanged), packages/shared/src/threadPullRequests.ts
// (resolveThreadCurrentPullRequest / resolveThreadCurrentPullRequestLink, over
// r4-surfaces-prs.ts resolveChains) and ChatView.tsx copyActiveThreadReference
// (the success toast names the copied value; a failure is a stacked error toast).
import { num, obj, str, type Obj } from './domain';
import type { T3Client } from './client';
import type { Native } from './protocol';
import { pushToast } from './toast';
import { visiblePullRequests } from './shell-pr';
import { resolveChains } from './r4-surfaces-prs';
import { panelState } from './r4-surfaces-panel';
import { letGo } from './let-go';

export interface ThreadReferenceCopyTarget {
  readonly kind: 'pull-request' | 'thread';
  readonly value: string;
  readonly clipboardTarget: string;
  readonly successTitle: string;
  readonly failureTitle: string;
}

const isOpen = (link: Obj) => link.snapshot == null || obj(link.snapshot).state === 'open';
const latestUpdatedAt = (link: Obj) => { const ms = Date.parse(str(obj(link.snapshot).updatedAt) || str(link.linkedAt)); return Number.isNaN(ms) ? 0 : ms; };

/** resolveThreadCurrentPullRequestLink: open work first, the highest open layer of the newest chain, else the newest terminal link. */
export function resolveThreadCurrentPullRequestLink(links: unknown): Obj | null {
  const visible = visiblePullRequests(links);
  if (visible.length === 0) return null;
  const open = visible.filter(isOpen);
  if (open.length === 1) return open[0]!;
  const chains = resolveChains(visible);
  if (open.length > 1) {
    const ordered = chains.map(chain => [...chain.layers].reverse().filter(isOpen)).filter(layers => layers.length > 0)
      .sort((left, right) => Math.max(...right.map(link => Date.parse(str(link.linkedAt)))) - Math.max(...left.map(link => Date.parse(str(link.linkedAt))))).flat();
    return ordered[0]!;
  }
  if (chains.length === 1) return chains[0]!.layers[chains[0]!.layers.length - 1]!;
  return [...visible].sort((left, right) => latestUpdatedAt(right) - latestUpdatedAt(left))[0]!;
}

/** resolveThreadReferenceCopyTarget: the open panel's PR, the thread's current PR, the linked PR, else the thread ID; null while the panel URL loads. */
export function resolveThreadReferenceCopyTarget(input: { threadId: string; openPanelPullRequestUrl?: string | null; pullRequests?: unknown; linkedPullRequestUrl?: string | null }): ThreadReferenceCopyTarget | null {
  if (input.openPanelPullRequestUrl === null) return null;
  const current = resolveThreadCurrentPullRequestLink(input.pullRequests ?? []);
  const pullRequestUrl = input.openPanelPullRequestUrl ?? (current ? str(current.url) : undefined) ?? input.linkedPullRequestUrl;
  return pullRequestUrl
    ? { kind: 'pull-request', value: pullRequestUrl, clipboardTarget: 'pull request link', successTitle: 'PR link copied', failureTitle: 'Failed to copy PR link' }
    : { kind: 'thread', value: input.threadId, clipboardTarget: 'thread ID', successTitle: 'Thread ID copied', failureTitle: 'Failed to copy thread ID' };
}

/** useOpenPanelPullRequestUrl: the active right-panel surface's pull request URL; null while it has none yet, undefined without a PR surface. */
export function openPanelPullRequestUrl(client: T3Client): string | null | undefined {
  const state = panelState(client);
  const surface = state.visible ? state.surfaces.find(entry => entry.id === state.active) : undefined;
  if (surface?.kind !== 'pull-request') return undefined;
  return str(surface.pr?.url) || null;
}

/** The open thread's target: its PR panel (when one is the active surface), its links, its legacy linked PR. */
export function threadReferenceTarget(client: T3Client, panelUrl?: string | null): ThreadReferenceCopyTarget | null {
  const thread = client.threadId ? client.shell.threads.find(entry => entry.id === client.threadId) : undefined;
  if (!thread) return null;
  const linked = thread.linkedPullRequest && typeof thread.linkedPullRequest === 'object' ? obj(thread.linkedPullRequest) : null;
  return resolveThreadReferenceCopyTarget({ threadId: str(thread.id), openPanelPullRequestUrl: panelUrl, pullRequests: thread.pullRequests,
    linkedPullRequestUrl: linked && num(linked.number) > 0 ? str(linked.url) || null : null });
}

/** copyActiveThreadReference: copy, then "PR link copied" / "Thread ID copied" with the value, or the failure toast. */
export async function copyThreadReference(client: T3Client, native: Native, panelUrl: string | null | undefined = openPanelPullRequestUrl(client)): Promise<string> {
  const target = threadReferenceTarget(client, panelUrl);
  if (!target) return '';
  try {
    const reply = await client.restAccess(native).call({ op: 'copyText', text: target.value });
    if (reply.copied === false) throw new Error(`Could not copy the ${target.clipboardTarget}.`);
    pushToast(client, { kind: 'success', title: target.successTitle, description: target.value });
  } catch (error) {
    if (letGo(error)) throw error;
    pushToast(client, { kind: 'error', stacked: true, title: target.failureTitle, description: error instanceof Error && error.message ? error.message : 'An error occurred.' });
  }
  return '';
}
