// Lane r9-connect: checking a pull request out from the branch picker, after T3 Code (MIT; see
// LICENSE-T3, upstream f90b77d809): BranchToolbarBranchSelector's checkout item (a ref search that
// parses as a pull request reference, offered while the thread is a local draft:
// ChatView canCheckoutPullRequestIntoThread) and PullRequestThreadDialog (resolve the reference with
// git.resolvePullRequest, then git.preparePullRequestThread in "local" or "worktree" mode; worktree
// passes the draft's thread id so the server runs the project's setup script for that thread, as the
// hand-off does, r7-handoff-thread.ts). On success the draft moves onto the checkout
// (ChatView handlePreparedPullRequestThread → openOrReuseProjectDraftThread).
//
// The dialog's text is the field's own (the field is bound to the reference it opened with); each
// edit reaches this file as `chatlocal:git-pr-text`. Lane r10-connect, as the HEAD dialog behaves
// (measured against its served build): each edit looks its reference up at once (the dialog's
// readCachedPullRequestResolution reads the lookup's atom, which starts it), but what the dialog
// shows follows useDebouncedValue { wait: 450 }: "Resolving pull request..." from the edit until
// 450 ms after the last one (a pull request cached before the edit shows at once), then the
// settled reference's answer or error. The command draws its state at once and waits the 450 ms of
// wall time itself (r10-connect-timing.ts).
// Opening and confirming are gated commands, whose shell clock lets the dialog show "Resolving…"
// and "Preparing worktree..." while they wait. A command another one superseded may never hear its
// reply, so no request is shared between commands.
import type { T3Client } from './client';
import { obj, str, num, type Obj } from './domain';
import { ClientError, type Native } from './protocol';
import { parsePullRequestReference } from './palette-linkpr';
import { providerMark, terminology } from './r4-git-logic';
import { ensureDraftThreadId } from './r7-handoff-thread';
import { settle, wakeShell } from './r10-connect-timing'; // lane r10-connect: the 450 ms debounce

export type CheckoutItem = { reference: string; title: string; mark: string };
type Resolution = { pullRequest: Obj } | { error: string };
interface CheckoutState {
  open: boolean; serial: number; reference: string; text: string; dirty: boolean; cwd: string; draftKey: string;
  provider: Obj | null; preparing: string; prepareError: string; cache: Map<string, Resolution>; lastProvider: Obj | null;
  /** r10-connect: edits so far, the reference the debounce last settled on (debouncedReference, parsed), whether
   *  an edit's 450 ms is still running, and the cached answer the edit found (cachedPullRequest, memoized per edit). */
  edits: number; debounced: string | null; waiting: boolean; memo: { parsed: string | null; pr: Obj | null };
}
const states = new WeakMap<T3Client, CheckoutState>();
function stateOf(client: T3Client): CheckoutState {
  let state = states.get(client);
  if (!state) {
    state = { open: false, serial: 0, reference: '', text: '', dirty: false, cwd: '', draftKey: '', provider: null, preparing: '', prepareError: '', cache: new Map(), lastProvider: null, edits: 0, debounced: null, waiting: false, memo: { parsed: null, pr: null } };
    states.set(client, state);
  }
  return state;
}

// getSourceControlPresentation: the host's name and words (a status without a provider reads as GitHub's).
const NAMES: Record<string, string> = { github: 'GitHub', gitlab: 'GitLab', forgejo: 'Forgejo', 'azure-devops': 'Azure DevOps', bitbucket: 'Bitbucket' };
const providerOf = (status: Obj | null): Obj | null => {
  const provider = status?.sourceControlProvider;
  return provider && typeof provider === 'object' && !Array.isArray(provider) ? provider as Obj : null;
};
function presentation(provider: Obj | null) {
  const status = provider ? { sourceControlProvider: provider } : null, words = terminology(status);
  return { name: str(provider?.name) || NAMES[str(provider?.kind) || 'github'] || 'source control', singular: words.singular, short: words.shortLabel, mark: providerMark(status) };
}
const capitalize = (text: string) => text.replace(/\b\w/g, letter => letter.toUpperCase()); // CSS text-transform: capitalize

