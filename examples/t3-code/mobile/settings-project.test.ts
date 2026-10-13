import { afterEach, beforeEach, expect, test } from 'bun:test';
import { mobileClient } from './client';
import { obj, type Obj } from './shared/domain';
import { environmentSources } from './shared/connections';
import { fleet, environmentKey, type FleetEntry } from './shared/settings-b-fleet';
import type { Native } from './shared/protocol';
import { mobileProjectOverview, mobileProjectGlyph, mobileProjectIdentity, mobileProjectMembers, mobileProjectProjection, mobileProjectRename } from './settings-project';
import { answer } from './app';
const scope={environmentIds:['a','b'],members:[{environmentId:'a',id:'p'},{environmentId:'b',id:'p'}],projectLabel:'Group'};
function source(id:string){return environmentSources({environmentId:id,origin:`https://${id}.test`,connection:'connected',statusMessage:'',scopes:[],config:{}},[{environmentId:id,origin:`https://${id}.test`}],new Map())[0]!;}
const projects=(name='repo')=>[{id:'p',title:name,workspaceRoot:'/work/repo',repositoryIdentity:{name:'repo',displayName:'owner/repo'}}];
test('member projection qualifies IDs and filters actual connected selected checkouts',()=>{
 const sources=[source('a'),{...source('b'),phase:'disconnected' as const}];
 const members=mobileProjectMembers(scope,sources,[{environmentId:'a',shell:{projects:projects()}},{environmentId:'b',shell:{projects:projects()}}]);
 expect(members.map(m=>m.environmentId)).toEqual(['a']);expect(mobileProjectProjection(scope,sources,members,new Set(['a'])).checkoutsLabel).toBe('1 checkout');
 expect(mobileProjectMembers({...scope,members:[]},sources,[])).toEqual([]);expect(mobileProjectMembers({...scope,members:null},sources,[])).toEqual([]);
});
test('group label and mobile Lucide override use shared identity without inventing a favicon',()=>{
 const members=[{environmentId:'a',project:projects()[0]!},{environmentId:'b',project:projects()[0]!}];
 const data=mobileProjectProjection(scope,[source('a'),source('b')],members,new Set(['a']));expect(data.name).toBe('owner/repo');expect(data.writable).toBe(false);expect(data.checkouts.map(c=>c.id)).toEqual(['a:p','b:p']);
 expect(mobileProjectGlyph({title:'T3 Code',projectIcon:{kind:'lucide',name:'rocket',color:'blue'}},'https://ignored.test/icon')).toMatchObject({kind:'monogram',text:'T3',src:''});
 expect(mobileProjectGlyph({title:'Repo'},'').kind).toBe('folder');
});
let originalClient: PropertyDescriptorMap, originalFleet: PropertyDescriptorMap, originalEntries: Map<string, FleetEntry>;
beforeEach(() => {
 originalClient = Object.getOwnPropertyDescriptors(mobileClient);
 originalFleet = Object.getOwnPropertyDescriptors(fleet);
 originalEntries = new Map(fleet.entries);
 fleet.entries.clear();
});
afterEach(() => {
 for (const key of Reflect.ownKeys(mobileClient)) if (!(key in originalClient)) Reflect.deleteProperty(mobileClient, key);
 Object.defineProperties(mobileClient, originalClient);
 for (const key of Reflect.ownKeys(fleet)) if (!(key in originalFleet)) Reflect.deleteProperty(fleet, key);
 Object.defineProperties(fleet, originalFleet);
 fleet.entries.clear();for (const [key, entry] of originalEntries) fleet.entries.set(key, entry);
});
function transport(options:{denyB?:boolean;failB?:boolean;missingB?:boolean}={}){
 const a=source('a'),b=source('b'),calls:Obj[]=[],aProjects=projects(),bProjects=options.missingB?[]:projects();let serial=0;
 Object.assign(mobileClient,{origin:a.origin,environmentId:'a',connection:'connected',generation:51,config:{},shell:{projects:aProjects,threads:[],sequence:1}});
 const entry:FleetEntry={key:environmentKey(b.origin,'b'),origin:b.origin,environmentId:'b',phase:'connected',generation:52,synchronized:52,config:{},shell:{projects:bProjects,threads:[],sequence:1},message:'',traceId:'',lastEvent:0,subscriptions:{},scopes:[],error:'',requested:true};fleet.entries.set(entry.key,entry);
 const native:Native={available:true,watch(){},async later(input){const r=obj(input),second=typeof r.fleet==='string',rows=second?bProjects:aProjects;calls.push(r);
 if(r.method==='projects.mutate') {if(second&&options.failB)return {ok:false,generation:52,error:{kind:'server',message:'B rename refused'}};rows[0]!.title=String(obj(r.payload).title);}
 const value=r.op==='environments'?{saved:[a,b].map(s=>({environmentId:s.environmentId,origin:s.origin}))}:r.path==='/api/auth/session'?{authenticated:true,permissions:second&&options.denyB?[]:['orchestration:operate']}:r.path==='/api/orchestration/shell'?{projects:rows,threads:[],snapshotSequence:2}:r.op==='ids'?Array.from({length:Number(r.count)},()=>`project-command-${++serial}`):{};
 return {ok:true,generation:second?52:51,value};}};
 const identity=mobileProjectIdentity([{environmentId:'a',project:projects()[0]!},{environmentId:'b',project:projects()[0]!}]);return {native,calls,identity};
}
test('rename validates every selected permission and captured membership before any write',async()=>{
 for(const options of [{denyB:true},{missingB:true}]){const f=transport(options);const result=await mobileProjectRename(JSON.stringify(scope),f.identity,'Renamed',f.native);expect(result.saved).toBe(false);expect(result.message).not.toBe('');expect(f.calls.some(c=>c.method==='projects.mutate')).toBe(false);}
});
test('rename writes real qualified members and refreshes canonical shells after full acknowledgment',async()=>{
 const f=transport();const result=await mobileProjectRename(JSON.stringify(scope),f.identity,'  Renamed  ',f.native);expect(result).toMatchObject({saved:true,message:''});
 const writes=f.calls.filter(c=>c.method==='projects.mutate');expect(writes).toHaveLength(2);expect(writes.map(c=>obj(c.payload).title)).toEqual(['Renamed','Renamed']);expect(writes[0]!.fleet).toBeUndefined();expect(writes[1]!.fleet).toBe(environmentKey('https://b.test','b'));
 expect(mobileClient.shell.projects[0]!.title).toBe('Renamed');
});
test('partial rename keeps failure visible and adopts only actual server records',async()=>{
 const f=transport({failB:true});const result=await mobileProjectRename(JSON.stringify(scope),f.identity,'Changed',f.native);expect(result.saved).toBe(false);expect(result.message).toContain('B rename refused');expect(mobileClient.shell.projects[0]!.title).toBe('Changed');expect(fleet.entries.get(environmentKey('https://b.test','b'))!.shell.projects[0]!.title).toBe('repo');
});


