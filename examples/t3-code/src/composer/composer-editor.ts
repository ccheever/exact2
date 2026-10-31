import { composerInput } from './requests';
import { contextLinks, replaceLinks } from './context-links';
import { removeStaged } from './composer-staged-copies';
import { rememberCopiedChips, stagedInUse } from './composer-copied-chips';
import { noteComposerFilePaths, refreshFileMetadata } from '../shared/file-metadata';
// The shared field's source/selection feeds command menus and guarded edits;
// Contract draws the menus.
import { preserveCitationDraft } from '../timeline/citation-draft';
import type { T3Client } from '../client/client';
import { bridgeReply, ClientError, type Native } from '../client/protocol';
import { letGo } from '../client/let-go';
import { arr, messages, num, obj, str, type Obj } from '../client/domain';
import { pushToast } from '../shell/toast';
import { uuid } from '../transport/transport-wire'; // drawn ahead: code resumed by a server reply may not draw
import { commandChords } from './composer-presentation';
import { openUsageLimits } from './composer-controls-usage';
import { MAX_STASH_ENTRIES, deleteStashEntry, stashPrompt, stashView, stashEntries, type StashView } from './composer-editor-stash';
import {
  LIST_LABEL, contextId, contextLabel, contextLink, contextReferences, emptyText, fileLink, pathRows, promptLengthMessage,
  pullRequestRecord, pullRequestRows, pullRequestState, repositorySelector,
  skillChipLabels, skillRows, slashRows, threadRows, type EditorTrigger, type MenuRow,
} from './composer-editor-menu';
import { composerFileRecords, draftFiles, fileChipContexts, foldLimit, pruneFiles } from './composer-editor-files';
import { imageChipContexts, imageContextRecords } from './composer-editor-attach';
import { composerDrawer, composerStackHold, NO_DRAWER, type ComposerDrawer, type StackHold } from './composer-editor-drawer';
import { layoutFrames } from '../shell/layout-frames';
import { WorkspaceDiscovery, workspaceValues } from './composer-workspace-snapshots';
import { parseTerminalContext, formatTerminalContextReference, saveTerminalContext, terminalMessageRecords, terminalDraftRecords } from '../terminal/terminal-integrations';
import { terminalOpen } from '../terminal/terminal-drawer-view'; // the real terminalOpen
import { videoOp } from './composer-attachments';
import { annotationChipContexts, annotationMessageRecords } from '../browser/browser-annotation'; // Annotate's chips
import { composerFieldOwner } from './auto-balance-owner';
import { requestComposerField, type ComposerFieldCompletion } from './composer-field-request';
import { preferenceFiles } from '../client/protocol';
import { composerFieldCurrent } from './composer-field-bridge';
import { composerChips, composerTrigger } from './composer-source-chips';
import { composerSourceSession } from './composer-rich-state';
import { composerHistoryComplete, composerHistoryRecall } from './composer-prompt-history';
import { importDroppedThread } from './composer-thread-import';
import { parseFleetThreadId } from '../connections/settings-b-fleet';
import { checkMissingEditThreads } from './composer-controls-queue';

export type ComposerMenuRow = Omit<MenuRow, 'insert'>;
export type ComposerMenu = { open: boolean; kind: string; listLabel: string; searchKey: string; loading: boolean; emptyText: string; count: number; rows: ComposerMenuRow[] };
export type ComposerStash = StashView & { menuOpen: boolean; keyStash: string };
/** `foldLimit`: the byte limit for folding a large paste into a file (0: it cannot be staged), which the composer's `paste` reads. */
export type ComposerEditorView = { owner: string; menu: ComposerMenu; promptLimit: string; overLimit: boolean; usageLimits: number; stash: ComposerStash; foldLimit: number; drawer: ComposerDrawer };

type Search = { entries: Obj[]; failed: boolean; providers?: Obj[] };
type EditorCache = {
  discovery: WorkspaceDiscovery;
  trigger: EditorTrigger | null; dismissed: EditorTrigger | null; rows: MenuRow[]; owner: string;
  /** The search `rows` answer (ComposerPathSearchTarget): a slower search for an older query, or for another
   *  environment or workspace, can finish after a newer trigger was read. */
  rowsTarget: { query: string; environmentId: string; cwd: string } | null;
  searches: Map<string, Search>; pending: Map<string, number>; usageLimits: number;
  /** A send was refused for length: the line shows until the prompt fits. */
  limitArmed: boolean;
  stashOpen: boolean;
};
const caches = new WeakMap<T3Client, EditorCache>();
function cache(client: T3Client): EditorCache {
  let entry = caches.get(client);
  if (!entry) { entry = { discovery: new WorkspaceDiscovery(), trigger: null, dismissed: null, rows: [], rowsTarget: null, owner: '', searches: new Map(), pending: new Map(), usageLimits: 0, limitArmed: false, stashOpen: false }; caches.set(client, entry); }
  return entry;
}

