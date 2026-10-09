// Ordinary Voice publication only; native rich Voice still requires an awaited CAS outcome.
// @ref llp/1109.008-mobile-voice.decision.md#draft-and-selection
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import type { T3Client } from './shared/client';
import { ClientError } from './shared/protocol';
import type { MobileComposerTarget } from './composer-target';
import { mobileComposerContextRead } from './composer-command-context';
import { mobileEditorCaptureDocumentIntent, mobileEditorCommitDocumentIntent, mobileEditorOwner } from './composer-editor-owner';
import { mobileEditorDocumentMembership, mobileEditorDocumentIntentCurrent, type EditorDocumentIntent } from './composer-editor-persistence';

export type VoiceDocumentCapture =
  | { kind:'legacy'; target:MobileComposerTarget }
  | { kind:'owned'; target:MobileComposerTarget; intent:EditorDocumentIntent };
const ordinary=(target:MobileComposerTarget)=>target.kind==='ordinary'&&!!target.threadId&&!target.threadId.startsWith('new:')
  &&target.key===`${target.environmentId}:${target.threadId}`;
function mounted(client:T3Client,target:MobileComposerTarget):boolean {
  const active=mobileEditorOwner(client);
  return !!active?.state.mountId&&active.target.key===target.key&&active.target.origin===target.origin&&active.target.environmentId===target.environmentId;
}
/** Capture before readiness/selection awaits; never enroll an untouched textarea draft. */
export function mobileVoiceDocumentCapture(client:T3Client,target:MobileComposerTarget):VoiceDocumentCapture|null {
  if(!ordinary(target))return null;
  const membership=mobileEditorDocumentMembership(client,target);
  if(mounted(client,target)||membership==='unavailable')throw new ClientError('This draft is not available for dictation. Keep the original draft.','retained');
  if(membership==='unenrolled')return {kind:'legacy',target:{...target}};
  const intent=mobileEditorCaptureDocumentIntent(client,target,'voice');
  if(!intent)throw new ClientError('The saved draft changed before dictation could start.','superseded');
  return {kind:'owned',target:{...target},intent};
}
/** Captured membership, revision and incarnation survive every asynchronous operation. */
export function mobileVoiceDocumentCurrent(client:T3Client,capture:VoiceDocumentCapture|null):boolean {
  if(!capture)return true;
  if(mounted(client,capture.target))return false;
  const membership=mobileEditorDocumentMembership(client,capture.target);
  return capture.kind==='legacy'?membership==='unenrolled':membership==='enrolled'&&mobileEditorDocumentIntentCurrent(client,capture.intent);
}
export function mobileVoiceDocumentCommit(client:T3Client,capture:VoiceDocumentCapture,value:string,selection:{start:number;end:number}):boolean {
  if(capture.kind!=='owned'||!mobileVoiceDocumentCurrent(client,capture))return false;
  const context=mobileComposerContextRead(client,capture.target.key,capture.intent.before);
  if(!context.ok||!Number.isSafeInteger(context.revision)||context.revision>=Number.MAX_SAFE_INTEGER)return false;
  return mobileEditorCommitDocumentIntent(client,capture.intent,{value,selection});
}
