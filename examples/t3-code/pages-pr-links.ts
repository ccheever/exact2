// Pull requests and threads (pr-links-previews-and-routing, C9/C10): the detail panel's linked-thread count
// (`pullRequests.linkedThreads`, read again every 10 s and on the server's refresh announcements), the More
// menu's Link / Unlink item and its thread picker, the palette of linked threads, the back arrow beside a
// thread, and the hover card and click of a pull request link in the panel's text. Sources: T3 Code (MIT,
// see LICENSE-T3) apps/web/src/components/pullRequest/{PullRequestThreadLinks,PullRequestLinkPreview,
// PullRequestDetailPanel}.tsx, hooks/usePullRequestLinking.ts, commandPaletteBus.ts,
// components/CommandPalette.logic.ts (buildLinkedThreadActionItems), lib/openPullRequestLink.ts.
import type { T3Client } from './client';
import { arr, num, obj, str, type Obj } from './domain';
import { ClientError, bridgeReply, type Files, type Native } from './protocol';
import { pushToast } from './toast';
import { letGo } from './let-go';
import { findProjectForChangeRequest, findProjectOnChangeRequestHost, linkMode, parseChangeRequestUrl, threadPullRequestKey } from './palette-linkpr';
import { buildLinkedThreadActionItems, isPullRequestLinked, linkedThreadsLabel, planThreadPullRequestMutation, pullRequestCandidateUrlFromReferenceAutolink,
  resolvePullRequestPreviewTarget, threadPickerCandidates, type LinkMode } from './pages-pr-links-logic';
import { visiblePullRequests } from './shell-pr';
import { repositorySelector } from './composer-editor-menu';
import { fleetThreadId } from './settings-b-fleet';
import { environmentRequest, prEnvironment, readsPullRequests, type PrEnvironment } from './pages-pr-environments';
import { pullRequestRefreshEpoch } from './pages-pr-refresh';
import { relativeLabel } from './pages-prs';
import { row, closedView, type Item, type PaletteView } from './palette';

// ── The panel's link context ────────────────────────────────────────────────

/** What the panel showing a pull request knows about it: its server, its reference, its URL, and where it is drawn. */
export type LinkContext = { environmentId: string; reference: Obj; url: string; page: boolean };
type Relations = { threads: Obj[] | null; error: string; tick: number; epoch: number; due: boolean };
type LinksState = { relations: Map<string, Relations>; pending: string; context: LinkContext | null };
const states = new WeakMap<object, LinksState>();
function stateOf(client: object): LinksState {
  let state = states.get(client);
  if (!state) { state = { relations: new Map(), pending: '', context: null }; states.set(client, state); }
  return state;
}
const relationsKey = (context: { environmentId: string; url: string }) => `${context.environmentId}\n${context.url}`;
const messageOf = (error: unknown, fallback: string) => (error instanceof Error && error.message.trim() ? error.message : fallback);

export function emptyLinks() {
  return { shown: false, url: '', countShown: false, countText: '', countLabel: 'Linked threads', countTip: '', menuShown: false, menuLabel: '', menuIcon: 'link-2',
    menuPicker: false, pending: false, polling: false, backShown: false };
}
export type PrLinksView = ReturnType<typeof emptyLinks>;

/** The thread the panel stands beside (threadRef ?? the composer's draft): none on the page, none for an unsent draft (c8d7d50a73). */
function besideThread(client: T3Client, environment: PrEnvironment, context: LinkContext): Obj | null {
  if (context.page || !environment.focused || !client.threadId) return null;
  return client.shell.threads.find(thread => thread.id === client.threadId) ?? null;
}
/** usePullRequestLinking.canLink: a project on the link's host (its own repository where a thread holds one link). */
function canLink(projects: Obj[], mode: LinkMode, url: string): boolean {
  const parsed = parseChangeRequestUrl(url);
  if (parsed === null || mode === 'unsupported') return false;
  return (mode === 'multiple' ? findProjectOnChangeRequestHost : findProjectForChangeRequest)(projects, parsed) !== undefined;
}
/** The linked-thread query's input: the panel's reference with the link's normalized key (PullRequestThreadLinks). */
function relationsInput(context: LinkContext): Obj {
  const parsed = parseChangeRequestUrl(context.url);
  return parsed === null ? { ...context.reference } : { ...context.reference, ...threadPullRequestKey({ ...parsed, url: context.url }) };
}

