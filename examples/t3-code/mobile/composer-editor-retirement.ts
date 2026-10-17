// Durable Send completion over existing mounted CAS / explicit unmounted publication.
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import type { T3Client } from './shared/client';
import type { Native, Files } from './shared/protocol';
import { ClientError } from './shared/protocol';
import { mobileComposerTarget } from './composer-target';
import { mobileEditorOwner,mobileEditorCaptureDocumentIntent,mobileEditorCommitDocumentIntent,type EditorRouteInput } from './composer-editor-owner';
import { mobileEditorRequestRetirement,type EditorResult } from './composer-editor-runtime';
import { mobileEditorRetirementReceipt,mobileEditorRetirementAllowed,mobileEditorRetirementComplete,mobileEditorRetirementSnapshot,
  mobileEditorDocument } from './composer-editor-persistence';

/** Retains no operation or handle. A retirement key is plain independently persisted work. */
export async function mobileEditorFlushRetirement(client:T3Client,key:string,route:EditorRouteInput,native:Native,storage:Files):Promise<EditorResult> {
  const r=mobileEditorRetirementReceipt(client,key);
  if(!r)throw new ClientError('The confirmed draft receipt is unavailable. Keep the draft.','retained');
  const owner=mobileEditorOwner(client);
  const result=(message:string):EditorResult=>({revision:client.revision+mobileEditorRetirementSnapshot(client).revision,message,
    admission:owner?.admission??'',effect:owner?.effects[0]??null});
  if(r.phase==='retired'||r.phase==='preserved'){
    // A prior save may have committed and lost its reply, or failed before commit.
    // A fresh explicit flush persists the same terminal receipt without another native edit.
    await client.persist(storage);return result(r.phase==='preserved'?'Message sent. The draft text was kept.':'');
  }
  if(r.phase!=='confirmed')throw new ClientError('This message has not been confirmed.','retained');
  if(!mobileEditorRetirementAllowed(client,key)){
    mobileEditorRetirementComplete(client,key,'preserved');await client.persist(storage);return result('Message sent. The draft text was kept.');
  }
  const d=mobileEditorDocument(client,r.documentKey);
  if(!d)throw new ClientError('The draft ownership changed.','superseded');
  if(owner?.target.key===d.draftKey&&owner.state.mountId)return mobileEditorRequestRetirement(client,key,route,native,storage);
  const focused=mobileComposerTarget(client),target={...focused,kind:'ordinary' as const,environmentId:d.environmentId,threadId:d.threadId,key:d.draftKey,
    editorOwner:d.draftKey,editOwner:'',incarnation:'',owner:JSON.stringify(['ordinary',client.origin,d.environmentId,client.generation,d.draftKey])};
  const capture=mobileEditorCaptureDocumentIntent(client,target,'send-retirement',{start:0,end:0});
  if(!capture||capture.key!==r.documentKey||capture.incarnation!==r.incarnation||capture.revision!==r.revision
    ||!mobileEditorCommitDocumentIntent(client,capture,{value:'',selection:{start:0,end:0}})){
    mobileEditorRetirementComplete(client,key,'preserved');await client.persist(storage);return result('The draft changed; its text was kept.');
  }
  mobileEditorRetirementComplete(client,key,'retired');await client.persist(storage);return result('');
}
