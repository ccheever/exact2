// @ref llp/1109.005-composer-and-transcript.decision.md#queued-command-construction
// Pure protocol 2 mapping from pinned 365aa87982 projectThreadStartTurn and commands.ts.
import { arr, obj, str, type Obj } from './shared/domain';
import { launchPayload, sendPayload } from './shared/protocol';
import { assistantCitationsToPlainText } from './shared/diff-citations';
import { contextReferences } from './shared/composer-editor-menu';
import { mobileModelSelectionUnavailable } from './model-availability';
import { mobileOutboxPlanCommand, type MobileOutboxCommandPlan } from './mobile-outbox-command';
import type { MobileOutboxInlineTemplate } from './mobile-outbox-inline';
import { mobileOutboxCreationSendable, mobileOutboxResolveSettings,
  type MobileOutboxRecord, type MobileOutboxSettings } from './mobile-outbox-model';

export interface MobileOutboxWireOwner {
  origin: string; environmentId: string; threadId: string; messageId: string; commandId: string;
}
export interface MobileOutboxWireRequest {
  owner: MobileOutboxWireOwner; stage: 'settings-sync' | 'start-turn';
  method: 'orchestration.launchThread' | 'orchestration.dispatchCommand'; payload: Obj;
}
/** Preparation validates metadata and native CAS adopts modern references.
 * Plan builders also admit source legacy inline images, which must pass the
 * persistence stage before becoming a sendable command. */
export interface MobileOutboxPreparedAttachment { localId: string; kind?: 'reference' | 'inline-image'; attachment: Obj }
export interface MobileOutboxWireFacts {
  origin: string; environmentId: string; config: Obj;
  /** Prepared entries in captured order. Request builders require adopted references;
   * Plan builders can return a separate persistence stage for legacy inline images. */
  attachments: readonly MobileOutboxPreparedAttachment[];
}
/** Native inline capture reads the existing UUID file; this descriptor is never a sendable attachment. */
export type MobileOutboxNativePreparedAttachment =
  { localId: string; kind: 'reference'; attachment: Obj } |
  { localId: string; kind: 'inline-image-metadata'; attachment: { type: 'image'; name: string; mimeType: string; sizeBytes: number } };
export interface MobileOutboxNativeWireFacts extends Omit<MobileOutboxWireFacts, 'attachments'> {
  attachments: readonly MobileOutboxNativePreparedAttachment[];
}
export type MobileOutboxNativeCommandPlan = MobileOutboxWireResult<MobileOutboxWireRequest> |
  { status: 'needs-inline-reservation'; value: MobileOutboxInlineTemplate };
type AssemblyFacts = Omit<MobileOutboxWireFacts, 'attachments'> & {
  attachments: readonly (MobileOutboxPreparedAttachment | MobileOutboxNativePreparedAttachment)[];
};
type AttachmentMode = 'references' | 'inline-bytes' | 'inline-metadata';
export interface MobileOutboxThreadFacts extends MobileOutboxSettings {
  origin: string; environmentId: string; threadId: string;
}
export type MobileOutboxWireResult<T> = { status: 'ready'; value: T } | { status: 'blocked'; reason: string } |
  { status: 'needs-projection' };
const copy = <T>(value: T): T => JSON.parse(JSON.stringify(value));
const blocked = (reason: string): MobileOutboxWireResult<never> => ({ status: 'blocked', reason });
const owner = (record: MobileOutboxRecord): MobileOutboxWireOwner => ({ origin: record.origin, environmentId: record.environmentId,
  threadId: record.threadId, messageId: record.messageId, commandId: record.commandId });
