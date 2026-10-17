// Source365aa87982 pending-thread-feed.ts and ThreadFeed.tsx pending messages.
// @ref llp/1109.005-composer-and-transcript.decision.md#queued-command-construction
import type { T3Client } from './shared/client';
import { arr, obj, str } from './shared/domain';
import { bridgeReply, ClientError, type Native } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { mobileQueuedEditOrigin } from './queued-edit-origin';
import { mobileMediaURL } from './media-preview';
import { mobileOutboxSnapshot } from './mobile-outbox';
import { mobileOutboxDriveSnapshot, mobileOutboxThreadCompleted } from './mobile-outbox-drive';
import type { MobileOutboxRecord, MobileOutboxAttachment } from './mobile-outbox-model';

export interface ThreadOutboxMessage {
  owner:string; record:MobileOutboxRecord; status:string; reason:string; canRetry:boolean; acknowledged:boolean;
}
export const threadOutboxOwner=(r:MobileOutboxRecord)=>JSON.stringify({origin:r.origin,environmentId:r.environmentId,
  threadId:r.threadId,messageId:r.messageId,commandId:r.commandId});
const scope=(client:T3Client)=>JSON.stringify([mobileQueuedEditOrigin(client),client.environmentId,client.threadId,client.generation]);
/** A synchronized feed supplies echo IDs, including hidden rows. adoptStatus may
 * retain an old projection across generation/origin replacement; it cannot retire a new ACK. */
export function threadOutboxEchoes(client:T3Client):Set<string> {
  if(!client.threadLive||obj(client.projection.thread).id!==client.threadId)return new Set();
  return new Set([...arr(client.projection.messages).map(message=>str(message.id)),
    ...arr(client.projection.visibleTurnItems).filter(row=>row.sourceThreadId===client.threadId)
      .map(row=>str(obj(row.item).messageId))].filter(Boolean));
}
export function mobileThreadOutbox(client:T3Client,now:number):ThreadOutboxMessage[] {
  if(!client.environmentId||!client.threadId)return [];
  const home=mobileQueuedEditOrigin(client),echoes=threadOutboxEchoes(client),drive=mobileOutboxDriveSnapshot(client,now);
  const matches=(r:MobileOutboxRecord)=>!r.creation&&r.origin===home&&r.environmentId===client.environmentId&&r.threadId===client.threadId;
  const result=mobileOutboxSnapshot(client).rows.filter(row=>matches(row.record)&&!echoes.has(row.record.messageId)).map(row=>{
    const owner=threadOutboxOwner(row.record),item=drive.items.find(item=>item.owner===owner);
    return {owner,record:row.record,status:row.status==='optimistic'?'saving':row.status==='uncertain'?'recovery-required':item?.status??'queued',
      reason:row.status==='uncertain'?'The saved message needs its exact outcome checked.':item?.reason??'',
      canRetry:row.status==='confirmed'&&item?.canRetry===true,acknowledged:false};
  });
  for(const record of mobileOutboxThreadCompleted(client,home,echoes)) {
    if(matches(record)&&!echoes.has(record.messageId)&&!result.some(item=>item.record.messageId===record.messageId))
      result.push({owner:threadOutboxOwner(record),record,status:'delivered',reason:'',canRetry:false,acknowledged:true});
  }
  return result.sort((a,b)=>a.record.createdAt.localeCompare(b.record.createdAt));
}
export function threadOutboxStatus(status:string):string {
  return ({saving:'Saving…',sending:'Sending…',waiting:'Waiting to send',retry:'Waiting to retry',
    'recovery-required':'Needs attention','retirement-pending':'Finishing saved draft…','cleanup-pending':'Finishing send…',editing:'Being edited'} as Record<string,string>)[status]??'';
}

