import { mobileComposerRootSnapshot, mobileComposerRootAction, type ComposerRootInput, type ComposerRootAction, type ComposerRootResult } from './composer-root';
import { mobileNewTaskEditorView, mobileNewTaskEditorAction, mobileNewTaskEditorEffect, mobileNewTaskEditorFlushKey, type NewTaskEditorRoute, type NewTaskEditorIntent } from './new-task-editor';
import { mobileNewTaskFlowCurrent } from './new-task-flow';
import { mobileComposerEditorDecodeEvent } from './composer-editor-state';
import { mobileComposerTarget } from './composer-target';
import { mobileComposerAttachmentMenuChoice } from './composer-attachment-menu';
import { mobileNativeComposerTheme } from './design';
import { obj, str } from './shared/domain';
import type { MobileDraftClient } from './mobile-draft-recovery';
import type { Native, Files } from './shared/protocol';

export interface ComposerSurfaceInput extends ComposerRootInput {
  surface: 'thread' | 'new-task' | 'pending-task' | 'inactive'; flowOwner: string;
}
export function mobileComposerSurfaceInput(input: ComposerRootInput & { surface: string; flowOwner: string }): ComposerSurfaceInput {
  const surface = input.surface;
  if (surface !== 'thread' && surface !== 'new-task' && surface !== 'pending-task' && surface !== 'inactive') throw new Error('Unknown composer surface.');
  return { ...input, surface };
}
export interface ComposerSurfaceResult extends ComposerRootResult { navigation: string; location: string }
const route = (input: ComposerSurfaceInput): NewTaskEditorRoute => ({ owner: input.flowOwner, visit: input.visit, location: input.url, editorId: input.editorId });
const send = (input: ComposerSurfaceInput, alternate: boolean): NewTaskEditorIntent => ({ kind: 'send', alternate, now: input.now,
  preferences: { ready: input.preferencesReady, serialized: input.preferencesJSON } });
const destinations = { model: '/new/draft/settings', branch: '/new/draft/branch', environment: '/new/draft/environment' };