function request(record: MobileOutboxRecord, payload: Obj, stage: MobileOutboxWireRequest['stage'],
  method: MobileOutboxWireRequest['method'] = 'orchestration.dispatchCommand'): MobileOutboxWireRequest {
  return { owner: owner(record), stage, method, payload: copy(payload) };
}
function assertOwner(record: MobileOutboxRecord, facts: Pick<MobileOutboxWireFacts, 'origin' | 'environmentId' | 'config'>,
  thread?: MobileOutboxThreadFacts): void {
  if (facts.origin !== record.origin || facts.environmentId !== record.environmentId ||
      obj(facts.config.environment).environmentId !== record.environmentId || thread &&
      (thread.origin !== record.origin || thread.environmentId !== record.environmentId || thread.threadId !== record.threadId))
    throw new Error('Outbox endpoint or thread no longer matches the captured owner.');
}
function settings(record: MobileOutboxRecord, facts: Pick<MobileOutboxWireFacts, 'origin' | 'environmentId' | 'config'>, thread: MobileOutboxSettings): MobileOutboxSettings {
  const resolved = mobileOutboxResolveSettings(record, thread, arr(facts.config.providers).map(provider => ({
    instanceId: str(provider.instanceId), ...(typeof provider.showInteractionModeToggle === 'boolean' ? { showInteractionModeToggle: provider.showInteractionModeToggle } : {}) })));
  if (mobileModelSelectionUnavailable(facts.config, copy(resolved.modelSelection) as unknown as Obj))
    throw new Error('Antigravity model unavailable. Set it up on web or desktop, or choose another model.');
  return resolved;
}
function failed(error: unknown): MobileOutboxWireResult<never> {
  return blocked(error instanceof Error ? error.message : 'The queued message could not be prepared.');
}
/** Source title seed retains context links and uses the first attachment, including files. */
export function mobileOutboxTitle(text: string, attachments: readonly { name: string }[]): string {
  const normalize = (value: string) => assistantCitationsToPlainText(value).trim().replace(/\s+/gu, ' ');
  const name = normalize(attachments[0]?.name ?? '');
  const value = normalize(text) || (name ? `Image: ${name}` : 'New thread');
  return value.length > 50 ? `${value.slice(0, 50)}...` : value;
}
/** Input bytes were already uploaded and adopted durably by the queue owner. */
export function mobileOutboxMessageContent(record: MobileOutboxRecord, prepared: readonly MobileOutboxPreparedAttachment[],
  supportsInlineMessageContext: boolean, trimText = false): { text: string; context?: Obj; attachments: Obj[] } {
  return messageContent(record, prepared, supportsInlineMessageContext, trimText, 'references');
}
function messageContent(record: MobileOutboxRecord, prepared: AssemblyFacts['attachments'],
  supportsInlineMessageContext: boolean, trimText: boolean, mode: AttachmentMode): { text: string; context?: Obj; attachments: Obj[] } {
  if (prepared.length !== record.attachments.length) throw new Error('Outbox attachment preparation is incomplete.');
  const ids = new Map<string, string>(), seen = new Set<string>();
  for (let index = 0; index < prepared.length; index++) {
    const input = prepared[index]!, local = record.attachments[index]!;
    const remote: Obj = input.attachment;
    if (input.localId !== local.id || seen.has(input.localId)) throw new Error('Outbox attachment order does not match the captured bytes.');
    seen.add(input.localId);
    if (input.kind === 'inline-image' || input.kind === 'inline-image-metadata') {
      const metadata = input.kind === 'inline-image-metadata';
      if ((metadata ? mode !== 'inline-metadata' : mode !== 'inline-bytes') || local.kind !== 'image' || remote.type !== 'image' || 'id' in remote || 'source' in remote ||
        remote.name !== local.name || remote.mimeType !== local.mimeType || remote.sizeBytes !== local.sizeBytes ||
        (metadata ? Object.keys(remote).length !== 4 || Object.keys(remote).some(key => !['type', 'name', 'mimeType', 'sizeBytes'].includes(key))
          : !str(remote.dataUrl).startsWith(`data:${local.mimeType};base64,`)))
        throw new Error('Outbox inline image preparation does not match the captured bytes.');
      continue;
    }
    if (!str(remote.id) ||
        !['image', 'file'].includes(str(remote.type)) || 'dataUrl' in remote ||
        local.uploadId !== remote.id || local.uploadEnvironmentId !== record.environmentId)
      throw new Error('Outbox attachment preparation does not match the captured bytes.');
    ids.set(local.id, str(remote.id));
  }
  const context: Obj | undefined = record.context ? { version: 1, records: arr(record.context.records).map(entry =>
    'attachmentId' in entry ? { ...entry, attachmentId: ids.get(str(entry.attachmentId)) ?? entry.attachmentId } : entry) } : undefined;
  const text = trimText ? record.text.trim() : record.text;
  const content = supportsInlineMessageContext ? { text, ...(context ? { context } : {}) } :
    { text: legacyContextMessage(text, context ? arr(context.records) : []) };
  return copy({ ...content, attachments: prepared.map(input => input.attachment) });
}