const CLOSED: ComposerMenu = { open: false, kind: '', listLabel: '', searchKey: '', loading: false, emptyText: '', count: 0, rows: [] };
const NO_STASH: ComposerStash = { count: 0, pulse: 0, rows: [], menuOpen: false, keyStash: '' };
export const EDITOR_BAKED: ComposerEditorView = { owner: '', menu: CLOSED, promptLimit: '', overLimit: false, usageLimits: 0, stash: NO_STASH, foldLimit: 0, drawer: NO_DRAWER };

async function editorCall(native: Native, request: Obj): Promise<Obj> {
  const reply = await bridgeReply(native, request);
  if (!reply.ok) throw new ClientError(reply.error!.message, 'Editor');
  return obj(reply.value);
}

/** The selected provider instance's snapshot: its skills feed the `$` menu, the chip labels and a skill chip's details (composer-chip-popover.ts). */
export function selectedProvider(client: T3Client): Obj { return arr(client.config.providers).find(entry => entry.instanceId === client.providerId) ?? {}; }
function project(client: T3Client): Obj { return client.shell.projects.find(entry => entry.id === client.projectId) ?? {}; }
/** The thread's worktree, else the project root (ChatView gitCwd). */
export function workspaceCwd(client: T3Client): string {
  const thread = obj(client.projection.thread);
  return str(thread.worktreePath) || str(project(client).workspaceRoot);
}
function planModeUiEnabled(client: T3Client): boolean {
  const current = selectedProvider(client);
  return client.local.deviceSettings.planModeEnabled && !!current.instanceId && current.showInteractionModeToggle !== false;
}
function prefs(client: T3Client): Obj { return obj(client.local.clientSettings); }

/** ChatView compactThreadUnavailable, for the `/compact` row: an idle server thread with a conversation. */
function compactAvailable(client: T3Client, blankAround: boolean): boolean {
  if (!client.threadId || !blankAround || !client.thread) return false;
  const conversation = messages(client.thread).some(message => message.kind === 'user' && message.body.trim().toLowerCase() !== '/compact');
  const runs = arr(client.projection.runs).some(run => ['preparing', 'starting', 'queued', 'running', 'waiting'].includes(str(run.status)));
  const requests = arr(client.projection.runtimeRequests).some(request => request.status === 'pending');
  return conversation && !runs && !requests && client.snapshotDrafts.length === 0;
}

/** The thread records behind the prompt's thread chips (ThreadContextRecord). */
export function threadContextRecords(client: T3Client, text: string): Obj[] {
  return contextReferences(text).flatMap(reference => {
    if (reference.kind !== 'thread' || !reference.id.startsWith('thread_')) return [];
    const thread = client.shell.threads.find(entry => contextId('thread', str(entry.id)) === reference.id);
    if (!thread) return [];
    const threadId = str(thread.id);
    const label = contextLabel(reference.label, 'thread');
    return [{ version: 1, kind: 'thread', contextId: reference.id, label, environmentId: client.environmentId, threadId, title: label }];
  });
}
/** The message `context` for a send, including shelf images without inline references. */
export function messageContext(client: T3Client, text: string): Obj | undefined { return pressContext(client, text)(client.snapshotDrafts); }
/**
 * messageContext as a send's press reads it, from the draft shown then; the records of its images come from the images
 * it sends, `sent`, which may land after the press.
 */
export function pressContext(client: T3Client, text: string): (sent: Obj[]) => Obj | undefined {
  const leading = [...threadContextRecords(client, text), ...pullRequestRecords(client, text)], trailing = [...composerFileRecords(client, text), ...terminalMessageRecords(client, text)];
  return sent => {
    const images = imageContextRecords(client, sent);
    const records = [...leading, ...images, ...trailing, ...annotationMessageRecords(client, text, new Set(images.map(image => str(image.contextId).replace(/^image_/, ''))))];
    return records.length ? { version: 1, records } : undefined;
  };
}
/** Attach the prompt's context records to a dispatch or launch payload. */
export function withMessageContext(client: T3Client, payload: Obj, text: string): Obj { return withContext(payload, messageContext(client, text)); }
export function withContext(payload: Obj, context: Obj | undefined): Obj {
  if (!context) return payload;
  if (obj(payload.initialMessage).text !== undefined) return { ...payload, initialMessage: { ...obj(payload.initialMessage), context } };
  return { ...payload, context };
}

