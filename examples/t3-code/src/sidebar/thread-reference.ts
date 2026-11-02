// thread.copyReference, adapted from
// T3 Code 6d450a4284 (MIT); see LICENSE-T3. Sources:
// packages/client-runtime/src/threadReference.ts (threadChipCopyTarget,
// resolveThreadReferenceCopyTarget), packages/shared/src/threadPullRequests.ts
// (resolveThreadCurrentPullRequest / resolveThreadCurrentPullRequestLink, over
// linked-prs.ts resolveChains), apps/web/src/lib/copyThreadReference.ts and ChatView.tsx copyActiveThreadReference
// (the success toast names what was copied; a failure is a stacked error toast).
// A thread is copied as its chip: the reference link (composerContextReferences.ts formatComposerContextReference),
// which pastes back into a composer as the chip. Exact's clipboard carries text only, so the reference's
// composer-context fragment beside it is not written.
import { num, obj, str, type Obj } from '../client/domain';
import type { T3Client } from '../client/client';
import type { Native } from '../client/protocol';
import { pushToast } from '../shell/toast';
import { visiblePullRequests } from '../panel/shell-pr';
import { layersFromTop, resolveChains } from '../panel/linked-prs';
import { panelState } from '../panel/panel-surfaces';
import { letGo } from '../client/let-go';

export interface ThreadReferenceCopyTarget {
  readonly kind: 'pull-request' | 'thread';
  /** The plain text: a PR URL, or a thread chip's reference link. */
  readonly value: string;
  /** What a toast or palette row shows for the copy. */
  readonly description: string;
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
    const ordered = chains.map(chain => layersFromTop(chain).filter(isOpen)).filter(layers => layers.length > 0)
      .sort((left, right) => Math.max(...right.map(link => Date.parse(str(link.linkedAt)))) - Math.max(...left.map(link => Date.parse(str(link.linkedAt))))).flat();
    return ordered[0]!;
  }
  if (chains.length === 1) return layersFromTop(chains[0]!)[0]!;
  return [...visible].sort((left, right) => latestUpdatedAt(right) - latestUpdatedAt(left))[0]!;
}

/**
 * threadChipCopyTarget: a thread chip, which pastes into a composer as that thread's reference. The link is
 * composer-editor-menu.ts contextLink('thread', contextId('thread', id), title), written here so that this module,
 * which the sidebar's worktree cleanup reaches, does not load the composer: a thread id is a context id as it stands.
 */
export function threadChipCopyTarget(threadId: string, title: string): ThreadReferenceCopyTarget {
  const label = title.replace(/[[\]\\\r\n]/g, ' ').replace(/\s+/g, ' ').trim().slice(0, 200) || 'thread';
  return { kind: 'thread', value: `[${label}](t3-context://v1/thread/thread_${threadId})`, description: label, clipboardTarget: 'thread',
    successTitle: 'Thread copied', failureTitle: 'Failed to copy thread' };
}

/** resolveThreadReferenceCopyTarget: the open panel's PR, the thread's current PR, the linked PR, else the thread's chip; null while the panel URL loads. */
export function resolveThreadReferenceCopyTarget(input: { threadId: string; title: string; openPanelPullRequestUrl?: string | null; pullRequests?: unknown; linkedPullRequestUrl?: string | null }): ThreadReferenceCopyTarget | null {
  if (input.openPanelPullRequestUrl === null) return null;
  const current = resolveThreadCurrentPullRequestLink(input.pullRequests ?? []);
  const pullRequestUrl = input.openPanelPullRequestUrl ?? (current ? str(current.url) : undefined) ?? input.linkedPullRequestUrl;
  return pullRequestUrl
    ? { kind: 'pull-request', value: pullRequestUrl, description: pullRequestUrl, clipboardTarget: 'pull request link', successTitle: 'PR link copied', failureTitle: 'Failed to copy PR link' }
    : threadChipCopyTarget(input.threadId, input.title);
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
  return resolveThreadReferenceCopyTarget({ threadId: str(thread.id), title: str(thread.title), openPanelPullRequestUrl: panelUrl, pullRequests: thread.pullRequests,
    linkedPullRequestUrl: linked && num(linked.number) > 0 ? str(linked.url) || null : null });
}

/** copyActiveThreadReference: the open thread's PR link or chip (copyThreadTarget). */
export async function copyThreadReference(client: T3Client, native: Native, panelUrl: string | null | undefined = openPanelPullRequestUrl(client)): Promise<string> {
  const target = threadReferenceTarget(client, panelUrl);
  if (target) await copyThreadTarget(client, native, target);
  return '';
}

/** copyThreadReference.ts: copy, then "PR link copied" with the URL or "Thread copied" with the title, or the failure toast. */
export async function copyThreadTarget(client: T3Client, native: Native, target: ThreadReferenceCopyTarget): Promise<void> {
  try {
    const reply = await client.restAccess(native).call({ op: 'copyText', text: target.value });
    if (reply.copied === false) throw new Error(`Could not copy the ${target.clipboardTarget}.`);
    pushToast(client, { kind: 'success', title: target.successTitle, description: target.description });
  } catch (error) {
    if (letGo(error)) throw error;
    pushToast(client, { kind: 'error', stacked: true, title: target.failureTitle, description: error instanceof Error && error.message ? error.message : 'An error occurred.' });
  }
}
