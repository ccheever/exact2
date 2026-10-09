// @ref llp/1109.005-composer-and-transcript.decision.md#queued-command-construction
import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import type { Native } from './shared/protocol';
import { mobileOutboxRead } from './mobile-outbox';
import { mobilePendingTaskEditorsHydrate } from './mobile-pending-task-state';
import type { MobileOutboxRecord, MobileOutboxAttachment } from './mobile-outbox-model';
import { mobileThreadOutbox, prepareThreadOutboxPreviews, threadOutboxPreview } from './thread-outbox';
import { mobileThreadRows } from './thread';

const now=Date.parse('2026-10-09T12:00:00.000Z');
const image:MobileOutboxAttachment={kind:'image',id:'11111111-1111-4111-a111-111111111111',name:'one.png',mimeType:'image/png',sizeBytes:3,status:'staged',uploadId:''};
const file:MobileOutboxAttachment={kind:'file',id:'22222222-2222-4222-a222-222222222222',name:'two.txt',mimeType:'text/plain',sizeBytes:4,status:'staged',uploadId:'',contextId:'ctx',source:'picker'};
const record=(messageId='message',extra:Partial<MobileOutboxRecord>={}):MobileOutboxRecord=>({schemaVersion:1,origin:'https://home.test',environmentId:'env',threadId:'thread',messageId,commandId:`command-${messageId}`,text:'Captured prompt',attachments:[],createdAt:new Date(now).toISOString(),...extra});
const dataUrl='data:image/png;base64,YWJj';
async function fixture(initial:MobileOutboxRecord[]=[]) {
  const client=new T3Client();Object.assign(client,{origin:'https://home.test',environmentId:'env',threadId:'thread',projectId:'project',generation:1,connection:'disconnected'});
  mobilePendingTaskEditorsHydrate(client,{});
  let records=initial,floor=initial.length;
  const revisions:Record<string,number>=Object.fromEntries(initial.map(r=>[r.messageId,1]));
  const tokens:Record<string,string>=Object.fromEntries(initial.map(r=>[r.messageId,'epoch:1']));
  const calls:Obj[]=[];
  let preview:(request:Obj)=>Promise<unknown>=async()=>({dataUrl});
  const native:Native={available:true,watch(){throw Error('No watch')},async later(raw){
    const request=obj(raw);calls.push(request);
    const value=request.op==='mobileOutbox'?{ownerEpoch:'epoch',sequenceFloor:floor,complete:true,errors:[],
      records:records.map(record=>({record,revision:revisions[record.messageId],token:tokens[record.messageId],held:false,pending:false})),
      revisions,tokens,outcomes:[],mutations:[],transfers:[]}:await preview(request);
    return {ok:true,generation:request.generation??1,value};
  }};
  const read=async()=>{expect(await mobileOutboxRead(client,native)).toBe(true)};
  await read();calls.length=0;
  return {client,native,calls,read,setRecords(value:MobileOutboxRecord[]){
    floor++;for(const id of new Set([...records,...value].map(r=>r.messageId))){revisions[id]=(revisions[id]??0)+1;tokens[id]=`epoch:${floor}`}records=value;
  },setPreview(value:typeof preview){preview=value}};
}
function feed(client:T3Client,ids:string[],thread='thread') {
  client.threadLive=true;
  client.thread={sequence:1,historyCursor:null,hasMore:false,latestLocalTurnOrdinal:null,projection:{thread:{id:thread},
    messages:ids.map(id=>({id})),visibleTurnItems:[],runs:[],attempts:[],nodes:[],checkpoints:[],runtimeRequests:[]}};
}
function gate(){let resolve!:(value:unknown)=>void;const promise=new Promise(done=>{resolve=done});return {promise,resolve}}

