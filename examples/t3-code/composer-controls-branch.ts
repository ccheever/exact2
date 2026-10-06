// The workspace / branch context strip under the draft composer (lane
// composer-controls), adapted from T3 Code (MIT); see LICENSE-T3. Sources:
// components/BranchToolbar.tsx, BranchToolbarEnvModeSelector.tsx,
// BranchToolbarBranchSelector.tsx, BranchPicker.tsx, BranchToolbar.logic.ts
// (shouldShowComposerContextStrip, resolveBranchTriggerLabel) and
// packages/client-runtime/src/operations/commands.ts (launch workspaceStrategy).
// The server owns the repository: status, refs and checkouts come from vcs.*.
import { arr, obj, str, num, type Obj } from './domain';
import { ClientError, type Files, type Native } from './protocol';
import { pushToast } from './toast';
import type { T3Client } from './client';
import { fanoutSelections } from './r3-composer-controls-fanout';
import { defaultRefName, originLabel, resetOrigin, startFromOrigin } from './r4-git-branch'; // lane r4-git: Start from origin, the default base ref
import { REF_PAGE, firstPage, morePages, refsStatus, scrollEnds } from './r5-composer-paging';
import { NO_STRIP_PR, stripPr } from './r7-handoff-strip'; // lane r7-handoff: the strip's pull request badge
import { checkoutItems, type CheckoutItem } from './r9-connect-checkout'; // lane r9-connect: the picker's checkout item
import { NO_RUN_ON, stripRunOn } from './r4-git-env'; // lane r4-git: MobileRunContextSelector's "Run on"
import { gitlessStrip } from './r12-threads-strip'; // lane r12-threads: the non-Git strip (BranchToolbar.logic.ts)

export type DraftContext = { envMode: string; branch: string; worktreePath: string };
type Repo = { isRepo: boolean; refName: string; status?: Obj; checked: boolean; refs: Obj[]; total: number; refsQuery: string | null; error: string; nextCursor?: number | null; ends?: number; loadingMore?: boolean };
const repos = new WeakMap<T3Client, Map<string, Repo>>();
function repo(client: T3Client, cwd: string): Repo {
  let byCwd = repos.get(client);
  if (!byCwd) { byCwd = new Map(); repos.set(client, byCwd); }
  let entry = byCwd.get(cwd);
  if (!entry) { entry = { isRepo: false, refName: '', checked: false, refs: [], total: 0, refsQuery: null, error: '' }; byCwd.set(cwd, entry); }
  return entry;
}
const contexts = (client: T3Client): Record<string, DraftContext> => client.local.composerControls.contexts ??= {};
/** The draft's workspace mode (Settings › New threads default), chosen branch and an existing worktree it points at. */
export function draftContext(client: T3Client, key = client.draftKey): DraftContext {
  const saved = contexts(client)[key];
  const fallback = str(obj(client.config.settings).defaultThreadEnvMode) === 'worktree' ? 'worktree' : 'local';
  return { envMode: saved?.envMode === 'worktree' || saved?.envMode === 'local' ? saved.envMode : fallback, branch: saved?.branch ?? '', worktreePath: saved?.worktreePath ?? '' };
}
const projectRoot = (client: T3Client) => str(client.shell.projects.find(entry => entry.id === client.projectId)?.workspaceRoot);
/** The thread's (or draft's) worktree, else null: the strip's activeWorktreePath. */
function activeWorktree(client: T3Client): string {
  return client.threadId ? str(obj(client.projection.thread).worktreePath) : draftContext(client).worktreePath;
}
function cwdFor(client: T3Client): string { return activeWorktree(client) || projectRoot(client); }
/** resolveEffectiveEnvMode: a draft pointed at an existing worktree is local to it; a thread with a worktree is a worktree. */
function effectiveEnvMode(client: T3Client): string {
  const path = activeWorktree(client);
  if (client.threadId) return path ? 'worktree' : 'local';
  return path ? 'local' : draftContext(client).envMode;
}
/** resolvePreviousWorktreeSeed: the project's most recently updated worktree the composer is not already in. */
export function previousWorktree(client: T3Client): { branch: string; worktreePath: string } | null {
  if (client.threadId) return null;
  const current = activeWorktree(client);
  let latest: { branch: string; worktreePath: string; at: number } | null = null;
  for (const thread of client.shell.threads) {
    const path = str(thread.worktreePath), at = Date.parse(str(thread.updatedAt));
    if (thread.projectId !== client.projectId || !path || path === current || thread.archivedAt != null || !Number.isFinite(at)) continue;
    if (!latest || at > latest.at) latest = { branch: str(thread.branch), worktreePath: path, at };
  }
  return latest && { branch: latest.branch, worktreePath: latest.worktreePath };
}

