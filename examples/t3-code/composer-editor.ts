// The prompt editor's projection and commands. The native editor
// (T3ComposerEditor.swift) reports the trigger at the caret; this builds the
// command menu's rows (composer-editor-menu.ts), applies a picked row back
// into the text view, keeps the editor's history and chip records in sync,
// and builds the send's context records. Contract draws composer-editor.contract.
import type { T3Client } from './client';
import { bridgeReply, ClientError, type Native } from './protocol';
import { arr, messages, obj, str, type Obj } from './domain';
import { pushToast } from './toast';
import { commandChords } from './composer-presentation';
import { openUsageLimits } from './composer-controls-usage';
import { MAX_STASH_ENTRIES, deleteStashEntry, stashPrompt, stashView, takeStashEntry, type StashView } from './composer-editor-stash';
import {
  LIST_LABEL, contextId, contextLabel, contextLink, contextReferences, emptyText, fileLink, historyEntries, pathRows, promptLengthMessage,
  pullRequestRecord, pullRequestRows, pullRequestState, repositorySelector,
  skillRows, slashRows, threadRows, type EditorTrigger, type MenuRow,
} from './composer-editor-menu';
import { composerFileRecords, fileChipContexts, foldLimit, pruneFiles, takeFold } from './composer-editor-files';
import { imageChipContexts, imageContextRecords } from './composer-editor-attach';
import { composerDrawer, composerStackHold, NO_DRAWER, type ComposerDrawer, type StackHold } from './composer-editor-drawer';
import { WorkspaceDiscovery, workspaceValues } from './composer-workspace-snapshots';
import { videoOp } from './r4-composer-attachments';

export type ComposerMenuRow = Omit<MenuRow, 'insert'>;
export type ComposerMenu = { open: boolean; kind: string; listLabel: string; searchKey: string; loading: boolean; emptyText: string; count: number; rows: ComposerMenuRow[] };
export type ComposerStash = StashView & { menuOpen: boolean; keyStash: string };
export type ComposerEditorView = { owner: string; menu: ComposerMenu; promptLimit: string; overLimit: boolean; usageLimits: number; stash: ComposerStash; drawer: ComposerDrawer };

type Search = { entries: Obj[]; failed: boolean; providers?: Obj[] };
type EditorCache = {
  discovery: WorkspaceDiscovery;
  trigger: EditorTrigger | null; rows: MenuRow[]; owner: string;
  searches: Map<string, Search>; pending: Map<string, number>; usageLimits: number;
  /** A send was refused for length: the line shows until the prompt fits. */
  limitArmed: boolean;
  stashOpen: boolean;
  /** Review-comment records behind the draft's pull-request chips, by context id. */
  prRecords: Map<string, Obj>;
};
const caches = new WeakMap<T3Client, EditorCache>();
function cache(client: T3Client): EditorCache {
  let entry = caches.get(client);
  if (!entry) { entry = { discovery: new WorkspaceDiscovery(), trigger: null, rows: [], owner: '', searches: new Map(), pending: new Map(), usageLimits: 0, limitArmed: false, stashOpen: false, prRecords: new Map() }; caches.set(client, entry); }
  return entry;
}

const CLOSED: ComposerMenu = { open: false, kind: '', listLabel: '', searchKey: '', loading: false, emptyText: '', count: 0, rows: [] };
const NO_STASH: ComposerStash = { count: 0, pulse: 0, rows: [], menuOpen: false, keyStash: '' };
export const EDITOR_BAKED: ComposerEditorView = { owner: '', menu: CLOSED, promptLimit: '', overLimit: false, usageLimits: 0, stash: NO_STASH, drawer: NO_DRAWER };

async function editorCall(native: Native, request: Obj): Promise<Obj> {
  const reply = await bridgeReply(native, request);
  if (!reply.ok) throw new ClientError(reply.error!.message, 'Editor');
  return obj(reply.value);
}

