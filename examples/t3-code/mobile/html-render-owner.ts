// App-owned HTML render demand, over the pinned metadata reader and existing asset RPC.
// @ref llp/1109.005-composer-and-transcript.decision.md#work-log-detail-rows
import type { T3Client } from './shared/client';
import { arr, obj, str, type Obj } from './shared/domain';
import { ClientError, type Native } from './shared/protocol';
import { letGo } from './shared/let-go';
import { mobileMediaURL } from './media-preview';
import { htmlRenderFileName, htmlRenderFrameHeight, htmlRenderFromToolItem,
  htmlRenderReferencesEqual, type HtmlRenderReference } from './html-render';

export interface HtmlRenderTheme { appearance: 'light' | 'dark'; variables: Readonly<Record<string, string>> }
export interface HtmlRenderDocumentSource { identifier: string; name: string; kind: string; sourceJSON: string; ready: boolean; error: string }
export interface HtmlRenderSnapshot { context: string; key: string; identifier: string; sourceJSON: string; title: string; height: number; available: boolean }
export interface HtmlRenderEventResult { context: string; identifier: string; revision: number; openKey: string; message: string }
export interface HtmlRenderContext { context: string; requestRoute: string; key: string; attachmentId: string; full: boolean }
interface Selection { key: string; reference: HtmlRenderReference }
interface Entry extends Selection {
  storageKey: string; identifier: string; mountId: string; attempt: number; serial: number; nested: boolean;
  phase: 'pending' | 'ready' | 'failed'; url: string; pending: boolean; retried: boolean; width: number;
  retiredMounts: Set<string>;
}
interface State { owner: string; context: string; visit: string; active: boolean; revision: number; entries: Map<string, Entry> }
interface Owners { owner: string; sequence: number; inline: State; document?: State }
const states = new WeakMap<T3Client, Owners>();
const owner = (client: T3Client) => JSON.stringify([client.origin, client.environmentId,
  client.generation, client.threadId, client.threadEpoch]);

/** The selected thread owns presentation; the source tuple may name an inherited item. */
export function mobileHtmlRenderSourceKey(row: Obj): string {
  return typeof row.sourceThreadId === 'string' && row.sourceThreadId !== ''
    && typeof row.sourceItemId === 'string' && row.sourceItemId !== ''
    && htmlRenderFromToolItem(obj(row.item)) !== undefined
    ? JSON.stringify([str(row.visibility), row.sourceThreadId, row.sourceItemId]) : '';
}
export function mobileHtmlRenderSelection(client: T3Client, key: string): Selection | undefined {
  let tuple: unknown;
  try { tuple = JSON.parse(key); } catch { return; }
  if (!Array.isArray(tuple) || tuple.length !== 3 || tuple.some(part => typeof part !== 'string')) return;
  const row = arr(client.projection.visibleTurnItems).find(row => str(row.visibility) === tuple[0]
    && row.sourceThreadId === tuple[1] && row.sourceItemId === tuple[2]);
  const reference = row && htmlRenderFromToolItem(obj(row.item));
  return reference ? { key, reference } : undefined;
}

const identifier = (context: string, selection: Selection, nested: boolean) => JSON.stringify([context, selection.key, selection.reference, nested]);
const newState = (currentOwner: string): State => ({ owner: currentOwner, context: '', visit: '', active: false, revision: 0, entries: new Map() });
function observed(client: T3Client): Owners {
  const currentOwner = owner(client), previous = states.get(client);
  const owners: Owners = previous && previous.owner === currentOwner ? previous
    : { owner: currentOwner, sequence: previous?.sequence ?? 0, inline: newState(currentOwner) };
  states.set(client, owners);
  for (const state of [owners.inline, owners.document]) if (state) {
    for (const [key, entry] of state.entries) {
      const selected = mobileHtmlRenderSelection(client, entry.key);
      if (!selected || !htmlRenderReferencesEqual(selected.reference, entry.reference)) state.entries.delete(key);
    }
  }
  return owners;
}
function deactivate(state: State | undefined): void {
  if (!state?.active) return;
  state.active = false; state.visit = '';
  for (const entry of state.entries.values()) {
    if (entry.mountId) entry.retiredMounts.add(entry.mountId);
    entry.mountId = ''; entry.serial++; entry.pending = false;
  }
}
function activate(owners: Owners, state: State, visit: string): string {
  if (state.active && state.visit === visit) return state.context;
  deactivate(state);
  state.context = JSON.stringify([owners.owner, visit, ++owners.sequence]);
  state.visit = visit; state.active = true;
  // Content survives a covering route, but no earlier native admission does.
  for (const entry of state.entries.values()) {
    entry.identifier = identifier(state.context, entry, entry.nested);
    entry.retiredMounts.clear();
  }
  return state.context;
}

