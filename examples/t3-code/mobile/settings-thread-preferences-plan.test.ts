import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import type { Native, Files } from './shared/protocol';
import { normalizeMobilePreferences, mobileSavePreference } from './settings-preferences';
import { applyMobileComposerBehavior, mobileSend } from './composer-behavior';
function fixture(enabled:boolean,started=true){
 const client=new T3Client();Object.assign(client,{environmentId:'env',projectId:'project',threadId:started?'thread':'',origin:'https://plan.test',connection:'connected',configLive:true,shellLive:true,threadLive:true,shellLoaded:true,scopes:['orchestration:operate'],providerId:'provider',modelId:'model',interactionMode:'plan'});
 client.config={environment:{capabilities:{serverResolvedCommandContext:true}},providers:[{instanceId:'provider',driver:'codex',enabled:true,installed:true,status:'ready',models:[{slug:'model',name:'Model'}]}]};
 client.shell.projects=[{id:'project',title:'Project',workspaceRoot:'/project'}];
 if(started)client.thread={sequence:1,historyCursor:null,hasMore:false,latestLocalTurnOrdinal:1,projection:{thread:{id:'thread',projectId:'project',interactionMode:'plan',modelSelection:{instanceId:'provider',model:'model'}},runs:[],turnItems:[]}};
 client.local.drafts[client.draftKey]='Continue this task';
 const calls:Obj[]=[];let serial=0;const storage:Files={fs:{async mkdir(){},async readFile(){return new ArrayBuffer(0);},async atomicWriteFile(){}}};
 const native:Native={available:true,watch(){},async later(input){const r=obj(input);calls.push(r);const value=r.op==='mobilePreferences'?{planModeEnabled:enabled,followUpBehavior:'queue'}:r.op==='ids'?Array.from({length:Number(r.count)},()=>`plan-id-${++serial}`):{};return {ok:true,generation:client.generation,value};}};
 return {client,native,storage,calls};
}
test('mobile Plan Mode default and hydration feed the actual shared device setting without changing mode',()=>{
 const f=fixture(false);expect(normalizeMobilePreferences({}).planModeEnabled).toBe(false);
 applyMobileComposerBehavior(f.client,{planModeEnabled:true});expect(f.client.local.deviceSettings.planModeEnabled).toBe(true);expect(f.client.interactionMode).toBe('plan');
 applyMobileComposerBehavior(f.client,{planModeEnabled:false});expect(f.client.local.deviceSettings.planModeEnabled).toBe(false);expect(f.client.interactionMode).toBe('plan');expect(f.calls).toEqual([]);
});
test('saved Plan Mode patches a boolean through the unified native preference owner',async()=>{
 const f=fixture(false);await mobileSavePreference('planModeEnabled','true',f.native);expect(obj(f.calls[0]!.patch)).toEqual({planModeEnabled:true});
 const count=f.calls.length;expect((await mobileSavePreference('planModeEnabled','yes',f.native)).message).not.toBe('');expect(f.calls).toHaveLength(count);
});
test('disabled Plan Mode stages Build at actual started-thread send admission',async()=>{
 const f=fixture(false);expect((await mobileSend(f.client,false,f.native,f.storage)).message).toBe('');
 const commands=f.calls.filter(c=>c.method==='orchestration.dispatchCommand').map(c=>obj(c.payload));
 expect(commands.find(c=>c.type==='thread.interaction-mode.set')?.interactionMode).toBe('default');expect(commands.some(c=>c.type==='message.dispatch')).toBe(true);expect(f.client.local.deviceSettings.planModeEnabled).toBe(false);
});
test('disabled Plan Mode sends a fresh draft as Build while enabled mode remains Plan',async()=>{
 for(const enabled of [false,true]){const f=fixture(enabled,false);await mobileSend(f.client,false,f.native,f.storage);const launch=f.calls.find(c=>c.method==='orchestration.launchThread');expect(launch).toBeDefined();expect(obj(launch!.payload).interactionMode).toBe(enabled?'plan':'default');}
});
test('a project switch during late preference read neither sends nor normalizes the new draft',async()=>{
 const f=fixture(false),later=f.native.later;
 f.native.later=async input=>{if(obj(input).op==='mobilePreferences'){f.client.projectId='other';f.client.interactionMode='plan';}return later(input);};
 await expect(mobileSend(f.client,false,f.native,f.storage)).rejects.toMatchObject({kind:'superseded'});
 expect(f.calls.some(c=>c.op==='request')).toBe(false);expect(f.client.interactionMode).toBe('plan');
});

test('a provider that hides Plan Mode admits Build even with the preference enabled',async()=>{
 const f=fixture(true,false);obj((f.client.config.providers as Obj[])[0]).showInteractionModeToggle=false;
 await mobileSend(f.client,false,f.native,f.storage);expect(obj(f.calls.find(c=>c.method==='orchestration.launchThread')!.payload).interactionMode).toBe('default');
});
