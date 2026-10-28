// The workspace / branch context strip under the draft composer,
// adapted from T3 Code (MIT); see LICENSE-T3. Sources at 9c161462c9 (the two selectors'
// tooltip shortcuts at 74541bab17): components/BranchToolbar.tsx, BranchToolbarEnvModeSelector.tsx,
// BranchToolbarBranchSelector.tsx, BranchPicker.tsx, BranchToolbar.logic.ts
// (shouldShowComposerContextStrip, resolveBranchTriggerLabel) and
// packages/client-runtime/src/operations/commands.ts (launch workspaceStrategy).
// The server owns the repository: status, refs and checkouts come from vcs.*.
import { arr, obj, str, num, type Obj } from '../client/domain';
import { ClientError, type Files, type Native } from '../client/protocol';
import { pushToast } from '../shell/toast';
import type { T3Client } from '../client/client';
import { fanoutSelections } from './composer-controls-fanout';
import { defaultRefName, originDefault, originLabel, resetOrigin, startFromOrigin } from '../details/git-branch'; // Start from origin, the default base ref
import { REF_PAGE, firstPage, morePages, refScrollEnds, refsStatus, scrollEnds } from './ref-paging';
import { NO_STRIP_PR, stripPr } from './strip-pr-badge'; // the strip's pull request badge
import { checkoutItems, type CheckoutItem } from './pr-checkout'; // the picker's checkout item
import { NO_RUN_ON, stripRunOn } from '../details/git-env'; // MobileRunContextSelector's "Run on"
import { gitlessStrip, noteStripLayout } from './context-strip'; // the non-Git strip (BranchToolbar.logic.ts) and the labels' measurement
import { autoBalanceState } from './auto-balance'; // the Run on menu's "Auto balance"
import { letGo } from '../client/let-go';
import { draftFiles } from './composer-editor-files';
import { newThreadEnvMode, refreshNewThreadContext } from './new-thread-context';
import { peekVcsStatus, restartVcsStatus, vcsStreamFollows } from '../details/shell-vcs'; // the workspace status the card's stream holds
import { commandShortcut } from '../shell/shell'; // the workspace and host tooltips' shortcuts

export type DraftContext = { envMode: string; branch: string; worktreePath: string };
// `switched`: the ref a switch checked out, kept while the stream still reports the status it had then.
type Repo = { isRepo: boolean; refName: string; status?: Obj; checked: boolean; switched?: { name: string; status: Obj | null }; refs: Obj[]; total: number; refsQuery: string | null; error: string; nextCursor?: number | null; ends?: number; loadingMore?: boolean };
type RepoCache = { environmentId: string; generation: number; origin: string; byCwd: Map<string, Repo> };
const repos = new WeakMap<T3Client, RepoCache>();
function repositoryCache(client: T3Client): Map<string, Repo> {
  let cache = repos.get(client);
  if (!cache || cache.environmentId !== client.environmentId || cache.generation !== client.generation || cache.origin !== client.origin) {
    cache = { environmentId: client.environmentId, generation: client.generation, origin: client.origin, byCwd: new Map() };
    repos.set(client, cache);
  }
  return cache.byCwd;
}
function repo(client: T3Client, cwd: string): Repo {
  const byCwd = repositoryCache(client);
  let entry = byCwd.get(cwd);
  if (!entry) { entry = { isRepo: false, refName: '', checked: false, refs: [], total: 0, refsQuery: null, error: '' }; byCwd.set(cwd, entry); }
  return entry;
}
const contexts = (client: T3Client): Record<string, DraftContext> => client.local.composerControls.contexts ??= {};
/** The draft's workspace mode (Settings › New threads default), chosen branch and an existing worktree it points at. */
export function draftContext(client: T3Client, key = client.draftKey): DraftContext {
  const saved = contexts(client)[key];
  const fallback = newThreadEnvMode(client);
  return { envMode: saved?.envMode === 'worktree' || saved?.envMode === 'local' ? saved.envMode : fallback, branch: saved?.branch ?? '', worktreePath: saved?.worktreePath ?? '' };
}
/** setDraftThreadContext: changes some of the draft's context and keeps the rest (auto-balance clears branch and worktree). */
export function patchDraftContext(client: T3Client, patch: Partial<DraftContext>, key = client.draftKey): void {
  contexts(client)[key] = { ...draftContext(client, key), ...patch };
}
const projectRoot = (client: T3Client) => str(client.shell.projects.find(entry => entry.id === client.projectId)?.workspaceRoot);

