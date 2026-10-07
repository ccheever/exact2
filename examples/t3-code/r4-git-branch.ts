// The workspace card's branch picker (lane r4-git), adapted from T3 Code (MIT; see
// LICENSE-T3): components/BranchToolbarBranchSelector.tsx in its panel display mode
// (the whole Version Control branch row is the combobox trigger; the popup hangs
// below it at the row's width), BranchPicker.tsx (search, refs with their badges,
// "Create new ref", the status line, the Start from origin switch) and
// BranchToolbar.logic.ts (resolveBranchTriggerLabel, shouldIncludeBranchPickerItem).
// The refs load with the card, as usePaginatedBranches does; selecting or creating a
// ref runs composer-controls-branch.ts's selectBranch, so the strip and the card
// check out the same way. Start from origin is the draft's (resolveNewDraftStartFromOrigin
// seeds it from the project's "Start new worktrees from origin" setting).
import type { T3Client } from './client';
import { arr, obj, str, num, type Obj } from './domain';
import { ClientError, type Files, type Native } from './protocol';
import { composerBranches, draftContext, refBadge, sanitizeNewRefName, selectBranch } from './composer-controls-branch';
import { fanoutSelections } from './r3-composer-controls-fanout';
import { REF_PAGE, firstPage, morePages, refsStatus, scrollEnds } from './r5-composer-paging';
import { checkoutItems, type CheckoutItem } from './r9-connect-checkout'; // lane r9-connect: the picker's checkout item
import { peekVcsStatus } from './shell-vcs';

type Refs = { cwd: string; query: string; refs: Obj[]; total: number; nextCursor: number | null; loaded: boolean; generation: number; stale: boolean; ends: number; loadingMore?: boolean };
interface BranchState { open: string; query: string; refs: Refs | null; origin: Map<string, boolean>; pendingBranch: string; picked: boolean }
const states = new WeakMap<T3Client, BranchState>();
export function branchState(client: T3Client): BranchState {
  let state = states.get(client);
  if (!state) { state = { open: '', query: '', refs: null, origin: new Map(), pendingBranch: '', picked: false }; states.set(client, state); }
  return state;
}

/** The project's (else the server's) "Start new worktrees from origin", on by default. */
export function originDefault(client: T3Client): boolean {
  const settings = obj(client.config.settings), override = obj(obj(settings.projectSettingsOverrides)[client.projectId]);
  const value = typeof override.newWorktreesStartFromOrigin === 'boolean' ? override.newWorktreesStartFromOrigin : settings.newWorktreesStartFromOrigin;
  return value !== false;
}
/** A draft creating a new worktree starts it from origin unless the switch turned that off. */
export function startFromOrigin(client: T3Client, key = client.draftKey): boolean {
  if (client.threadId) return false;
  const context = draftContext(client, key);
  if (context.envMode !== 'worktree' || context.worktreePath) return false;
  return branchState(client).origin.get(key) ?? originDefault(client);
}

/** A workspace mode change drops the switch's explicit choice. */
export function resetOrigin(client: T3Client, key: string): void { branchState(client).origin.delete(key); }

/** The repository's default ref from the loaded refs (the worktree base before one is chosen). */
export function defaultRefName(client: T3Client): string {
  const refs = branchState(client).refs;
  return refs && !refs.query ? str(refs.refs.find(ref => ref.isDefault === true)?.name) : '';
}

/** resolveBranchTriggerLabel's origin prefix: "From origin/main" for a local base ref. */
export function originLabel(client: T3Client, branch: string): string {
  if (!startFromOrigin(client) || !branch) return branch;
  const known = branchState(client).refs?.refs.find(ref => ref.name === branch);
  return known && known.isRemote !== true ? `origin/${branch}` : branch;
}

async function loadRefs(client: T3Client, native: Native, cwd: string, query: string): Promise<Refs> {
  const state = branchState(client), current = state.refs;
  const search = sanitizeNewRefName(query).slice(0, 256);
  // A shared read (activeBranchRefQuery is one query per input): the card asked again before the reply joins it.
  const list = (cursor?: number) => client.restAccess(native).read('vcs.listRefs', { cwd, limit: REF_PAGE, ...(search ? { query: search } : {}), ...(cursor === undefined ? {} : { cursor }) });
  // r5-composer: a scroll toward the list's end loads the next page (r5-composer-paging.ts).
  if (current && current.cwd === cwd && current.query === query && current.generation === client.generation && !current.stale)
    return Object.assign(current, await morePages(current, client.presentation, 'details-refs', list));
  const result = await list();
  const next: Refs = { cwd, query, ...firstPage(result, scrollEnds(client.presentation, 'details-refs')), loaded: true, generation: client.generation, stale: false };
  state.refs = next;
  return next;
}

/** shouldIncludeBranchPickerItem over a ref name. */
export function includeRef(name: string, query: string): boolean {
  const normalized = query.trim().toLowerCase();
  if (!normalized) return true;
  const lower = name.toLowerCase();
  if (lower.includes(normalized)) return true;
  const sanitized = sanitizeNewRefName(normalized);
  return sanitized.length > 0 && sanitized !== normalized && lower.includes(sanitized);
}

export type CardBranch = Awaited<ReturnType<typeof cardBranchView>>;
const hiddenView = { show: false, open: false, label: '', value: '', disabled: true, query: '', refs: [] as { name: string; badge: string; selected: boolean; index: number }[],
  creatable: '', status: '', empty: '', enterOp: '', enterValue: '', originShown: false, originOn: false, count: 0, picked: false, checkout: [] as CheckoutItem[] };
