// Pinned365aa87982 use-thread-composer-state run-loss recovery and composerContext.
// @ref llp/1106.005-composer-and-transcript.decision.md#scratch-tasks-and-queue-boundaries
import { T3Client } from './shared/client';
import { arr, obj, str, type Obj } from './shared/domain';
import { ClientError, type Native, type Files } from './shared/protocol';
import { contextId, contextReferences } from './shared/composer-editor-menu';
import { draftFiles, setDraftFiles } from './shared/composer-editor-files';
import { adoptTerminalContexts } from './shared/terminal-integrations';
import type { MobileQueuedEditSession } from './queued-edit-state';
import { mobileQueuedEditOrigin } from './queued-edit-origin';

const PATH = 'app:/data/t3-code.json';
const IMAGE_TYPES = new Set(['image/png', 'image/jpeg', 'image/gif', 'image/webp']);
const hydrated = new WeakSet<object>();
type Recovery = MobileQueuedEditSession & { recoveryKey?: string };
const clone = (value: Obj): Obj => obj(JSON.parse(JSON.stringify(value)));
/** App-owned extension serialized by shared persist; never a pretend shared context field. */
function markers(client: T3Client): Obj { return obj(obj(client.local).mobileRecoveredDrafts); }
function setMarkers(client: T3Client, value: Obj) { Object.assign(client.local, { mobileRecoveredDrafts: value }); }
function validImage(image: Obj): boolean {
  return /^[a-f0-9-]{36}$/i.test(str(image.id)) && IMAGE_TYPES.has(str(image.mimeType))
    && typeof image.sizeBytes === 'number' && image.sizeBytes > 0 && image.sizeBytes <= 10 * 1024 * 1024;
}
function referenced(text: string, context: Obj | undefined): Obj[] {
  const ids = new Set(contextReferences(text).map(ref => ref.id)), records = arr(context?.records);
  for (const record of records) if (ids.has(str(record.contextId)) && record.kind === 'preview-annotation' && str(record.screenshotContextId)) ids.add(str(record.screenshotContextId));
  return records.filter(record => ids.has(str(record.contextId)));
}
function adoptTerminals(client: T3Client, text: string, context: Obj | undefined) {
  const records = { ...obj(obj(client.local).terminalContexts) };
  for (const record of referenced(text, context)) if (record.kind === 'terminal' && typeof record.text === 'string') records[str(record.contextId)] = record;
  // This is the real exported shared owner, needed before its expired-terminal pruning.
  adoptTerminalContexts(client.local, { terminalContexts: records });
}
/** Pure memory adoption. Caller persists this same local owner before considering
 * native recovery retirement. The native record remains the crash-recovery owner. */