test('overview icon targets the first filtered checkout and does not request signed URLs or extra shells', async () => {
 const f=transport();
 const priorLoaded=mobileClient.shellLoaded;mobileClient.shellLoaded=true;
 try {
  const data=await mobileProjectOverview(JSON.stringify(scope),f.native);
  expect(data.faviconTarget).toMatchObject({environmentId:'a',cwd:'/work/repo',faviconPath:'',key:'["a","/work/repo",null]'});
  expect(data.icon.src).toBe('');expect(data.checkouts).toHaveLength(2);
  expect(f.calls.filter(c=>c.path==='/api/auth/session')).toHaveLength(2);
  expect(f.calls.some(c=>c.method==='assets.createUrl'||c.path==='/api/orchestration/shell')).toBe(false);
  const members=mobileProjectMembers(scope,[{...source('a'),phase:'disconnected' as const},source('b')],[{environmentId:'b',shell:{projects:projects()}}]);
  expect(mobileProjectProjection(scope,[source('b')],members,new Set()).faviconTarget.environmentId).toBe('b');
  members[0]!.project.projectIcon={kind:'emoji',emoji:'🌲'};
  expect(mobileProjectProjection(scope,[source('b')],members,new Set()).faviconTarget.key).toBe('');
 } finally {mobileClient.shellLoaded=priorLoaded;}
});