/** One selected thread retains inline content; route admission never outlives its visit. */
export function mobileHtmlRenderObserve(client: T3Client, visit: string, active: boolean): string {
  const owners = observed(client);
  deactivate(owners.document); owners.document = undefined;
  if (active && visit) return activate(owners, owners.inline, visit);
  deactivate(owners.inline); return '';
}

/** Full-page ownership is separate from the selected thread's retained inline content. */
export function mobileHtmlRenderContext(client: T3Client, route: string, visit: string, environmentId: string,
  threadId: string, key: string, attachmentId: string, retry: number): HtmlRenderContext {
  const full = route === 'threadMedia' && key !== '';
  const selection = full ? mobileHtmlRenderSelection(client, key) : undefined;
  const matched = environmentId !== '' && threadId !== ''
    && environmentId === client.environmentId && threadId === client.threadId;
  const owners = observed(client);
  let context = '';
  if (matched && route === 'thread' && visit) {
    deactivate(owners.document); owners.document = undefined;
    context = activate(owners, owners.inline, visit);
  } else {
    deactivate(owners.inline);
    const documentVisit = `${visit}:${retry}`;
    if (matched && full && visit && selection?.reference.attachmentId === attachmentId) {
      if (!owners.document || owners.document.visit !== documentVisit) {
        deactivate(owners.document); owners.document = newState(owners.owner);
      }
      context = activate(owners, owners.document, documentVisit);
    } else { deactivate(owners.document); owners.document = undefined; }
  }
  return { context, requestRoute: visit, key: full ? key : '', attachmentId: full ? attachmentId : '', full };
}

function selectedOwner(client: T3Client, context: string, key: string, nested: boolean) {
  const owners = states.get(client), state = nested ? owners?.inline : owners?.document;
  const selection = mobileHtmlRenderSelection(client, key);
  if (!context || !state?.active || state.context !== context || state.owner !== owner(client) || !selection) return;
  return { state, selection, storageKey: JSON.stringify([key, selection.reference]), identifier: identifier(context, selection, nested) };
}

/** Pure per-row projection. A thousand metadata rows mint zero URLs until native asks to mount. */
export function mobileHtmlRenderSnapshot(client: T3Client, context: string, key: string,
  nested: boolean, width: number, theme: HtmlRenderTheme, iconColor: string): HtmlRenderSnapshot {
  const selected = selectedOwner(client, context, key, nested);
  if (!selected) return { context: '', key: '', identifier: '', sourceJSON: '', title: '', height: 80, available: false };
  const { state, selection, storageKey, identifier } = selected, entry = state.entries.get(storageKey);
  return { context, key, identifier, title: selection.reference.title, available: true,
    height: htmlRenderFrameHeight(selection.reference, entry?.width || width),
    sourceJSON: JSON.stringify({ mode: 'html-render', identifier, active: true,
      title: nested ? selection.reference.title : htmlRenderFileName(selection.reference.title),
      nested, theme, iconColor, attempt: entry?.attempt ?? 0, phase: entry?.phase ?? 'pending',
      url: entry?.phase === 'ready' ? entry.url : '' }) };
}

/** The root checks this captured page identity immediately before navigating. */
export function mobileHtmlRenderCanOpen(client: T3Client, context: string, key: string, identifier: string): boolean {
  const selected = selectedOwner(client, context, key, true);
  const entry = selected ? selected.state.entries.get(selected.storageKey) : undefined;
  return selected?.identifier === identifier && entry?.phase === 'ready' && !!entry.url;
}