/**
 * `pullRequests.linkedThreads` for the page's panel, where links are many per thread: read on arrival, again when
 * the 10 s tick moves (`refreshInterval: 10_000`), when the server announces a change, and after a link changes.
 * A failed read keeps the threads it had (the reference keeps `lastRelations` so polling never hides the count).
 */
export async function readLinkedThreads(client: T3Client, native: Native, context: LinkContext, tick: number): Promise<void> {
  const state = stateOf(client);
  state.context = context;
  const environment = prEnvironment(client, context.environmentId);
  if (!environment || !context.page || linkMode(environment.config) !== 'multiple' || parseChangeRequestUrl(context.url) === null) return;
  const key = relationsKey(context), epoch = pullRequestRefreshEpoch(client);
  const held = state.relations.get(key);
  if (held && held.tick === tick && held.epoch === epoch && !held.due) return;
  const next: Relations = { threads: held?.threads ?? null, error: '', tick, epoch, due: false };
  try { next.threads = arr((await environmentRequest(client, native, environment.id, 'pullRequests.linkedThreads', relationsInput(context))).threads); }
  catch (error) { if (letGo(error)) throw error; next.error = messageOf(error, 'Linked threads could not be read.'); }
  state.relations.set(key, next);
}

/** PullRequestThreadLinks' three displays, as the panel draws them. */
export function presentLinks(client: T3Client, context: LinkContext): PrLinksView {
  const view = emptyLinks(), state = stateOf(client);
  state.context = context;
  const environment = prEnvironment(client, context.environmentId);
  if (!environment) return view;
  const mode = linkMode(environment.config), parsed = parseChangeRequestUrl(context.url);
  view.url = context.url; view.pending = state.pending === context.url;
  view.polling = context.page && mode === 'multiple' && parsed !== null;
  const thread = besideThread(client, environment, context);
  // RightPanel: the back arrow when the thread has more than one pull request and its list can open.
  view.backShown = !context.page && thread !== null && obj(obj(environment.config.environment).capabilities).threadPullRequests === true
    && visiblePullRequests(thread.pullRequests).length > 1;
  if (mode === 'unsupported' || parsed === null) return view;
  const linkedHere = isPullRequestLinked(thread, context.url, mode);
  if (!linkedHere && !canLink(environment.projects, mode, context.url)) return view;
  view.shown = true;
  const relations = state.relations.get(relationsKey(context));
  const threads = mode === 'multiple' ? relations?.threads ?? [] : [];
  view.countShown = context.page && mode === 'multiple' && (threads.length > 0 || !!relations?.error);
  view.countText = threads.length ? String(threads.length) : '?';
  view.countLabel = linkedThreadsLabel(threads.length);
  view.countTip = `${view.countLabel}. Search in the command palette.`;
  view.menuShown = true;
  view.menuLabel = linkedHere ? 'Unlink from this thread' : thread ? 'Link to this thread' : 'Link to thread';
  view.menuIcon = linkedHere ? 'unlink-2' : 'link-2';
  view.menuPicker = !linkedHere && thread === null;
  return view;
}

// ── Linking and unlinking ───────────────────────────────────────────────────

/**
 * usePullRequestLinking.changeLink over the command the environment advertises: `thread.pull-request.link` /
 * `.unlink`, or the legacy one-link metadata update. A refusal is a toast ("Could not link the pull request");
 * the linked threads are read again after it lands.
 */