export function mobileAdoptRecoveredDraft(client: T3Client, edit: Recovery): 'kept' | 'blocked' | 'already-adopted' {
  const key = edit.recoveryKey || `${edit.environmentId}:${edit.threadId}`, saved = markers(client);
  if (obj(saved[edit.owner]).key === key) return 'already-adopted';
  if (!edit.owner || !edit.threadId || key !== `${edit.environmentId}:${edit.threadId}` || !edit.text.trim()
    || edit.origin !== mobileQueuedEditOrigin(client) || edit.environmentId !== client.environmentId) return 'blocked';
  const files = draftFiles(client.local);
  if ((client.local.drafts[key] ?? '').trim() || (client.local.snapshotDrafts[key] ?? []).length || files.some(file => file.draftKey === key)) return 'blocked';
  const images = edit.attachments.filter(file => file.kind === 'image').map(file => ({ id: file.id, name: file.name,
    mimeType: file.mimeType, sizeBytes: file.sizeBytes, ...(file.uploadId ? { uploadId: file.uploadId } : {}) }));
  if (edit.attachments.length > 100 || images.some(image => !validImage(image))) throw new ClientError('The recovered attachments are invalid. The saved edit has been retained.');
  const recoveredFiles = edit.attachments.filter(file => file.kind === 'file').map(file => {
    const record = arr(edit.context?.records).find(record => record.kind === 'file' && (record.attachmentId === file.id || file.uploadId && record.attachmentId === file.uploadId));
    return { id: file.id, contextId: str(record?.contextId) || contextId('file', file.id), draftKey: key,
      environmentId: edit.environmentId, name: file.name, mimeType: file.mimeType, sizeBytes: file.sizeBytes,
      source: 'attached', attachmentId: file.uploadId, status: file.uploadId ? 'ready' as const : 'staged' as const };
  });
  client.local.drafts[key] = edit.text; client.local.snapshotDrafts[key] = images;
  setDraftFiles(client.local, [...files, ...recoveredFiles]);
  setMarkers(client, { ...saved, [edit.owner]: clone({ key, owner: edit.owner, revision: edit.revision, origin: edit.origin,
    environmentId: edit.environmentId, context: edit.context ?? null,
    attachments: edit.attachments.map(file => ({ id: file.id, uploadId: file.uploadId })) }) });
  adoptTerminals(client, edit.text, edit.context); client.revision++; return 'kept';
}
/** The marker prevents replay into a subsequently cleared draft after restart.
 * Native retirement does not remove context needed by the eventual ordinary Send. */
export function mobileRetireRecoveredDraft(client: T3Client, owner: string): void {
  const saved = { ...markers(client) }; if (!(owner in saved)) return;
  saved[owner] = { ...obj(saved[owner]), nativeRetired: true }; setMarkers(client, saved); client.revision++;
}
export function mobileRecoveredDraftMarker(client: T3Client, owner: string): Obj | null {
  const marker = obj(markers(client)[owner]); return str(marker.key) ? clone(marker) : null;
}
/** Adopt only metadata from the exact document the shared loader consumed. */
function hydrate(client: T3Client, saved: Obj) {
  if (!client.preferencesLoaded || hydrated.has(client.local) || saved.version !== 1) return;
  hydrated.add(client.local);
  for (const [key, value] of Object.entries(obj(saved.snapshotDrafts))) {
    const raw = arr(value), shared = raw.filter(image => validImage(image) && image.mimeType === 'image/png').slice(0, 100);
    // Do not overwrite a composer changed between load and the first native call.
    if (JSON.stringify(client.local.snapshotDrafts[key] ?? []) === JSON.stringify(shared)) client.local.snapshotDrafts[key] = raw.filter(validImage).slice(0, 100);
  }
  const recovered: Obj = {};
  for (const [owner, raw] of Object.entries(obj(saved.mobileRecoveredDrafts))) {
    const marker = obj(raw);
    if (marker.owner === owner && str(marker.key) && str(marker.origin) && str(marker.environmentId) && Number.isSafeInteger(marker.revision)) recovered[owner] = clone(marker);
  }
  setMarkers(client, recovered);
  for (const raw of Object.values(recovered)) {
    const marker = obj(raw); adoptTerminals(client, client.local.drafts[str(marker.key)] ?? '', obj(marker.context));
  }
  client.revision++;
}
/** Each refresh/command answer gets its own wrappers, with no retained handles or
 * promises. The shared load finishes before its devicePresentation request. */
export function mobileDraftRecoveryHandles(client: T3Client, native: Native | null | undefined, storage: Files) {
  let saved: Obj | null = null;
  const files: Files = { fs: { mkdir: path => storage.fs.mkdir(path), atomicWriteFile: (path, bytes) => storage.fs.atomicWriteFile(path, bytes),
    async readFile(path) {
      const capture = path === PATH && !client.preferencesLoaded;
      const bytes = await storage.fs.readFile(path);
      if (capture) { try { saved = obj(JSON.parse(new TextDecoder().decode(bytes))); } catch { /* Shared load reports malformed JSON. */ } }
      return bytes;
    } } };
  const handle: Native | null | undefined = native ? { available: native.available, watch: topic => native.watch(topic), later: request => {
    if (obj(request).op === 'devicePresentation' && saved) { hydrate(client, saved); saved = null; }
    return native.later(request);
  } } : native;
  return { native: handle, storage: files };
}
/** Context is filtered to actual outgoing references, then local attachment IDs
 * are rebound only to attachments this same payload sends. Retained remote
 * attachments from the original queued message are never recovered as new files. */
