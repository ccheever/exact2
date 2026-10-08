// Pinned365aa87982 commands.ts persistAttachments/startThreadTurn and ws.ts362.
// @ref llp/1109.005-composer-and-transcript.decision.md#queued-command-construction
import { arr, obj, str, type Obj } from './shared/domain';
import type { MobileOutboxWireOwner, MobileOutboxWireRequest, MobileOutboxWireResult } from './mobile-outbox-wire';

export interface MobileOutboxInlinePersistence {
  owner: MobileOutboxWireOwner;
  request: { method: 'assets.persistChatAttachments'; payload: { threadId: string; messageId: string; attachments: Obj[] } };
  /** Not a sendable protocol request while its message still contains dataUrl. */
  commandTemplate: MobileOutboxWireRequest;
}
export type MobileOutboxCommandPlan = MobileOutboxWireResult<MobileOutboxWireRequest> |
  { status: 'needs-inline-persistence'; value: MobileOutboxInlinePersistence };
const copy = <T>(value: T): T => JSON.parse(JSON.stringify(value));
const content = (command: MobileOutboxWireRequest): Obj => command.method === 'orchestration.launchThread'
  ? obj(command.payload.initialMessage) : command.payload;
const blocked = (reason: string): MobileOutboxWireResult<never> => ({ status: 'blocked', reason });

/** Split the source command only where its first RPC persists inline images.
 * This pure plan allocates no IDs, performs no I/O and mutates no queue record. */
export function mobileOutboxPlanCommand(result: MobileOutboxWireResult<MobileOutboxWireRequest>): MobileOutboxCommandPlan {
  if (result.status !== 'ready') return copy(result);
  const commandTemplate = copy(result.value), uploads = arr(content(commandTemplate).attachments).filter(item => 'dataUrl' in item);
  if (uploads.length === 0) return { status: 'ready', value: commandTemplate };
  return { status: 'needs-inline-persistence', value: { owner: copy(commandTemplate.owner), commandTemplate,
    request: { method: 'assets.persistChatAttachments', payload: { threadId: commandTemplate.owner.threadId,
      messageId: commandTemplate.owner.messageId, attachments: copy(uploads) } } } };
}

/** The executor must match this response to the frozen plan and current native
 * receipt. IDs alone cannot adopt it into an outbox row or authorize dispatch. */
export function mobileOutboxMaterializeCommand(plan: MobileOutboxInlinePersistence,
  response: unknown): MobileOutboxWireResult<MobileOutboxWireRequest> {
  const template = plan.commandTemplate, message = content(template);
  const before = arr(message.attachments), uploads = before.filter(item => 'dataUrl' in item);
  const result = obj(response).attachments;
  if (template.stage !== 'start-turn' || template.payload.commandId !== plan.owner.commandId ||
    template.payload.threadId !== plan.owner.threadId || message.messageId !== plan.owner.messageId ||
    plan.request.method !== 'assets.persistChatAttachments' || !Array.isArray(result) || result.length !== uploads.length || uploads.length === 0 ||
    JSON.stringify(uploads) !== JSON.stringify(plan.request.payload.attachments) ||
    plan.owner.threadId !== plan.request.payload.threadId || plan.owner.messageId !== plan.request.payload.messageId ||
    JSON.stringify(plan.owner) !== JSON.stringify(plan.commandTemplate.owner))
    return blocked('The inline attachment result does not match its captured command.');
  const persisted: Obj[] = [];
  for (let index = 0; index < uploads.length; index++) {
    const expected = uploads[index]!, actual = obj(result[index]), id = str(actual.id);
    // Pinned ws.ts preserves these fields and assigns a deterministic ID from
    // threadId/messageId/inline-subset index. Refuse a partial or unrelated reply.
    if (actual.type !== 'image' || !/^[a-z0-9_-]{1,128}$/i.test(id) || 'dataUrl' in actual ||
      ['name', 'mimeType', 'sizeBytes'].some(field => actual[field] !== expected[field]))
      return blocked('The server returned an invalid persisted image.');
    persisted.push({ type: 'image', id, name: actual.name, mimeType: actual.mimeType, sizeBytes: actual.sizeBytes });
  }
  let index = 0;
  const attachments = before.map(item => 'dataUrl' in item ? persisted[index++]! : item);
  // Source remaps only IDs present BEFORE persistence. Inline source images
  // intentionally have no id; their local context binding stays unchanged.
  const ids = new Map(before.flatMap((item, at) => typeof item.id === 'string' ? [[item.id, str(attachments[at]?.id)] as const] : []));
  const command = copy(plan.commandTemplate), output = content(command);
  output.attachments = copy(attachments);
  if (output.context !== undefined) {
    const context = obj(output.context);
    output.context = { ...context, records: arr(context.records).map(record =>
      ['image', 'file'].includes(str(record.kind)) && 'attachmentId' in record
        ? { ...record, attachmentId: ids.get(str(record.attachmentId)) ?? record.attachmentId } : record) };
  }
  return { status: 'ready', value: command };
}
