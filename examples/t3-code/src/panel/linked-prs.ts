// The Linked pull requests surface (MIT reference, see LICENSE-T3:
// components/pullRequest/ThreadPullRequestsPanel.tsx, pullRequestListLines.ts,
// PullRequestListRow.tsx, packages/shared/src/threadPullRequests.ts, upstream
// 9c161462c9 "watch for changes"): the thread's visible links as stacked rows, the
// row menu (Copy link, Open, Watch for changes / Stop watching, Unlink) and the
// footer. Watch and unlink are orchestration commands (thread.pull-request.watch,
// thread.pull-request.unlink); nothing here writes to the code host.
import { quickRow, queueQuickAction } from '../pull-requests/pages-pr-quick';
import { insertContext } from '../composer/composer-editor';
import { providerOfUrl } from './pr-surface';
import { sharedPullRequestLinks } from '../pull-requests/pages-pr-shared';
import type { T3Client } from '../client/client';
import { arr, num, obj, str, type Obj } from '../client/domain';
import { ClientError, type Files, type Native } from '../client/protocol';
import { pushToast } from '../shell/toast';
import { visiblePullRequests } from './shell-pr';
import { relativeLabel } from '../pull-requests/pages-prs';
import { letGo } from '../client/let-go';

export type LinkedRow = {
  key: string; index: number; number: number; title: string; url: string; host: string; repository: string; depth: number;
  state: string; stateLabel: string; waiting: boolean; conflict: string; open: boolean; watching: boolean;
  checks: string; checksLabel: string; review: string; reviewLabel: string; additions: string; deletions: string;
  author: string; avatar: string; initial: string; head: string; base: string; age: string; stackSize: number; stackLabel: string;
  sourceLabel: string; unlinkLabel: string; canWatch: boolean; context: string;
  speed: ReturnType<typeof quickRow>['speed']; speedPending: boolean;
};
export type PrsView = { supported: boolean; rows: LinkedRow[]; footer: string; canWatch: boolean; speedMode: boolean };

const SOURCE_LABELS: Record<string, string> = { manual: 'Linked by you', created: 'Created from this thread', agent: 'Linked by the agent', stack: 'Found in the stack', 'stack-dismissed': 'Dismissed' };
const STATE_LABELS: Record<string, string> = { open: 'Open', draft: 'Draft', closed: 'Closed', merged: 'Merged' };
const CHECKS_LABELS: Record<string, string> = { passing: 'Checks passing', failing: 'Checks failing', pending: 'Checks running', none: 'No checks' };
const REVIEW_LABELS: Record<string, string> = { approved: 'Approved', changes_requested: 'Changes requested', 'changes-requested': 'Changes requested', review_required: 'Review required', 'review-required': 'Review required' };

const keyOf = (link: Obj) => `${str(link.host).toLowerCase()}/${str(link.repository).toLowerCase()}#${num(link.number)}`;
const snapshotOf = (link: Obj): Obj | null => (link.snapshot && typeof link.snapshot === 'object' ? obj(link.snapshot) : null);

