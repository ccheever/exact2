import {expect, test} from 'bun:test';
import * as fs from 'node:fs';
import {tmpdir, homedir} from 'node:os';
import {resolve, relative} from 'node:path';
import {fileURLToPath} from 'node:url';

test('push seals every Rust, TypeScript and graph bake against an inherited proof profile', async () => {
  const root=fs.mkdtempSync(resolve(tmpdir(),'exact-push-profile-'));
  const app={id:'fixture',name:'fixture',dir:root,target:root,manifest:{deploy:{signing:{keys:{test:'public'}}}}};
  const compat={id:'a'.repeat(32),target:'fixture',inputs:{app:'fixture',platform:'linux',store:{L:'A'},rustMode:'native',grantCeiling:''}};
  const graph={artifacts:[{name:'app.plan',requires:{sources:{}},sha256:'plan'}]};
  const cohort={version:1,compat,binary:'binary'};
  const build={trust:'production',compat,graph};
  const calls=[];
  const sealed=(name,env)=>{
    calls.push(name);
    expect(env?.EXACT_GAME_PROOF_PROFILE,`${name} shipping profile`).toBe('release');
    expect(env?.EXACT_UPDATE_TRUST,`${name} production trust`).toBe('production');
  };
  // Run the publisher's functions with boundary doubles. Bun's native subprocess
  // import bypasses module mocks; explicit bindings also isolate deployment tests.
  const bindings={
    ...Object.fromEntries(['existsSync','readFileSync','writeFileSync','mkdtempSync','rmSync','watch'].map(name=>[name,fs[name]])),
    resolve,relative,homedir,ROOT:fileURLToPath(new URL('..',import.meta.url)),
    process:{env:{EXACT_GAME_PROOF_PROFILE:'gpu-dev',EXACT_UPDATE_TRUST:'development'}},
    console:{log(){}},Refusal:Error,refuse(message){throw Error(message)},
    loadSigner:()=>({keyId:'test'}),resolveApp:()=>app,readBuilds:()=>[build],
    cohortReceipt:()=>cohort,classifyArtifacts:()=>({bundle:true}),
    openOrigin:()=>({writable:true,list:async()=>[],describe:()=>root}),
    channelOrigin:()=>root,sha256:()=> 'digest',blobPath:x=>x,streamPath:()=> 'stream',
    defaultRelease:()=> 'r-fixture',inspectHead(){throw Error('unexpected head')},
    requireReleasePins(){}, // the release pins' gate (game/inputs.mjs), not what this seals
    async publishStream({build}){
      expect(build.trust).toBe('production');calls.push('publish');
      return {action:'current',platform:'linux',compatibilityId:compat.id};
    },
    RustBaker:class {constructor(app,env){sealed('resident',env)}close(){calls.push('close')}},
    async buildRust(app,opts){sealed('rust',opts.env);expect(opts.profile).toBe('release');return {plan:Buffer.from('plan')}},
    rustBundle:(app,bundle,build)=>({bundle,build}),
    spawnSync(tool,args,opts){
      if(tool==='git')return {status:0,stdout:'b'.repeat(40)};
      const name=tool==='cargo'?'tools':args[0]==='graph'?'graph':'typescript';
      sealed(name,opts.env);
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
  try {
    for(const typescript of [false,true]){
      if(typescript)fs.writeFileSync(resolve(root,'app.ts'),'');
      calls.length=0;
      expect(await pusher({_:[root],origin:root,keys:root})).toBe(0);
      expect(calls).toEqual(['tools','resident',typescript?'typescript':'rust','graph','publish','close']);
      expect(bindings.process.env).toEqual({EXACT_GAME_PROOF_PROFILE:'gpu-dev',EXACT_UPDATE_TRUST:'development'});
    }
  } finally {fs.rmSync(root,{recursive:true,force:true})}
});
