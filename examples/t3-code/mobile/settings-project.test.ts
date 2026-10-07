import { afterEach, expect, test } from 'bun:test';
import { mobileClient } from './client';
import { obj, type Obj } from './shared/domain';
import { environmentSources } from './shared/connections';
import { fleet, environmentKey, type FleetEntry } from './shared/settings-b-fleet';
import type { Native } from './shared/protocol';
import { mobileProjectGlyph, mobileProjectIdentity, mobileProjectMembers, mobileProjectProjection, mobileProjectRename } from './settings-project';
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
const original={origin:mobileClient.origin,environmentId:mobileClient.environmentId,connection:mobileClient.connection,config:mobileClient.config,generation:mobileClient.generation,shell:mobileClient.shell};
afterEach(()=>{Object.assign(mobileClient,original);fleet.entries.clear();});
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