// ── An unstarted server thread's workspace (ChatView canOverrideServerThreadEnvMode, pendingServerThreadEnvMode) ──
type ServerEnv = { threadId: string; envMode: string; branch: string };
const serverEnvs = new WeakMap<T3Client, ServerEnv>();
// pendingServerThreadStartFromOriginByThreadId: each thread's own choice, kept while the client lives; neither a
// workspace switch nor a thread change clears it (ChatView resets only the mode and the branch).
const serverOrigins = new WeakMap<T3Client, Map<string, boolean>>();
/** envLocked: a server thread with a message or a runtime (a run or a provider thread) keeps its workspace. */
export function threadEnvLocked(client: T3Client): boolean {
  if (!client.threadId) return false;
  const projection = client.projection;
  return arr(projection.messages).length > 0 || arr(projection.runs).length > 0 || !!str(obj(projection.thread).activeProviderThreadId);
}
/** canOverrideServerThreadEnvMode: a server thread with no message, no runtime and no worktree still picks its workspace. */
export function canOverrideServerEnv(client: T3Client): boolean {
  return !!client.threadId && !threadEnvLocked(client) && !str(obj(client.projection.thread).worktreePath)
    && !str(client.shell.threads.find(thread => thread.id === client.threadId)?.worktreePath);
}
/** The thread's pending mode and base; they end when the thread changes or starts (the reference resets them then). */
function serverEnv(client: T3Client): ServerEnv | null {
  const entry = serverEnvs.get(client);
  if (entry && entry.threadId === client.threadId && canOverrideServerEnv(client)) return entry;
  if (entry) serverEnvs.delete(client);
  return null;
}
/** The workspace an unstarted server thread will start in: Current checkout until New worktree is chosen. */
export function serverEnvMode(client: T3Client): string { return serverEnv(client)?.envMode ?? 'local'; }
/** activeThreadBranch while the thread can still choose: the picked base, else the thread's branch. */
export function serverEnvBranch(client: T3Client): string { return serverEnv(client)?.branch || str(obj(client.projection.thread).branch); }
function setServerEnv(client: T3Client, patch: Partial<ServerEnv>): void {
  const current = serverEnv(client) ?? { threadId: client.threadId, envMode: 'local', branch: '' };
  serverEnvs.set(client, { ...current, ...patch });
}
/** Picking the base ref of the thread's new worktree only records it (setPendingServerThreadBranch). */
export function pickServerBase(client: T3Client, branch: string): boolean {
  if (!canOverrideServerEnv(client) || serverEnvMode(client) !== 'worktree') return false;
  setServerEnv(client, { branch });
  return true;
}
/** Start from origin for the thread's new worktree: its own choice, else the project's setting (pendingServerThreadStartFromOrigin). */
export function serverOrigin(client: T3Client): boolean {
  if (!canOverrideServerEnv(client) || serverEnvMode(client) !== 'worktree') return false;
  return serverOrigins.get(client)?.get(client.threadId) ?? originDefault(client);
}
/** onStartFromOriginChange on an unstarted server thread; false when the thread cannot choose. */
export function toggleServerOrigin(client: T3Client): boolean {
  if (!canOverrideServerEnv(client) || serverEnvMode(client) !== 'worktree') return false;
  const origins = serverOrigins.get(client) ?? new Map<string, boolean>();
  origins.set(client.threadId, !serverOrigin(client));
  serverOrigins.set(client, origins);
  return true;
}
/**
 * startThreadTurn's bootstrap for the first message of an unstarted server thread in New worktree:
 * launchThread on the existing thread (reuseExistingThread) with a worktree from its base ref; null otherwise.
 */