/** resolveThreadPullRequestChains: native stacks by their layer order, then base→head chains, then the rest. */
export type PullRequestChain = { kind: string; layers: Obj[]; depths: number[] };
export function resolveChains(links: Obj[]): PullRequestChain[] {
  const visible = visiblePullRequests(links), chains: PullRequestChain[] = [], placed = new Set<string>();
  const stacks = new Map<string, Obj[]>();
  for (const link of visible) {
    const stack = link.stack && typeof link.stack === 'object' ? obj(link.stack) : null;
    if (!stack) continue;
    const id = `${str(link.host).toLowerCase()}/${str(link.repository).toLowerCase()}#stack:${str(stack.id)}`;
    stacks.set(id, [...(stacks.get(id) ?? []), link]);
  }
  for (const members of stacks.values()) {
    const order = new Map(arr(obj(members[0]!.stack).layers).map((layer, index) => [num(layer.number), index]));
    members.sort((a, b) => (order.get(num(a.number)) ?? 0) - (order.get(num(b.number)) ?? 0));
    for (const member of members) placed.add(keyOf(member));
    chains.push({ kind: 'native', layers: members, depths: members.map((_, index) => index) });
  }
  const remaining = visible.filter(link => !placed.has(keyOf(link)));
  const branchKey = (link: Obj, branch: string) => `${str(link.host).toLowerCase()}/${str(link.repository).toLowerCase()}:${branch}`;
  const byHead = new Map<string, Obj | null>();
  for (const link of remaining) {
    const snapshot = snapshotOf(link); if (!snapshot) continue;
    const key = branchKey(link, str(snapshot.headBranch));
    byHead.set(key, byHead.has(key) ? null : link);
  }
  const parentOf = (link: Obj): Obj | undefined => {
    const snapshot = snapshotOf(link);
    if (!snapshot) return undefined;
    const parent = byHead.get(branchKey(link, str(snapshot.baseBranch)));
    return parent == null || parent === link ? undefined : parent;
  };
  const children = new Map<string, Obj[]>();
  for (const link of remaining) {
    const parent = parentOf(link);
    if (!parent) continue;
    const siblings = children.get(keyOf(parent)) ?? [];
    siblings.push(link); children.set(keyOf(parent), siblings);
  }
  for (const base of remaining) {
    if (parentOf(base)) continue;
    const layers: Obj[] = [], depths: number[] = [];
    const pending = [{ link: base, depth: 0 }];
    while (pending.length > 0) {
      const { link, depth } = pending.pop()!;
      if (placed.has(keyOf(link))) continue;
      placed.add(keyOf(link)); layers.push(link); depths.push(depth);
      const siblings = children.get(keyOf(link)) ?? [];
      for (let index = siblings.length - 1; index >= 0; index--) pending.push({ link: siblings[index]!, depth: depth + 1 });
    }
    if (layers.length) chains.push({ kind: 'derived', layers, depths });
  }
  for (const link of remaining) if (!placed.has(keyOf(link))) chains.push({ kind: 'derived', layers: [link], depths: [0] });
  return chains;
}
export function layersFromTop(chain: PullRequestChain): Obj[] {
  return chain.layers.map((link, index) => ({ link, index, depth: chain.depths[index]! }))
    .sort((left, right) => right.depth - left.depth || right.index - left.index).map(entry => entry.link);
}
const activityAt = (link: Obj) => { const ms = Date.parse(str(snapshotOf(link)?.updatedAt, str(link.linkedAt))); return Number.isNaN(ms) ? 0 : ms; };
/** pullRequestListLines: newest chain first, each read bottom to top with its depth. */
export function listLines(chains: PullRequestChain[]): { link: Obj; depth: number; stack: { kind: string; size: number } | null }[] {
  return [...chains].sort((a, b) => Math.max(...b.layers.map(activityAt)) - Math.max(...a.layers.map(activityAt)))
    .flatMap(chain => chain.layers.map((link, index) => ({ link, depth: chain.depths[index]!, stack: index === 0 && chain.layers.length > 1 ? { kind: chain.kind, size: chain.layers.length } : null })));
}

function thread(client: T3Client): Obj | undefined { return client.shell.threads.find(entry => entry.id === client.threadId); }
const caps = (client: T3Client) => obj(obj(obj(client.config).environment).capabilities);

export function prsView(client: T3Client, now: number): PrsView {
  const supported = caps(client).threadPullRequests === true, canWatch = caps(client).threadPullRequestWatch === true;
  const links = sharedPullRequestLinks(client, arr(thread(client)?.pullRequests), str(thread(client)?.projectId));
  const lines = listLines(resolveChains(links));
  const rows = lines.map(({ link, depth, stack }, index): LinkedRow => {
    const snapshot = snapshotOf(link), state = !snapshot ? 'open' : snapshot.state === 'open' && snapshot.isDraft === true ? 'draft' : str(snapshot.state, 'open');
    const open = !snapshot || snapshot.state === 'open', author = obj(snapshot?.author);
    const additions = num(snapshot?.additions), deletions = num(snapshot?.deletions), measured = additions !== 0 || deletions !== 0;
    const login = str(author.login, 'ghost');
    return {
      key: keyOf(link), index, number: num(link.number), title: snapshot ? str(snapshot.title, str(link.repository)) : str(link.repository), url: str(link.url),
      host: str(link.host), repository: str(link.repository), depth: Math.min(depth, 3), state, stateLabel: STATE_LABELS[state] ?? 'Open', waiting: !snapshot,
      conflict: open && snapshot?.mergeability === 'conflicting' && snapshot.isDraft !== true ? `Conflicts with ${str(snapshot.baseBranch, 'the base branch')}` : '',
      open, watching: link.watch != null, checks: open ? str(snapshot?.checksState) : '', checksLabel: CHECKS_LABELS[str(snapshot?.checksState)] ?? '',
      review: open ? str(snapshot?.reviewDecision) : '', reviewLabel: REVIEW_LABELS[str(snapshot?.reviewDecision)] ?? '',
      additions: measured ? `+${additions.toLocaleString('en-US')}` : '', deletions: measured ? `-${deletions.toLocaleString('en-US')}` : '',
      author: snapshot?.author ? login : '', avatar: str(author.avatarUrl), initial: login.slice(0, 1).toUpperCase(), head: str(snapshot?.headBranch), base: str(snapshot?.baseBranch),
      age: snapshot ? relativeLabel(snapshot.updatedAt, now) : '', stackSize: stack?.size ?? 0,
      stackLabel: stack ? (stack.kind === 'native' ? `GitHub stack of ${stack.size}: merging a layer lands the ones below it.` : `${stack.size} pull requests chained by base branch.`) : '',
      ...((row) => ({ speed: row.speed, speedPending: row.speedPending }))(quickRow(client, { ...snapshot, projectId: str(thread(client)?.projectId), host: str(link.host), repository: str(link.repository), number: num(link.number), provider: providerOfUrl(str(link.url)), stack: link.stack })),
      context: JSON.stringify({ number: num(link.number), title: str(snapshot?.title), url: str(link.url), headBranch: str(snapshot?.headBranch), baseBranch: str(snapshot?.baseBranch), state: str(snapshot?.state, 'open'), isDraft: snapshot?.isDraft === true }),
      sourceLabel: `${SOURCE_LABELS[str(link.source)] ?? 'Linked'} · ${relativeLabel(link.linkedAt, now)}`,
      unlinkLabel: link.source === 'stack' ? 'Dismiss from thread' : 'Unlink from thread', canWatch: canWatch && open,
    };
  });
  const visible = visiblePullRequests(links);
  const openCount = visible.filter(link => !snapshotOf(link) || snapshotOf(link)!.state === 'open').length;
  let lastSynced = '';
  for (const link of visible) { const at = str(snapshotOf(link)?.syncedAt); if (at && at > lastSynced) lastSynced = at; }
  return { supported, rows, canWatch, speedMode: false, footer: `${openCount} open · ${visible.length} linked${lastSynced ? ` · synced ${relativeLabel(lastSynced, now)}` : ''}` };
}
export const emptyPrs = (): PrsView => ({ supported: false, rows: [], footer: '', canWatch: false, speedMode: false });