/**
 * The review-comment records behind the draft's review-comment and pull-request chips, by context id. They are saved with
 * the drafts, as composerDraftStore persists a draft's `reviewComments`, so a relaunched draft's chips still resolve.
 */
type ReviewHolder = { reviewComments?: Record<string, Obj> };
export function reviewCommentRecords(client: T3Client): Record<string, Obj> {
  const local = client.local as unknown as ReviewHolder | undefined;
  return local ? local.reviewComments ??= {} : {};
}
/** Keeps the saved records some saved draft still names (adoptTerminalContexts' shape check, the reference's per-draft lists). */
export function adoptReviewComments(next: object, saved: Obj): void {
  const named = new Set(Object.values(obj(saved.drafts)).flatMap(text => typeof text === 'string' ? contextReferences(text).filter(reference => reference.kind === 'review-comment').map(reference => reference.id) : []));
  const records: Record<string, Obj> = {};
  for (const [id, value] of Object.entries(obj(saved.reviewComments))) {
    const record = obj(value);
    if (record.kind === 'review-comment' && record.contextId === id && named.has(id) && typeof record.filePath === 'string') records[id] = record;
  }
  Object.assign(next, { reviewComments: records });
}
/** The review-comment records behind the prompt's pull-request chips. */
export function pullRequestRecords(client: T3Client, text: string): Obj[] {
  const records = reviewCommentRecords(client);
  return contextReferences(text).flatMap(reference => reference.kind === 'review-comment' && records[reference.id] ? [records[reference.id]!] : []);
}
/** Context ids the native editor draws as resolved chips, with their chip kind. */
export function chipContexts(client: T3Client, text: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const record of threadContextRecords(client, text)) out[`thread/${str(record.contextId)}`] = 'thread';
  // A line comment (diff-review.ts) draws as the review-comment chip; a pull request as its state's.
  for (const record of pullRequestRecords(client, text)) out[`review-comment/${str(record.contextId)}`] = record.pullRequest ? `pr-${pullRequestState(obj(record.pullRequest))}\t\t${str(obj(record.pullRequest).title)}` : 'review-comment';
  // A terminal reference with a record draws as its excerpt (expired when the record holds no text); one with no record is
  // composerContextPresentation's UnresolvedContextChip, as any context with none.
  for (const record of terminalDraftRecords(client, text)) out[`terminal/${str(record.contextId)}`] = `terminal\t\t${str(record.text)}`;
  return { ...out, ...fileChipContexts(client, text), ...imageChipContexts(client, text), ...annotationChipContexts(client, text) };
}

async function pathSearch(client: T3Client, native: Native, entry: EditorCache, cwd: string, query: string): Promise<Search | 'loading'> {
  const key = `${client.environmentId}\u0000${cwd}\u0000${query}`;
  const cached = entry.searches.get(key);
  if (cached) return cached;
  // PROJECT_PATH_SEARCH_DEBOUNCE_MS: the first answer shows the loading row
  // and asks the editor to re-ask after the debounce; the re-ask searches.
  if (!entry.pending.has(key)) {
    entry.pending.set(key, 1);
    await editorCall(native, { op: 'editorPoke', afterMs: 120 }).catch(error => { if (letGo(error)) throw error; return {}; });
    return 'loading';
  }
  let result: Search;
  try {
    const response = await client.rpc(native, 'projects.searchEntries', { cwd, query, limit: 80 });
    result = { entries: arr(response.entries), failed: false };
  } catch (error) { if (letGo(error)) throw error; result = { entries: [], failed: true }; }
  entry.pending.delete(key);
  entry.searches.set(key, result);
  if (entry.searches.size > 40) entry.searches.delete(entry.searches.keys().next().value!);
  return result;
}

/** One debounced (180ms, like the reference) and cached server read for the `#` menu. */
async function prRead(native: Native, entry: EditorCache, key: string, read: () => Promise<Search>): Promise<Search | 'loading'> {
  const cached = entry.searches.get(key);
  if (cached) return cached;
  if (!entry.pending.has(key)) {
    entry.pending.set(key, 1);
    await editorCall(native, { op: 'editorPoke', afterMs: 180 }).catch(() => ({}));
    return 'loading';
  }
  let result: Search;
  try { result = await read(); } catch { result = { entries: [], failed: true }; }
  entry.pending.delete(key);
  entry.searches.set(key, result);
  if (entry.searches.size > 40) entry.searches.delete(entry.searches.keys().next().value!);
  return result;
}