export function serverThreadWorkspace(client: T3Client): Obj | null {
  if (!canOverrideServerEnv(client) || serverEnvMode(client) !== 'worktree') return null;
  const entry = repositoryCache(client).get(projectRoot(client));
  const base = serverEnvBranch(client) || defaultRefName(client) || (entry?.refName ?? '');
  if (!base) throw new ClientError('Select a base branch before sending in New worktree mode.');
  return { type: 'worktree', baseRef: base, ...(serverOrigin(client) ? { startFromOrigin: true } : {}) };
}
/** The thread's (or draft's) worktree, else null: the strip's activeWorktreePath. */
function activeWorktree(client: T3Client): string {
  return client.threadId ? str(obj(client.projection.thread).worktreePath) : draftContext(client).worktreePath;
}
function cwdFor(client: T3Client): string { return activeWorktree(client) || projectRoot(client); }
/**
 * The workspace whose status the context strip and branch notice use. The stream follows it
 * while the details card is closed (shell-details.ts), as ChatView and BranchToolbarBranchSelector
 * read `vcsEnvironment.status` for their workspace.
 */
export function stripWorkspace(client: T3Client): string {
  if (client.connection !== 'connected' || !client.ready || !client.projectId) return '';
  // ChatView's mismatch notice needs status even with the workspace card and context strip closed.
  return cwdFor(client);
}
/** resolveEffectiveEnvMode: a draft pointed at an existing worktree is local to it; a thread with a worktree is a worktree. */
function effectiveEnvMode(client: T3Client): string {
  const path = activeWorktree(client);
  if (client.threadId) return path ? 'worktree' : canOverrideServerEnv(client) ? serverEnvMode(client) : 'local';
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
async function load(client: T3Client, native: Native, cwd: string, open: boolean, query: string, card: boolean): Promise<Repo> {
  const entry = repo(client, cwd), access = client.restAccess(native);
  // BranchToolbarBranchSelector reads `vcsEnvironment.status`, the subscribeVcsStatus stream, and the
  // reference asks vcs.refreshStatus only on window focus and when a git menu opens (GitActionsControl).
  // So the strip and the card's branch row read the stream that follows this workspace (shell-vcs.ts).
  const streamed = peekVcsStatus(client, cwd);
  if (streamed && entry.switched?.status === streamed) {
    entry.refName = entry.switched.name; // the stream has not reported since the switch
  } else if (streamed) {
    entry.switched = undefined;
    entry.isRepo = streamed.isRepo === true; entry.refName = str(streamed.refName); entry.status = streamed; entry.error = ''; entry.checked = true; // its pr feeds the strip's badge
  } else if (!entry.checked && !vcsStreamFollows(client, cwd)) {
    // No stream follows this workspace yet: one read, shared with an identical one still pending, so an
    // answer let go before the reply (the reply can wait on a remote fetch) does not make the next one,
    // 500 ms later, send another (unshared: four a second).
    try {
      const status = await access.read('vcs.refreshStatus', { cwd });
      entry.isRepo = status.isRepo === true; entry.refName = str(status.refName); entry.status = status; entry.error = '';
    } catch (error) { if (letGo(error)) throw error; entry.isRepo = false; entry.error = error instanceof Error ? error.message : 'Could not read the repository.'; }
    entry.checked = true;
  }
  const search = sanitizeNewRefName(query).slice(0, 256);
  // A shared read: the strip asked again before the reply joins the read still pending.
  const list = (cursor?: number) => access.read('vcs.listRefs', { cwd, limit: REF_PAGE, ...(search ? { query: search } : {}), ...(cursor === undefined ? {} : { cursor }) });
  const isRepo = entry.checked ? entry.isRepo : true;
  if (open && isRepo && entry.refsQuery !== query) {
    Object.assign(entry, firstPage(await list(), scrollEnds(refScrollEnds(client), 'strip-refs'))); entry.refsQuery = query;
  } else if (open && isRepo) {
    // A scroll toward the list's end loads the next page (ref-paging.ts).
    Object.assign(entry, await morePages({ refs: entry.refs, total: entry.total, nextCursor: entry.nextCursor ?? null, ends: entry.ends ?? 0, loadingMore: entry.loadingMore }, refScrollEnds(client), 'strip-refs', list));
  }
  // The workspace card's read (card) leaves the strip picker's open pages alone.
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

/** The resource behind the strip, and the auto-balance load the root's `balanceLoad` task sends (auto-balance.ts). */
export async function composerBranches(client: T3Client, native: Native | null | undefined, open: boolean, query: string, card = false) {
  if (native?.available && client.ready && !client.threadId) await refreshNewThreadContext(client, native);
  const view = await stripView(client, native, open, query, card);
  if (!card) noteStripLayout(client, view);
  return { ...view, balanceFetch: card ? '' : autoBalanceState(client).fetch };
}
/**
 * Each branch picker's opening, for BranchToolbarBranchSelector's handleOpenChange: the strip's picker was open at its
 * last read (`stripOpen`), and the workspace card's picker opened since its last read (`cardOpened`, git-branch.ts).
 */
const pickers = new WeakMap<T3Client, { stripOpen: boolean; cardOpened: boolean }>();
function pickerState(client: T3Client): { stripOpen: boolean; cardOpened: boolean } {
  let state = pickers.get(client);
  if (!state) { state = { stripOpen: false, cardOpened: false }; pickers.set(client, state); }
  return state;
}
/** The workspace card's branch picker opened (git-branch.ts `git-open`); the card's next read asks for the branch. */
export function cardBranchPickerOpened(client: T3Client): void { pickerState(client).cardOpened = true; }

/** shouldShowComposerContextStrip with Git controls. */
async function stripView(client: T3Client, native: Native | null | undefined, open: boolean, query: string, card: boolean) {
  // handleOpenChange: a picker that opens asks the server to read the checkout's current branch again, without a remote
  // fetch; the answer reaches it through the status stream, and a failure is not reported. Each picker keeps its own
  // open state, as the reference's isBranchMenuOpen, whatever folder or thread its reads find.
  const picker = pickerState(client), opened = card ? picker.cardOpened : open && !picker.stripOpen;
  if (card) picker.cardOpened = false; else picker.stripOpen = open;
  const hidden = { show: false, envMode: 'local', forceWorktree: false, envLabel: 'Current checkout', envIcon: 'folder', envLocked: false, previous: false, previousBranch: '',
    branchLabel: '', branch: '', loading: false, refs: [] as Array<{ name: string; badge: string; selected: boolean; index: number }>, status: '', creatable: '', enterOp: '', enterValue: '', checkout: [] as CheckoutItem[], ...NO_RUN_ON, ...NO_STRIP_PR, gitless: false,
    // BranchToolbarEnvModeSelector and BranchToolbarEnvironmentSelector add these to their tooltips (shortcutLabelForCommand).
    keyWorkspace: commandShortcut(client.config, 'composer.workspace'), keyHost: commandShortcut(client.config, 'composer.host') };
  if (!native?.available || client.connection !== 'connected' || !client.ready || !client.projectId) return hidden;
  const persist = (client.local as { clientSettings?: Obj }).clientSettings?.persistComposerContextStrip === true;
  if (client.threadId && !persist && !card) return hidden; // the workspace card's branch row (git-branch.ts) shows on threads too
  const cwd = cwdFor(client);
  if (!cwd) return hidden;
  if (opened) void client.restAccess(native).request('vcs.refreshStatus', { cwd, localOnly: true }).catch(() => undefined);
  let entry: Repo;
  try { entry = await load(client, native, cwd, open, query, card); } catch { return hidden; }
  // shouldShowComposerContextStrip without Git controls: only the machine selector, when it shows.
  // ChatView: before a checkout's status arrives, a never-seen checkout assumes Git.
  if (!(entry.checked ? entry.isRepo : true)) return card ? hidden : gitlessStrip(client, hidden, persist);
  const thread = obj(client.projection.thread), context = draftContext(client);
  // forceNewWorktree: a multi-model draft starts each model in its own new worktree.
  const forceWorktree = !client.threadId && !!fanoutSelections(client);
  const worktreePath = forceWorktree ? '' : activeWorktree(client), envMode = forceWorktree ? 'worktree' : effectiveEnvMode(client);
  // A started thread (messages or a runtime) keeps its workspace: the locked row (envModeLocked).
  const locked = !!client.threadId && !canOverrideServerEnv(client);
  const threadBranch = client.threadId ? (locked ? str(thread.branch) : serverEnvBranch(client)) : context.branch;
  // resolveBranchToolbarValue: a new worktree starts from the chosen base, else the checkout's current branch.
  const branch = envMode === 'worktree' && !worktreePath ? threadBranch || defaultRefName(client) || entry.refName : entry.refName || threadBranch;
  const created = sanitizeNewRefName(query);
  const creatable = created && !entry.refs.some(ref => ref.name === created) && !(envMode === 'worktree' && !worktreePath) ? created : '';
  const first = str(entry.refs[0]?.name);
  const checkout = open ? checkoutItems(client, query, entry.status ?? null) : []; // unshifted before the refs
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
    // BranchPicker's originControl: a draft choosing its new worktree's base (git-branch.ts).
    ...stripPr(client, { cwd, threadBranch, branch, status: entry.status ?? null }), // ThreadPullRequestBadgeControl before the branch trigger
    originShown: envMode === 'worktree' && !worktreePath && !locked, originOn: envMode === 'worktree' && !worktreePath && !locked && (forceWorktree || startFromOrigin(client)), gitless: false,
    keyWorkspace: hidden.keyWorkspace, keyHost: hidden.keyHost };
}

/** composer.workspace / composer.branch / composer.previousWorktree: live while the strip shows its controls (cached repository state). */
export function stripShortcuts(client: T3Client): { workspace: boolean; branch: boolean; previous: boolean } {
  const none = { workspace: false, branch: false, previous: false };
  if (client.connection !== 'connected' || !client.ready || !client.projectId) return none;
  if (client.threadId && (client.local as { clientSettings?: Obj }).clientSettings?.persistComposerContextStrip !== true) return none;
  const cwd = cwdFor(client), entry = cwd ? repositoryCache(client).get(cwd) : undefined;
  if (!entry || (entry.checked && !entry.isRepo)) return none; // a checkout whose status has not arrived is assumed Git
  return { workspace: !client.threadId || canOverrideServerEnv(client), branch: true, previous: !!previousWorktree(client) };
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
  if (!client.threadId && fanoutSelections(client)) return; // onEnvModeChange: several models always start in new worktrees
  if (client.threadId) {
    if (!canOverrideServerEnv(client) || mode === 'previous') throw new ClientError('A started thread keeps its workspace.');
    if (mode !== 'local' && mode !== 'worktree') throw new ClientError('Choose Current checkout or New worktree.');
    setServerEnv(client, { envMode: mode }); // onEnvModeChange: setPendingServerThreadEnvMode only
    return;
  }
  const key = client.draftKey, current = draftContext(client, key);
  if (mode === 'previous') {
    const seed = previousWorktree(client);
    if (!seed) throw new ClientError('There is no previous worktree in this project.');
    contexts(client)[key] = { envMode: 'worktree', branch: seed.branch, worktreePath: seed.worktreePath };
    return;
  }
  if (mode !== 'local' && mode !== 'worktree') throw new ClientError('Choose Current checkout or New worktree.');
  // onEnvModeChange keeps the draft's branch (the new worktree's base); only New worktree leaves an existing tree.
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
  // Choosing the base of a new worktree only records it (an unstarted server thread's too).
  if (client.threadId && !create && pickServerBase(client, name)) return '';
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
    // runBranchAction: the refs and the status stream are read again (branchRefState.refresh(), branchStatusQuery.refresh());
    // the switched name shows until the stream reports again.
    for (const path of [checkoutCwd, cwd]) {
      const cached = repo(client, path); cached.checked = false; cached.refsQuery = null;
      cached.switched = { name: switched, status: peekVcsStatus(client, path) };
      restartVcsStatus(client, path);
    }
    entry.refName = switched;
    await setThreadBranch(client, native, storage, switched, nextWorktree);
    return '';
  } catch (error) {
    if (letGo(error)) throw error;
    pushToast(client, { kind: 'error', title: create ? 'Failed to create and switch ref.' : 'Failed to switch ref.',
      description: error instanceof Error ? error.message : 'An error occurred.' });
    return '';
  }
}