function linkOf(client: T3Client, key: string): Obj {
  const link = visiblePullRequests(thread(client)?.pullRequests).find(entry => keyOf(entry) === key);
  if (!link) throw new ClientError('That pull request is no longer linked to this thread.');
  return link;
}
/** `shelllocal:surface-pr-*`: the row menu's copy action. */
export async function prsLocal(client: T3Client, native: Native, op: string, id: string, _value: string): Promise<string> {
  if (op === 'insert') return insertContext(client, native, 'pull-request', _value);
  if (op === 'quick') return queueQuickAction(client, native, _value, ref => {
    const requested = obj(JSON.parse(ref));
    const link = visiblePullRequests(thread(client)?.pullRequests).find(entry => str(entry.host) === str(requested.host) && str(entry.repository) === str(requested.repository) && num(entry.number) === num(requested.number));
    return link ? { ...obj(link.snapshot), ...requested, provider: providerOfUrl(str(link.url)), stack: link.stack } : null;
  }, () => { client.revision++; });
  if (op === 'copy') {
    const link = linkOf(client, id);
    try { await client.restAccess(native).call({ op: 'copyText', text: str(link.url) }); }
    catch (error) { if (letGo(error)) throw error; pushToast(client, { kind: 'error', title: 'Failed to copy link', description: error instanceof Error ? error.message : 'An error occurred.' }); return ''; }
    pushToast(client, { kind: 'success', title: 'Link copied', description: str(link.url) });
    return '';
  }
  return '';
}
/** threadEnvironment.watchPullRequest / unlinkPullRequest: orchestration commands for this thread. */
export function prCommandPayload(op: string, threadId: string, link: Obj, commandId: string, watching: boolean): Obj {
  const key = { host: str(link.host), repository: str(link.repository), number: num(link.number) };
  return op === 'watch' ? { type: 'thread.pull-request.watch', commandId, threadId, ...key, watching }
    : { type: 'thread.pull-request.unlink', commandId, threadId, ...key };
}
export async function prsCommand(client: T3Client, native: Native, storage: Files, op: string, id: string, value: string): Promise<string> {
  if (op !== 'watch' && op !== 'unlink') throw new ClientError(`Unknown pull request action: ${op}`);
  if (op === 'watch' && caps(client).threadPullRequestWatch !== true) throw new ClientError('This environment cannot watch pull requests.');
  const link = linkOf(client, id), access = client.restAccess(native);
  const [commandId] = await access.ids(1);
  const watching = value === 'true';
  await access.dispatch(storage, prCommandPayload(op, client.threadId, link, commandId!, watching),
    op === 'unlink' ? `Unlink #${num(link.number)}` : `${watching ? 'Watch' : 'Stop watching'} #${num(link.number)}`);
  return '';
}
