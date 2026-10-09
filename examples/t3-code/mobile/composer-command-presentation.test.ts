// Source365aa87982 parent visibility and AppSymbol mappings, including iOS Tabler aliases.
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import {expect,test} from 'bun:test';
import {mobileComposerCommandPresentation as present} from './composer-command-presentation';
import {mobileCommandColors} from './design';
import type {ComposerCommandItem} from './composer-command-model';
const base={admission:'a',trigger:'path' as const,items:[] as ComposerCommandItem[],loading:false,error:null,voiceBusy:false};

test('empty path loading stays hidden while PR loading and errors stay visible',()=>{
  expect(present({...base,loading:true})).toMatchObject({visible:false,title:'FILES',empty:'Searching files…'});
  expect(present({...base,trigger:'pull-request',loading:true})).toMatchObject({visible:true,title:'PULL REQUESTS',empty:'Loading…'});
  expect(present({...base,trigger:'pull-request',error:'Unavailable'}).empty).toBe('Unavailable');
  expect(present({...base,trigger:'pull-request',error:''}).empty).toBe('');
  expect(present({...base,trigger:'pull-request',voiceBusy:true}).visible).toBe(false);
  expect(present({...base,trigger:'slash-model'}).visible).toBe(false);
});

test('file rows use bundled pinned PNGs and directory rows use the Tabler folder',()=>{
  const items:ComposerCommandItem[]=['clip.mp4','tsconfig.build.json','unknown.zzz'].map(path=>({
    id:path,type:'path',kind:'file',path,label:path,description:''}));
  items.push({id:'folder',type:'path',kind:'directory',path:'src',label:'src',description:''});
  const result=present({...base,items});
  expect(result.visible).toBe(true);
  expect(result.rows.map(row=>row.image)).toEqual(['assets/file-icons/pierre_video.png','assets/file-icons/pierre_typescript.png','assets/file-icons/pierre_default.png','']);
  expect(result.rows.map(row=>row.icon)).toEqual(['','','','folder']);
  expect(result.rows.map(row=>row.last)).toEqual([false,false,false,true]);
  expect(result.rows.map(row=>row.iconSize)).toEqual([16,16,16,16]);
});

test('slash skill prefix styling is separate from the exact skill name',()=>{
  const items:ComposerCommandItem[]=[{id:'s',type:'skill',skill:{name:'raw-name',enabled:true,path:'/repo/.agents/skills/raw-name/SKILL.md',scope:'repo'},label:'skill:raw-name',description:'Description'}];
  expect(present({...base,trigger:'slash-command',items}).rows[0]).toMatchObject({slashSkill:true,skillName:'raw-name',icon:'folder',iconSize:14});
  expect(present({...base,trigger:'skill',items}).rows[0]?.slashSkill).toBe(false);
});

test('command, thread and PR icons preserve the source alias classes',()=>{
  const items:ComposerCommandItem[]=[{id:'m',type:'slash-command',command:'model',label:'/model',description:'Switch model'},
    {id:'t',type:'thread',thread:{environmentId:'e',threadId:'t'},label:'Thread',description:'Thread'},
    {id:'p',type:'pull-request',pullRequest:{number:1,title:'PR',url:'https://example.test',headBranch:'a',baseBranch:'b',state:'open',isDraft:false},label:'#1',description:'open · PR'}];
  expect(present({...base,items}).rows.map(row=>row.icon)).toEqual(['terminal','thread','pull-request']);
});

test('popover palette uses source glass surface and all theme roles',()=>{
  for(const scheme of ['light','dark']) for(const palette of ['t3-code','t3-chat','grove','ocean','ember','iris']) {
    const colors=mobileCommandColors(scheme,palette);
    expect(Object.values(colors).every(value=>typeof value==='string' && value.length>0)).toBe(true);
  }
});