/** Refreshes repository state the strip shows; the menu's refs load while the branch picker is open. */
async function load(client: T3Client, native: Native, cwd: string, open: boolean, query: string, card = false): Promise<Repo> {
  const entry = repo(client, cwd), access = client.restAccess(native);
  if (!entry.checked) {
    try {
      const status = await access.request('vcs.refreshStatus', { cwd });
      entry.isRepo = status.isRepo === true; entry.refName = str(status.refName); entry.status = status; entry.error = ''; // r7-handoff: its pr feeds the strip's badge
    } catch (error) { entry.isRepo = false; entry.error = error instanceof Error ? error.message : 'Could not read the repository.'; }
    entry.checked = true;
  }
  const search = sanitizeNewRefName(query).slice(0, 256);
  const list = (cursor?: number) => access.request('vcs.listRefs', { cwd, limit: REF_PAGE, ...(search ? { query: search } : {}), ...(cursor === undefined ? {} : { cursor }) });
  if (open && entry.isRepo && entry.refsQuery !== query) {
    Object.assign(entry, firstPage(await list(), scrollEnds(client.presentation, 'strip-refs'))); entry.refsQuery = query;
  } else if (open && entry.isRepo) {
    // r5-composer: a scroll toward the list's end loads the next page (r5-composer-paging.ts).
    Object.assign(entry, await morePages({ refs: entry.refs, total: entry.total, nextCursor: entry.nextCursor ?? null, ends: entry.ends ?? 0, loadingMore: entry.loadingMore }, client.presentation, 'strip-refs', list));
  }
  // r5-composer: the workspace card's read (card) leaves the strip picker's open pages alone.
  if (!open && !card) entry.refsQuery = null;
  return entry;
}

/** sanitizeNewRefName: git rejects ASCII whitespace in ref names, so runs of it become a dash. */
export function sanitizeNewRefName(raw: string): string { return raw.trim().replace(/[ \t\n\r\f\v]+/g, '-'); }

/** BranchPickerRefItem's badge: current, worktree, remote or default. */
export function refBadge(ref: Obj, cwd: string): string {
  if (ref.current === true) return 'current';
  if (str(ref.worktreePath) && str(ref.worktreePath) !== cwd) return 'worktree';
  if (ref.isRemote === true) return 'remote';
  return ref.isDefault === true ? 'default' : '';
}