/** canCheckoutPullRequestIntoThread: the active thread is a local draft. */
const canCheckout = (client: T3Client) => !client.threadId && !!client.projectId;
const projectRoot = (client: T3Client) => str(client.shell.projects.find(project => project.id === client.projectId)?.workspaceRoot);

/** The picker's checkout item for a ref search (`query`) over the repository `status`, or none. */
export function checkoutItems(client: T3Client, query: string, status: Obj | null): CheckoutItem[] {
  if (!canCheckout(client)) return [];
  const reference = parsePullRequestReference(query.trim());
  if (!reference) return [];
  const provider = providerOf(status), shown = presentation(provider);
  stateOf(client).lastProvider = provider;
  return [{ reference, title: `Checkout ${shown.singular}`, mark: shown.mark }];
}

export const CLOSED_CHECKOUT = { open: false, key: '', reference: '', settled: '', mark: 'github', title: '', description: '', label: '', placeholder: '',
  prTitle: '', prMeta: '', prState: '', resolving: false, resolvingLabel: '', error: '', preparing: '', canConfirm: false };

const cacheKey = (state: CheckoutState, reference: string) => `${state.cwd}\n${reference}`;

/** The dialog for the shell answer. */
export function checkoutView(client: T3Client): typeof CLOSED_CHECKOUT {
  const state = states.get(client);
  if (!state?.open) return CLOSED_CHECKOUT;
  const shown = presentation(state.provider);
  const parsed = parsePullRequestReference(state.text);
  // resolvedPullRequest: the settled reference's answer once the field holds it, else what the cache held
  // when the field took its text; the lookup's error is the settled reference's (pullRequestResolution).
  const settledLookup = state.debounced ? state.cache.get(cacheKey(state, state.debounced)) : undefined;
  const live = parsed !== null && parsed === state.debounced && settledLookup && 'pullRequest' in settledLookup ? settledLookup.pullRequest : null;
  const pr = live ?? (state.memo.parsed === parsed ? state.memo.pr : null);
  const resolving = !!parsed && !pr && !!state.cwd && (state.waiting || parsed !== state.debounced || !settledLookup);
  const validation = !state.dirty ? '' : !state.text.trim() ? `Paste a ${shown.singular} URL, checkout command, or enter 123 / #123.`
    : parsed === null ? `Use a ${shown.singular} URL, checkout command, 123, or #123.` : '';
  const error = validation || (!pr && settledLookup && 'error' in settledLookup ? settledLookup.error : '') || state.prepareError;
  return {
    open: true, key: String(state.serial), reference: state.reference, settled: state.debounced ?? '', mark: shown.mark,
    title: `Checkout ${shown.singular}`,
    description: `Resolve a ${shown.name} ${shown.singular}, then create the draft thread in the main repo or in a dedicated worktree.`,
    label: capitalize(shown.singular), placeholder: `${shown.short} URL, checkout command, or #42`,
    prTitle: pr ? str(pr.title) : '', prMeta: pr ? `#${num(pr.number)} · ${str(pr.headBranch)} to ${str(pr.baseBranch)}` : '', prState: pr ? capitalize(str(pr.state)) : '',
    resolving, resolvingLabel: `Resolving ${shown.singular}...`, error, preparing: state.preparing, canConfirm: !!state.cwd && !!pr && !resolving && !state.preparing,
  };
}

/** cachedPullRequest at the moment the field takes `parsed`. */
function memoFor(state: CheckoutState, parsed: string | null): CheckoutState['memo'] {
  const entry = parsed ? state.cache.get(cacheKey(state, parsed)) : undefined;
  return { parsed, pr: entry && 'pullRequest' in entry ? entry.pullRequest : null };
}

/** The query's error text: a SourceControlProviderError reads by its message getter, which the wire omits. */
function resolutionError(state: CheckoutState, error: unknown): string {
  if (error instanceof ClientError && error.kind === 'SourceControlProviderError' && error.detail)
    return `Source control provider ${str(state.provider?.kind) || 'github'} failed in getChangeRequest: ${error.detail}`;
  return error instanceof Error ? error.message : String(error);
}

