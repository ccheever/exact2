import {expect,test} from 'bun:test';
import {mobileComposerAttachmentMenuSnapshot as menu,mobileComposerAttachmentMenuAction as choose} from './composer-attachment-menu';
import {mobileComposerRootSnapshot,mobileComposerRootAction,type ComposerRootInput} from './composer-root';
import {mobileEditorOwner} from './composer-editor-owner';
import {mobileComposerTarget} from './composer-target';
import {mobileComposerPickerHasWork} from './composer-picker';
import {MobileDraftClient,mobileDraftRecoveryHandles} from './mobile-draft-recovery';
import {fleet} from './shared/settings-b-fleet';
import {obj,type Obj} from './shared/domain';
import type {Native,Files} from './shared/protocol';

let serial=0;
async function fixture() {
  const client=new MobileDraftClient(),requests:Obj[]=[],writes:Obj[]=[];
  const storage:Files={fs:{async mkdir(){},async readFile(){return new TextEncoder().encode('{}').buffer},async atomicWriteFile(_p,bytes){writes.push(obj(JSON.parse(new TextDecoder().decode(bytes))))}}};
  const hydration=mobileDraftRecoveryHandles(client,{available:true,watch(){},async later(){return {ok:false,generation:0,error:{message:'Offline'}}}},storage);
  await client.refresh(hydration.native,hydration.storage);
  Object.assign(client,{origin:'https://attachment-menu.test',environmentId:`attachment-menu-${++serial}`,projectId:'p',threadId:'t',generation:1,connection:'disconnected'});
  fleet.saved.push({environmentId:client.environmentId,origin:client.origin});
  client.shell.projects=[{id:'p',workspaceRoot:'/repo'}];client.shell.threads=[{id:'t',projectId:'p',title:'Current'}];
  client.config={providers:[],environment:{capabilities:{fileAttachments:{maxUploadBytes:1024}}}};
  client.local.drafts[client.draftKey]='before';
  const input:ComposerRootInput={active:true,visit:'menu-visit',url:`/threads/${client.environmentId}/t`,environmentId:client.environmentId,threadId:'t',editorId:'composer-menu',
    preferencesReady:true,preferencesJSON:JSON.stringify({planModeEnabled:false,followUpBehavior:'queue'}),scheme:'light',themeId:'t3-code',enterBehavior:'send',focusSerial:'',focusAttempt:0,focusOperation:'none',now:1000};
  let failFinish=false,held:Promise<void>|null=null;
  const native:Native={available:true,watch(){},async later(raw){
    const request=obj(raw);requests.push(request);
    if(request.op!=='composerPickerIntake')throw Error(`Unexpected menu native operation: ${request.op}`);
    if(request.action==='pick')await held;
    if(request.action==='finish'&&failFinish){failFinish=false;throw Error('Lost empty picker finish')}
    return {ok:true,generation:1,value:{identity:request.identity,operationId:request.operationId,status:request.action==='finish'?'finished':'staged',files:[],error:''}};
  }};
  const root=()=>mobileComposerRootSnapshot(client,input);
  const event=async(value:string,selection:{start:number;end:number},kind='text')=>{
    const owner=mobileEditorOwner(client)!,state=owner.state;
    await mobileComposerRootAction(client,input,{op:'event',admission:owner.admission,key:'',payload:JSON.stringify({...state.identity,mountId:state.mountId||'menu-mount',kind,eventCount:state.eventCount+1,value,selection,focused:false,composing:false})},native,storage);
  };
  root();await event('before',{start:6,end:6},'ready');root();
  const route=()=>({visit:input.visit,url:input.url,active:input.active});
  const view=()=>obj(JSON.parse(menu(client,route()).menuConfiguration));
  const pick=(source='photos',capture=view())=>choose(JSON.stringify({...capture,source}),route(),mobileComposerTarget(client).owner,native,storage,client);
  return {client,input,native,storage,requests,writes,route,view,pick,event,root,failFinish:()=>{failFinish=true},hold:(wait:Promise<void>)=>{held=wait}};
}

test('opening or abandoning menu projects ownership without capturing insertion or starting native work',async()=>{
  const f=await fixture(),owner=mobileEditorOwner(f.client)!,before=owner.serial;
  expect(f.view()).toMatchObject({enabled:true,supportsFiles:true,owner:owner.target.owner,admission:owner.admission,mountId:'menu-mount',visit:f.input.visit,url:f.input.url});
  f.view();f.view();expect(owner.serial).toBe(before);expect(f.requests).toHaveLength(0);expect(f.writes).toHaveLength(0);
  const capture=f.view();await f.event('latest text',{start:2,end:6});
  await f.pick('files',capture);
  expect(f.requests[0]).toMatchObject({action:'pick',source:'files',document:{revision:mobileEditorOwner(f.client)!.document.revision}});
  expect(mobileEditorOwner(f.client)!.serial).toBe(before+1);
  expect(f.requests.map(r=>r.action)).toEqual(['pick','finish']);expect(f.client.draft).toBe('latest text');
});

test('native choices refuse departed route, new admission or mount, wrong owner and malformed source before IO',async()=>{
  const f=await fixture(),capture=f.view();
  for(const patch of [{owner:'other'},{admission:'old'},{mountId:'other'},{visit:'old'},{url:'/settings'}]) {
    expect(()=>f.pick('photos',{...capture,...patch})).toThrow('attachment button changed');
  }
  expect(()=>f.pick('remove-file')).toThrow('attachment button changed');
  expect(()=>choose('{',f.route(),String(capture.owner),f.native,f.storage,f.client)).toThrow('attachment button changed');
  f.input.active=false;expect(f.view().enabled).toBe(false);expect(()=>f.pick('photos',capture)).toThrow('attachment button changed');
  f.input.active=true;f.input.visit='replacement-visit';f.root();
  expect(()=>f.pick('photos',capture)).toThrow('attachment button changed');expect(f.requests).toHaveLength(0);
});

test('disabled, busy, voice and read-only policy is rechecked, and a server without Files keeps direct Photos',async()=>{
  const f=await fixture(),owner=mobileEditorOwner(f.client)!;
  for(const field of ['readOnly','voiceBusy'] as const) {
    owner.route[field]=true;expect(f.view().enabled).toBe(false);expect(()=>f.pick()).toThrow('attachment button changed');owner.route[field]=false;
  }
  f.client.busy=true;expect(f.view().enabled).toBe(false);expect(()=>f.pick()).toThrow('attachment button changed');f.client.busy=false;
  f.client.config.environment={capabilities:{}};expect(f.view()).toMatchObject({enabled:true,supportsFiles:false});
  expect(()=>f.pick('files')).toThrow('attachment button changed');await f.pick('photos');
  expect(f.requests[0]).toMatchObject({source:'photos'});
});

test('actual source picker owns busy state and an unresolved finish remains an explicit retry',async()=>{
  const f=await fixture();let release!:()=>void;const wait=new Promise<void>(done=>{release=done});f.hold(wait);
  f.failFinish();const pending=f.pick();expect(f.view().enabled).toBe(false);expect(()=>f.pick()).toThrow('attachment button changed');
  release();expect((await pending).message).toContain('Lost empty picker finish');expect(mobileComposerPickerHasWork(f.client)).toBe(true);
  expect(f.view().enabled).toBe(true);await f.pick();
  expect(f.requests.map(r=>r.action)).toEqual(['pick','finish','status','finish']);expect(mobileComposerPickerHasWork(f.client)).toBe(false);
});