/**
 * The `#` menu's pull requests: the project's recent list (99 rows) with
 * text handed to the host's own search, plus an exact lookup for a typed
 * number the list does not hold (ChatComposer pullRequestLookup and
 * exactPullRequestLookup).
 */
async function prSearch(client: T3Client, native: Native, entry: EditorCache, query: string, repository: string): Promise<Search | 'loading'> {
  const text = query && !/^\d+$/.test(query) ? query.trim().slice(0, 200) : '';
  const list = await prRead(native, entry, `pr\u0000${client.projectId}\u0000${text}`, async () => {
    const response = await client.rpc(native, 'pullRequests.list', { state: 'all', projectId: client.projectId, limit: 99, ...(text ? { query: text } : {}) });
    return { entries: arr(response.entries), providers: arr(response.providers), failed: arr(response.errors).some(error => str(error.projectId) === client.projectId) };
  });
  if (list === 'loading') return list;
  const number = /^\d+$/.test(query) ? Number(query) : 0;
  if (!Number.isSafeInteger(number) || number < 1 || list.entries.some(item => Number(item.number) === number && str(item.projectId) === client.projectId)) return list;
  const exact = await prRead(native, entry, `prn\u0000${client.projectId}\u0000${repository}\u0000${number}`, async () => {
    const detail = await client.rpc(native, 'pullRequests.detail', { projectId: client.projectId, repository, number });
    return { entries: Number(detail.number) === number ? [detail] : [], failed: false };
  });
  if (exact === 'loading') return list;
  return { ...list, entries: [...exact.entries, ...list.entries] };
}

/** A v4 UUID for a staged file or stash entry (Math.random is unavailable in data sources). */
export const localId = (): string => uuid();


