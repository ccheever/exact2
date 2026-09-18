import {test, expect} from 'bun:test';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
const root = new URL('../../../../', import.meta.url).pathname;
function harness(fetch, requested=['a.tex']) {
  const source = readFileSync(root + '/host/web/gpu-glue.js','utf8').split('\nfunction size(')[0].replace(/^import .*;$/m,'').replace('20_000','35').replaceAll('5_000','10').replaceAll('250','1');
  const delivered=[], failed=[], names=[...requested];
  const context = {fetch, URL, Uint8Array, AbortController, setTimeout, clearTimeout, performance, console,
    document:{createElement:()=>({}),head:{append(){}},baseURI:'https://example.test/'}, exact:{},
    live:()=>entry, messages(){}, schedule(){}, module:{gpu_assets:()=>JSON.stringify(names.splice(0)),gpu_asset:(...args)=>{delivered.push(args);return true},gpu_asset_failed:(...args)=>{failed.push(args);return true}}};
  const entry={id:1,view:1};
  vm.createContext(context);
  vm.runInContext(source+'\ngpu=module; surfaces.set(1, globalThis.entry); finishReady(true); globalThis.testAssets={assets,settled,cancelAssets:typeof cancelAssets===\"function\"?cancelAssets:null};', Object.assign(context,{entry}));
  return {...context.testAssets,entry,delivered,failed};
}
test('404 is missing; transient failures retry before a named failure', async()=>{
 let calls=0;
 const h=harness(async()=>{calls++;return new Response('',{status:503});});
 await h.settled();
 expect(calls).toBe(3); expect(h.delivered.length).toBe(0);
 expect(h.failed[0][1]).toBe('a.tex'); expect(h.failed[0][2]).toContain('503');
 const missing=harness(async()=>new Response('',{status:404})); await missing.settled();
 expect(missing.delivered[0][2]).toBe(null); expect(missing.failed.length).toBe(0);
});
test('hung responses have a deadline and destroyed surfaces cancel flights', async()=>{
 let aborted=0;
 const fetch=(_, {signal}={})=>new Promise((_,reject)=>signal?.addEventListener('abort',()=>{aborted++;reject(new Error('aborted'))},{once:true}));
 const h=harness(fetch); const start=performance.now(); expect(await Promise.race([h.settled().then(()=>true),new Promise(r=>setTimeout(()=>r(false),150))])).toBe(true);
 expect(performance.now()-start).toBeLessThan(200); expect(aborted).toBeGreaterThan(0);
 const k=harness(fetch); k.assets(k.entry); expect(k.cancelAssets).not.toBe(null); k.cancelAssets(k.entry);
 await new Promise(r=>setTimeout(r,5)); expect(k.delivered.length+k.failed.length).toBe(0);
});

test('web and Swift enumerate the Rust name grammar cases', async()=>{
 const good=['a','a/b.tex','space name','x:y','x%20y','x?y#z','..foo','a_-.tex','a'.repeat(128)];
 const bad=['','/a','a/','a//b','.','..','a/./b','a/../b','a\\b','雪','a\n','a\x7f','a'.repeat(129)];
 const source=readFileSync(root+'/host/web/gpu-glue.js','utf8');
 const validate=vm.runInNewContext(source.slice(source.indexOf('function assetName('),source.indexOf('\nfunction cancelAssets'))+';assetName');
 for(const n of good) expect(validate(n)).toBe(true);
 for(const n of bad) expect(validate(n)).toBe(false);
 if(process.platform==='darwin') {
   const {mkdtempSync,writeFileSync,rmSync}=await import('node:fs');
   const {spawnSync}=await import('node:child_process');
   const temp=mkdtempSync('/tmp/s3ab-swift-names-');
   try {
     const encoded=Buffer.from(JSON.stringify({good,bad})).toString('base64');
     writeFileSync(temp+'/main.swift',`import Foundation\nlet cases = try! JSONSerialization.jsonObject(with: Data(base64Encoded:"${encoded}")!) as! [String:[String]]\nfor n in cases["good"]! { precondition(AssetResolver.validAssetName(n), n) }\nfor n in cases["bad"]! { precondition(!AssetResolver.validAssetName(n), n) }\n`);
     const built=spawnSync('/Library/Developer/CommandLineTools/usr/bin/swiftc',[root+'/host/apple/Sources/ExactKit/Assets.swift',temp+'/main.swift','-o',temp+'/names'],{encoding:'utf8',env:{...process.env,DEVELOPER_DIR:'/Library/Developer/CommandLineTools',SDKROOT:'/Library/Developer/CommandLineTools/SDKs/MacOSX26.sdk'}});
     if (built.status !== 0) throw new Error(JSON.stringify({stdout:built.stdout,stderr:built.stderr,error:built.error,status:built.status}));
     expect(spawnSync(temp+'/names').status).toBe(0);
   }finally{rmSync(temp,{recursive:true,force:true});}
 }
},20000);

test('settlement returns still outstanding names at its deadline', async()=>{
 const h=harness(()=>new Promise(()=>{}));
 const pending=await h.settled();
 expect(pending.map(p=>p.name)).toEqual(['a.tex']);
 h.cancelAssets(h.entry);
});
test('cancelled flights do not hold later settlement even if a transport ignores abort', async()=>{
 const h=harness(()=>new Promise(()=>{}));
 h.assets(h.entry); h.cancelAssets(h.entry);
 expect(await h.settled()).toEqual([]);
});
test('a transient response can recover and reserved URL characters remain filename bytes', async()=>{
 let calls=0, url;
 const h=harness(async(path)=>{url=String(path); return ++calls===1?new Response('',{status:500}):new Response(new Uint8Array([7,8]));},['space name/x?y#z%.tex']);
 await h.settled(); expect(calls).toBe(2); expect([...h.delivered[0][2]]).toEqual([7,8]);
 expect(url).toBe('https://example.test/assets/space%20name/x%3Fy%23z%25.tex');
});