/** Root owns the coalesced observation and ordered action queue for both surfaces. */
export function mobileComposerSurfaceSnapshot(client: MobileDraftClient, input: ComposerSurfaceInput, latch: string) {
  const ordinary = mobileComposerRootSnapshot(client, { ...input, active: input.surface === 'thread' && input.active }, input.surface === 'thread' ? latch : '');
  if (input.surface !== 'new-task') return ordinary;
  const captured = route(input), view = mobileNewTaskEditorView(client, captured, latch), effect = mobileNewTaskEditorEffect(client, captured);
  const control = view.node ? obj(JSON.parse(view.node.configuration)) : {};
  const target = mobileComposerTarget(client);
  const configuration = view.node ? JSON.stringify({ ...control,
    voiceOwner: target.editorOwner,
    focusIntent: input.focusOperation === 'none' ? control.focusIntent
      : { serial: input.focusSerial, attempt: input.focusAttempt, operation: input.focusOperation },
    presentation: { themeJson: JSON.stringify(mobileNativeComposerTheme(input.scheme, input.themeId)), placeholder: 'Ask anything…',
      fontSize: 16, lineHeight: 23, fontFamily: 'DMSans-Regular', enterBehavior: input.enterBehavior,
      maxInputChars: 1_000_000, submitTitle: 'Send', alternateSubmitTitle: 'Send' } }) : '';
  return { ...ordinary, enabled: !!view.node, admission: view.node?.key ?? effect?.id ?? '', configuration,
    owner: target.owner, effectId: effect?.id ?? '', effectKind: effect ? 'new-task' : '',
    eventKey: mobileNewTaskEditorFlushKey(client, captured), message: view.message, revision: client.revision };
}
export async function mobileComposerSurfaceAction(client: MobileDraftClient, input: ComposerSurfaceInput, action: ComposerRootAction,
  native: Native, storage: Files): Promise<ComposerSurfaceResult> {
  if (input.surface !== 'new-task') return { ...await mobileComposerRootAction(client, { ...input, active: input.surface === 'thread' && input.active }, action, native, storage), navigation: '', location: '' };
  const captured = route(input), shown = mobileComposerSurfaceSnapshot(client, input, '');
  const answer: ComposerSurfaceResult = { revision: client.revision, message: '', visit: input.visit, editorId: input.editorId,
    owner: shown.owner, admission: shown.admission, effectId: '', effectKind: '', alternate: false, attachmentId: '', navigation: '', location: '' };
  const invoke = (intent: NewTaskEditorIntent) => mobileNewTaskEditorAction(client, captured, intent, native, storage);
  if (action.op === 'claim-effect') {
    const result = await invoke({ kind: 'claim', id: action.key }), effect = result.effect;
    if (!effect) return answer;
    answer.effectKind = 'new-task'; answer.effectId = effect.id;
    if (effect.kind === 'focus') { answer.navigation = 'focus'; return answer; }
    if (effect.kind === 'preview') { answer.attachmentId = effect.destination; return answer; }
    if (effect.kind === 'navigate') {
      answer.navigation = effect.destination === 'back' ? 'back' : 'push';
      answer.location = effect.destination === 'project' ? `/new?draftRoute=${encodeURIComponent(input.visit)}`
        : destinations[effect.destination as keyof typeof destinations] ?? '';
    } else if (effect.flow) {
      answer.message = effect.flow.message;
      answer.navigation = effect.flow.submitted ? 'thread' : effect.flow.nextLocation ? 'push' : '';
      answer.location = effect.flow.submitted ? `/threads/${encodeURIComponent(effect.flow.environmentId)}/${encodeURIComponent(effect.flow.threadId)}` : effect.flow.nextLocation;
    }
    return answer;
  }
  if (action.op === 'new-task' && action.key === 'back' && !shown.enabled
    && mobileNewTaskFlowCurrent(captured.owner, captured.visit, captured.location, client))
    return { ...answer, effectKind: 'new-task', navigation: 'back' };
  if (action.op === 'retry') { answer.message = (await invoke({ kind: 'retry' })).message; return answer; }
  if (!shown.enabled || !action.admission || action.admission !== shown.admission) return { ...answer, message: 'The new task editor changed.' };
  let intent: NewTaskEditorIntent | null = null;
  if (action.op === 'new-task') {
    if (action.key === 'send' || action.key === 'send-alternate') intent = send(input, action.key === 'send-alternate');
    else if (['model', 'branch', 'environment', 'project', 'back'].includes(action.key)) intent = { kind: 'navigate', destination: action.key as keyof typeof destinations | 'project' | 'back' };
    else if (action.key === 'preview') intent = { kind: 'preview', id: action.payload };
    else if (action.key === 'remove-image' || action.key === 'remove-file') intent = { kind: 'remove', source: action.key, id: action.payload };
    else if (action.key === 'voice') intent = { kind: 'voice', label: 'New task', owner: mobileComposerTarget(client).editorOwner };
    else if (action.key === 'attachment') intent = { kind: 'pick', source: mobileComposerAttachmentMenuChoice(action.payload,
      { visit: input.visit, url: input.url, active: true }, shown.owner, client) };
  } else if (action.op === 'event' || action.op === 'rich') {
    let raw: unknown; try { raw = JSON.parse(action.payload); } catch { raw = null; }
    const rich = obj(raw), event = mobileComposerEditorDecodeEvent(raw) ?? mobileComposerEditorDecodeEvent(rich.editorEvent);
    const observed = await invoke({ kind: 'observe', payload: event ?? raw });
    if (!observed.accepted && rich.kind !== 'contextPress' && rich.kind !== 'contentSize') return answer;
    if (event?.kind === 'submit') intent = send(input, event.alternate === true);
    else if (event?.kind === 'focus' || event?.kind === 'blur') answer.effectKind = event.kind;
    else if (rich.kind === 'contextPress') intent = { kind: 'preview', id: '', token: action.payload };
    else if (rich.kind === 'pasteText') answer.message = 'The pasted text is too long for this draft.';
    else if (rich.kind === 'pasteImages') answer.message = 'Paste an image using Add attachment → Photo Library.';
  }
  if (intent) answer.message = (await invoke(intent)).message;
  answer.revision = client.revision;
  return answer;
}