/** The `composerEditor` resource: the menu for the trigger at the caret, the prompt-length line, and where the drawers go. */
export async function composerEditorView(client: T3Client, native: Native | null | undefined, now = 0): Promise<ComposerEditorView> {
  preserveCitationDraft(client);
  // A queued edit whose thread left the active shell: archived or deleted (composer-controls-queue.ts).
  if (native?.available) void checkMissingEditThreads(client, native, now).catch(() => undefined);
  const view = await editorView(client, native, now);
  return { ...view, foldLimit: client.ready ? foldLimit(client) : 0, drawer: { ...composerDrawer(layoutFrames(client)), hold: composerStackHold(holds(client), client.snapshotOwner, layoutFrames(client)) } }; // composer-editor-drawer.ts
}
const stackHolds = new WeakMap<T3Client, StackHold>();
function holds(client: T3Client): StackHold {
  let hold = stackHolds.get(client);
  if (!hold) { hold = { owner: '', height: 0 }; stackHolds.set(client, hold); }
  return hold;
}
/** Root readiness resource; independent from the editor/menu resource. */
/** `shown`: whether the chat view (and so ChatComposer) is on screen; omitted, the last answer's. */
export function composerWorkspaceView(client: T3Client, shown?: boolean, now?: number) {
  return cache(client).discovery.state(client.ready ? client.environmentId : '', client.generation, selectedProvider(client), workspaceCwd(client), { prompt: client.draft, config: client.config }, shown, now);
}
/** Root mutation: await the RPC, then Contract starts its retry clock on completion. */
export async function refreshComposerWorkspace(client: T3Client, native: Native | null | undefined, key: string) {
  const { wake } = composerWorkspaceView(client); // Recheck the current environment/provider before dispatch.
  if (!native?.available || !client.ready) return { key, retry: false, wake };
  return cache(client).discovery.refresh(key, (method, payload) => client.restAccess(native).request(method, payload, true));
}
// useComposerMenuItems' `now`: thread mentions rank by recency as of when the composer mounted, here the clock when
// the draft's @ menu first listed threads.
const mentionClocks = new WeakMap<T3Client, Map<string, number>>();
function mentionClock(client: T3Client, now: number): number {
  const clocks = mentionClocks.get(client) ?? new Map<string, number>();
  mentionClocks.set(client, clocks);
  if (!clocks.has(client.draftKey)) clocks.set(client.draftKey, now);
  return clocks.get(client.draftKey)!;
}
async function editorView(client: T3Client, native: Native | null | undefined, now: number): Promise<Omit<ComposerEditorView, 'drawer' | 'foldLimit'>> {
  const entry = cache(client);
  const stash: ComposerStash = { ...stashView(client.local, now), menuOpen: entry.stashOpen && stashView(client.local, now).count > 0, keyStash: commandChord(client, 'composer.stash', 'Meta+S') };
  const limit = promptLengthMessage(client.draft);
  if (!limit) entry.limitArmed = false;
  const promptLimit = entry.limitArmed ? limit : '';
  const overLimit = limit !== '';
  const literal = composerInput(client)?.active?.initialAnswer !== undefined;
  const draft = composerInput(client)?.custom ?? client.draft, files = draftFiles(client.local);
  const fieldOwner = composerFieldOwner(client);
  const observed = composerFieldCurrent(client, fieldOwner);
  const mentioned = composerSourceSession(client);
  const source = observed?.value ?? draft;
  noteComposerFilePaths(client, (mentioned.matches(fieldOwner, source) ? mentioned.chips : composerChips(source)).flatMap(chip => chip.kind === 'mention' ? [chip.path] : []));
  if (native?.available) {
    native.watch('t3.editor');
    await refreshFileMetadata(client, native);
    // A dropped attached file's staged copy goes with it (composer-editor-attach.ts).
    const current = composerFieldCurrent(client, composerFieldOwner(client));
    for (const dropped of pruneFiles(client.local, client.local.drafts, current?.value === '' ? '' : client.draftKey, files)) {
      if (dropped.source === 'attached' && dropped.status === 'staged' && !stagedInUse(client.local, dropped.id)) await removeStaged(dropped.id);
    }
  }
  const owner = composerFieldOwner(client), field = composerFieldCurrent(client, owner), session = composerSourceSession(client);
  const candidate = field && !field.composing && field.start === field.end && !literal
    ? composerTrigger(field.value, field.start, session.matches(owner, field.value) ? [...session.chips] : composerChips(field.value),
      prefs(client).composerRichTextEnabled !== false, session.matches(owner, field.value) ? session.blockOrigins : undefined) : null;
  if (entry.owner !== owner) entry.dismissed = null;
  const trigger = candidate && entry.dismissed?.kind === candidate.kind && entry.dismissed.start === candidate.start ? null : candidate;
  if (!candidate || trigger) entry.dismissed = null;
  entry.trigger = trigger;
  entry.owner = owner;
  if (literal || !trigger || !client.ready) { entry.rows = []; return { owner: client.snapshotOwner, menu: CLOSED, promptLimit, overLimit, usageLimits: entry.usageLimits, stash }; }
  const current = selectedProvider(client);
  const cwd = workspaceCwd(client);
  const driver = str(current.driver);
  const skills = workspaceValues(current, cwd, 'skills');
  let rows: MenuRow[] = [];
  let loading = false;
  let prFailed = false;
  const capabilities = obj(obj(client.config.environment).capabilities);
  const prAvailable = capabilities.pullRequests === true && !!client.projectId && !!repositorySelector(obj(project(client).repositoryIdentity));
  if (trigger.kind === 'slash-command') {
    rows = slashRows({ query: trigger.query, atPromptStart: trigger.start === 0, planModeUiEnabled: planModeUiEnabled(client),
      compactAvailable: compactAvailable(client, !!field && !`${field.value.slice(0, trigger.start)}${field.value.slice(trigger.end)}`.trim()), driver,
      slashCommands: workspaceValues(current, cwd, 'slashCommands'),
      skills, showSkillsInSlashMenu: prefs(client).showSkillsInSlashMenu !== false });
  } else if (trigger.kind === 'skill') {
    rows = skillRows(skills, trigger.query, driver);
  } else if (trigger.kind === 'pull-request' && prAvailable && native?.available) {
    const repository = repositorySelector(obj(project(client).repositoryIdentity));
    const search = await prSearch(client, native, entry, trigger.query, repository);
    if (search === 'loading') loading = true;
    else { rows = pullRequestRows(search.entries, client.projectId, repository, trigger.query, search.providers ?? [], arr(obj(client.projection.thread).pullRequests ?? client.shell.threads.find(thread => thread.id === client.threadId)?.pullRequests)); prFailed = search.failed; }
  } else if (trigger.kind === 'path') {
    const threads = threadRows(client.shell.threads, client.threadId, trigger.query, mentionClock(client, now), client.environmentId);
    const query = trigger.query.trim();
    let entries: Obj[] = [];
    if (query && cwd && native?.available) {
      const search = await pathSearch(client, native, entry, cwd, query);
      if (search === 'loading') loading = true; else entries = search.entries;
    }
    rows = [...threads, ...pathRows(entries)];
  }
  rows = rows.map((row, index) => ({ ...row, index }));
  entry.rows = rows;
  entry.rowsTarget = { query: trigger.query.trim(), environmentId: client.environmentId, cwd };
  const searchKey = `${client.snapshotOwner}:${trigger.kind}:${trigger.query.trim().toLowerCase()}`;
  return {
    owner: client.snapshotOwner, promptLimit, overLimit, usageLimits: entry.usageLimits, stash,
    menu: { open: true, kind: trigger.kind, listLabel: LIST_LABEL[trigger.kind] ?? '', searchKey, loading: loading && rows.length === 0,
      emptyText: loading ? (trigger.kind === 'pull-request' ? 'Finding pull request...' : trigger.kind === 'skill' ? 'Searching workspace skills...' : 'Searching workspace files...') : emptyText(trigger.kind, trigger.query, prAvailable, prFailed),
      count: rows.length, rows: rows.map(({ insert: _insert, ...rest }) => rest) },
  };
}