function provider(client: T3Client): Obj { return arr(client.config.providers).find(entry => entry.instanceId === client.providerId) ?? {}; }
function project(client: T3Client): Obj { return client.shell.projects.find(entry => entry.id === client.projectId) ?? {}; }
/** The thread's worktree, else the project root (ChatView gitCwd). */
export function workspaceCwd(client: T3Client): string {
  const thread = obj(client.projection.thread);
  return str(thread.worktreePath) || str(project(client).workspaceRoot);
}
function planModeUiEnabled(client: T3Client): boolean {
  const current = provider(client);
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
    const threadId = reference.id.slice('thread_'.length);
    const thread = client.shell.threads.find(entry => str(entry.id) === threadId);
    if (!thread) return [];
    const label = contextLabel(reference.label, 'thread');
    return [{ version: 1, kind: 'thread', contextId: reference.id, label, environmentId: client.environmentId, threadId, title: label }];
  });
}
/** The message `context` for a send, or undefined when the prompt references none. */
export function messageContext(client: T3Client, text: string): Obj | undefined {
  const records = [...threadContextRecords(client, text), ...pullRequestRecords(client, text), ...composerFileRecords(client, text), ...imageContextRecords(client, text)];
  return records.length ? { version: 1, records } : undefined;
}
/** Attach the prompt's context records to a dispatch or launch payload. */
export function withMessageContext(client: T3Client, payload: Obj, text: string): Obj {
  const context = messageContext(client, text);
  if (!context) return payload;
  if (obj(payload.initialMessage).text !== undefined) return { ...payload, initialMessage: { ...obj(payload.initialMessage), context } };
  return { ...payload, context };
}

/** The review-comment records behind the prompt's pull-request chips. */
export function pullRequestRecords(client: T3Client, text: string): Obj[] {
  const records = cache(client).prRecords;
  return contextReferences(text).flatMap(reference => reference.kind === 'review-comment' && records.has(reference.id) ? [records.get(reference.id)!] : []);
}
/** Context ids the native editor draws as resolved chips, with their chip kind. */
function chipContexts(client: T3Client, text: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const record of threadContextRecords(client, text)) out[`thread/${str(record.contextId)}`] = 'thread';
  // A line comment (diff-review.ts) draws as the review-comment chip; a pull request as its state's.
  for (const record of pullRequestRecords(client, text)) out[`review-comment/${str(record.contextId)}`] = record.pullRequest ? `pr-${pullRequestState(obj(record.pullRequest))}` : 'review-comment';
  return { ...out, ...fileChipContexts(client, text), ...imageChipContexts(client, text) };
}

