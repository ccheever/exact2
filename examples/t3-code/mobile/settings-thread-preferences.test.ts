import { afterEach, expect, test } from 'bun:test';
import { mobileClient } from './client';
import { obj, type Obj } from './shared/domain';
import { environmentSources } from './shared/connections';
import { fleet, environmentKey, type FleetEntry } from './shared/settings-b-fleet';
import type { Native } from './shared/protocol';
import { mobileThreadPreferences, mobileAutoSettleDays, mobileThreadTargets, mobileThreadPreferencesProjection, mobileThreadSettingsPatch, mobileThreadPreferencesCommand } from './settings-thread-preferences';
const scope = {environmentIds:['a','b'], members:null, projectLabel:''};
function source(id='a',settings:Obj={},capable=true) {return environmentSources({environmentId:id,origin:`http://${id}.test`,connection:'connected',statusMessage:'',scopes:[],config:{settings:{sidebarAutoSettleOnMerge:true,sidebarAutoSettleAfterDays:3,autoResumeLimitedThreads:true,snoozeLimitedThreads:false,...settings},environment:{capabilities:{threadAutoSettlement:capable,projectSettingsOverrides:true}}}},[{environmentId:id,origin:`http://${id}.test`}],new Map())[0]!;}
test('source excludes incapable servers but retains local controls; mixed usage and reference auto-settle differ',()=>{
 const sources=[source(),source('b',{autoResumeLimitedThreads:false,sidebarAutoSettleAfterDays:8})];
 const data=mobileThreadPreferencesProjection(scope,sources,new Set(['a','b']),{workingEnabled:true,planModeEnabled:true},true);
 expect(data.autoResumeMixed).toBe(true);expect(data.afterDays).toBe(3);expect(data.mismatchLabels).toBe('b.test');expect(data.workingEnabled).toBe(true);expect(data.planModeEnabled).toBe(true);
 const empty=mobileThreadPreferencesProjection(scope,[source('a',{},false)],new Set(),{},true);expect(empty.serverVisible).toBe(false);expect(empty.preferencesReady).toBe(true);
});
test('project membership order owns the reference; usage fields cannot be written through project scope',()=>{
 const project={...scope,members:[{environmentId:'b',id:'p2'},{environmentId:'a',id:'p1'}],projectLabel:'P'};
 const targets=mobileThreadTargets(project,[source(),source('b',{sidebarAutoSettleAfterDays:8})]);expect(targets[0]!.projectId).toBe('p2');
 expect(mobileThreadSettingsPatch('apply',JSON.stringify({sidebarAutoSettleOnMerge:true,sidebarAutoSettleAfterDays:8}),project,targets)).toEqual({sidebarAutoSettleOnMerge:true,sidebarAutoSettleAfterDays:8});
 expect(()=>mobileThreadSettingsPatch('snoozeLimitedThreads','true',project,targets)).toThrow('scope');
 expect(mobileThreadTargets({...project,members:[]},[source()])).toEqual([]);
});
test('inactive days accepts only complete 1–90 integers; toggling on uses source default3',()=>{
 for(const value of ['','1.5','12x','0','91','-3','Infinity'])expect(mobileAutoSettleDays(value,3)).toBeNull();
 expect(mobileAutoSettleDays(' 09 ',3)).toBe(9);expect(mobileAutoSettleDays('3',3)).toBeNull();
 const targets=mobileThreadTargets(scope,[source()]);expect(mobileThreadSettingsPatch('inactive','true',scope,targets)).toEqual({sidebarAutoSettleAfterDays:3});expect(mobileThreadSettingsPatch('inactive','false',scope,targets)).toEqual({sidebarAutoSettleAfterDays:null});
});
const original={origin:mobileClient.origin,environmentId:mobileClient.environmentId,connection:mobileClient.connection,config:mobileClient.config,generation:mobileClient.generation};
afterEach(()=>{Object.assign(mobileClient,original);fleet.entries.clear();});
function transport(denied=false,failB=false){
 const a=source(),b=source('b'),calls:Obj[]=[];Object.assign(mobileClient,{origin:a.origin,environmentId:'a',connection:'connected',generation:41,config:a.config});
 const entry: FleetEntry={key:environmentKey(b.origin,'b'),origin:b.origin,environmentId:'b',phase:'connected',generation:42,synchronized:42,config:b.config,shell:{projects:[],threads:[],sequence:0},message:'',traceId:'',lastEvent:0,subscriptions:{},scopes:[],error:'',requested:true};fleet.entries.set(entry.key,entry);
 const native:Native={available:true,watch(){},async later(input){const r=obj(input),second=typeof r.fleet==='string',item=second?b:a;calls.push(r);if(second&&failB&&r.method==='server.updateSettings')return {ok:false,generation:42,error:{kind:'server',message:'B refused'}};
 const value=r.op==='environments'?{saved:[a,b].map(s=>({origin:s.origin,environmentId:s.environmentId}))}:r.path==='/api/auth/session'?{authenticated:true,permissions:denied?[]:['settings:write']}:r.method==='server.getConfig'?item.config:r.method==='server.updateSettings'?{...obj(item.config.settings),...obj(obj(r.payload).patch)}:{};return {ok:true,generation:second?42:41,value};}};return {native,calls};
}
test('permission denial prevents every write, invalid day drafts dispatch nothing',async()=>{
 const f=transport(true);expect((await mobileThreadPreferencesCommand(JSON.stringify(scope),'inactive','true',f.native)).message).toContain('do not allow');expect(f.calls.some(c=>c.method==='server.updateSettings')).toBe(false);
 const count=f.calls.length;expect((await mobileThreadPreferencesCommand(JSON.stringify(scope),'days','5x',f.native)).message).toBe('');expect(f.calls.length).toBe(count);
});
test('real fan-out adopts successful config and reports failed environments',async()=>{
 const f=transport(false,true);const result=await mobileThreadPreferencesCommand(JSON.stringify(scope),'sidebarAutoSettleOnMerge','false',f.native);
 expect(result.message).toContain('Saved on some environments');expect(result.message).toContain('B refused');expect(obj(mobileClient.config.settings).sidebarAutoSettleOnMerge).toBe(false);expect(f.calls.filter(c=>c.method==='server.updateSettings')).toHaveLength(2);
});