export async function changeLink(client: T3Client, native: Native, storage: Files, context: LinkContext, threadId: string, linked: boolean): Promise<boolean> {
  const state = stateOf(client);
  if (state.pending) return false;
  const environment = prEnvironment(client, context.environmentId);
  const parsed = parseChangeRequestUrl(context.url);
  state.pending = context.url; client.revision++;
  const error = client.error;
  try {
    if (!environment || parsed === null || (linked && !canLink(environment.projects, linkMode(environment.config), context.url))) throw new ClientError('The pull request is not available in this environment.');
    const legacy = findProjectForChangeRequest(environment.projects, parsed);
    const mutation = planThreadPullRequestMutation({ mode: linkMode(environment.config), threadId, reference: { ...parsed, url: context.url }, legacyProjectId: legacy ? str(legacy.id) : null,
      ...(legacy && repositorySelector(obj(legacy.repositoryIdentity)) ? { legacyRepository: repositorySelector(obj(legacy.repositoryIdentity)) } : {}), linked });
    if (mutation === null) throw new ClientError('This environment does not support linking this pull request.');
    const [commandId] = await client.restAccess(native).ids(1);
    const payload = { ...mutation, commandId: commandId! };
    if (environment.focused) await client.dispatch(native, storage, payload, linked ? 'Link pull request' : 'Unlink pull request');
    else await environmentRequest(client, native, environment.id, 'orchestration.dispatchCommand', payload, { write: true });
  } catch (failure) {
    if (letGo(failure)) throw failure;
    client.error = error; // a toast, not the transcript banner (PullRequestThreadLinks toastManager.add)
    pushToast(client, { kind: 'error', title: linked ? 'Could not link the pull request' : 'Could not unlink the pull request', description: messageOf(failure, String(failure)) });
    return false;
  } finally { state.pending = ''; client.revision++; }
  const held = state.relations.get(relationsKey(context));
  if (held) held.due = true;
  return true;
}

/** Whether the panel last drawn is the page's (a link's click selects there) rather than a thread's surface. */
export const linkPanelOnPage = (client: object) => stateOf(client).context?.page === true;

/** `pageslocal:pr-act-links`: the More menu's item ("toggle"), or the count / picker opening the palette (answered as `palette:…`). */
export async function linksCommand(client: T3Client, native: Native, storage: Files, value: string): Promise<string> {
  const context = stateOf(client).context;
  if (!context) throw new ClientError('Choose a pull request first.');
  if (value === 'linked') return `palette:pr-linked|${context.url}`;
  if (value === 'picker') return 'palette:pr-link-thread';
  if (value !== 'toggle') throw new ClientError(`Unknown link action: ${value}`);
  const environment = prEnvironment(client, context.environmentId);
  const thread = environment ? besideThread(client, environment, context) : null;
  if (!environment || !thread) return 'palette:pr-link-thread';
  await changeLink(client, native, storage, context, str(thread.id), !isPullRequestLinked(thread, context.url, linkMode(environment.config)));
  return '';
}

// ── The palette's pages: the linked threads, and the thread picker ─────────

/**
 * The count's palette (openCommandPalette({ query: url, linkedThreads })): while the query is still the pull request's URL,
 * the thread results are its linked threads, archived ones too ("Linked thread", "Archived thread"); a thread on another
 * server opens it there.
 */
export function linkedThreadItems(client: T3Client, page: string, query: string): Item[] | null {
  if (!page.startsWith('pr-linked|')) return null;
  const url = page.slice('pr-linked|'.length), state = stateOf(client), context = state.context;
  if (query !== url || !context || context.url !== url) return null;
  const relations = state.relations.get(relationsKey(context));
  if (!relations?.threads) return null;
  const focused = !context.environmentId || context.environmentId === client.environmentId;
  return buildLinkedThreadActionItems({ environmentId: context.environmentId || client.environmentId, threads: relations.threads, query }).map(item => ({ terms: item.searchTerms,
    row: row({ key: item.value, op: 'thread', arg: focused ? item.threadId : fleetThreadId(item.environmentId, item.threadId), icon: 'message-square', title: item.title, description: item.description }) }));
}