/** handleBranchContextMenu's one item, "Copy branch name": the trigger's and each ref's context popover
 * (composer-controls.contract BranchNameMenu) runs it with the right-clicked name. */
export async function copyBranchName(client: T3Client, native: Native, name: string): Promise<string> {
  if (!name) return '';
  const access = client.restAccess(native);
  try {
    const copied = await access.call({ op: 'copyText', text: name });
    if (copied.copied === false) throw new ClientError('The clipboard refused the branch name.');
    pushToast(client, { kind: 'success', title: 'Branch name copied', description: name });
  } catch (error) {
    if (letGo(error)) throw error;
    pushToast(client, { kind: 'error', title: 'Failed to copy branch name', description: error instanceof Error ? error.message : 'An error occurred.' });
  }
  return '';
}

/** The launch's workspace: a new worktree from its base ref, an existing worktree, else the project root (on a chosen branch). */
/** A multi-model draft's base: the repository check and the branch each new worktree starts from. */
export function fanoutBase(client: T3Client): { isRepo: boolean; branch: string; startFromOrigin: boolean } {
  const entry = repositoryCache(client).get(projectRoot(client));
  return { isRepo: entry?.isRepo === true, branch: draftContext(client).branch || (entry?.refName ?? ''), startFromOrigin: false };
}
export function workspaceStrategy(client: T3Client): Obj {
  const context = draftContext(client);
  if (context.worktreePath) return { type: 'existing_worktree', worktreePath: context.worktreePath, ...(context.branch ? { branch: context.branch } : {}) };
  if (context.envMode === 'worktree') {
    const entry = repositoryCache(client).get(projectRoot(client));
    const base = context.branch || defaultRefName(client) || (entry?.refName ?? '');
    if (!base) throw new ClientError('Choose a base ref for the new worktree.');
    return { type: 'worktree', baseRef: base, ...(startFromOrigin(client) ? { startFromOrigin: true } : {}) };
  }
  return { type: 'root', ...(context.branch ? { branch: context.branch } : {}) };
}

