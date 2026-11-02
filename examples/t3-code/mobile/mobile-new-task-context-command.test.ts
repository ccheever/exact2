import { test, expect } from 'bun:test';
import { MobileDraftClient, mobileDraftRecoveryHandles } from './mobile-draft-recovery';
import { mobileNewTaskDraftCreate as create, mobileNewTaskDraftBind as bind, mobileNewTaskDraftRetarget as retarget, mobileNewTaskDraftDiscard as discard, mobileNewTaskDraftStore as store } from './mobile-new-task-drafts';
import { mobileNewTaskContextGuard as guard, mobileNewTaskContextWrite as write, mobileNewTaskContextRead as read } from './mobile-new-task-context';
import { composerEditorView, pullRequestRecords } from './shared/composer-editor';
import { contextLink } from './shared/composer-editor-menu';
import { obj, type Obj } from './shared/domain';
import type { Native, Files } from './shared/protocol';
import { setFanout } from './shared/r3-composer-controls-fanout';
const A='new-task:A', B='new-task:B', origin='https://commands.test';
const terminal = { version:1,kind:'terminal',contextId:'terminal_a',label:'Output',terminalId:'term',terminalLabel:'Term',lineStart:1,lineEnd:1,text:'frozen output' };
async function fixture() {
  const client = new MobileDraftClient(); let disk='{"version":1}', serial=0, hook: ((call:Obj)=>unknown)|undefined, writeHook: (()=>void)|undefined;
  const calls: Obj[]=[];
  const storage: Files={fs:{async mkdir(){},async readFile(){return new TextEncoder().encode(disk).buffer;},async atomicWriteFile(_path,bytes){writeHook?.();disk=new TextDecoder().decode(bytes);}}};
  const native: Native={available:true,watch(){},async later(input){const request=obj(input);calls.push(request); const override=hook?.(request); if(override!==undefined)return await override;
    const value=request.op==='status'?{phase:'disconnected'}:request.op==='ids'?Array.from({length:Number(request.count)},()=>`id-${++serial}`)
      :request.op==='editorSync'?{owner:client.snapshotOwner,trigger:{kind:'pull-request',query:'17',start:Math.max(0,client.draft.length-3),end:client.draft.length}}
      :request.method==='pullRequests.list'?{entries:[{projectId:'project',repository:'owner/repo',number:17,title:'A  title\nwith tabs\t'+ 'x'.repeat(2050),url:'https://example.test/pull/17',headBranch:'feat  branch',baseBranch:'main',state:'open',isDraft:false}]}
      :request.op==='http'?{authenticated:true,permissions:['orchestration:operate']}
      :request.op==='snapshotDraftRead'?{base64:'aGk='}:request.method==='attachments.createUploadUrl'?{attachmentId:'upload-a',relativeUrl:'/api/attachments/upload/a'}:{};
    return {ok:true,generation:client.generation,value};}};
  const handles=mobileDraftRecoveryHandles(client,native,storage);await client.refresh(handles.native,handles.storage);
  Object.assign(client,{origin,environmentId:'env',projectId:'project',threadId:'',generation:1,connection:'connected',configLive:true,shellLive:true,threadLive:true,scopes:['orchestration:operate'],providerId:'p',modelId:'m',runtimeMode:'full-access',interactionMode:'default'});
  client.config={environment:{capabilities:{serverResolvedCommandContext:true,pullRequests:true}},providers:[{instanceId:'p',driver:'codex',enabled:true,installed:true,status:'ready',auth:{status:'authenticated'},models:[{slug:'m'}],supportedRuntimeModes:['full-access']}]};
  client.shell.projects=[{id:'project',workspaceRoot:'/repo',repositoryIdentity:{owner:'owner',name:'repo'}}];client.shell.threads=[{id:'same',projectId:'project',title:'Original thread'}];
  for(const id of ['A','B'])create(client,{id,origin,environmentId:'env',projectId:'project',createdAt:'2026-10-08T00:00:00.000Z'});bind(client,A,'A');
  const command=(op:string,id='',value='')=>client.command(op,id,value,0,native,storage);
  const menu=async()=>{await composerEditorView(client,native);return composerEditorView(client,native);};
  return {client,native,storage,calls,command,menu,hook(fn?:typeof hook){hook=fn;},onWrite(fn?:typeof writeHook){writeHook=fn;}};
}
function records(c:MobileDraftClient,key=A){const r=read(c,key);if(!r.ok)throw Error(r.error);return r.context?.records??[];}
function attach(c:MobileDraftClient,record:Obj=terminal){const text=c.draft+contextLink(String(record.kind),String(record.contextId),String(record.label))+' ';return write(c,guard(c,c.draftKey)!,text,record);}
const launches=(f:Awaited<ReturnType<typeof fixture>>)=>f.calls.filter(x=>x.method==='orchestration.launchThread');
const run=async(p:Promise<unknown>)=>{try{return await p;}catch(e){return e;}};
test('actual PR menu picks allocate distinct source UUIDs and preserve raw metadata in one draft',async()=>{
  const f=await fixture();f.client.local.drafts[A]='#17';let menu=await f.menu();expect(menu.menu.rows).toHaveLength(1);
  expect((await f.command('editorlocal:pick',menu.menu.rows[0].id)).message).toBe('');
  const first=records(f.client)[0];expect(String(first.contextId)).toMatch(/^[a-f0-9-]{36}$/);expect(obj(first.pullRequest).title).toBe(('A  title\nwith tabs\t'+'x'.repeat(2050)).slice(0,2048));
  f.client.local.drafts[A]+='#17';menu=await f.menu();expect((await f.command('editorlocal:pick',menu.menu.rows[0].id)).message).toBe('');
  const next=records(f.client);expect(next).toHaveLength(2);expect(next[0].contextId).not.toBe(next[1].contextId);expect(pullRequestRecords(f.client,f.client.draft)).toEqual([]);
});
test('same-text A suggestion cannot be picked after switching to independent B',async()=>{
  const f=await fixture();f.client.local.drafts[A]='#17';f.client.local.drafts[B]='#17';const menu=await f.menu();bind(f.client,B,'B');
  const reply=await run(f.command('editorlocal:pick',menu.menu.rows[0].id));
  console.log('cross-key-pick',JSON.stringify({reply,records:records(f.client,B)}));
  expect(records(f.client,B)).toEqual([]);expect(f.client.local.drafts[B]).toBe('#17');
});
test('same thread ID keeps original captured environment after retarget and subsequent insertion',async()=>{
  const f=await fixture();expect((await f.command('editorlocal:insert-context','thread','same')).message).toBe('');const original=records(f.client)[0];
  retarget(f.client,A,{origin,environmentId:'other',projectId:'project'});f.client.environmentId='other';bind(f.client,A,'again');f.client.shell.threads[0].title='Changed B title';
  expect((await f.command('editorlocal:insert-context','thread','same')).message).toBe('');expect(records(f.client)).toEqual([original]);expect(original.environmentId).toBe('env');
});
test('real direct Send uses owned PR thread terminal payloads without global caches',async()=>{
  const f=await fixture();f.client.local.drafts[A]='#17';const menu=await f.menu();await f.command('editorlocal:pick',menu.menu.rows[0].id);await f.command('editorlocal:insert-context','thread','same');attach(f.client);const before=records(f.client);
  f.hook(call=>{if(call.method==='orchestration.launchThread')throw Error('reply lost');});const result=await run(f.command('send'));console.log('send-result',JSON.stringify(result));
  expect(launches(f)).toHaveLength(1);expect(obj(obj(launches(f)[0].payload).initialMessage).context).toEqual({version:1,records:before});expect(f.client.pending?.uncertain).toBe(true);
});
test('context payload changes during upload, IDs or durable pending save refuse before dispatch',async()=>{
  for(const stage of ['upload','ids','persist']){
    const f=await fixture();attach(f.client);let changed=false;
    const change=()=>{if(changed)return;changed=true;const next={...terminal,text:'new frozen output'};write(f.client,guard(f.client,A)!,f.client.draft,next);};
    if(stage==='upload')f.client.local.snapshotDrafts[A]=[{id:'11111111-1111-4111-a111-111111111111',name:'image.png',mimeType:'image/png',sizeBytes:2}];
    f.hook(call=>{if(stage==='upload'&&call.op==='uploadAttachment'||stage==='ids'&&call.op==='ids')change();});
    if(stage==='persist')f.onWrite(()=>{if(f.client.pending)change();});
    const result=await run(f.command('send'));console.log('guard-stage',stage,JSON.stringify(result));expect(changed).toBe(true);expect(launches(f)).toHaveLength(0);expect(records(f.client)[0].text).toBe('new frozen output');
  }
});
test('uncertain retry preserves original payload even after context changes',async()=>{
  const f=await fixture();attach(f.client);f.hook(call=>{if(call.method==='orchestration.launchThread')throw Error('lost');});await run(f.command('send'));
  expect(launches(f)).toHaveLength(1);const original=JSON.parse(JSON.stringify(launches(f)[0].payload));
  write(f.client,guard(f.client,A)!,f.client.draft,{...terminal,text:'new text after send'});f.hook();await run(f.command('retry'));
  expect(launches(f)).toHaveLength(2);expect(launches(f)[1].payload).toEqual(original);expect(records(f.client)[0].text).toBe('new text after send');
});