async function pathSearch(client: T3Client, native: Native, entry: EditorCache, cwd: string, query: string): Promise<Search | 'loading'> {
  const key = `${cwd}\u0000${query}`;
  const cached = entry.searches.get(key);
  if (cached) return cached;
  // PROJECT_PATH_SEARCH_DEBOUNCE_MS: the first answer shows the loading row
  // and asks the editor to re-ask after the debounce; the re-ask searches.
  if (!entry.pending.has(key)) {
    entry.pending.set(key, 1);
    await editorCall(native, { op: 'editorPoke', afterMs: 120 }).catch(() => ({}));
    return 'loading';
  }
  let result: Search;
  try {
    const response = await client.rpc(native, 'projects.searchEntries', { cwd, query, limit: 80 });
    result = { entries: arr(response.entries), failed: false };
  } catch { result = { entries: [], failed: true }; }
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
export function localId(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  bytes[6] = (bytes[6]! & 0x0f) | 0x40; bytes[8] = (bytes[8]! & 0x3f) | 0x80;
  return Array.from(bytes, (byte, index) => `${[4, 6, 8, 10].includes(index) ? '-' : ''}${byte.toString(16).padStart(2, '0')}`).join('');
}

/** folderDropTarget: Finder folders only reach a server on this Mac. */
export function isLoopback(origin: string): boolean {
  try { return ['127.0.0.1', 'localhost', '[::1]', '::1'].includes(new URL(origin).hostname); } catch { return false; }
}

/** The `composerEditor` resource: the menu for the trigger at the caret, the prompt-length line, and where the drawers go. */
export async function composerEditorView(client: T3Client, native: Native | null | undefined, now = 0): Promise<ComposerEditorView> {
  const view = await editorView(client, native, now);
  return { ...view, drawer: { ...composerDrawer(client.presentation), hold: composerStackHold(holds(client), client.snapshotOwner, client.presentation) } }; // composer-editor-drawer.ts
}
const stackHolds = new WeakMap<T3Client, StackHold>();
function holds(client: T3Client): StackHold {
  let hold = stackHolds.get(client);
  if (!hold) { hold = { owner: '', height: 0 }; stackHolds.set(client, hold); }
  return hold;
}
/** Root readiness resource; independent from the editor/menu resource. */
export function composerWorkspaceView(client: T3Client) {
  return cache(client).discovery.state(client.ready ? client.environmentId : '', client.generation, provider(client), workspaceCwd(client), { prompt: client.draft, config: client.config });
}
/** Root mutation: await the RPC, then Contract starts its retry clock on completion. */
export async function refreshComposerWorkspace(client: T3Client, native: Native | null | undefined, key: string) {
  const { wake } = composerWorkspaceView(client); // Recheck the current environment/provider before dispatch.
  if (!native?.available || !client.ready) return { key, retry: false, wake };
  return cache(client).discovery.refresh(key, (method, payload) => client.restAccess(native).request(method, payload, true));
}
async function editorView(client: T3Client, native: Native | null | undefined, now: number): Promise<Omit<ComposerEditorView, 'drawer'>> {
  const entry = cache(client);
  const stash: ComposerStash = { ...stashView(client.local, now), menuOpen: entry.stashOpen && stashView(client.local, now).count > 0, keyStash: commandChord(client, 'composer.stash', 'Meta+S') };
  const limit = promptLengthMessage(client.draft);
  if (!limit) entry.limitArmed = false;
  const promptLimit = entry.limitArmed ? limit : '';
  const overLimit = limit !== '';
  if (!native?.available) return { ...EDITOR_BAKED, promptLimit, overLimit, stash };
  native.watch('t3.editor');
  const draft = client.draft;
  let state: Obj;
  try {
    state = await editorCall(native, { op: 'editorSync', owner: client.draftKey, richText: prefs(client).composerRichTextEnabled !== false, localEnvironment: isLoopback(client.origin),
      history: client.thread ? historyEntries(messages(client.thread)) : [], contexts: chipContexts(client, draft), foldLimit: client.ready ? foldLimit(client) : 0 });
  } catch { return { owner: client.snapshotOwner, menu: CLOSED, promptLimit, overLimit, usageLimits: entry.usageLimits, stash }; }
  for (const notice of arr(state.notices)) pushToast(client, { kind: str(notice.kind) === 'error' ? 'error' : 'info', title: str(notice.title), description: str(notice.description), hideCopy: true });
  // Large pastes the editor held back become pasted-text.txt chips (composer-editor-files.ts).
  for (const fold of arr(state.folds)) await takeFold(client, native, fold, localId()).catch(() => undefined);
  // A dropped attached file's staged copy goes with it (composer-editor-attach.ts).
  for (const dropped of pruneFiles(client.local, client.local.drafts, state.empty === true ? '' : client.draftKey)) {
    if (dropped.source === 'attached' && dropped.status === 'staged') await bridgeReply(native, { op: 'composerAttachRemove', id: dropped.id }).catch(() => undefined);
  }
  const raw = obj(state.trigger);
  const trigger: EditorTrigger | null = str(raw.kind) && str(state.owner) === client.snapshotOwner
    ? { kind: str(raw.kind), query: str(raw.query), start: Number(raw.start) || 0, end: Number(raw.end) || 0 } : null;
  entry.trigger = trigger;
  entry.owner = client.snapshotOwner;
  if (!trigger || !client.ready) { entry.rows = []; return { owner: client.snapshotOwner, menu: CLOSED, promptLimit, overLimit, usageLimits: entry.usageLimits, stash }; }
  const current = provider(client);
  const cwd = workspaceCwd(client);
  const driver = str(current.driver);
  const skills = workspaceValues(current, cwd, 'skills');
  let rows: MenuRow[] = [];
  let loading = false;
  let prFailed = false;
  const capabilities = obj(obj(client.config.environment).capabilities);
  const prAvailable = capabilities.pullRequests === true && !!client.projectId && !!repositorySelector(obj(project(client).repositoryIdentity));
  if (trigger.kind === 'slash-command') {
    rows = slashRows({ query: trigger.query, atPromptStart: state.atStart === true, planModeUiEnabled: planModeUiEnabled(client),
      compactAvailable: compactAvailable(client, state.blankAround === true), driver,
      slashCommands: workspaceValues(current, cwd, 'slashCommands'),
      skills, showSkillsInSlashMenu: prefs(client).showSkillsInSlashMenu !== false });
  } else if (trigger.kind === 'skill') {
    rows = skillRows(skills, trigger.query, driver);
  } else if (trigger.kind === 'pull-request' && prAvailable) {
    const repository = repositorySelector(obj(project(client).repositoryIdentity));
    const search = await prSearch(client, native, entry, trigger.query, repository);
    if (search === 'loading') loading = true;
    else { rows = pullRequestRows(search.entries, client.projectId, repository, trigger.query, search.providers ?? []); prFailed = search.failed; }
  } else if (trigger.kind === 'path') {
    const threads = threadRows(client.shell.threads, client.threadId, trigger.query);
    const query = trigger.query.trim();
    let entries: Obj[] = [];
    if (query && cwd) {
      const search = await pathSearch(client, native, entry, cwd, query);
      if (search === 'loading') loading = true; else entries = search.entries;
    }
    rows = [...threads, ...pathRows(entries)];
  }
  rows = rows.map((row, index) => ({ ...row, index }));
  entry.rows = rows;
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
  if (op.startsWith('r4c-video-')) return videoOp(client, native, op, id); // the shelf's videos (r4-composer-attachments.ts)
  const entry = cache(client);
  if (op === 'pick') {
    const trigger = entry.trigger;
    const row = entry.rows.find(candidate => candidate.id === id);
    if (!trigger || !row) throw new ClientError('That suggestion is no longer available.');
    const range = { start: trigger.start, end: trigger.end, kind: trigger.kind };
    let text = row.insert;
    let focus = true;
    // /model opens the picker, /plan and /default set the mode: key buttons the editor presses after the edit.
    let then = '';
    if (row.type === 'slash-command') { text = ''; focus = row.id !== 'slash:model'; then = row.id === 'slash:model' ? 'models' : row.id === 'slash:plan' ? 'mode-plan' : 'mode-default'; }
    const usageLimits = row.type === 'provider-slash-command' && row.id.endsWith(':usage-limits');
    if (usageLimits) { text = ''; focus = false; }
    if (row.type === 'pull-request') {
      const record = pullRequestRecord(JSON.parse(row.insert));
      entry.prRecords.set(str(record.contextId), record);
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
    const result = await editorCall(native, { op: 'editorEdit', ...range, text, focus, then, extendSpace: text.endsWith(' ') });
    if (result.applied !== true) throw new ClientError('The prompt changed before the suggestion was applied.');
    return '';
  }
  if (op === 'insert-context') return insertContext(client, native, id, value);
  if (op === 'arm-limit') { entry.limitArmed = promptLengthMessage(client.draft) !== ''; return ''; }
  if (op === 'stash') {
    // The prompt as the editor holds it: ⌘S right after typing can beat the draft's own write.
    const live = await editorCall(native, { op: 'editorState' }).then(state => typeof state.text === 'string' ? state.text : client.draft, () => client.draft);
    const host = { local: client.local, draft: live, environmentId: client.environmentId };
    const result = stashPrompt(host, `stash-${localId()}`, new Date(n || 0));
    if (result.clear) {
      if (result.evicted) pushToast(client, { kind: 'warning', title: 'Oldest stashed prompt discarded',
        description: `The stash holds ${MAX_STASH_ENTRIES} prompts; the oldest was removed to make room.`, hideCopy: true });
      await replaceAll(client, native, '');
    } else if (result.restore) await restoreStash(client, native, result.restore.id);
    else entry.stashOpen = !entry.stashOpen;
    return '';
  }
  if (op === 'stash-restore') { await restoreStash(client, native, id); return ''; }
  if (op === 'stash-delete') { deleteStashEntry(client.local, id); return ''; }
  if (op === 'stash-menu') { entry.stashOpen = value === 'toggle' ? !entry.stashOpen : value === 'open'; return ''; }
  throw new ClientError(`Unknown editor action: ${op}`);
}

/** Replace the whole prompt as one undoable edit, caret at the end (stash, restore). */
async function replaceAll(client: T3Client, native: Native, text: string): Promise<void> {
  client.local.drafts[client.draftKey] = text; // first: the edit's own input supersedes this command
  const result = await editorCall(native, { op: 'editorEdit', all: true, text });
  if (result.applied !== true) throw new ClientError('The prompt could not be updated.');
}
async function restoreStash(client: T3Client, native: Native, id: string): Promise<void> {
  cache(client).stashOpen = false;
  const taken = takeStashEntry({ local: client.local, draft: client.draft, environmentId: client.environmentId }, id);
  if (!taken) throw new ClientError('That stashed prompt is no longer available.');
  await replaceAll(client, native, taken.prompt);
}
/** The chord a keybinding command currently resolves to, as aria-keyshortcuts. */
function commandChord(client: T3Client, command: string, fallback: string): string {
  return commandChords(client.config, command, fallback);
}

/** addReviewComment (diff-review.ts): the line comment's record goes behind a chip inserted at the caret. */
export async function addReviewCommentChip(client: T3Client, native: Native, record: Obj): Promise<void> {
  cache(client).prRecords.set(str(record.contextId), record);
  const result = await editorCall(native, { op: 'editorInsert', text: `${contextLink('review-comment', str(record.contextId), str(record.label))} ` });
  if (result.applied !== true) throw new ClientError('The composer is not ready');
}
/** removeReviewComment: the chip leaves the prompt as one undoable edit, and its record goes. */
export async function removeReviewCommentChip(client: T3Client, native: Native, contextId: string): Promise<void> {
  const link = new RegExp(`\\[[^\\]\\n]{0,512}\\]\\(t3-context://v1/review-comment/${contextId.replace(/[^a-z0-9_-]/gi, '')}\\) ?`, 'g');
  cache(client).prRecords.delete(contextId);
  const next = client.draft.replace(link, '');
  if (next !== client.draft) await replaceAll(client, native, next);
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
    const thread = client.shell.threads.find(entry => str(entry.id) === target);
    if (!thread) throw new ClientError('Use threads from this environment');
    text = contextLink('thread', contextId('thread', target), str(thread.title, 'Thread'));
  } else if (kind === 'file' || kind === 'folder') {
    const path = target.replace(/\/+$/, '');
    if (!path) throw new ClientError('Unable to add to chat');
    text = kind === 'folder' ? `@${/\s/.test(path) ? `"${path.split('\\').join('\\\\').split('"').join('\\"')}"` : path}` : fileLink(path);
  } else if (kind === 'citation') {
    // An assistant quote (AssistantCitationChip): `target` is its t3-citation:// href.
    if (!/^t3-citation:\/\/v1\/[^\s)]+$/.test(target)) throw new ClientError('Unable to add to chat');
    text = `[Assistant quote](${target})`;
  } else throw new ClientError('Unable to add to chat');
  const result = await editorCall(native, { op: 'editorInsert', text });
  if (result.applied !== true) throw new ClientError('Unable to add to chat');
  return '';
}