/** The branch row and its picker for the card's workspace (`cwd`). */
export async function cardBranchView(client: T3Client, native: Native, cwd: string, root: string, isRepo: boolean) {
  const state = branchState(client);
  if (!isRepo || !cwd) { state.open = ''; return hiddenView; }
  const strip = await composerBranches(client, native, false, '', true);
  let refs: Refs | null = null;
  try { refs = await loadRefs(client, native, cwd, state.query.trim()); } catch { refs = state.refs && state.refs.cwd === cwd ? state.refs : null; }
  const thread = client.threadId ? obj(client.projection.thread) : null;
  const context = draftContext(client);
  const worktreePath = thread ? str(thread.worktreePath) : context.worktreePath;
  // forceNewWorktree (a multi-model draft) and an unstarted New worktree draft pick a base ref instead of checking out.
  const forceWorktree = !client.threadId && !!fanoutSelections(client);
  const selectingBase = !client.threadId && (forceWorktree || (context.envMode === 'worktree' && !worktreePath));
  // resolveLiveThreadBranchUpdate: a draft on a checkout follows the checkout's branch, so a later New
  // worktree starts from it (the default ref is the base only for a draft that never had one).
  if (!client.threadId && !selectingBase && strip.branch && context.branch !== strip.branch) {
    const contexts = ((client.local as { composerControls: { contexts?: Record<string, Obj> } }).composerControls.contexts ??= {});
    contexts[client.draftKey] = { ...context, branch: strip.branch };
  }
  const value = state.pendingBranch || (selectingBase ? (context.branch || defaultRefName(client) || strip.branch) : strip.branch);
  const label = !value ? 'Select ref' : selectingBase ? `From ${originLabel(client, value)}` : value;
  const query = state.query.trim(), created = sanitizeNewRefName(query);
  const list = (refs?.refs ?? []).filter(ref => includeRef(str(ref.name), query));
  const creatable = !selectingBase && created && !list.some(ref => ref.name === created) ? created : '';
  const first = str(list[0]?.name);
  const loading = !refs;
  // r9-connect: a local draft's search that parses as a pull request reference leads with "Checkout pull request".
  const checkout = state.open === 'branch' ? checkoutItems(client, query, peekVcsStatus(client, cwd)) : [];
  return {
    show: true, open: state.open === 'branch', label, value, disabled: loading, query: state.query,
    refs: list.map((ref, index) => ({ name: str(ref.name), badge: refBadge(ref, root), selected: ref.name === value, index: index + checkout.length })),
    creatable, status: refsStatus(refs, loading),
    empty: !list.length && !creatable && !checkout.length ? (loading ? '' : 'No refs found.') : '',
    enterOp: checkout.length ? 'chatlocal:git-pr-open' : first ? 'shell:git-branch' : creatable ? 'shell:git-branch-create' : '', enterValue: checkout[0]?.reference ?? (first || creatable),
    originShown: selectingBase, originOn: selectingBase && startFromOrigin(client), count: list.length + (creatable ? 1 : 0) + checkout.length, picked: state.picked, checkout,
  };
}

/** `chatlocal:git-open|git-query|git-origin`: the picker's own state. */
export function branchLocal(client: T3Client, op: string, id: string, value: string): string {
  const state = branchState(client);
  // The row and the chevron open their popovers (the host toggles them); the picker starts a fresh search.
  if (op === 'open') { state.open = id; state.query = ''; state.picked = false; if (state.refs) state.refs.stale = true; return ''; }
  if (op === 'query') { state.query = value.slice(0, 256); return ''; }
  if (op === 'origin') {
    if (client.threadId) throw new ClientError('A started thread keeps its workspace.');
    state.origin.set(client.draftKey, !startFromOrigin(client));
    return '';
  }
  throw new ClientError(`Unknown branch action: ${op}`);
}

/** `shell:git-branch|git-branch-create`: selectBranch / createRef from the card. */
export async function branchCommand(client: T3Client, native: Native, storage: Files, op: string, name: string, keyboard = false): Promise<string> {
  const state = branchState(client);
  // A pointer pick hides the popover itself (popovertargetaction); Return cannot, so the picker empties.
  state.open = ''; state.query = ''; state.picked = keyboard;
  const known = state.refs?.refs.find(ref => ref.name === name);
  const context = draftContext(client);
  // Picking the base of a new worktree only records it on the draft.
  if (op === 'branch' && !client.threadId && (fanoutSelections(client) || (context.envMode === 'worktree' && !context.worktreePath))) {
    const contexts = ((client.local as { composerControls: { contexts?: Record<string, Obj> } }).composerControls.contexts ??= {});
    contexts[client.draftKey] = { envMode: 'worktree', branch: name, worktreePath: '' };
    return '';
  }
  state.pendingBranch = op === 'branch-create' ? sanitizeNewRefName(name) : known?.isRemote === true ? name.replace(/^[^/]+\//, '') : name;
  try { await selectBranch(client, native, storage, name, op === 'branch-create', known); }
  finally { state.pendingBranch = ''; if (state.refs) state.refs.stale = true; }
  return '';
}

/** r6-polish: the card picker's pages (r6-polish-refs.ts asks whether a scroll still wants its page). */
export function cardPages(client: T3Client): Refs | null { return states.get(client)?.refs ?? null; }