function current(client: T3Client, state: State, entry: Entry, serial = entry.serial): boolean {
  const selected = selectedOwner(client, state.context, entry.key, entry.nested);
  return selected?.state === state && selected.identifier === entry.identifier
    && state.entries.get(entry.storageKey) === entry && entry.serial === serial;
}
function assertCurrent(client: T3Client, state: State, entry: Entry, serial: number) {
  if (!current(client, state, entry, serial)) throw new ClientError('That HTML page is no longer active.', 'superseded');
}
function guarded(client: T3Client, state: State, entry: Entry, serial: number, native: Native): Native {
  return { available: native.available,
    watch: topic => { assertCurrent(client, state, entry, serial); native.watch(topic); },
    later: async request => {
      assertCurrent(client, state, entry, serial);
      const reply = await native.later(request);
      assertCurrent(client, state, entry, serial);
      return reply;
    } };
}
async function sign(client: T3Client, state: State, entry: Entry, native: Native, refresh: boolean) {
  if (entry.pending) return;
  const serial = ++entry.serial; entry.pending = true;
  try {
    assertCurrent(client, state, entry, serial);
    if (!native.available || !client.ready || client.connection !== 'connected') throw new ClientError('Reconnect to load this page.');
    const result = obj(await client.rpc(guarded(client, state, entry, serial, native), 'assets.createUrl', {
      resource: { _tag: 'attachment', attachmentId: entry.reference.attachmentId,
        fileName: htmlRenderFileName(entry.reference.title), mimeType: 'text/html', disposition: 'inline' },
    }));
    assertCurrent(client, state, entry, serial);
    entry.url = mobileMediaURL(client.origin, str(result.relativeUrl)); entry.phase = 'ready';
    if (refresh) entry.attempt++;
    state.revision++;
  } catch (error) {
    if (letGo(error)) { if (current(client, state, entry, serial)) state.entries.delete(entry.storageKey); throw error; }
    if (current(client, state, entry, serial)) { entry.phase = 'failed'; state.revision++; }
  } finally { if (current(client, state, entry, serial)) entry.pending = false; }
}


/** Only an explicit full-document route may demand a URL before its native view mounts. */
export async function mobileHtmlRenderDocumentSource(client: T3Client, context: string, key: string,
  attachmentId: string, native: Native, fresh = false): Promise<HtmlRenderDocumentSource> {
  const selected = selectedOwner(client, context, key, false);
  const empty = (error = ''): HtmlRenderDocumentSource => ({ identifier: '', name: '', kind: '', sourceJSON: '', ready: false, error });
  if (!selected || selected.selection.reference.attachmentId !== attachmentId) return empty('That HTML page is no longer available.');
  const { state, selection, storageKey, identifier } = selected;
  let entry = state.entries.get(storageKey);
  if (!entry) {
    entry = { ...selection, storageKey, identifier, mountId: '', attempt: 0, serial: 0, nested: false,
      phase: 'pending', url: '', pending: false, retried: false, width: 0, retiredMounts: new Set() };
    state.entries.set(storageKey, entry);
  }
  if (!entry.url && entry.phase !== 'failed') await sign(client, state, entry, native, false);
  assertCurrent(client, state, entry, entry.serial);
  if (!entry.url || entry.phase === 'failed') return empty(entry.phase === 'failed' ? 'Page unavailable' : '');
  let url = entry.url;
  if (fresh) {
    const serial = entry.serial;
    if (!native.available || !client.ready || client.connection !== 'connected') return empty('Reconnect to load this page.');
    const result = obj(await client.rpc(guarded(client, state, entry, serial, native), 'assets.createUrl', {
      resource: { _tag: 'attachment', attachmentId, fileName: htmlRenderFileName(entry.reference.title),
        mimeType: 'text/html', disposition: 'inline' },
    }));
    assertCurrent(client, state, entry, serial);
    url = mobileMediaURL(client.origin, str(result.relativeUrl));
  }
  const name = htmlRenderFileName(entry.reference.title);
  return { identifier, name, kind: 'file', ready: true, error: '', sourceJSON: JSON.stringify({
    identifier, name, kind: 'file', source: 'remote', id: attachmentId, url, mimeType: 'text/html', sizeBytes: 0,
  }) };
}

/** The returned native is used only within its caller's awaited document operation. */
export function mobileHtmlRenderDocumentNative(client: T3Client, context: string, key: string,
  identifier: string, native: Native): Native {
  const selected = selectedOwner(client, context, key, false), entry = selected ? selected.state.entries.get(selected.storageKey) : undefined;
  if (!selected || selected.identifier !== identifier || !entry || !entry.url)
    throw new ClientError('That HTML page is no longer active.', 'superseded');
  return guarded(client, selected.state, entry, entry.serial, native);
}