/** The open strip picker's pages (ref-loading.ts asks whether a scroll still wants its page). */
export function stripPages(client: T3Client): Repo[] { return [...repositoryCache(client).values()].filter(entry => entry.refsQuery !== null); }

// Branch-notice-and-navigation: ported from BranchToolbar.logic.ts and ChatView.tsx at
// 1e2ecbd975. Exact shares the workspace status stream and represents the AlertDialog request
// as a command result; revelation and dismissal remain session state, as in ChatView.logic.ts.
export function resolveLocalCheckoutBranchMismatch(input: {
  effectiveEnvMode: string; activeWorktreePath: string | null; activeThreadBranch: string | null; currentGitBranch: string | null;
}): { threadBranch: string; currentBranch: string } | null {
  const { effectiveEnvMode, activeWorktreePath, activeThreadBranch, currentGitBranch } = input;
  if (effectiveEnvMode !== 'local' || activeWorktreePath !== null) return null;
  if (!activeThreadBranch || !currentGitBranch || activeThreadBranch === currentGitBranch) return null;
  return { threadBranch: activeThreadBranch, currentBranch: currentGitBranch };
}
const mismatchSessions = new WeakMap<T3Client, { revealed: string; dismissed: Set<string>; confirm: string }>();
function mismatchSession(client: T3Client) {
  let state = mismatchSessions.get(client);
  if (!state) { state = { revealed: '', dismissed: new Set(), confirm: '' }; mismatchSessions.set(client, state); }
  return state;
}
function checkoutMismatch(client: T3Client) {
  const cwd = projectRoot(client), cached = repositoryCache(client).get(cwd), status = peekVcsStatus(client, cwd) ?? cached?.status;
  const current = cached?.switched && cached.switched.status === status ? cached.switched.name : str(status?.refName);
  const mismatch = client.threadId ? resolveLocalCheckoutBranchMismatch({ effectiveEnvMode: effectiveEnvMode(client),
    activeWorktreePath: activeWorktree(client) || null, activeThreadBranch: serverEnvBranch(client), currentGitBranch: current }) : null;
  return mismatch ? { ...mismatch, cwd, dirty: status?.hasWorkingTreeChanges === true,
    key: `${client.environmentId}:${client.threadId}:${mismatch.threadBranch}:${mismatch.currentBranch}`,
    confirmationKey: `${client.environmentId}:${client.generation}:${client.origin}:${cwd}:${client.threadId}:${mismatch.threadBranch}:${mismatch.currentBranch}` } : null;
}
export function branchMismatchNotice(client: T3Client) {
  const mismatch = checkoutMismatch(client), state = mismatchSession(client), key = mismatch?.key ?? '';
  if (state.revealed !== key) state.revealed = '';
  const hasContent = !!client.draft.trim() || client.snapshotDrafts.length > 0 || draftFiles(client.local).some(file => file.draftKey === client.draftKey);
  if (!mismatch || state.dismissed.has(key) || (!hasContent && state.revealed !== key)) return null;
  state.revealed = key;
  return { id: `branch-mismatch:${key}`, icon: 'git-branch', title: 'Branch changed from', code: mismatch.threadBranch,
    tip: `This thread last ran on ${mismatch.threadBranch}. Sending will continue on ${mismatch.currentBranch}.`,
    action: 'cc:branch-restore', actionLabel: 'Restore branch', dismiss: 'cclocal:branch-mismatch-dismiss',
    dismissId: key, dismissLabel: 'Dismiss branch change notice' };
}
export function dismissBranchMismatch(client: T3Client, key: string): void { mismatchSession(client).dismissed.add(key); }