function launchRequest(record: MobileOutboxRecord, facts: AssemblyFacts,
  worktreeBranchName: string, mode: AttachmentMode): MobileOutboxWireResult<MobileOutboxWireRequest> {
  try {
    assertOwner(record, facts);
    if (!mobileOutboxCreationSendable(record) || !record.creation || !record.modelSelection)
      return blocked('Edit the pending task before sending it.');
    const creation = record.creation, resolved = settings(record, facts, { modelSelection: record.modelSelection,
      runtimeMode: 'full-access', interactionMode: 'default' });
    if (creation.workspaceMode === 'worktree' && !worktreeBranchName) return blocked('Prepare a temporary branch for this worktree.');
    const capabilities = obj(obj(facts.config.environment).capabilities);
    const content = messageContent(record, facts.attachments, capabilities.inlineMessageContext === true, true,
      capabilities.attachmentUploads !== true ? mode : 'references');
    const payload = launchPayload(record.commandId, record.threadId, record.messageId, creation.projectId, content.text,
      copy(resolved.modelSelection) as unknown as Obj, resolved.runtimeMode, resolved.interactionMode, content.attachments);
    payload.creationSource = 'mobile';
    payload.title = mobileOutboxTitle(content.text, content.attachments.map(file => ({ name: str(file.name) })));
    payload.initialMessage = { messageId: record.messageId, ...content };
    payload.workspaceStrategy = creation.workspaceMode === 'worktree' ? {
      type: 'worktree', baseRef: creation.branch, branch: worktreeBranchName, ...(creation.startFromOrigin ? { startFromOrigin: true } : {}),
    } : creation.worktreePath ? { type: 'existing_worktree', worktreePath: creation.worktreePath,
      ...(creation.branch === null ? {} : { branch: creation.branch }) } : {
      type: 'root', ...(creation.branch === null ? {} : { branch: creation.branch }),
    };
    return { status: 'ready', value: request(record, payload, 'start-turn', 'orchestration.launchThread') };
  } catch (error) { return failed(error); }
}
export const mobileOutboxLaunchRequest = (record: MobileOutboxRecord, facts: MobileOutboxWireFacts, worktreeBranchName = '') =>
  launchRequest(record, facts, worktreeBranchName, 'references');
export const mobileOutboxLaunchPlan = (record: MobileOutboxRecord, facts: MobileOutboxWireFacts, worktreeBranchName = ''): MobileOutboxCommandPlan =>
  mobileOutboxPlanCommand(launchRequest(record, facts, worktreeBranchName, 'inline-bytes'));

/** Ordered source plan only: caller must revalidate and settle each command before the next. */
export function mobileOutboxSettingsRequests(record: MobileOutboxRecord, facts: MobileOutboxWireFacts,
  thread: MobileOutboxThreadFacts): MobileOutboxWireResult<MobileOutboxWireRequest[]> {
  try {
    assertOwner(record, facts, thread);
    if (record.creation) return blocked('Creation settings belong to the launch command.');
    const resolved = settings(record, facts, thread), commands: MobileOutboxWireRequest[] = [];
    if (resolved.runtimeMode !== thread.runtimeMode) commands.push(request(record, {
      type: 'thread.runtime-mode.set', commandId: `${record.commandId}:runtime-mode`, threadId: record.threadId, runtimeMode: resolved.runtimeMode,
    }, 'settings-sync'));
    if (resolved.interactionMode !== thread.interactionMode) commands.push(request(record, {
      type: 'thread.interaction-mode.set', commandId: `${record.commandId}:interaction-mode`, threadId: record.threadId, interactionMode: resolved.interactionMode,
    }, 'settings-sync'));
    return { status: 'ready', value: commands };
  } catch (error) { return failed(error); }
}