const allSelection = () => ({allEnvironments:true,environmentIds:[] as string[],projectKey:''});
const environmentRows = () => ['a','b'].map(environmentId=>({environmentId,label:environmentId.toUpperCase(),state:'connected',url:`https://${environmentId}.test`,machine:'laptop'}));
function scopeTransport() {
 const fixture=transport();mobileClient.shellLoaded=true;mobileClient.config={projectScope:true};
 for(const entry of fleet.entries.values())entry.config={projectScope:true};
 fleet.saved=['a','b'].map(environmentId=>({environmentId,origin:`https://${environmentId}.test`}));
 return fixture;
}
function scopeOperation(kind:string,value:string,header:string,visitToken='root:1',requestRoute='2') {
 return answer('settingsScopeEvent',[JSON.stringify({kind,value,visitToken,requestRoute}),header,visitToken,requestRoute]);
}
test('Project overview projects typed visit selection without replacing Back or inventing an absolute reply', async () => {
 const f=scopeTransport(), rows=environmentRows();
 const prepare=(selection=allSelection(),routeKey='2')=>answer('settingsRoot',['{}','light',rows,0,'1',selection,0,routeKey,false,'root:1']);
 const initial=prepare(), first=obj(JSON.parse(initial.projectHeader)), project=obj((first.projects as Obj[])[0]);
 expect(typeof project.id).toBe('string');
 expect(scopeOperation('project',String(project.id),initial.projectHeader)).toEqual({kind:'project',value:project.id,visitToken:'root:1',requestRoute:'2',availableEnvironmentIds:['a','b']});
 const selection={...allSelection(),projectKey:String(project.id)}, scoped=prepare(selection), header=obj(JSON.parse(scoped.projectHeader));
 expect(obj(JSON.parse(scoped.header))).toMatchObject({routeKey:'1',visitToken:'root:1',close:false});
 expect(header).toMatchObject({routeKey:'2',visitToken:'root:1',close:false,back:false,filtered:true,projectKey:project.id});
 expect(header.addActionID).toBeUndefined();expect((header.projects as Obj[]).find(row=>row.id===project.id)?.selected).toBe(true);
 expect(obj(JSON.parse(prepare(selection,'3').projectHeader))).toMatchObject({routeKey:'3',visitToken:'root:1',projectKey:project.id});
 expect(scopeOperation('project','',scoped.projectHeader)).toMatchObject({kind:'project',value:'',visitToken:'root:1',requestRoute:'2'});
 const cleared=prepare(), clearedHeader=obj(JSON.parse(cleared.projectHeader));
 expect(clearedHeader).toMatchObject({routeKey:'2',close:false,back:false,projectKey:'',projectLabel:'All projects'});
 const data=await mobileProjectOverview(cleared.serverScope,f.native);
 expect(data.empty).toBe(true);expect(data.emptyMessage).toBe('This project has no checkout on the selected connected environments. Change the filter above.');
 expect(f.calls.some(call=>call.method==='projects.mutate'||call.op==='writePreferences')).toBe(false);
});