/** The resource behind the strip (shouldShowComposerContextStrip with Git controls). */
export async function composerBranches(client: T3Client, native: Native | null | undefined, open: boolean, query: string, card = false) {
  const hidden = { show: false, envMode: 'local', forceWorktree: false, envLabel: 'Current checkout', envIcon: 'folder', envLocked: false, previous: false, previousBranch: '',
    branchLabel: '', branch: '', loading: false, refs: [] as Array<{ name: string; badge: string; selected: boolean; index: number }>, status: '', creatable: '', enterOp: '', enterValue: '', checkout: [] as CheckoutItem[], ...NO_RUN_ON, ...NO_STRIP_PR, gitless: false };
  if (!native?.available || client.connection !== 'connected' || !client.ready || !client.projectId) return hidden;
  const persist = (client.local as { clientSettings?: Obj }).clientSettings?.persistComposerContextStrip === true;
  if (client.threadId && !persist && !card) return hidden; // the workspace card's branch row (r4-git-branch.ts) shows on threads too
  const cwd = cwdFor(client);
  if (!cwd) return hidden;
  let entry: Repo;
  try { entry = await load(client, native, cwd, open, query, card); } catch { return hidden; }
  // shouldShowComposerContextStrip without Git controls: only the machine selector, when it shows (lanes r12/r13-threads).
  if (!entry.isRepo) return card ? hidden : gitlessStrip(client, hidden, persist);
  const thread = obj(client.projection.thread), context = draftContext(client);
  // forceNewWorktree: a multi-model draft starts each model in its own new worktree.
  const forceWorktree = !client.threadId && !!fanoutSelections(client);
  const worktreePath = forceWorktree ? '' : activeWorktree(client), envMode = forceWorktree ? 'worktree' : effectiveEnvMode(client);
  // A started thread (messages or a runtime) keeps its workspace: the locked row.
  const locked = !!client.threadId;
  const threadBranch = client.threadId ? str(thread.branch) : context.branch;
  // resolveBranchToolbarValue: a new worktree starts from the chosen base, else the checkout's current branch.
  const branch = envMode === 'worktree' && !worktreePath ? threadBranch || defaultRefName(client) || entry.refName : entry.refName || threadBranch;
  const created = sanitizeNewRefName(query);
  const creatable = created && !entry.refs.some(ref => ref.name === created) && !(envMode === 'worktree' && !worktreePath) ? created : '';
  const first = str(entry.refs[0]?.name);
  const checkout = open ? checkoutItems(client, query, entry.status ?? null) : []; // r9-connect: unshifted before the refs
  const previous = locked ? null : previousWorktree(client);
  return { show: true, envMode, forceWorktree,
    envLabel: locked ? (worktreePath ? 'Worktree' : envMode === 'worktree' ? 'New worktree' : 'Local checkout')
      : envMode === 'worktree' ? 'New worktree' : worktreePath ? 'Current worktree' : 'Current checkout',
    envIcon: envMode === 'worktree' ? 'folder-git' : worktreePath ? 'folder-git-1' : 'folder',
    envLocked: locked, previous: !!previous, previousBranch: previous?.branch ?? '', branch,
    branchLabel: !branch ? 'Select ref' : envMode === 'worktree' && !worktreePath ? `From ${originLabel(client, branch)}` : branch, loading: open && entry.refsQuery === null,
    refs: entry.refs.map((ref, index) => ({ name: str(ref.name), badge: refBadge(ref, cwd), selected: ref.name === branch, index: index + checkout.length })),
    status: open ? refsStatus({ refs: entry.refs, total: entry.total, nextCursor: entry.nextCursor ?? (entry.refs.length < entry.total ? 0 : null), ends: 0, loadingMore: entry.loadingMore }, false) : '',
    // Return picks the auto-highlighted first item (Combobox autoHighlight): the first ref, else "Create new ref".
    creatable, enterOp: checkout.length ? 'chatlocal:git-pr-open' : first ? 'cc:branch' : creatable ? 'cc:branch-create' : '', enterValue: checkout[0]?.reference ?? (first || creatable), checkout, ...stripRunOn(client, !!worktreePath),
    // BranchPicker's originControl: a draft choosing its new worktree's base (r4-git-branch.ts).
    ...stripPr(client, { cwd, threadBranch, branch, status: entry.status ?? null }), // r7-handoff: ThreadPullRequestBadgeControl before the branch trigger
    originShown: envMode === 'worktree' && !worktreePath && !locked, originOn: envMode === 'worktree' && !worktreePath && !locked && (forceWorktree || startFromOrigin(client)), gitless: false };
}