/** `editorlocal:` commands: menu picks and chip insertion. Returns the result message. */
export async function editorLocal(client: T3Client, native: Native, op: string, id: string, value: string, n = 0): Promise<string> {
  if (op.startsWith('shelf-video-')) return videoOp(client, native, op, id); // the shelf's videos (composer-attachments.ts)
  const entry = cache(client);
  if (op === 'dismiss') { entry.dismissed = entry.trigger; entry.trigger = null; entry.rows = []; return ''; }
  // A copy or cut of the composer's selection (`id` "<start>:<end>" in `value`, the field's source).
  if (op === 'chips-copied') {
    const [start = 0, end = 0] = id.split(':').map(Number);
    if (Number.isSafeInteger(start) && Number.isSafeInteger(end) && start < end) rememberCopiedChips(client, value.slice(start, end));
    return '';
  }
  if (op === 'history') {
    const owner = composerFieldOwner(client), field = composerFieldCurrent(client, owner);
    if (!field || field.composing || composerInput(client) || field.start !== field.end || id !== 'previous' && id !== 'next') return '';
    const step = composerHistoryRecall(client, owner, field.value, id === 'previous');
    if (step) {
      try {
        const admission = await requestComposerField(native, { op: 'editorEdit', all: true, expect: field.value, text: step.prompt, history: step.receipt.history });
        if (!admission.accepted) composerHistoryComplete(client, step.receipt, false);
      } catch (error) { composerHistoryComplete(client, step.receipt, false); throw error; }
    }
    return '';
  }
  if (op === 'pick') {
    const trigger = entry.trigger;
    const row = entry.rows.find(candidate => candidate.id === id);
    if (!trigger || !row) throw new ClientError('That suggestion is no longer available.');
    // A file row answers the query it was searched for; once the field holds another, the pick is refused
    // (isPathSuggestionCurrent) rather than putting an older query's file in.
    // isPathSuggestionCurrent: the rows answer this query, in this environment and workspace.
    const target = entry.rowsTarget;
    if (trigger.kind === 'path' && (target?.query !== trigger.query.trim() || target.environmentId !== client.environmentId || target.cwd !== workspaceCwd(client))) {
      throw new ClientError('That suggestion is no longer available.');
    }
    const range = { start: trigger.start, end: trigger.end, kind: trigger.kind, ...(trigger.kind === 'path' ? { expect: `@${trigger.query}` } : {}) };
    let text = row.insert;
    let focus = true;
    // /model opens the picker, /plan and /default set the mode: key buttons the editor presses after the edit.
    let then = '';
    if (row.type === 'slash-command') { text = ''; focus = row.id !== 'slash:model'; then = row.id === 'slash:model' ? 'models' : row.id === 'slash:plan' ? 'mode-plan' : 'mode-default'; }
    const usageLimits = row.type === 'provider-slash-command' && row.id.endsWith(':usage-limits');
    if (usageLimits) { text = ''; focus = false; }
    if (row.type === 'pull-request') {
      const record = pullRequestRecord(JSON.parse(row.insert));
      reviewCommentRecords(client)[str(record.contextId)] = record;
      text = `${contextLink('review-comment', str(record.contextId), str(record.label))} `;
    }
    if (row.type === 'thread') {
      const threadId = row.id.slice('thread:'.length);
      text = `${contextLink('thread', contextId('thread', threadId), row.label)} `;
    }
    // Everything the pick changes happens before the edit: the edit's own
    // input supersedes this command, so nothing after its reply is certain to run.
    entry.rows = []; entry.trigger = null;
    // /usage-limits opens the composer's Usage limits banner (composer-controls-usage.ts).
    if (usageLimits) { entry.usageLimits += 1; openUsageLimits(client, n || 0); }
    const result = await requestComposerField(native, { op: 'editorEdit', ...range, text, focus, then, extendSpace: text.endsWith(' ') });
    if (!result.accepted) throw new ClientError('The prompt changed before the suggestion was applied.');
    return '';
  }
  if (op === 'insert-context') return insertContext(client, native, id, value);
  if (op === 'arm-limit') { entry.limitArmed = promptLengthMessage(client.draft) !== ''; return ''; }
  if (op === 'stash') {
    // The prompt as the editor holds it: ⌘S right after typing can beat the draft's own write.
    const live = await editorCall(native, { op: 'editorState' }).then(state => typeof state.text === 'string' ? state.text : client.draft, () => client.draft);
    const id = `stash-${localId()}`, now = new Date(n || 0);
    if (live.trim()) {
      await replaceAll(client, native, '', { applied: async () => {
        const result = stashPrompt({ local: client.local, draft: live, environmentId: client.environmentId }, id, now);
        if (result.evicted) pushToast(client, { kind: 'warning', title: 'Oldest stashed prompt discarded', description: `The stash holds ${MAX_STASH_ENTRIES} prompts; the oldest was removed to make room.`, hideCopy: true });
        await client.persist(preferenceFiles());
      } }, live);
    } else if (stashEntries(client.local).length === 1) await restoreStash(client, native, stashEntries(client.local)[0]!.id);
    else entry.stashOpen = !entry.stashOpen;
    return '';
  }
  if (op === 'stash-restore') { await restoreStash(client, native, id); return ''; }
  if (op === 'stash-delete') { deleteStashEntry(client.local, id); return ''; }
  if (op === 'stash-menu') { entry.stashOpen = value === 'toggle' ? !entry.stashOpen : value === 'open'; return ''; }
  throw new ClientError(`Unknown editor action: ${op}`);
}