function messageRequest(record: MobileOutboxRecord, facts: AssemblyFacts,
  thread: MobileOutboxThreadFacts, projection: Obj | null, attachmentMode: AttachmentMode): MobileOutboxWireResult<MobileOutboxWireRequest> {
  try {
    assertOwner(record, facts, thread);
    if (record.creation) return blocked('A pending task must use its launch command.');
    const resolved = settings(record, facts, thread), capabilities = obj(obj(facts.config.environment).capabilities);
    const serverResolves = capabilities.serverResolvedCommandContext === true, mode = record.dispatchMode ?? 'start';
    if (!serverResolves && mode !== 'start' && projection === null) return { status: 'needs-projection' };
    if (projection && obj(projection.thread).id !== record.threadId) return blocked('Outbox projection belongs to another thread.');
    const content = messageContent(record, facts.attachments, capabilities.inlineMessageContext === true, false,
      capabilities.attachmentUploads !== true ? attachmentMode : 'references');
    const payload = sendPayload(record.commandId, record.threadId, record.messageId, content.text, content.attachments);
    payload.creationSource = 'mobile'; delete payload.deliveryIntent;
    payload.modelSelection = copy(resolved.modelSelection) as unknown as Obj;
    if (content.context) payload.context = content.context;
    if (mode !== 'start') {
      if (serverResolves) {
        payload.dispatchMode = { type: mode === 'queue' ? 'queue_after_active' : 'start_immediately' };
        if (mode !== 'queue') payload.deliveryIntent = mode;
      } else {
        const active = arr(projection?.runs).findLast(run => ['preparing', 'starting', 'running', 'waiting'].includes(str(run.status)));
        const providerThread = active ? arr(projection?.providerThreads).find(value => value.id === active.providerThreadId) : undefined;
        const session = providerThread?.providerSessionId == null ? undefined :
          arr(projection?.providerSessions).find(value => value.id === providerThread.providerSessionId);
        const turns = obj(obj(session?.capabilities).turns);
        payload.dispatchMode = !active ? { type: 'start_immediately' } : mode === 'steer' ? { type: 'steer_active', targetRunId: active.id } :
          mode === 'restart' ? { type: 'restart_active', targetRunId: active.id } : mode === 'queue' ? { type: 'queue_after_active' } :
          turns.supportsActiveSteering === true ? { type: 'steer_active', targetRunId: active.id } :
          turns.supportsQueuedMessages === true ? { type: 'queue_after_active' } :
          turns.supportsSteeringByInterruptRestart === true ? { type: 'restart_active', targetRunId: active.id } : { type: 'queue_after_active' };
      }
      if (serverResolves || arr(projection?.messages).length === 0) payload.titleSeed = mobileOutboxTitle(record.text, record.attachments);
    }
    return { status: 'ready', value: request(record, payload, 'start-turn') };
  } catch (error) { return failed(error); }
}
export const mobileOutboxMessageRequest = (record: MobileOutboxRecord, facts: MobileOutboxWireFacts,
  thread: MobileOutboxThreadFacts, projection: Obj | null = null) => messageRequest(record, facts, thread, projection, 'references');
export const mobileOutboxMessagePlan = (record: MobileOutboxRecord, facts: MobileOutboxWireFacts,
  thread: MobileOutboxThreadFacts, projection: Obj | null = null): MobileOutboxCommandPlan =>
  mobileOutboxPlanCommand(messageRequest(record, facts, thread, projection, 'inline-bytes'));

function nativePlan(result: MobileOutboxWireResult<MobileOutboxWireRequest>,
  prepared: readonly MobileOutboxNativePreparedAttachment[]): MobileOutboxNativeCommandPlan {
  if (result.status !== 'ready') return result;
  const inline = prepared.flatMap((value, index) => value.kind === 'inline-image-metadata' ? [{ index, localId: value.localId }] : []);
  if (inline.length === 0) return result;
  return { status: 'needs-inline-reservation', value: { owner: copy(result.value.owner), commandTemplate: result.value, inline } };
}
/** Native planning never admits inline bytes or fabricates a dataURL. The compact
 * result must pass native capture before its assets stage can be issued. */
export const mobileOutboxLaunchNativePlan = (record: MobileOutboxRecord, facts: MobileOutboxNativeWireFacts,
  worktreeBranchName = ''): MobileOutboxNativeCommandPlan =>
  nativePlan(launchRequest(record, facts, worktreeBranchName, 'inline-metadata'), facts.attachments);
export const mobileOutboxMessageNativePlan = (record: MobileOutboxRecord, facts: MobileOutboxNativeWireFacts,
  thread: MobileOutboxThreadFacts, projection: Obj | null = null): MobileOutboxNativeCommandPlan =>
  nativePlan(messageRequest(record, facts, thread, projection, 'inline-metadata'), facts.attachments);

