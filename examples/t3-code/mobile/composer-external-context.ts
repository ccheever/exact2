// Ordinary Terminal/Review publication; queued edits and New Task keep their own owners.
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import type { T3Client } from './shared/client';
import type { Obj } from './shared/domain';
import { ClientError, type Files } from './shared/protocol';
import { letGo } from './shared/let-go';
import type { MobileComposerTarget } from './composer-target';
import { mobileEditorDocumentMembership } from './composer-editor-persistence';
import { mobileEditorOwner, mobileEditorCaptureContextInsertion, mobileEditorContextInsertionCurrent,
  mobileEditorCommitContextInsertion, mobileEditorContextTargetCurrent, mobileEditorContextInsertionPreflight,
  type EditorContextInsertionCapture } from './composer-editor-owner';

export const mobileExternalContextPreflight = mobileEditorContextInsertionPreflight;

export type ExternalContextCapture =
  | { kind:'legacy'; target:MobileComposerTarget; routePolicy:string }
  | { kind:'owned'; target:MobileComposerTarget; routePolicy:string; intent:EditorContextInsertionCapture };
function mounted(client:T3Client,target:MobileComposerTarget):boolean {
  const active=mobileEditorOwner(client);
  return !!active?.state.mountId&&active.target.key===target.key&&active.target.origin===target.origin
    &&active.target.environmentId===target.environmentId;
}
/** Capture before the producer's first await. Selection is the durable remembered
 * range, or the end; the plain textarea does not report its live caret here. */
export function mobileExternalContextCapture(client:T3Client,target:MobileComposerTarget,producer:'terminal'|'review',routePolicy:string):ExternalContextCapture|null {
  if(target.kind!=='ordinary'||!target.threadId||target.threadId.startsWith('new:')||target.key!==`${target.environmentId}:${target.threadId}`)return null;
  if(!mobileEditorContextInsertionPreflight(client))throw new ClientError('The saved draft cleanup metadata is unavailable. Keep the original draft.','retained');
  const membership=mobileEditorDocumentMembership(client,target);
  if(mounted(client,target)||membership==='unavailable')throw new ClientError('This draft is not available for context. Keep the original draft.','retained');
  if(membership==='unenrolled')return {kind:'legacy',target:{...target},routePolicy};
  const intent=mobileEditorCaptureContextInsertion(client,target,producer,routePolicy);
  if(!intent)throw new ClientError('The composer changed. Try again in the current draft.','superseded');
  return {kind:'owned',target:{...target},routePolicy,intent};
}
export function mobileExternalContextCurrent(client:T3Client,capture:ExternalContextCapture|null,routePolicy:string):boolean {
  if(!capture)return true;
  if(capture.routePolicy!==routePolicy||!mobileEditorContextInsertionPreflight(client)||!mobileEditorContextTargetCurrent(client,capture.target)||mounted(client,capture.target))return false;
  return capture.kind==='legacy'?mobileEditorDocumentMembership(client,capture.target)==='unenrolled'
    :mobileEditorContextInsertionCurrent(client,capture.intent,routePolicy);
}
/** Synchronous acceptance must precede clearing a producer's sheet or creating its card. */
export function mobileExternalContextInsert(client:T3Client,capture:ExternalContextCapture,text:string,record:Obj,routePolicy:string):void {
  if(capture.kind!=='owned'||!mobileExternalContextCurrent(client,capture,routePolicy))throw new ClientError('The composer changed. Keep the original draft.','superseded');
  const result=mobileEditorCommitContextInsertion(client,capture.intent,{text,context:{version:1,records:[record]}},routePolicy);
  if(!result.ok)throw new ClientError('This context could not be attached. Keep the original draft.',result.reason);
}
/** An accepted edit survives a failed save. Existing refresh owns guarded byte cleanup;
 * never replay the insertion or unlink files in this invocation's failure path. */
export async function mobileExternalContextPersist(client:T3Client,storage:Files):Promise<string> {
  try{await client.persist(storage);return ''}
  catch(error){if(letGo(error))throw error;return 'Context was attached, but the draft could not be saved. Keep the app open and try saving the draft again.'}
}