test('Apply carries displayed defaults rather than replacing them with a refreshed reference',()=>{
 const targets=mobileThreadTargets(scope,[source('a',{sidebarAutoSettleAfterDays:8})]);
 expect(mobileThreadSettingsPatch('apply',JSON.stringify({sidebarAutoSettleOnMerge:false,sidebarAutoSettleAfterDays:3}),scope,targets)).toEqual({sidebarAutoSettleOnMerge:false,sidebarAutoSettleAfterDays:3});
 expect(()=>mobileThreadSettingsPatch('apply',JSON.stringify({sidebarAutoSettleOnMerge:true,sidebarAutoSettleAfterDays:999}),scope,targets)).toThrow('invalid');
});

test('pending writes keep the displayed server values and do not issue preparation reads',async()=>{
 const f=transport(),scopeJSON=JSON.stringify(scope);await mobileThreadPreferences(scopeJSON,{},true,f.native);
 const original=f.native.later;let release!:()=>void;let entered!:()=>void;const started=new Promise<void>(resolve=>{entered=resolve;});
 f.native.later=async input=>{if(obj(input).method==='server.updateSettings' && !obj(input).fleet){entered();await new Promise<void>(resolve=>{release=resolve;});}return original(input);};
 const saving=mobileThreadPreferencesCommand(scopeJSON,'sidebarAutoSettleOnMerge','false',f.native);await started;const count=f.calls.length;
 const pending=await mobileThreadPreferences(scopeJSON,{workingEnabled:true},true,f.native);expect(pending.settleMerged).toBe(true);expect(pending.disabled).toBe(true);expect(pending.workingEnabled).toBe(true);expect(f.calls).toHaveLength(count);
 release();expect((await saving).message).toBe('');
});


test('native day confirmation binds the original scope and reference value before writing',async()=>{
 const scopeJSON=JSON.stringify(scope), f=transport();
 const event=(owner:string,initial:number,raw:string)=>JSON.stringify({scope:owner,initial,raw});
 expect((await mobileThreadPreferencesCommand(scopeJSON,'days-native',event('other',3,'5'),f.native)).message).toContain('picker changed');
 expect(f.calls).toHaveLength(0);
 expect((await mobileThreadPreferencesCommand(scopeJSON,'days-native',event(scopeJSON,8,'5'),f.native)).message).toContain('value changed');
 expect(f.calls.filter(c=>c.method==='server.updateSettings')).toHaveLength(0);
 const saved=await mobileThreadPreferencesCommand(scopeJSON,'days-native',event(scopeJSON,3,'5'),f.native);
 expect(saved.message).toBe('');expect(f.calls.filter(c=>c.method==='server.updateSettings').map(c=>obj(obj(c.payload).patch).sidebarAutoSettleAfterDays)).toEqual([5,5]);
});
test('active native route exposes actual native picker and exact theme tokens; inactive route does no I/O',async()=>{
 const scopeJSON=JSON.stringify(scope),f=transport();
 const inactive=await mobileThreadPreferences(scopeJSON,{},true,null);expect(inactive.nativePicker).toBe(false);expect(f.calls).toHaveLength(0);
 const active=await mobileThreadPreferences(scopeJSON,{themeMode:'dark',baseFontSize:20},true,f.native);
 expect(active.scopeJSON).toBe(scopeJSON);expect(active.nativePicker).toBe(true);
 const appearance=obj(JSON.parse(active.pickerAppearance));expect(appearance.themeMode).toBe('dark');expect(appearance.baseFontSize).toBe(20);expect(typeof obj(appearance.dark).sheet).toBe('string');
});