test('ordinary offline rows retain mixed attachment order and follow the full feed',async()=>{
  const f=await fixture([record('later',{createdAt:new Date(now+1).toISOString()}),record('earlier',{text:'',attachments:[file,image]})]);
  expect(mobileThreadRows(f.client,now).map(row=>[row.id,row.body,row.timestamp,row.media.map(item=>item.id),row.first,row.last])).toEqual([
    ['outbox:earlier','','Pending',[file.id,image.id],true,false],['outbox:later','Captured prompt','Pending',[],false,true]]);
  feed(f.client,['earlier']);
  expect(mobileThreadOutbox(f.client,now).map(item=>item.record.messageId)).toEqual(['later']);
  f.client.thread!.projection.messages=[];
  f.client.thread!.projection.visibleTurnItems=[{sourceThreadId:'thread',item:{messageId:'later'}}];
  expect(mobileThreadOutbox(f.client,now).map(item=>item.record.messageId)).toEqual(['earlier']);
});
test('server, environment, thread and creation scope stay distinct',async()=>{
  const f=await fixture([record(),record('home',{origin:'https://other.test'}),record('env',{environmentId:'other'}),record('thread',{threadId:'other'}),
    record('creation',{creation:{projectId:'p',workspaceMode:'local',branch:null,worktreePath:null}})]);
  expect(mobileThreadOutbox(f.client,now).map(item=>item.record.messageId)).toEqual(['message']);
  feed(f.client,['message'],'foreign');expect(mobileThreadOutbox(f.client,now)).toHaveLength(1);
  f.client.threadId='other';expect(mobileThreadOutbox(f.client,now).map(item=>item.record.messageId)).toEqual(['thread']);
  f.client.environmentId='unknown';expect(mobileThreadOutbox(f.client,now)).toEqual([]);
});
test('local preview asks native for canonical bytes and never assets.createUrl',async()=>{
  const r=record('image',{attachments:[image,file]}),f=await fixture([r]);
  await prepareThreadOutboxPreviews(f.client,f.native,now);
  expect(f.calls).toEqual([{op:'mobileAttachmentPreview',id:image.id,image:true}]);
  expect(threadOutboxPreview(f.client,r,image,now)).toBe(dataUrl);
  expect(mobileThreadRows(f.client,now)[0]!.media[0]!.url).toBe(dataUrl);
  await prepareThreadOutboxPreviews(f.client,f.native,now+1);expect(f.calls).toHaveLength(1);
});
test('route changes and concurrent reads cannot publish a stale local preview',async()=>{
  const r=record('image',{attachments:[image]}),f=await fixture([r]),held=gate();f.setPreview(()=>held.promise);
  const preparing=prepareThreadOutboxPreviews(f.client,f.native,now);
  await prepareThreadOutboxPreviews(f.client,f.native,now);expect(f.calls).toHaveLength(1);
  f.client.threadId='other';held.resolve({dataUrl});await preparing;
  f.client.threadId='thread';expect(threadOutboxPreview(f.client,r,image,now)).toBe('');
  f.setPreview(async()=>({dataUrl}));await prepareThreadOutboxPreviews(f.client,f.native,now);
  expect(f.calls).toHaveLength(2);expect(threadOutboxPreview(f.client,r,image,now)).toBe(dataUrl);
});
test('deleting a queued image during native read discards its answer',async()=>{
  const r=record('image',{attachments:[image]}),f=await fixture([r]),held=gate();f.setPreview(()=>held.promise);
  const preparing=prepareThreadOutboxPreviews(f.client,f.native,now);
  f.setRecords([]);await f.read();held.resolve({dataUrl});await preparing;
  await prepareThreadOutboxPreviews(f.client,f.native,now);
  expect(threadOutboxPreview(f.client,r,image,now)).toBe('');
});
test('uploaded images mint a server URL from upload ID and expire at supplied time',async()=>{
  const uploaded={...image,status:'ready' as const,uploadId:'asset-one',uploadEnvironmentId:'env'},r=record('image',{attachments:[uploaded]}),f=await fixture([r]);
  f.client.connection='connected';
  const rpc:Obj[]=[];f.client.rpc=async(_native,method,payload)=>{rpc.push({method,payload});return {relativeUrl:'/assets/one',expiresAt:now+1000}};
  await prepareThreadOutboxPreviews(f.client,f.native,now);
  expect(rpc).toEqual([{method:'assets.createUrl',payload:{resource:{_tag:'attachment',attachmentId:'asset-one',fileName:'one.png',mimeType:'image/png',disposition:'inline'}}}]);
  expect(f.calls).toEqual([]);expect(threadOutboxPreview(f.client,r,uploaded,now)).toBe('https://home.test/assets/one');
  expect(threadOutboxPreview(f.client,r,uploaded,now+1000)).toBe('');
  f.client.rpc=async()=>({relativeUrl:'/assets/new',expiresAt:now+2000});await prepareThreadOutboxPreviews(f.client,f.native,now+1000);
  expect(threadOutboxPreview(f.client,r,uploaded,now+1000)).toBe('https://home.test/assets/new');
});
test('malformed previews retry on the supplied clock and let-go stays cancellation',async()=>{
  const r=record('image',{attachments:[image]}),f=await fixture([r]);f.setPreview(async()=>({dataUrl:'file:///private/image.png'}));
  await prepareThreadOutboxPreviews(f.client,f.native,now);expect(threadOutboxPreview(f.client,r,image,now)).toBe('');
  await prepareThreadOutboxPreviews(f.client,f.native,now+299999);expect(f.calls).toHaveLength(1);
  f.setPreview(async()=>{throw {name:'FetchError',kind:'Aborted'}});
  await expect(prepareThreadOutboxPreviews(f.client,f.native,now+300000)).rejects.toHaveProperty('kind','superseded');
  f.setPreview(async()=>({dataUrl}));await prepareThreadOutboxPreviews(f.client,f.native,now+300001);
  expect(threadOutboxPreview(f.client,r,image,now+300001)).toBe(dataUrl);
});