async function resolve(client: T3Client, native: Native, state: CheckoutState, parsed = parsePullRequestReference(state.text)): Promise<void> {
  if (!parsed || !state.cwd) return;
  const key = cacheKey(state, parsed);
  if (state.cache.has(key)) return;
  try {
    const result = obj(await client.restAccess(native).request('git.resolvePullRequest', { cwd: state.cwd, reference: parsed }));
    state.cache.set(key, { pullRequest: obj(result.pullRequest) });
  } catch (error) {
    if (error instanceof ClientError && error.kind === 'superseded') return;
    state.cache.set(key, { error: resolutionError(state, error) });
  }
}

/** `chatlocal:git-pr-open|text|close`: the dialog's opening, its field and Cancel / Escape. */
export async function checkoutLocal(client: T3Client, native: Native, op: string, value: string): Promise<string> {
  const state = stateOf(client);
  if (op === 'open') {
    if (!canCheckout(client)) throw new ClientError('Open a draft thread to check a pull request out into it.');
    // A fresh dialog (key = Date.now() in the reference): failed lookups are asked again, answers are kept.
    for (const [key, entry] of state.cache) if ('error' in entry) state.cache.delete(key);
    Object.assign(state, { open: true, serial: state.serial + 1, reference: value, text: value, dirty: false, cwd: projectRoot(client),
      draftKey: client.draftKey, provider: state.lastProvider, preparing: '', prepareError: '', edits: 0, waiting: false });
    const parsed = parsePullRequestReference(value);
    state.debounced = parsed; state.memo = memoFor(state, parsed);
    await resolve(client, native, state, parsed);
    return '';
  }
  if (op === 'text') {
    if (!state.open) return '';
    state.text = value.slice(0, 2048); state.dirty = true;
    const edit = ++state.edits, serial = state.serial, parsed = parsePullRequestReference(state.text);
    state.waiting = true; state.memo = memoFor(state, parsed);
    await wakeShell(native); // the field's own render: "Resolving pull request..." while the wait runs
    const lookup = resolve(client, native, state, parsed); // started now, as the reference's cache read starts it
    await settle(native, 450);
    // A later edit (its own command) or another opening owns the dialog now.
    if (edit !== state.edits || serial !== state.serial || !state.open) return '';
    state.waiting = false; state.debounced = parsed;
    await wakeShell(native);
    await lookup;
    return '';
  }
  if (op === 'close') {
    // The dialog stays while a checkout is being prepared (onOpenChange ignores it while pending).
    if (!state.preparing) state.open = false;
    return '';
  }
  throw new ClientError(`Unknown checkout action: ${op}`);
}

/** `shell:git-pr-checkout`: Local or Worktree (Enter in the field is Worktree), over the field's last edit. */
export async function checkoutConfirm(client: T3Client, native: Native, mode: string, _text = ''): Promise<string> {
  const state = stateOf(client);
  if (!state.open || state.preparing) return '';
  if (mode !== 'local' && mode !== 'worktree') throw new ClientError('Choose Local or Worktree.');
  const parsed = parsePullRequestReference(state.text);
  if (!parsed) { state.dirty = true; return ''; }
  const resolution = state.cache.get(cacheKey(state, parsed));
  if (!resolution || !('pullRequest' in resolution) || !state.cwd) return '';
  state.preparing = mode; state.prepareError = '';
  try {
    const threadId = mode === 'worktree' ? await ensureDraftThreadId(client, native, state.draftKey) : '';
    const prepared = obj(await client.rpc(native, 'git.preparePullRequestThread', { cwd: state.cwd, reference: parsed, mode, ...(threadId ? { threadId } : {}) }, true));
    const branch = str(prepared.branch), worktreePath = str(prepared.worktreePath);
    // onPrepared: the draft takes the checkout (envMode follows the worktree), then the dialog closes.
    (client.local.composerControls.contexts ??= {})[state.draftKey] = { envMode: worktreePath ? 'worktree' : 'local', branch, worktreePath };
    state.open = false;
  } catch (error) {
    // A server failure is a tagged error, not an Error, in the reference: the dialog shows its generic line.
    if (!(error instanceof ClientError && error.kind === 'superseded')) state.prepareError = `Failed to prepare ${presentation(state.provider).singular} thread.`;
  } finally { state.preparing = ''; }
  return '';
}

/** For tests: the dialog's state. */
export const checkoutState = (client: T3Client) => stateOf(client);