/** composer.workspace / composer.branch / composer.previousWorktree: live while the strip shows its controls (cached repository state). */
export function stripShortcuts(client: T3Client): { workspace: boolean; branch: boolean; previous: boolean } {
  const none = { workspace: false, branch: false, previous: false };
  if (client.connection !== 'connected' || !client.ready || !client.projectId) return none;
  if (client.threadId && (client.local as { clientSettings?: Obj }).clientSettings?.persistComposerContextStrip !== true) return none;
  const cwd = cwdFor(client), entry = cwd ? repos.get(client)?.get(cwd) : undefined;
  if (!entry?.isRepo) return none;
  return { workspace: !client.threadId, branch: true, previous: !!previousWorktree(client) };
}

/** keyboard-dispatch.ts: the strip's three chords, as its `add` builder takes them. */
export function addStripShortcuts(client: T3Client, add: (command: string, kind: string, target: string, label: string) => void): void {
  const strip = stripShortcuts(client);
  if (strip.workspace) add('composer.workspace', 'options', 'workspace', 'Workspace');
  if (strip.branch) add('composer.branch', 'options', 'branch', 'Branch');
  if (strip.previous) add('composer.previousWorktree', 'command', 'cclocal:previous-worktree', 'Previous Worktree');
}

/** The workspace menu: Current checkout, New worktree, or the previous worktree (a draft only). */
export function setEnvMode(client: T3Client, mode: string): void {
  if (client.threadId) throw new ClientError('A started thread keeps its workspace.');
  const key = client.draftKey, current = draftContext(client, key);
  if (mode === 'previous') {
    const seed = previousWorktree(client);
    if (!seed) throw new ClientError('There is no previous worktree in this project.');
    contexts(client)[key] = { envMode: 'worktree', branch: seed.branch, worktreePath: seed.worktreePath };
    return;
  }
  if (mode !== 'local' && mode !== 'worktree') throw new ClientError('Choose Current checkout or New worktree.');
  // onEnvModeChange keeps the draft's branch (the new worktree's base); only New worktree leaves an existing tree (lane r4-git).
  contexts(client)[key] = { envMode: mode, branch: current.branch, worktreePath: mode === 'worktree' ? '' : current.worktreePath };
  resetOrigin(client, key); // resolveNewDraftStartFromOrigin: each mode change starts from the project setting
}

/** resolveDraftEnvModeAfterBranchChange. */
function envAfterBranch(nextWorktreePath: string, currentWorktreePath: string, envMode: string): string {
  return nextWorktreePath || (envMode === 'worktree' && !currentWorktreePath) ? 'worktree' : 'local';
}
/** setThreadBranch: a draft records branch and worktree; a started thread updates its metadata. */
async function setThreadBranch(client: T3Client, native: Native, storage: Files, branch: string, worktreePath: string): Promise<void> {
  if (!client.threadId) {
    contexts(client)[client.draftKey] = { envMode: envAfterBranch(worktreePath, activeWorktree(client), effectiveEnvMode(client)), branch, worktreePath };
    return;
  }
  const access = client.restAccess(native);
  const [commandId] = await access.ids(1);
  await access.dispatch(storage, { type: 'thread.metadata.update', commandId, threadId: client.threadId,
    branch: branch || null, worktreePath: worktreePath || null }, 'Change branch');
}