test('stale independent binding cannot fall back to global terminal insertion',async()=>{
  const f=await fixture();f.client.projectId='other';
  const result=await run(f.command('editorlocal:insert-context','terminal',JSON.stringify({id:'stale',threadId:'',terminalId:'term',terminalLabel:'Term',lineStart:1,lineEnd:1,text:'original output',createdAt:'2026-10-08T00:00:00.000Z'})));
  console.log('stale-bound-command',JSON.stringify({result,calls:f.calls.filter(c=>String(c.op).startsWith('editor')),terminalContexts:obj(f.client.local).terminalContexts}));
  expect(f.calls.some(c=>c.op==='editorInsert')).toBe(false);expect(obj(obj(f.client.local).terminalContexts)).toEqual({});
});
test('same-text recreated incarnation cannot reuse original PR suggestion',async()=>{
  const f=await fixture();f.client.local.drafts[A]='#17';const menu=await f.menu();discard(f.client,A);
  create(f.client,{id:'A',origin,environmentId:'env',projectId:'project',createdAt:'2026-10-08T00:00:01.000Z'});bind(f.client,A,'new');f.client.local.drafts[A]='#17';
  await run(f.command('editorlocal:pick',menu.menu.rows[0].id));expect(records(f.client)).toEqual([]);expect(f.client.draft).toBe('#17');
});
test('malformed or missing owned context blocks direct Send without global fallback',async()=>{
  for(const raw of [null,{version:2,records:[]},undefined]){
    const f=await fixture();f.client.local.drafts[A]=contextLink('terminal','terminal_a','Output');store(f.client).records[A].context=raw;
    Object.assign(f.client.local,{terminalContexts:{terminal_a:terminal}});await run(f.command('send'));
    expect(launches(f)).toHaveLength(0);expect(f.client.draft).toContain('terminal_a');
  }
});
test('desktop fanout state cannot divert independent direct Send away from owned context',async()=>{
  const f=await fixture();attach(f.client);setFanout(f.client,[{instanceId:'p',model:'m',options:[]},{instanceId:'p',model:'other',options:[]}]);
  f.hook(call=>{if(call.method==='orchestration.launchThread')throw Error('lost');});await run(f.command('send'));
  expect(launches(f)).toHaveLength(1);expect(obj(obj(launches(f)[0].payload).initialMessage).context).toEqual({version:1,records:[terminal]});
});
test('direct Send attachment mapping prefers local identity over earlier remote alias',async()=>{
  const f=await fixture(), localA='11111111-1111-4111-a111-111111111111',localB='22222222-2222-4222-a222-222222222222';
  f.client.local.snapshotDrafts[A]=[{id:localA,uploadId:localB,name:'first.png',mimeType:'image/png',sizeBytes:10},{id:localB,uploadId:'remote-b',name:'second.png',mimeType:'image/png',sizeBytes:20}];
  attach(f.client,{version:1,kind:'image',contextId:'image_b',label:'Second',attachmentId:localB,name:'second.png',mimeType:'image/png',sizeBytes:20});
  f.hook(call=>{if(call.method==='orchestration.launchThread')throw Error('lost');});const result=await run(f.command('send'));console.log('attachment-alias-send',JSON.stringify(result));
  expect(launches(f)).toHaveLength(1);expect(obj(obj(obj(launches(f)[0].payload).initialMessage).context).records).toEqual([{version:1,kind:'image',contextId:'image_b',label:'Second',attachmentId:'remote-b',name:'second.png',mimeType:'image/png',sizeBytes:20}]);
});
test('direct Send refuses frozen attachment descriptor disagreement',async()=>{
  for(const patch of [{name:'wrong.png'},{mimeType:'image/jpeg'},{sizeBytes:3}]){
    const f=await fixture(),id='11111111-1111-4111-a111-111111111111';
    f.client.local.snapshotDrafts[A]=[{id,uploadId:'remote',name:'actual.png',mimeType:'image/png',sizeBytes:2}];
    attach(f.client,{version:1,kind:'image',contextId:'image_a',label:'Photo',attachmentId:id,name:'actual.png',mimeType:'image/png',sizeBytes:2,...patch});
    await run(f.command('send'));expect(launches(f)).toHaveLength(0);expect(f.client.draft).toContain('image_a');
  }
});

test('repeated thread reference uses the new visible label but keeps its first captured payload', async () => {
  const f=await fixture();
  await f.client.command('editorlocal:insert-context','thread','same',0,f.native,f.storage);
  const first=read(f.client,A);expect(first.ok).toBe(true);
  f.client.shell.threads[0]!.title='Changed thread title';
  const result=await f.client.command('editorlocal:insert-context','thread','same',0,f.native,f.storage);
  expect(result.message).toBe('');expect(f.client.draft).toContain('[Changed thread title]');
  expect(read(f.client,A)).toEqual(first);
});
