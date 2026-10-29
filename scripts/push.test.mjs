import {expect, test} from 'bun:test';
import * as fs from 'node:fs';
import {tmpdir, homedir} from 'node:os';
import {resolve, relative} from 'node:path';
import {fileURLToPath} from 'node:url';
import {classifyArtifacts} from './app.mjs';

// The publisher's functions with boundary doubles, for an app at a fresh
// root whose cohort's binary has prelude runtime 4 and whose checkout's
// compat says 5. Bun's native subprocess import bypasses module mocks;
// explicit bindings also isolate deployment tests.
function harness(classify=()=>({bundle:true})) {
  const root=fs.mkdtempSync(resolve(tmpdir(),'exact-push-profile-'));
  const app={id:'fixture',name:'fixture',dir:root,target:root,manifest:{deploy:{signing:{keys:{test:'public'}}}}};
  const compat={id:'a'.repeat(32),target:'fixture',inputs:{app:'fixture',platform:'linux',store:{L:'A'},rustMode:'native',grantCeiling:'',keys:{test:'public'},typescriptRuntime:4}};
  const graph={artifacts:[{name:'app.plan',requires:{sources:{}},sha256:'plan'}]};
  const cohort={version:1,compat,binary:'binary',sources:{}};
  const build={trust:'production',compat,graph};
  const calls=[], published=[];
  const sealed=(name,env)=>{
    calls.push(name);
    expect(env?.EXACT_GAME_PROOF_PROFILE,`${name} shipping profile`).toBe('release');
    expect(env?.EXACT_UPDATE_TRUST,`${name} production trust`).toBe('production');
  };
  const bindings={
    ...Object.fromEntries(['existsSync','readFileSync','writeFileSync','mkdtempSync','rmSync','watch'].map(name=>[name,fs[name]])),
    resolve,relative,homedir,ROOT:fileURLToPath(new URL('..',import.meta.url)),
    process:{env:{EXACT_GAME_PROOF_PROFILE:'gpu-dev',EXACT_UPDATE_TRUST:'development'}},
    console:{log(){}},Refusal:Error,refuse(message){throw Error(message)},
    loadSigner:()=>({keyId:'test'}),resolveApp:()=>app,readBuilds:()=>[build],
    cohortReceipt:()=>cohort,classifyArtifacts:classify,
    openOrigin:()=>({writable:true,list:async()=>[],describe:()=>root}),
    channelOrigin:()=>root,sha256:()=> 'digest',blobPath:x=>x,streamPath:()=> 'stream',
    defaultRelease:()=> 'r-fixture',inspectHead(){throw Error('unexpected head')},
    requireReleasePins(){}, // the release pins' gate (game/inputs.mjs), not what this seals
    async publishStream({build}){
      expect(build.trust).toBe('production');calls.push('publish');published.push(build);
      return {action:'current',platform:'linux',compatibilityId:compat.id};
    },
    RustBaker:class {constructor(app,env){sealed('resident',env)}close(){calls.push('close')}},
    async buildRust(app,opts){sealed('rust',opts.env);expect(opts.profile).toBe('release');return {plan:Buffer.from('plan')}},
    rustBundle:(app,bundle,build)=>({bundle,build}),
    spawnSync(tool,args,opts){
      if(tool==='git')return {status:0,stdout:'b'.repeat(40)};
      const name=tool==='cargo'?'tools':['graph','compat'].includes(args[0])?args[0]:'typescript';
      sealed(name,opts.env);
      // The runtime this checkout's prelude provides (`exact-bake compat`).
      if(name==='compat')return {status:0,stdout:JSON.stringify({inputs:{typescriptRuntime:5}})};
      if(name==='typescript'){
        const out=args[args.indexOf('--out')+1];fs.mkdirSync(out,{recursive:true});
        fs.writeFileSync(resolve(out,'app.plan'),'plan');
        fs.writeFileSync(resolve(out,'app.hbc'),'bytecode');
        fs.writeFileSync(resolve(out,'app.module.json'),JSON.stringify({appId:'fixture',grants:''}));
      }
      return {status:0,stdout:JSON.stringify(graph)};
    },
  };
  const source=fs.readFileSync(new URL('./push.mjs',import.meta.url),'utf8');
  const functions=source.slice(source.indexOf('function parseArgs('),source.indexOf('\nif (process.argv[1]'))
    .replace('export async function pusher','async function pusher');
  const pusher=new Function(...Object.keys(bindings),functions+'\nreturn pusher;')(...Object.values(bindings));
  return {root,calls,published,bindings,pusher};
}

test('push seals every Rust, TypeScript and graph bake against an inherited proof profile', async () => {
  const {root,calls,published,bindings,pusher}=harness();
  try {
    for(const typescript of [false,true]){
      if(typescript)fs.writeFileSync(resolve(root,'app.ts'),'');
      calls.length=0;published.length=0;
      expect(await pusher({_:[root],origin:root,keys:root})).toBe(0);
      expect(calls).toEqual(['tools','resident',typescript?'typescript':'rust','graph',...(typescript?['compat']:[]),'publish','close']);
      expect(bindings.process.env).toEqual({EXACT_GAME_PROOF_PROFILE:'gpu-dev',EXACT_UPDATE_TRUST:'development'});
      // A module needs the runtime it was baked for, not the cohort's (LLP 1016.000 D10).
      if(typescript)for(const artifact of published[0].graph.artifacts)expect(artifact.requires.typescriptRuntime).toBe(5);
    }
  } finally {fs.rmSync(root,{recursive:true,force:true})}
});

test('push refuses a TypeScript module that needs a newer prelude than the cohort has', async () => {
  const {root,calls,pusher}=harness(classifyArtifacts);
  try {
    fs.writeFileSync(resolve(root,'app.ts'),'');
    await expect(pusher({_:[root],origin:root,keys:root})).rejects.toThrow(/typescriptRuntime \(requires a prelude of runtime 5 or later; this cohort's is 4\).*It needs a new binary/);
    expect(calls).not.toContain('publish');
  } finally {fs.rmSync(root,{recursive:true,force:true})}
});