/** BranchToolbarBranchSelector.selectBranch / createRef. */
export async function selectBranch(client: T3Client, native: Native, storage: Files, name: string, create = false, known?: Obj): Promise<string> {
  const root = projectRoot(client), cwd = cwdFor(client);
  if (!cwd || !name.trim()) throw new ClientError('Choose a ref.');
  const entry = repo(client, cwd), worktreePath = activeWorktree(client), envMode = effectiveEnvMode(client);
  const ref = known ?? entry.refs.find(candidate => candidate.name === name);
  // Choosing the base of a new worktree only records it.
  if (!client.threadId && envMode === 'worktree' && !worktreePath && !create) {
    contexts(client)[client.draftKey] = { envMode: 'worktree', branch: name, worktreePath: '' };
    return '';
  }
  // resolveBranchSelectionTarget: a ref checked out in a worktree is reused there.
  if (!create && str(ref?.worktreePath)) {
    const path = str(ref!.worktreePath);
    await setThreadBranch(client, native, storage, name, path === root ? '' : path);
    return '';
  }
  const nextWorktree = !create && worktreePath && ref?.isDefault === true ? '' : worktreePath;
  const checkoutCwd = nextWorktree || root;
  const access = client.restAccess(native);
  try {
    const result = create ? await access.request('vcs.createRef', { cwd: checkoutCwd, refName: sanitizeNewRefName(name), switchRef: true }, true)
      : await access.request('vcs.switchRef', { cwd: checkoutCwd, refName: name }, true);
    const local = ref?.isRemote === true ? name.replace(/^[^/]+\//, '') : name;
    const switched = create ? str(result.refName, sanitizeNewRefName(name)) : ref?.isRemote === true ? str(result.refName, local) : local;
    for (const path of [checkoutCwd, cwd]) { const cached = repo(client, path); cached.checked = false; cached.refsQuery = null; }
    entry.refName = switched;
    await setThreadBranch(client, native, storage, switched, nextWorktree);
    return '';
  } catch (error) {
    pushToast(client, { kind: 'error', title: create ? 'Failed to create and switch ref.' : 'Failed to switch ref.',
      description: error instanceof Error ? error.message : 'An error occurred.' });
    return '';
  }
}

/** handleBranchContextMenu: right-clicking the branch trigger or a ref offers "Copy branch name". */
export async function branchMenu(client: T3Client, native: Native, name: string): Promise<string> {
  if (!name) return '';
  const access = client.restAccess(native);
  const reply = await access.call({ op: 'contextMenu', items: [{ id: 'copy-branch-name', label: 'Copy branch name', icon: 'copy' }] });
  if (reply.clicked !== 'copy-branch-name') return '';
  try {
    const copied = await access.call({ op: 'copyText', text: name });
    if (copied.copied === false) throw new ClientError('The clipboard refused the branch name.');
    pushToast(client, { kind: 'success', title: 'Branch name copied', description: name });
  } catch (error) {
    pushToast(client, { kind: 'error', title: 'Failed to copy branch name', description: error instanceof Error ? error.message : 'An error occurred.' });
  }
  return '';
}

/** The launch's workspace: a new worktree from its base ref, an existing worktree, else the project root (on a chosen branch). */
/** A multi-model draft's base: the repository check and the branch each new worktree starts from. */
export function fanoutBase(client: T3Client): { isRepo: boolean; branch: string; startFromOrigin: boolean } {
  const entry = repos.get(client)?.get(projectRoot(client));
  return { isRepo: entry?.isRepo === true, branch: draftContext(client).branch || (entry?.refName ?? ''), startFromOrigin: false };
}
export function workspaceStrategy(client: T3Client): Obj {
  const context = draftContext(client);
  if (context.worktreePath) return { type: 'existing_worktree', worktreePath: context.worktreePath, ...(context.branch ? { branch: context.branch } : {}) };
  if (context.envMode === 'worktree') {
    const entry = repos.get(client)?.get(projectRoot(client));
    const base = context.branch || defaultRefName(client) || (entry?.refName ?? '');
    if (!base) throw new ClientError('Choose a base ref for the new worktree.');
    return { type: 'worktree', baseRef: base, ...(startFromOrigin(client) ? { startFromOrigin: true } : {}) };
  }
  return { type: 'root', ...(context.branch ? { branch: context.branch } : {}) };
}

/** r6-polish: the open strip picker's pages (r6-polish-refs.ts asks whether a scroll still wants its page). */
export function stripPages(client: T3Client): Repo[] { return [...(repos.get(client)?.values() ?? [])].filter(entry => entry.refsQuery !== null); }