export function mobileRecoveredMessageContext(client: T3Client, key: string, text: string, attachments: Obj[], next?: Obj): Obj | undefined {
  const liveIds = new Set(attachments.map(file => str(file.id))), records = new Map<string, Obj>();
  for (const raw of Object.values(markers(client))) {
    const marker = obj(raw);
    if (marker.key !== key || marker.origin !== mobileQueuedEditOrigin(client) || marker.environmentId !== client.environmentId) continue;
    const original = arr(marker.attachments), images = client.local.snapshotDrafts[key] ?? [], files = draftFiles(client.local).filter(file => file.draftKey === key);
    for (const record of referenced(text, obj(marker.context))) {
      let value = record;
      if ('attachmentId' in record) {
        const initial = original.find(file => file.id === record.attachmentId || file.uploadId && file.uploadId === record.attachmentId);
        if (!initial) continue;
        const id = str(images.find(image => image.id === initial.id)?.uploadId) || files.find(file => file.id === initial.id)?.attachmentId || '';
        if (!id || !liveIds.has(id)) continue;
        value = { ...record, attachmentId: id };
      }
      records.set(str(value.contextId), value);
    }
  }
  for (const record of arr(next?.records)) records.set(str(record.contextId), record);
  return records.size ? { version: 1, records: [...records.values()] } : undefined;
}
/** A confirmed native retirement ends replay ownership. Keep its explicit context
 * only while the corresponding ordinary text still references any record or its
 * preview dependency. Unretired markers remain even after clear or successful send. */
function pruneRetiredMarkers(client: T3Client) {
  const saved = markers(client), next = { ...saved }; let changed = false;
  for (const [owner, raw] of Object.entries(saved)) {
    const marker = obj(raw);
    if (marker.nativeRetired === true && referenced(client.local.drafts[str(marker.key)] ?? '', obj(marker.context)).length === 0) {
      delete next[owner]; changed = true;
    }
  }
  if (changed) { setMarkers(client, next); client.revision++; }
}
/** One shared client. Only a new ordinary send's context is extended, before
 * super.write installs/persists its pending object. Retry payloads stay immutable. */
export class MobileDraftClient extends T3Client {
  override async persist(storage: Files): Promise<void> {
    pruneRetiredMarkers(this);
    return super.persist(storage);
  }
  override async write(native: Native, storage: Files, pending: Parameters<T3Client['write']>[2], beforeRequest?: () => void): Promise<Obj> {
    const previous = this.local.pending[this.environmentId];
    const retry = previous && previous.method === pending.method && previous.payload.commandId === pending.payload.commandId;
    const launch = pending.method === 'orchestration.launchThread';
    if (!retry && (launch || pending.method === 'orchestration.dispatchCommand' && pending.payload.type === 'message.dispatch')) {
      const body = launch ? obj(pending.payload.initialMessage) : pending.payload;
      const key = launch ? `${this.environmentId}:new:${str(pending.payload.projectId)}` : `${this.environmentId}:${str(pending.payload.threadId)}`;
      const context = mobileRecoveredMessageContext(this, key, str(body.text), arr(body.attachments), body.context ? obj(body.context) : undefined);
      if (context) pending.payload = launch ? { ...pending.payload, initialMessage: { ...body, context } } : { ...pending.payload, context };
    }
    return super.write(native, storage, pending, beforeRequest);
  }
}