// Pinned composerContextLegacySend.ts. These functions consume captured records,
// never foreground editor caches. The existing parser validates each occurrence;
// scanning occurrences preserves repeated references that contextReferences deduplicates.
function legacyContextMessage(input: string, records: Obj[]): string {
  const byId = new Map(records.map(record => [str(record.contextId), record])), used = new Set<string>();
  const text = input.replace(/(!?)\[([^\]\n]{0,512})\]\((t3-context:\/\/v1\/[^\s)]{1,200})\)/g, source => {
    const reference = contextReferences(source)[0], record = reference ? byId.get(reference.id) : undefined;
    if (!record) return source;
    used.add(str(record.contextId));
    if (record.kind === 'review-comment') return review(record);
    if (record.kind === 'terminal' && 'terminalLabel' in record) {
      const slug = str(record.terminalLabel).trim().toLowerCase().replace(/\s+/g, '-');
      const range = record.lineStart === record.lineEnd ? `${record.lineStart}` : `${record.lineStart}-${record.lineEnd}`;
      return `@${slug}:${range}`;
    }
    return str(record.label);
  });
  const blocks = [block('terminal_context', records.filter(record => record.kind === 'terminal').map(terminal)),
    block('element_context', records.filter(record => record.kind === 'element').map(record => 'tagName' in record ? element(record) : '')),
    ...records.filter(record => record.kind === 'preview-annotation').map(record => `<preview_annotation>\n${preview(record)}\n</preview_annotation>`),
    ...records.filter(record => record.kind === 'review-comment' && !used.has(str(record.contextId))).map(review)].filter(value => value.length > 0);
  return [text.trimEnd(), ...blocks].filter(part => part.length > 0).join('\n\n');
}
const indent = (text: string) => text.split('\n').map(line => line.length === 0 ? '' : `  ${line}`).join('\n');
function block(tag: string, entries: string[]): string {
  const body = entries.filter(value => value.length > 0).join('\n');
  return body.length === 0 ? '' : `<${tag}>\n${body}\n</${tag}>`;
}
function terminal(record: Obj): string {
  if (!('terminalLabel' in record)) return '';
  const range = record.lineStart === record.lineEnd ? `line ${record.lineStart}` : `lines ${record.lineStart}-${record.lineEnd}`;
  const body = str(record.text).split('\n').slice(0, Number(record.lineEnd) - Number(record.lineStart) + 1)
    .map((line, index) => `${Number(record.lineStart) + index} | ${line}`).join('\n');
  return `- ${record.terminalLabel} ${range}:\n${indent(body)}`;
}
function element(record: Obj): string {
  const lines: string[] = [];
  if (record.pageUrl) lines.push(`url: ${record.pageUrl}`);
  if (record.selector) lines.push(`selector: ${record.selector}`);
  const source = obj(record.source);
  if (source.fileName) lines.push(`source: ${[source.fileName, ...[source.lineNumber, source.columnNumber].filter(part => part != null)].join(':')}`);
  if (record.htmlPreview) lines.push(`html:\n${indent(str(record.htmlPreview))}`);
  if (record.styles) lines.push(`styles:\n${indent(str(record.styles))}`);
  return `- <${record.componentName ?? record.tagName}>:\n${indent(lines.join('\n'))}`;
}
function preview(record: Obj): string {
  if (!('annotationId' in record)) return '';
  const lines = [`Id: ${record.annotationId}`, `Page: ${record.pageUrl || record.pageTitle || ''}`];
  if (record.comment) lines.push(`Comment: ${record.comment}`);
  if (record.targetSummary) lines.push(`Targets: ${record.targetSummary}`);
  const changes = Array.isArray(record.styleChanges) ? record.styleChanges : [];
  if (changes.length) lines.push('Requested visual changes:', ...changes.map(change => `- ${change}`));
  if (Array.isArray(record.elements) && record.elements.length) lines.push('<element_context>', arr(record.elements).map(element).join('\n'), '</element_context>');
  return lines.join('\n');
}
function review(record: Obj): string {
  if (!('sectionId' in record)) return str(record.label);
  const escape = (value: string) => value.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
  const attributes = ['sectionId', 'sectionTitle', 'filePath', 'rangeLabel'].map(key => `${key}="${escape(str(record[key]))}"`)
    .concat([`startIndex="${record.startIndex}"`, `endIndex="${record.endIndex}"`]).join(' ');
  const diff = str(record.diff), fence = '`'.repeat(Math.max(3, Math.max(0, ...[...diff.matchAll(/`+/g)].map(match => match[0].length)) + 1));
  const body = diff ? `${record.text}\n\n${fence}${record.fenceLanguage ?? 'diff'}\n${diff}\n${fence}` : str(record.text);
  return `<review_comment ${attributes}>\n${body}\n</review_comment>`;
}