/** Replace the whole prompt as one undoable edit, caret at the end (stash, restore). */
async function replaceAll(client: T3Client, native: Native, text: string, effects: ComposerFieldCompletion = {}, expect = composerFieldCurrent(client, composerFieldOwner(client))?.value): Promise<void> {
  const result = await requestComposerField(native, { op: 'editorEdit', all: true, text, ...(expect !== undefined ? { expect } : {}) }, effects);
  if (!result.accepted) throw new ClientError('The prompt could not be updated.');
}
async function restoreStash(client: T3Client, native: Native, id: string): Promise<void> {
  cache(client).stashOpen = false;
  const entry = stashEntries(client.local).find(item => item.id === id);
  if (!entry) throw new ClientError('That stashed prompt is no longer available.');
  const field = composerFieldCurrent(client, composerFieldOwner(client));
  if (!field) throw new ClientError('The composer is not ready.');
  const current = field.value;
  const prompt = !entry.prompt ? current : current.trim() ? `${current.replace(/\s+$/, '')}\n\n${entry.prompt}` : entry.prompt;
  await replaceAll(client, native, prompt, { applied: async () => {
    if (stashEntries(client.local).find(item => item.id === id) !== entry) return;
    deleteStashEntry(client.local, id);
    await client.persist(preferenceFiles());
  } }, current);
}
/** The chord a keybinding command currently resolves to, as aria-keyshortcuts. */
function commandChord(client: T3Client, command: string, fallback: string): string {
  return commandChords(client.config, command, fallback, false, { terminalOpen: terminalOpen(client) });
}

/** addReviewComment (diff-review.ts): the line comment's record goes behind a chip inserted at the caret. */
export async function addReviewCommentChip(client: T3Client, native: Native, record: Obj, effects: ComposerFieldCompletion = {}): Promise<void> {
  reviewCommentRecords(client)[str(record.contextId)] = record;
  const result = await requestComposerField(native, { op: 'editorInsert', text: `${contextLink('review-comment', str(record.contextId), str(record.label))} ` }, effects);
  if (!result.accepted) throw new ClientError('The composer is not ready');
}
/** A pull request hand-off's chips (pages-pr-handoffs.ts) resolve to these records when sent. */
export function rememberReviewCommentRecord(client: T3Client, record: Obj): void { reviewCommentRecords(client)[str(record.contextId)] = record; }
/** A hand-off into the composer on screen is one undoable edit of the whole prompt. */
export async function replaceComposerPrompt(client: T3Client, native: Native, text: string, effects: ComposerFieldCompletion = {}, expect?: string): Promise<void> { await replaceAll(client, native, text, effects, expect); }
/** removeReviewComment: the chip leaves the prompt as one undoable edit, and its record goes. */
export async function removeReviewCommentChip(client: T3Client, native: Native, contextId: string, effects: ComposerFieldCompletion = {}): Promise<void> {
  const href = `t3-context://v1/review-comment/${contextId.replace(/[^a-z0-9_-]/gi, '')}`;
  // Each link to it goes, with the space after it.
  const withoutChip = (value: string) => replaceLinks(value, contextLinks(value).filter(link => link.href === href).map(link => ({ ...link, end: value[link.end] === ' ' ? link.end + 1 : link.end })), () => '');
  const field = composerFieldCurrent(client, composerFieldOwner(client));
  if (!field) throw new ClientError('The composer is not ready.');
  const next = withoutChip(field.value);
  const applied = async (current: Native) => {
    const latest = composerFieldCurrent(client, composerFieldOwner(client));
    if (!latest || withoutChip(latest.value) !== latest.value) throw new ClientError('The review comment is still referenced by the prompt.');
    delete reviewCommentRecords(client)[contextId]; await effects.applied?.(current);
  };
  if (next !== field.value) await replaceAll(client, native, next, { ...effects, applied }, field.value);
  else await applied(native);
}