interface Preview {url:string;expires:number;retryAt:number;pending:boolean}
interface PreviewCache {scope:string;entries:Map<string,Preview>}
const previews=new WeakMap<T3Client,PreviewCache>();
const fileKey=(record:MobileOutboxRecord,file:MobileOutboxAttachment)=>JSON.stringify([threadOutboxOwner(record),file]);
function cache(client:T3Client):PreviewCache {
  const key=scope(client);let value=previews.get(client);
  if(!value||value.scope!==key){value={scope:key,entries:new Map()};previews.set(client,value)}return value;
}
export function threadOutboxPreview(client:T3Client,record:MobileOutboxRecord,file:MobileOutboxAttachment,now:number):string {
  const entry=cache(client).entries.get(fileKey(record,file));
  return entry&&entry.expires>now?entry.url:'';
}
/** Canonical local UUIDs stay readable while the outbox owns them. Never pass a local ID
 * to assets.createUrl. A descriptor or route change discards the old thumbnail answer. */
export async function prepareThreadOutboxPreviews(client:T3Client,nativeInput:Native,now:number):Promise<void> {
  if(!Number.isFinite(now)||now<0)return;
  const store=cache(client),native=letGoAware(nativeInput),items=mobileThreadOutbox(client,now);
  const wanted=items.flatMap(item=>item.record.attachments.filter(file=>file.kind==='image').map(file=>({record:item.record,file})));
  const live=new Set(wanted.map(item=>fileKey(item.record,item.file)));
  for(const key of store.entries.keys())if(!live.has(key))store.entries.delete(key);
  for(const {record,file}of wanted){
    const key=fileKey(record,file),old=store.entries.get(key);
    if(old&&(old.pending||old.retryAt>now))continue;
    const entry:Preview={url:old?.url??'',expires:old?.expires??0,retryAt:0,pending:true};
    store.entries.set(key,entry);
    const current=()=>store.entries.get(key)===entry&&previews.get(client)===store&&scope(client)===store.scope&&mobileThreadOutbox(client,now)
      .some(item=>threadOutboxOwner(item.record)===threadOutboxOwner(record)&&item.record.attachments.some(value=>fileKey(item.record,value)===key));
    try {
      if(!current())return;
      let url:string,expires=Number.MAX_SAFE_INTEGER;
      if(file.uploadId&&file.uploadEnvironmentId===record.environmentId) {
        if(client.connection!=='connected')throw new ClientError('Reconnect to load the uploaded preview.');
        const guarded:Native={available:true,watch:topic=>native.watch(topic),later:async request=>{
          if(!current())throw new ClientError('The pending message changed.','superseded');
          const reply=await native.later(request);
          if(!current())throw new ClientError('The pending message changed.','superseded');
          return reply;
        }};
        const reply=await client.rpc(guarded,'assets.createUrl',{resource:{_tag:'attachment',attachmentId:file.uploadId,
          fileName:file.name,mimeType:file.mimeType,disposition:'inline'}});
        if(!current())return;
        expires=Number(reply.expiresAt);
        if(!Number.isFinite(expires)||expires<=now)throw new ClientError('The attachment URL expired.');
        url=mobileMediaURL(client.origin,str(reply.relativeUrl));
      }else{
        const reply=await bridgeReply(native,{op:'mobileAttachmentPreview',id:file.id,image:true});
        if(!current())return;
        if(!reply.ok)throw new ClientError(reply.error!.message);
        url=str(obj(reply.value).dataUrl);
        if(!/^data:image\/(png|jpeg|webp);base64,[A-Za-z0-9+/]+=*$/.test(url))throw new ClientError('The local image preview is unavailable.');
      }
      Object.assign(entry,{url,expires,retryAt:file.uploadId?Math.min(expires,now+300_000):expires,pending:false});
    }catch(error){
      if(letGo(error)){if(current())store.entries.delete(key);throw error}
      if(current())Object.assign(entry,{url:'',expires:0,retryAt:now+300_000,pending:false});
    }finally{
      // Retire this invocation's abandoned slot even if focus moved away and back.
      if(entry.pending&&store.entries.get(key)===entry)store.entries.delete(key);
    }

  }
}