/** Restore the local checkout. A confirmation belongs to this exact thread and mismatch. */
export async function restoreThreadBranch(client: T3Client, native: Native, storage: Files, confirmed = false): Promise<string> {
  const mismatch = checkoutMismatch(client), state = mismatchSession(client);
  if (!mismatch || (confirmed && state.confirm !== mismatch.confirmationKey)) { state.confirm = ''; return ''; }
  if (!confirmed && mismatch.dirty) { state.confirm = mismatch.confirmationKey; return `branch-restore-confirm:${mismatch.threadBranch}`; }
  state.confirm = '';
  const access = client.restAccess(native), threadId = client.threadId;
  const environmentId = client.environmentId, generation = client.generation, origin = client.origin, projectId = client.projectId;
  const current = () => client.environmentId === environmentId && client.generation === generation && client.origin === origin
    && client.threadId === threadId && client.projectId === projectId;
  let result: Obj;
  try { result = await access.request('vcs.switchRef', { cwd: mismatch.cwd, refName: mismatch.threadBranch }, true); }
  catch (error) {
    if (!current()) return '';
    if (letGo(error)) throw error;
    pushToast(client, { kind: 'error', title: 'Failed to switch checkout', description: error instanceof Error ? error.message : 'An error occurred.', stacked: true });
    return '';
  }
  if (!current()) return '';
  const branch = str(result.refName, mismatch.threadBranch), cached = repo(client, mismatch.cwd);
  cached.checked = false; cached.refsQuery = null; cached.switched = { name: branch, status: peekVcsStatus(client, mismatch.cwd) };
  restartVcsStatus(client, mismatch.cwd);
  if (branch !== mismatch.threadBranch) {
    try {
      const [commandId] = await access.ids(1);
      if (!current()) return '';
      await access.dispatch(storage, { type: 'thread.metadata.update', commandId, threadId, branch, worktreePath: null }, 'Restore thread branch');
    } catch (error) {
      if (!current()) return '';
      if (letGo(error)) throw error;
      pushToast(client, { kind: 'error', title: 'Checkout switched, but the thread could not be updated',
        description: error instanceof Error ? error.message : 'An error occurred.', stacked: true });
      return '';
    }
  }
  return current() ? 'focus:composer' : '';
}