/** Keep the first URL, but no queued native event may act after its view is removed. */
export function mobileHtmlRenderRetireDocumentView(client: T3Client, context: string, key: string): void {
  const selected = selectedOwner(client, context, key, false), entry = selected ? selected.state.entries.get(selected.storageKey) : undefined;
  if (!entry?.mountId) return;
  entry.retiredMounts.add(entry.mountId); entry.mountId = '';
  entry.serial++; entry.pending = false;
  if (entry.url) entry.phase = 'ready';
}

/** Mount demand and native events carry both the Contract owner and native instance identity. */
export async function mobileHtmlRenderEvent(client: T3Client, context: string, key: string,
  nested: boolean, identifier: string, raw: string, native: Native): Promise<HtmlRenderEventResult> {
  const selected = selectedOwner(client, context, key, nested);
  const empty = (): HtmlRenderEventResult => ({ context, identifier, revision: selected?.state.revision ?? 0, openKey: '', message: '' });
  if (!selected || selected.identifier !== identifier) return empty();
  let event: Obj;
  try { event = obj(JSON.parse(raw)); } catch { return empty(); }
  if (event.identifier !== identifier || typeof event.mountId !== 'string' || !event.mountId
    || event.mountId.length > 128 || !Number.isInteger(event.attempt) || Number(event.attempt) < 0) return empty();
  const { state, selection, storageKey } = selected;
  let entry = state.entries.get(storageKey);
  if (event.attempt !== (entry?.attempt ?? 0)) return empty();
  if (event.kind === 'mount') {
    if (entry?.retiredMounts.has(event.mountId)) return empty();
    if (!entry || entry.mountId !== event.mountId) {
      const retiredMounts = entry?.retiredMounts ?? new Set<string>();
      if (entry) retiredMounts.add(entry.mountId);
      entry = { ...selection, storageKey, identifier, mountId: event.mountId, attempt: Number(event.attempt),
        serial: 0, nested, phase: entry?.url ? 'ready' : 'pending', url: entry?.url ?? '',
        pending: false, retried: false, width: 0, retiredMounts };
      state.entries.set(storageKey, entry);
      state.revision++;
    }
    if (typeof event.width === 'number' && Number.isFinite(event.width) && event.width > 0 && entry.width !== event.width) {
      entry.width = event.width; state.revision++;
    }
    if (!entry.url && entry.phase !== 'failed') await sign(client, state, entry, native, false);
    return empty();
  }
  if (!entry || entry.mountId !== event.mountId || !current(client, state, entry)) return empty();
  if (event.kind === 'width') {
    if (typeof event.width === 'number' && Number.isFinite(event.width) && event.width > 0 && entry.width !== event.width) {
      entry.width = event.width; state.revision++;
    }
  } else if (event.kind === 'error' && !entry.pending) {
    if (entry.retried) { entry.phase = 'failed'; state.revision++; }
    else { entry.retried = true; await sign(client, state, entry, native, true); }
  } else if (event.kind === 'retry' && entry.phase === 'failed' && !entry.pending) {
    entry.retried = false;
    if (entry.url) { entry.phase = 'ready'; entry.attempt++; state.revision++; }
    else { entry.phase = 'pending'; await sign(client, state, entry, native, true); }
  } else if (event.kind === 'open' && nested && entry.phase === 'ready' && entry.url) {
    return { ...empty(), openKey: key };
  } else if (event.kind === 'external' && entry.phase === 'ready' && entry.url && typeof event.url === 'string') {
    let url: URL;
    try { url = new URL(event.url); } catch { return empty(); }
    if (!['http:', 'https:'].includes(url.protocol) || !url.hostname || url.username || url.password) return empty();
    const serial = entry.serial;
    try {
      const reply = await client.restAccess(guarded(client, state, entry, serial, native)).call({ op: 'mobileOpenURL', url: url.href });
      assertCurrent(client, state, entry, serial);
      if (reply.opened !== true) return { ...empty(), message: 'This link could not be opened.' };
    } catch (error) { if (letGo(error)) throw error; if (current(client, state, entry, serial)) return { ...empty(), message: 'This link could not be opened.' }; }
  }
  return empty();
}