test('compact Settings binds its own physical toolbar to the shared logical visit and preserves project filtering', () => {
 scopeTransport();const rows=environmentRows();
 const prepare=(compact:boolean,routeKey:string,selection=allSelection())=>answer('settingsRoot',['{}','light',rows,0,'1',selection,0,routeKey,compact,'root:1']);
 const regular=prepare(false,'2'), compact=prepare(true,'2');
 expect(obj(JSON.parse(compact.header))).toEqual({...obj(JSON.parse(regular.header)),close:true,compactRoot:true});
 expect(obj(JSON.parse(regular.header))).toMatchObject({routeKey:'1',visitToken:'root:1',close:false,back:false,compactRoot:false});
 expect(compact.projectHeader).toBe(regular.projectHeader);
 expect(obj(JSON.parse(compact.projectHeader))).toMatchObject({routeKey:'2',visitToken:'root:1',close:false,back:false});
 expect(scopeOperation('environment','b',compact.projectHeader)).toEqual({kind:'environment',value:'b',visitToken:'root:1',requestRoute:'2',availableEnvironmentIds:['a','b']});
 const projectKey=String(obj((obj(JSON.parse(compact.projectHeader)).projects as Obj[])[0]).id);
 const selection={allEnvironments:false,environmentIds:['a'],projectKey};
 const next=prepare(true,'3',selection);
 expect(obj(JSON.parse(next.header))).toMatchObject({routeKey:'1',visitToken:'root:1',compactRoot:true,all:false,projectKey});
 expect(obj(JSON.parse(next.projectHeader))).toMatchObject({routeKey:'3',visitToken:'root:1',close:false,back:false,all:false,projectKey});
 expect(obj(JSON.parse(next.serverScope)).environmentIds).toEqual(['a']);
 const splitAgain=prepare(false,'3',selection);
 expect(obj(JSON.parse(splitAgain.header))).toEqual({...obj(JSON.parse(next.header)),close:false,compactRoot:false});
 expect(splitAgain.projectHeader).toBe(next.projectHeader);expect(splitAgain.serverScope).toBe(next.serverScope);
 expect(prepare(true,'3',selection)).toEqual(next);
 expect(scopeOperation('close','',compact.header,'root:1','1')).toMatchObject({kind:'close',requestRoute:'1',visitToken:'root:1'});
});

test('Settings header admission refuses stale physical routes, logical visits and unavailable or disallowed actions', () => {
 scopeTransport();const rows=environmentRows();
 const data=answer('settingsRoot',['{}','light',rows,0,'1',allSelection(),0,'2',true,'root:1']);
 const refused={kind:'',value:'',visitToken:'',requestRoute:'',availableEnvironmentIds:[]};
 for(const [kind,value,header,token,route] of [
  ['environment','missing',data.projectHeader,'root:1','2'],['project','missing',data.projectHeader,'root:1','2'],
  ['back','',data.projectHeader,'root:1','2'],['close','',data.projectHeader,'root:1','2'],
  ['environment','a',data.projectHeader,'root:9','2'],['environment','a',data.projectHeader,'root:1','9'],
  ['all','unexpected',data.projectHeader,'root:1','2'],['all','', '', 'root:1','2'],
 ]) expect(scopeOperation(kind!,value!,header!,token!,route!)).toEqual(refused);
 expect(answer('settingsScopeEvent',[JSON.stringify({kind:'all',value:'',visitToken:'root:9',requestRoute:'2'}),data.projectHeader,'root:1','2'])).toEqual(refused);
 expect(answer('settingsScopeEvent',[JSON.stringify({kind:'all',value:'',visitToken:'root:1',requestRoute:'9'}),data.projectHeader,'root:1','2'])).toEqual(refused);
 expect(answer('settingsScopeEvent',[JSON.stringify({kind:'all',value:''}),data.projectHeader,'root:1','2'])).toEqual(refused);
 for(const [kind,value] of [['all',''],['environment','a'],['project','']])
  expect(scopeOperation(kind!,value!,data.projectHeader)).toMatchObject({kind,value,visitToken:'root:1',requestRoute:'2',availableEnvironmentIds:['a','b']});
 const projectKey=String(obj((obj(JSON.parse(data.projectHeader)).projects as Obj[])[0]).id);
 const unavailable=answer('settingsRoot',['{}','light',rows,0,'1',{allEnvironments:false,environmentIds:[],projectKey},0,'2',false,'root:1']);
 expect(obj(JSON.parse(unavailable.projectHeader))).toMatchObject({projectKey,projectLabel:'Unavailable project',all:false});
 expect(obj(JSON.parse(unavailable.serverScope)).environmentIds).toEqual([]);
});