/** ThreadPicker ("Choose a thread"): the panel's server's open threads, a linked one marked and not chosen again. */
export function threadPickerView(client: T3Client, query: string, busy: boolean): PaletteView {
  const context = stateOf(client).context;
  const environment = context ? prEnvironment(client, context.environmentId) : null;
  const candidates = context && environment ? threadPickerCandidates({ threads: environment.threads, projects: environment.projects, query, url: context.url, mode: linkMode(environment.config) }) : [];
  const pending = busy || (!!context && stateOf(client).pending === context.url);
  let index = 0, top = 8;
  const rows = candidates.map(candidate => {
    const entry = row({ key: `pr-link-thread:${candidate.id}`, kind: pending || candidate.linked ? 'disabled' : 'action', op: 'flow', arg: 'pr-link-thread', arg2: candidate.id, icon: 'message-square',
      title: candidate.title || 'Untitled thread', description: candidate.project, trailing: candidate.linked ? 'Linked' : '', checked: candidate.linked });
    entry.index = entry.kind === 'disabled' ? -1 : index++; entry.top = top; entry.height = 12 + 20 + (candidate.project ? 16 : 0); top += entry.height;
    return entry;
  });
  return { ...closedView, open: true, page: 'pr-link-thread', panel: 'list', label: 'Choose a thread', testId: 'pull-request-thread-picker', placeholder: 'Search threads or projects...',
    rows, count: index, autoHighlight: true, empty: rows.length ? '' : 'No active threads found.', enterLabel: 'Select', escapeLabel: 'Close', loading: pending };
}

/** The picker's choice (palette `flow` op `pr-link-thread`): link the panel's pull request to that thread. */
export async function linkFromPicker(client: T3Client, native: Native, storage: Files, threadId: string): Promise<boolean> {
  const context = stateOf(client).context;
  if (!context) return false;
  return changeLink(client, native, storage, context, threadId, true);
}

// ── Link previews (PullRequestLinkPreview) and their click ─────────────────

export type LinkChip = { id: string; href: string; kind: string; label: string; size: string; tip: string; detail: string; icon: string; target: string; owner: string };
type Preview = { key: string; detail: Obj | null; error: string; read: boolean };
const previews = new WeakMap<object, Map<string, Preview>>();
const previewsOf = (client: object) => { let map = previews.get(client); if (!map) { map = new Map(); previews.set(client, map); } return map; };

export function emptyPreview() {
  return { key: '', phase: '', url: '', repository: '', number: '', stateKey: 'open', stateLabel: 'Open', title: '', author: '', avatar: '', initial: '', opened: '' };
}
export type PrPreviewView = ReturnType<typeof emptyPreview>;
const STATE_LABELS: Record<string, string> = { open: 'Open', draft: 'Draft', closed: 'Closed', merged: 'Merged' };

/**
 * The chip of a pull request link in the panel's text (its hover card and click): `target` is the preview target
 * (resolvePullRequestPreviewTarget, a row reference with its server), `detail` "reference" for a `#N` autolink (its
 * click asks the host first, confirmBeforeOpen), "commit" for a commit autolink (no card: its URL as a tooltip).
 */
export function linkChips(client: T3Client, environmentId: string, links: { href: string; kind: string }[]): LinkChip[] {
  const environment = prEnvironment(client, environmentId);
  if (!environment) return [];
  const seen = new Set<string>();
  return links.flatMap(link => {
    if (seen.has(link.href)) return [];
    seen.add(link.href);
    const candidate = link.kind === 'reference' ? pullRequestCandidateUrlFromReferenceAutolink(link.href) : link.kind === 'commit' ? null : link.href;
    const target = candidate ? resolvePullRequestPreviewTarget({ environmentId: environment.id, projects: environment.projects.map(project => ({ ...project, environmentId: environment.id })),
      pullRequestsEnabled: readsPullRequests(environment), url: candidate }) : null;
    if (!target && link.kind !== 'commit') return [];
    const ref = target ? JSON.stringify({ ...target.input, ...(environment.focused ? {} : { environmentId: environment.id }) }) : '';
    return [{ id: `pr-link-${seen.size}`, href: link.href, kind: 'pr-link', label: '', size: '', tip: link.href, detail: link.kind, icon: '', target: ref, owner: candidate ?? '' }];
  });
}

