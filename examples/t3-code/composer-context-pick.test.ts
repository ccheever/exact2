import {expect,test} from 'bun:test';
import {T3Client} from './client';
import {composerEditorView,editorLocal,peekComposerContextPick} from './composer-editor';
import {obj,type Obj} from './domain';
import type {Native} from './protocol';

function fixture() {
  const client=new T3Client();
  client.origin='https://example.test';client.environmentId='env';client.projectId='project';
  client.connection='connected';client.configLive=true;client.shellLive=true;
  client.config={environment:{capabilities:{pullRequests:true}}};
  client.shell.projects=[{id:'project',workspaceRoot:'/repo',repositoryIdentity:{provider:'github',owner:'acme',name:'atlas'}}];
  client.shell.threads=[{id:'thread',projectId:'project',title:'Thread'}];
  client.local.drafts[client.draftKey]='#17';
  let trigger:Obj={kind:'pull-request',query:'17',start:0,end:3};
  let stateOwner:string|null=null;const calls:Obj[]=[];
  const pr={projectId:'project',repository:'acme/atlas',number:17,title:'  Raw\n  '+ 'x'.repeat(2100),url:'https://github.com/acme/atlas/pull/17',headBranch:'one\n two',baseBranch:'main',state:'open',isDraft:false};
  const native:Native={available:true,watch(){},async later(input){const request=obj(input);calls.push(request);
    let value:unknown={};
    if(request.op==='editorSync') value={owner:stateOwner??client.snapshotOwner,trigger};
    if(request.op==='request'&&request.method==='pullRequests.list')value={entries:[pr],errors:[],providers:[]};
    if(request.op==='editorEdit')value={applied:true};
    return {ok:true,generation:client.generation,value};
  }};
  return {client,native,pr,calls,setTrigger:(next:Obj)=>{trigger=next},setOwner:(next:string)=>{stateOwner=next},async project(){await composerEditorView(client,native);return composerEditorView(client,native)}};
}

test('peek preserves original PR data and is detached, repeatable, and read-only',async()=>{
 const f=fixture();const view=await f.project();expect(view.menu.rows).toHaveLength(1);
 const id=view.menu.rows[0]!.id,before=f.calls.length,first=peekComposerContextPick(f.client,id)!;
 expect(JSON.parse(first.row.insert)).toEqual({number:17,title:f.pr.title,url:f.pr.url,headBranch:f.pr.headBranch,baseBranch:'main',state:'open',isDraft:false});
 expect(first.trigger).toEqual({kind:'pull-request',query:'17',start:0,end:3});
 first.row.insert='corrupted';first.trigger.start=999;
 expect(peekComposerContextPick(f.client,id)?.row.insert).not.toBe('corrupted');
 expect(peekComposerContextPick(f.client,id)?.trigger.start).toBe(0);expect(f.calls).toHaveLength(before);
 expect(peekComposerContextPick(f.client,'missing')).toBeNull();
 await editorLocal(f.client,f.native,'pick',id,'');expect(peekComposerContextPick(f.client,id)).toBeNull();
});

test('closed, unready and changed owner never expose prior rows',async()=>{
 const f=fixture();expect(peekComposerContextPick(f.client,'missing')).toBeNull();
 const id=(await f.project()).menu.rows[0]!.id;
 f.client.connection='disconnected';expect(peekComposerContextPick(f.client,id)).toBeNull();f.client.connection='connected';
 f.client.environmentId='other';expect(peekComposerContextPick(f.client,id)).toBeNull();f.client.environmentId='env';
 f.setOwner('stale');await f.project();expect(peekComposerContextPick(f.client,id)).toBeNull();
});

test('range validation rejects stale and malformed ranges without changing menu behavior',async()=>{
 const f=fixture(),id=(await f.project()).menu.rows[0]!.id;
 for(const [start,end] of [[-1,3],[2,1],[0,4],[0.5,3],[0,Infinity]]){
  f.setTrigger({kind:'pull-request',query:'17',start,end});await f.project();expect(peekComposerContextPick(f.client,id)).toBeNull();
 }
});

test('thread suggestion reads only its active exact row',async()=>{
 const f=fixture();f.client.local.drafts[f.client.draftKey]='@thr';f.setTrigger({kind:'path',query:'thr',start:0,end:4});
 const view=await f.project();expect(view.menu.rows).toHaveLength(1);
 const peek=peekComposerContextPick(f.client,'thread:thread');expect(peek?.row.label).toBe('Thread');expect(peek?.trigger.kind).toBe('path');
 f.setTrigger({});await f.project();expect(peekComposerContextPick(f.client,'thread:thread')).toBeNull();
});


test('generation or text changes and delayed stale projection cannot authorize a pick',async()=>{
 const f=fixture(),id=(await f.project()).menu.rows[0]!.id;
 f.client.generation++;expect(peekComposerContextPick(f.client,id)).toBeNull();
 await f.project();expect(peekComposerContextPick(f.client,id)).not.toBeNull();
 f.client.local.drafts[f.client.draftKey]='#18';expect(peekComposerContextPick(f.client,id)).toBeNull();
 await f.project();expect(peekComposerContextPick(f.client,id)).not.toBeNull();
 let release!:()=>void,started!:()=>void;
 const gate=new Promise<void>(resolve=>{release=resolve}),entered=new Promise<void>(resolve=>{started=resolve});
 const native:Native={...f.native,async later(input){const reply=await f.native.later(input);if(obj(input).op==='editorSync'){started();await gate;}return reply;}};
 const pending=composerEditorView(f.client,native);await entered;
 f.client.projectId='other';release();await pending;
 expect(peekComposerContextPick(f.client,id)).toBeNull();
 // Returning to the captured owner does not resurrect the stale publication.
 f.client.projectId='project';expect(peekComposerContextPick(f.client,id)).toBeNull();
});

// New boundary found while consuming the export from independent mobile drafts.
test('same-text independent draft keys never reuse another draft suggestion', async () => {
 const f=fixture();let key='new-task:A';Object.defineProperty(f.client,'draftKey',{get:()=>key});
 f.client.local.drafts[key]='#17';const id=(await f.project()).menu.rows[0]!.id;
 expect(peekComposerContextPick(f.client,id)).not.toBeNull();
 key='new-task:B';f.client.local.drafts[key]='#17';
 expect(peekComposerContextPick(f.client,id)).toBeNull();
});
