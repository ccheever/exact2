// @ref llp/1109.005-composer-and-transcript.decision.md#queued-command-construction
// Inline bytes stay in the existing UUID files. Native capture must bind their
// digests before this compact template can become a delivery receipt.
import { arr, obj, type Obj } from './shared/domain';
import { mobileOutboxMaterializeCommand, type MobileOutboxInlinePersistence } from './mobile-outbox-command';
import type { MobileOutboxRecord } from './mobile-outbox-model';
import type { MobileOutboxWireOwner, MobileOutboxWireRequest, MobileOutboxWireResult } from './mobile-outbox-wire';
import { mobileOutboxTransferCanonical as canonical } from './mobile-outbox-transfer-model';

export interface MobileOutboxInlineTemplate {
  owner: MobileOutboxWireOwner;
  commandTemplate: MobileOutboxWireRequest;
  inline: { index: number; localId: string }[];
}
const copy = <T>(value: T): T => JSON.parse(JSON.stringify(value));
const content = (command: MobileOutboxWireRequest): Obj => command.method === 'orchestration.launchThread'
  ? obj(command.payload.initialMessage) : command.payload;
const blocked = (): MobileOutboxWireResult<never> => ({ status: 'blocked', reason: 'The inline attachment template does not match the queued message.' });

/** Drop base64 only after binding every inline position to its captured local ID.
 * This is a reservation input, not a sendable request or proof of file durability. */
export function mobileOutboxCompactInline(record: MobileOutboxRecord,
  plan: MobileOutboxInlinePersistence): MobileOutboxWireResult<MobileOutboxInlineTemplate> {
  const owner = { origin: record.origin, environmentId: record.environmentId, threadId: record.threadId,
    messageId: record.messageId, commandId: record.commandId };
  if (canonical(owner) !== canonical(plan.owner)) return blocked();
  const attachments = arr(content(plan.commandTemplate).attachments);
  if (attachments.length !== record.attachments.length) return blocked();
  const inline: MobileOutboxInlineTemplate['inline'] = [], replies: Obj[] = [];
  for (let index = 0; index < attachments.length; index++) {
    const value = attachments[index]!, local = record.attachments[index]!;
    if (!('dataUrl' in value)) continue;
    if (local.kind !== 'image' || value.type !== 'image' || 'id' in value || 'source' in value
      || Object.keys(value).some(key => !['type', 'name', 'mimeType', 'sizeBytes', 'dataUrl'].includes(key))
      || typeof value.dataUrl !== 'string' || !value.dataUrl.startsWith(`data:${local.mimeType};base64,`)
      || value.name !== local.name || value.mimeType !== local.mimeType || value.sizeBytes !== local.sizeBytes) return blocked();
    inline.push({ index, localId: local.id });
    replies.push({ type: 'image', id: local.id, name: local.name, mimeType: local.mimeType, sizeBytes: local.sizeBytes });
  }
  // Reuse the source mapper's owner/template/request correspondence checks.
  if (mobileOutboxMaterializeCommand(plan, { attachments: replies }).status !== 'ready') return blocked();
  const commandTemplate = copy(plan.commandTemplate), message = content(commandTemplate);
  message.attachments = attachments.map((value, index) => {
    const result = copy(value);
    if (inline.some(item => item.index === index)) delete result.dataUrl;
    return result;
  });
  return { status: 'ready', value: { owner, commandTemplate, inline } };
}

/** Materialize only a compact template read from a validated, acknowledged native
 * receipt. Callers must separately establish that receipt's identity/durability. */
export function mobileOutboxMaterializeInline(template: MobileOutboxInlineTemplate,
  response: unknown): MobileOutboxWireResult<MobileOutboxWireRequest> {
  const commandTemplate = copy(template.commandTemplate), message = content(commandTemplate);
  if (!Array.isArray(message.attachments) || template.inline.length === 0) return blocked();
  const attachments = arr(message.attachments), seen = new Set<string>();
  let previous = -1;
  for (const item of template.inline) {
    const value = attachments[item.index];
    if (!Number.isSafeInteger(item.index) || item.index <= previous || !item.localId || seen.has(item.localId)
      || !value || value.type !== 'image' || 'id' in value || 'source' in value || 'dataUrl' in value
      || Object.keys(value).some(key => !['type', 'name', 'mimeType', 'sizeBytes'].includes(key))) return blocked();
    previous = item.index; seen.add(item.localId);
    // Presence marks the inline subset for the existing pure mapper. This value
    // is never sent, saved or interpreted as bytes.
    value.dataUrl = '';
  }
  if (attachments.some((item, index) => !template.inline.some(binding => binding.index === index) && ('dataUrl' in item || !item.id))) return blocked();
  message.attachments = attachments;
  return mobileOutboxMaterializeCommand({ owner: copy(template.owner), commandTemplate,
    request: { method: 'assets.persistChatAttachments', payload: { threadId: template.owner.threadId,
      messageId: template.owner.messageId, attachments: template.inline.map(item => copy(attachments[item.index]!)) } } }, response);
}