/** PullRequestLinkPreview's card: `pullRequests.detail` for the hovered link's target, kept per link. */
export async function readPreview(client: T3Client, native: Native, target: string, now: number): Promise<PrPreviewView> {
  const view = emptyPreview();
  if (!target) return view;
  let reference: Obj;
  try { reference = obj(JSON.parse(target)); } catch { return view; }
  const map = previewsOf(client);
  let held = map.get(target);
  if (!held) { held = { key: target, detail: null, error: '', read: false }; map.set(target, held); }
  if (!held.read) {
    try { held.detail = obj(await client.rpc(native, 'pullRequests.detail', reference)); held.error = ''; }
    catch (error) { if (letGo(error)) throw error; held.error = messageOf(error, 'The pull request could not be read.'); }
    held.read = true;
  }
  view.key = target;
  if (!held.detail) { view.phase = held.error ? 'error' : 'loading'; return view; }
  const detail = held.detail, author = detail.author === null ? null : obj(detail.author);
  const login = author ? str(author.login, 'ghost') : 'ghost', name = author ? str(author.name) : '';
  const state = detail.state === 'open' && detail.isDraft === true ? 'draft' : str(detail.state, 'open');
  return { ...view, phase: 'content', url: str(detail.url), repository: str(detail.repository), number: `#${num(detail.number)}`, stateKey: state, stateLabel: STATE_LABELS[state] ?? 'Open',
    title: str(detail.title), author: name && name !== login ? `${name} (@${login})` : login, avatar: author ? str(author.avatarUrl) : '', initial: login.slice(0, 1).toUpperCase(),
    opened: `opened ${relativeLabel(detail.createdAt, now)}` };
}
const hovering = new WeakMap<object, string>();
/**
 * `chatlocal:pr-preview`: the pointer entered (or left) a pull request link in the panel's text. The card's own
 * open and close (350 ms, 120 ms) belong to the window's hover layer; this says which pull request the panel's
 * resource reads for it (pullRequestDetail: the hovered link's `pullRequests.detail`, kept per link).
 */
export function notePreviewHover(client: object, target: string, inside: boolean): void {
  if (inside && target) hovering.set(client, target);
  else if (!inside && hovering.get(client) === target) hovering.delete(client);
}
export const hoveredLink = (client: object) => hovering.get(client) ?? '';
/** A preview is read again when the panel's detail or its conversation changes (keeps hover previews fresh after edits and turns). */
export function forgetPreviews(client: object): void { previews.delete(client); }

/**
 * A pull request link's click (useOpenChangeRequestLink, PullRequestLinkPreview confirmBeforeOpen): with ⌘, ⌃, ⇧ or
 * ⌥ held it stays an ordinary link (the system browser). A `#N` autolink asks `pullRequests.preview` first: a pull
 * request opens in the panel, anything else (an issue) opens in the browser. Beside a thread the panel is the
 * thread's surface; on the page, the page's selection (answered as `pr-select:<ref>`).
 */
export async function openLink(client: T3Client, native: Native, page: boolean, value: string, openSurface: (url: string, target: string) => Promise<void>): Promise<string> {
  const [kind = '', target = '', href = ''] = value.split('\n');
  const gesture = obj((await bridgeReply(native, { op: 'composerSendIntent' }).catch(() => ({ ok: false, value: {} }))).value);
  const browser = async (url: string) => { if (url) await bridgeReply(native, { op: 'remoteEditorsOpen', url }); return ''; };
  if (str(gesture.modifiers) || !target) return browser(href);
  let url = href;
  if (kind === 'reference') {
    try { url = str((await client.rpc(native, 'pullRequests.preview', obj(JSON.parse(target)))).url); }
    catch (error) { if (letGo(error)) throw error; return browser(href); }
    if (!parseChangeRequestUrl(url)) return browser(href);
  }
  if (page) return `pr-select:${target}`;
  await openSurface(url, target);
  return '';
}
