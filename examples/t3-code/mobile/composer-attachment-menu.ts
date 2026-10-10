// ComposerAttachmentButton / native UIButton.menu, upstream365aa87982.
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import {mobileClient} from './client';
import type {T3Client} from './shared/client';
import {obj} from './shared/domain';
import {ClientError,type Native,type Files} from './shared/protocol';
import {mobileComposerTarget} from './composer-target';
import {mobileEditorOwner} from './composer-editor-owner';
import {mobileComposerAttachments,mobileComposerAttachmentAction,mobileComposerAttachmentPicking} from './composer-attachments';
import {mobileComposerPickerHasWork} from './composer-picker';

export interface ComposerAttachmentMenuRoute {visit:string;url:string;active:boolean}
function identity(client:T3Client,route:ComposerAttachmentMenuRoute) {
  const target=mobileComposerTarget(client),owner=mobileEditorOwner(client);
  const mounted=owner?.target.owner===target.owner?owner:null;
  return {owner:target.owner,admission:mounted?.admission??'',mountId:mounted?.state.mountId??'',visit:route.visit,url:route.url};
}
function ready(client:T3Client,route:ComposerAttachmentMenuRoute):boolean {
  const target=mobileComposerTarget(client),owner=mobileEditorOwner(client);
  const work=mobileComposerPickerHasWork(client),snapshot=mobileComposerAttachments(client);
  if(!route.active||!route.visit||!route.url||client.busy||client.pending||mobileComposerAttachmentPicking(client)||(!snapshot.canPick&&!work))return false;
  return owner?.target.owner!==target.owner||!!owner&&owner.route.active&&owner.route.routeVisit===route.visit
    &&!owner.route.readOnly&&!owner.route.voiceBusy&&(!owner.pending||work)&&!owner.state.composing;
}
/** Menu projection captures only route ownership. Caret and insertion admission are
 * captured by the existing source producer after the user chooses an action. */
export function mobileComposerAttachmentMenuSnapshot(client:T3Client,route:ComposerAttachmentMenuRoute,now=0) {
  const snapshot=mobileComposerAttachments(client,now);
  return {...snapshot,menuConfiguration:JSON.stringify({...identity(client,route),enabled:ready(client,route),supportsFiles:snapshot.supportsFiles})};
}
/** The fresh root route is supplied at event invocation, not read from a retained
 * native answer. An old button cannot choose into a new visit or editor mount. */
export function mobileComposerAttachmentMenuAction(event:string,route:ComposerAttachmentMenuRoute,expectedOwner:string,
  native:Native|null|undefined,storage:Files,client:T3Client=mobileClient) {
  let value:Record<string,unknown>={};
  try {if(event.length<=8192)value=obj(JSON.parse(event))}catch {/* Refuse malformed native events. */}
  const current=identity(client,route);
  if(!ready(client,route)||expectedOwner!==current.owner
    ||Object.entries(current).some(([key,field])=>value[key]!==field)
    ||value.source!=='photos'&&value.source!=='files'
    ||value.source==='files'&&!mobileComposerAttachments(client).supportsFiles)
    throw new ClientError('The attachment button changed. Choose an action in the current composer.','superseded');
  return mobileComposerAttachmentAction(value.source,'',native,storage,client,current.owner);
}
