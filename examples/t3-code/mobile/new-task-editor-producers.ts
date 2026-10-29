import type { MobileDraftClient } from './mobile-draft-recovery';
import { mobileNewTaskFlowAction, type NewTaskFlowResult } from './new-task-flow';
import { mobileNewTaskAttachmentRemove } from './composer-attachments';
import { mobileComposerTargetText, type MobileComposerTarget } from './composer-target';
import { mobileVoiceAction, mobileVoiceSnapshot, mobileVoiceSelection } from './voice-data';
import type { NewTaskEditorRoute } from './new-task-editor';
import type { Files, Native } from './shared/protocol';
import { draftFiles } from './shared/composer-editor-files';
import { obj, str } from './shared/domain';
import { collectComposerContextReferences } from './composer-editor-document';
import { mobileNewTaskContextRead } from './mobile-new-task-context';
import { ClientError } from './shared/protocol';
import { fleet } from './shared/settings-b-fleet';

export type NewTaskDestination = 'model' | 'branch' | 'environment' | 'project' | 'back';
export type NewTaskProducerIntent =
  | { kind: 'navigate'; destination: NewTaskDestination }
  | { kind: 'preview'; id: string; token?: string }
  | { kind: 'remove'; source: 'remove-image' | 'remove-file'; id: string }
  | { kind: 'send'; alternate: boolean; now: number; preferences: { ready: boolean; serialized: string } }
  | { kind: 'voice'; label: string; owner: string };
export interface NewTaskProducerEffect {
  id: string; kind: 'navigate' | 'preview' | 'flow'; destination: string; flow: NewTaskFlowResult | null;
}
export interface NewTaskProducerResult {
  message: string; leave: boolean; voice: boolean;
  selection: { start: number; end: number } | null;
  effect: Omit<NewTaskProducerEffect, 'id'> | null;
}

/** Only the NewTask owner calls this after its final native source is durable.
 * Existing producers retain their own capture, cleanup and launch authority. */
export async function newTaskEditorProduce(client: MobileDraftClient, route: NewTaskEditorRoute,
  target: MobileComposerTarget, intent: NewTaskProducerIntent, selection: { start: number; end: number },
  native: Native, storage: Files): Promise<NewTaskProducerResult> {
  const base: NewTaskProducerResult = { message: '', leave: false, voice: false, selection: null, effect: null };
  switch (intent.kind) {
    case 'navigate': return { ...base, leave: true, effect: { kind: 'navigate', destination: intent.destination, flow: null } };
    case 'preview': {
      let id = intent.id;
      if (intent.token) {
        const raw = obj(JSON.parse(intent.token)), reference = obj(raw.payload), text = mobileComposerTargetText(client, target);
        if (raw.value !== text) throw new ClientError('The attachment source changed.', 'superseded');
        const token = collectComposerContextReferences(text ?? '').find(row => row.source === reference.source
          && row.start === reference.start && row.end === reference.end);
        if (!token) throw new ClientError('The attachment range changed.', 'superseded');
        if (token.kind === 'file') id = draftFiles(client.local).find(file => file.draftKey === target.key
          && file.environmentId === target.environmentId && file.contextId === token.contextId)?.id ?? '';
        else if (token.kind === 'image') {
          const context = mobileNewTaskContextRead(client, target.key);
          id = context.ok ? str(context.context?.records.find(row => row.kind === 'image' && row.contextId === token.contextId)?.attachmentId) : '';
        }
      }
      const present = (client.local.snapshotDrafts[target.key] ?? []).some(row => row.id === id)
        || draftFiles(client.local).some(row => row.draftKey === target.key && row.environmentId === target.environmentId && row.id === id);
      if (!id || !present) throw new ClientError('The attachment is no longer in this draft.', 'superseded');
      return { ...base, leave: true, effect: { kind: 'preview', destination: id, flow: null } };
    }
    case 'remove': {
      const before = mobileComposerTargetText(client, target) ?? '';
      const result = await mobileNewTaskAttachmentRemove(intent.source, intent.id, native, storage, client, target.owner);
      const after = mobileComposerTargetText(client, target) ?? '';
      // Removal is a single contiguous source deletion. Preserve carets before it,
      // move carets after it, and clamp a selected removed reference to its start.
      let prefix = 0; while (prefix < Math.min(before.length, after.length) && before[prefix] === after[prefix]) prefix++;
      const deleted = before.length - after.length;
      const offset = (value: number) => Math.min(after.length, value <= prefix ? value : Math.max(prefix, value - deleted));
      return { ...base, message: result.message, selection: { start: offset(selection.start), end: offset(selection.end) } };
    }
    case 'send': {
      const flow = await mobileNewTaskFlowAction(route.owner, route.visit, intent.alternate ? 'send-alternate' : 'send',
        target.owner, '', native, storage, client, fleet, intent.now, intent.preferences);
      return { ...base, message: flow.message, leave: flow.submitted || !!flow.nextLocation,
        effect: { kind: 'flow', destination: '', flow } };
    }
    case 'voice': {
      const result = await mobileVoiceAction('start', selection.start, selection.end, intent.label, intent.owner, native, storage, client);
      return { ...base, message: result.message, voice: mobileVoiceSnapshot(intent.owner, client).phase !== 'idle',
        selection: mobileVoiceSelection(client, target) };
    }
  }
}