/**
 * Insert a context chip at the composer's caret (insertInlineContextReference),
 * for other surfaces: `thread` (id = thread id), `file` / `folder` (id = path
 * relative to the workspace, inserted as an @mention link), `citation` (id =
 * the quote's t3-citation:// href). The `editorlocal:insert-context` command
 * (id = kind, value = target) reaches it from the Contract.
 */
export async function insertContext(client: T3Client, native: Native, kind: string, target: string): Promise<string> {
  let text = '';
  if (kind === 'thread') {
    const ref = parseFleetThreadId(target);
    if (ref && ref.environmentId !== client.environmentId) { text = await importDroppedThread(client, native, target); if (!text) return ''; }
    else {
      const id = ref?.threadId ?? target, thread = client.shell.threads.find(entry => str(entry.id) === id);
      if (!thread) throw new ClientError('That thread is no longer available.');
      text = contextLink('thread', contextId('thread', id), str(thread.title, 'Thread'));
    }
  } else if (kind === 'threads') {
    const targets: unknown = JSON.parse(target);
    if (!Array.isArray(targets) || targets.length > 100 || !targets.every(item => typeof item === 'string')) throw new ClientError('Unable to add to chat');
    const links: string[] = [];
    for (const item of targets) {
      const ref = parseFleetThreadId(item);
      if (ref && ref.environmentId !== client.environmentId) { const imported = await importDroppedThread(client, native, item); if (imported) links.push(imported); }
      else { const thread = client.shell.threads.find(entry => str(entry.id) === (ref?.threadId ?? item)); if (!thread) throw new ClientError('That thread is no longer available.'); links.push(contextLink('thread', contextId('thread', ref?.threadId ?? item), str(thread.title, 'Thread'))); }
    }
    text = links.join(' ');
    if (!text) return '';
  } else if (kind === 'file' || kind === 'folder') {
    const path = target.replace(/\/+$/, '');
    if (!path) throw new ClientError('Unable to add to chat');
    text = kind === 'folder' ? `@${/\s/.test(path) ? `"${path.split('\\').join('\\\\').split('"').join('\\"')}"` : path}` : fileLink(path);
  } else if (kind === 'terminal') {
    let decoded: unknown;
    try { decoded = JSON.parse(target); } catch { throw new ClientError('Unable to add to chat'); }
    const context = parseTerminalContext(decoded);
    if (!context || !context.text) throw new ClientError('Unable to add to chat');
    const record = terminalDraftRecords(client, client.draft).find(record => record.terminalId === context.terminalId && record.lineStart === context.lineStart && record.lineEnd === context.lineEnd);
    if (record) return '';
    saveTerminalContext(client, context);
    text = formatTerminalContextReference(context);
  } else if (kind === 'pull-request') {
    const parsed = obj(JSON.parse(target));
    if (num(parsed.number) <= 0 || !/^https?:\/\//.test(str(parsed.url))) throw new ClientError('Unable to add to chat');
    const record = pullRequestRecord({ number: num(parsed.number), title: str(parsed.title), url: str(parsed.url), headBranch: str(parsed.headBranch), baseBranch: str(parsed.baseBranch), state: str(parsed.state, 'open'), isDraft: parsed.isDraft === true });
    reviewCommentRecords(client)[str(record.contextId)] = record;
    text = `${contextLink('review-comment', str(record.contextId), str(record.label))} `;
  } else if (kind === 'citation') {
    // An assistant quote (AssistantCitationChip): `target` is its t3-citation:// href.
    if (!/^t3-citation:\/\/v1\/[^\s)]+$/.test(target)) throw new ClientError('Unable to add to chat');
    text = `[Assistant quote](${target})`;
  } else throw new ClientError('Unable to add to chat');
  const result = await requestComposerField(native, { op: 'editorInsert', text });
  if (!result.accepted) throw new ClientError('Unable to add to chat');
  return '';
}
