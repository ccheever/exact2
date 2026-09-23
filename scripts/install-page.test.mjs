import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync, writeFileSync, mkdirSync, renameSync, symlinkSync, existsSync } from 'node:fs';
import { spawn, spawnSync } from 'node:child_process';
import { filesystemRead } from './filesystem.mjs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createServer } from 'node:http';
import { developmentGate, developmentInstallPage, installPage, installProblems, writeInstallPages, installBrowserOrigins, installNetworkPage, localInstallURL } from './install-page.mjs';
import { readManifest, rustPolicy, rebuildPolicy } from './app.mjs';
import { developmentOpenPage, listPublicFiles, readStaticFile, serveStatic, staticWatchChanges, applyStaticTreeChange } from '../host/web/serve.mjs';
import { publishRoot } from './deploy.mjs';
import { DirectoryOrigin, webReleasePath } from './origin.mjs';
const manifest = {name:'Interview',app:{id:'com.interview.app',name:'Interview'}};

test('static ancestor notifications reconcile nested trees and retain refused replacements', () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-static-ancestor-'));
  const app = join(dir, 'app'), source = join(app, 'gpu/shaders'), target = join(dir, 'served/shaders');
  const trees = [[join(app, 'assets'), 'assets'], [source, 'shaders']];
  const reconcile = filename => {
    const changes = staticWatchChanges(app, trees, filename);
    assert.equal(changes.length, 1);
    assert.deepEqual(changes[0], {root:source,targetRoot:'shaders',relative:'',name:'shaders',tree:true});
    return applyStaticTreeChange(changes[0].root, target);
  };
  try {
    mkdirSync(source, { recursive: true });
    writeFileSync(join(source, 'live.wgsl'), 'first');
    assert.equal(reconcile('gpu').present, true);
    renameSync(join(app, 'gpu'), join(dir, 'retired'));
    assert.deepEqual(reconcile('gpu').files, [{name:'live.wgsl',bytes:null,removed:true}]);
    assert.equal(existsSync(target), false);
    mkdirSync(source, { recursive: true });
    writeFileSync(join(source, 'live.wgsl'), 'second');
    reconcile('gpu/');
    assert.equal(readFileSync(join(target, 'live.wgsl'), 'utf8'), 'second');
    rmSync(join(app, 'gpu'), { recursive: true });
    symlinkSync(join(dir, 'retired'), join(app, 'gpu'));
    assert.throws(() => reconcile('gpu'));
    assert.equal(readFileSync(join(target, 'live.wgsl'), 'utf8'), 'second');
    for (const name of ['gpu-other', 'assets-old', '../', join(dir, 'retired')]) {
      assert.deepEqual(staticWatchChanges(app, trees, name), []);
    }
    assert.equal(staticWatchChanges(app, trees, '.').length, 2);
    assert.equal(staticWatchChanges(app, trees, null).length, 2);
    assert.deepEqual(staticWatchChanges(app, trees, 'gpu/shaders/live.wgsl'),
      [{root:source,targetRoot:'shaders',relative:'live.wgsl',name:'shaders/live.wgsl',tree:false}]);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('Rust manifest policy inherits from global, environment, platform and platform environment', () => {
  for (const [platform, dev, prod] of [['web','browser','browser'],['ios','wasm','wasm'],['macos','native','native'],['linux','native','native'],['android','native','wasm'],['windows','native','native']]) {
    assert.equal(rustPolicy(manifest,platform),dev);
    assert.equal(rustPolicy(manifest,platform,'prod'),prod);
    assert.equal(rustPolicy({...manifest,rust:false},platform),'off');
  }
  const rust = {mode:'off',dev:true,prod:false,module:{package:'app-data'},platforms:{ios:{mode:'wasm',prod:false},linux:{prod:true}}};
  assert.equal(rustPolicy({...manifest,rust},'ios'),'wasm');
  assert.equal(rustPolicy({...manifest,rust},'ios','prod'),'off');
  assert.equal(rustPolicy({...manifest,rust},'linux','prod'),'native');
  assert.equal(rustPolicy({...manifest,rust},'macos','prod'),'off');
  assert.equal(rustPolicy({...manifest,rust:{prod:false,platforms:{ios:true}}},'ios','prod'),'wasm');
  assert.equal(rustPolicy({...manifest,rust:'wasm'},'macos'),'wasm');
  assert.deepEqual(rebuildPolicy(manifest),{rust:'save',typescript:'save'});
  assert.deepEqual(rebuildPolicy({...manifest,dev:{rebuild:{rust:'manual'}}}),{rust:'manual',typescript:'save'});
});

test('manifest refuses misspelled replacement and rebuild policy instead of silently enabling', () => {
  const dir = mkdtempSync(join(tmpdir(),'exact-rust-policy-'));
  try {
    for (const rust of [null,'jit',{enabled:false},{mode:false},{platforms:{iphone:false}},{dev:{prod:false}},{module:{package:'bad/path'}},{module:{}}]) {
      writeFileSync(join(dir,'app.json'),JSON.stringify({...manifest,rust}));
      assert.throws(()=>readManifest(dir,'interview'));
      assert.throws(()=>rustPolicy({...manifest,rust},'ios'));
    }
    for (const platform of ['web','ios']) assert.throws(()=>rustPolicy({...manifest,rust:'native'},platform),/native replacement is unavailable/);
    assert.throws(()=>rustPolicy(manifest,'macos','production'),/environment/);
    assert.throws(()=>rebuildPolicy({...manifest,dev:{rebuild:{rust:'watch'}}}));
    const good = {...manifest,rust:{prod:false,platforms:{ios:{prod:true}},module:{package:'app-data'}},dev:{rebuild:{rust:'manual',typescript:'save'}}};
    writeFileSync(join(dir,'app.json'),JSON.stringify(good));
    assert.deepEqual(readManifest(dir,'interview').rust,good.rust);
  } finally {rmSync(dir,{recursive:true,force:true});}
});

test('manifest methods reject wrong platforms, duplicates, unsafe URLs and recommendations', () => {
  const dir = mkdtempSync(join(tmpdir(),'exact-install-config-'));
  try {
    for (const install of [
      {ios:{methods:[{kind:'download',url:'https://example.com/a'}]}},
      {macos:{methods:[{kind:'download',url:'javascript:alert(1)'}]}},
      {macos:{methods:[{kind:'download',url:'https:example.com/file'}]}},
      {ios:{methods:[{kind:'go',url:'https://example.com/go',setupUrl:'javascript:alert(1)'}]}},
      {ios:{methods:[{kind:'direct',url:'itms-services://?action=download-manifest&url=http://example.com/m.plist'}]}},
      {macos:{methods:[{kind:'download',url:'https://user:pass@example.com/a'}]}},
      {ios:{recommended:'go',methods:[]}},
      {ios:{methods:[{kind:'testflight',url:'https://example.com/a'},{kind:'testflight',url:'https://example.com/b'}]}},
      {macos:{methods:[{kind:'terminal',url:'https://example.com/help'}]}},
      {android:{methods:[]}},
    ]) {
      writeFileSync(join(dir,'app.json'),JSON.stringify({...manifest,install}));
      assert.throws(()=>readManifest(dir,'interview'));
    }
    const good = {...manifest,install:{ios:{methods:[{kind:'direct',url:'itms-services://?action=download-manifest&url=https%3A%2F%2Fexample.com%2Fapp.plist'}]}}};
    assert.deepEqual(installProblems(good),[]);
    writeFileSync(join(dir,'app.json'),JSON.stringify(good));assert.ok(readManifest(dir,'interview'));
  } finally {rmSync(dir,{recursive:true,force:true});}
});

test('unavailable methods are not invented, text is escaped, terminal content cannot run', () => {
  const html = installPage(manifest);
  assert.ok(html.indexOf('id="web"') < html.indexOf('id="ios"'));
  assert.ok(html.indexOf('id="ios"') < html.indexOf('id="macos"'));
  assert.doesNotMatch(html,/<nav| hidden[ =>]|aria-current/);
  assert.match(html,/platform unavailable/);
  assert.match(html,/href="\/"/);assert.doesNotMatch(html,/href="https:\/\/testflight/);
  const configured = {...manifest,app:{...manifest.app,name:'<script>bad()</script>'},install:{macos:{recommended:'terminal',methods:[
    {kind:'download',url:'https://example.com/app.dmg'},
    {kind:'terminal',url:'https://example.com/help',command:'echo "</code><script>bad()</script>"'},
  ]}}};
  const page = installPage(configured);
  assert.match(page,/&lt;script&gt;bad\(\)&lt;\/script&gt;/);
  assert.doesNotMatch(page,/<script>bad/);
  assert.ok(page.indexOf('Install from Terminal')<page.indexOf('Download for Mac'));
  assert.match(page,/navigator.clipboard.writeText\(node.textContent\)/);
});

test('only a Mac development response adds local Simulator and signed-device actions', () => {
  const page = installPage(manifest);
  assert.match(page,/<!-- exact-local-ios -->/);
  assert.doesNotMatch(page,/data-local-ios/);
  const development = developmentInstallPage(page, 'a'.repeat(64));
  assert.match(development,/Development server/);
  assert.match(development,/data-local-ios/);
  assert.match(development,/Build, install, and open/);
  assert.match(development,/Simulator or device/);
  assert.match(development,/value\.targets/);
  assert.match(development,/\{target:select\.value\}/);
  assert.match(development,/x-exact-install-token/);
  assert.match(development,/\/__dev\/install\/ios/);
  assert.doesNotMatch(development,/<section class="platform unavailable" id="ios"/);
  assert.match(development,/<section class="platform unavailable" id="macos"/);
  assert.throws(() => developmentInstallPage(page, 'guessable'), /32 random bytes/);
});

test('install HTTP routes participate in atomic publication and preserve old pages', async () => {
  const dir = mkdtempSync(join(tmpdir(),'exact-install-publish-'));
  const web = join(dir,'web'), origin = new DirectoryOrigin(join(dir,'origin'));
  let server;
  try {
    writeInstallPages(web,{...manifest,install:{ios:{methods:[{kind:'testflight',url:'https://testflight.apple.com/join/example'}]}}});
    assert.equal(listPublicFiles(web).filter(p=>p.startsWith('.exact/install/')).length,4);
    assert.ok(readStaticFile(web,'/.exact/install'));
    const first = await publishRoot({origin,web,row:{}});
    const stream='install/'+'a'.repeat(64)+'/exact.json';
    const key={channel:'install',compatibilityId:'a'.repeat(64)};
    await origin.withLock(key,()=>origin.putHead(key,Buffer.from('{}'),{previousDigest:null}));
    assert.equal(readStaticFile(origin.dir,'/.exact/'+stream).body.toString(),'{}','install channel streams remain readable');
    server=createServer((req,res)=>serveStatic(origin.dir,req,res));
    await new Promise(r=>server.listen(0,'127.0.0.1',r));
    const base=`http://127.0.0.1:${server.address().port}`;
    for (const path of ['/.exact/install','/.exact/install/','/.exact/install/ios','/.exact/install/ios/','/.exact/install/macos/','/.exact/install/web/']) {
      const response=await fetch(base+path);assert.equal(response.status,200,path);
      assert.match(await response.text(),/Hosted release/);
      assert.match(response.headers.get('content-type'),/text\/html/);assert.equal(response.headers.get('cache-control'),'no-store');
    }
    const head=await fetch(base+'/.exact/install/ios/',{method:'HEAD'});assert.equal(head.status,200);assert.equal(await head.text(),'');
    assert.equal((await fetch(base+'/.exact/install/index.html',{headers:{accept:'application/vnd.exact.envelope+json'}})).status,200);
    for(const path of ['/.exact/install/linux/','/install','/.exact/install/private.pem']) assert.equal((await fetch(base+path)).status,404);
    writeInstallPages(web,manifest);
    await publishRoot({origin,web,row:{}});
    assert.doesNotMatch(await (await fetch(base+'/.exact/install/ios/')).text(),/testflight.apple.com/);
    const old=await fetch(base+`/${webReleasePath(first.root)}/.exact/install/ios/index.html`);
    const oldBody=await old.text();assert.match(oldBody,/testflight.apple.com/);assert.match(oldBody,/<!-- exact-serving -->Static hosting/);assert.match(old.headers.get('cache-control'),/immutable/);
  } finally {if(server)await new Promise(r=>server.close(r));rmSync(dir,{recursive:true,force:true});}
});

test('resident reads see root replacement, refuse links, and do not hold Bun open', async () => {
  const dir=mkdtempSync(join(tmpdir(),'exact-resident-read-'));
  try {
    const root=join(dir,'root'), other=join(dir,'other');
    mkdirSync(root);mkdirSync(other);
    writeFileSync(join(root,'value'),'before');writeFileSync(join(other,'value'),'other');
    const get=root=>filesystemRead({op:'get',root,path:'value'});
    const requests=Array.from({length:20},(_,i)=>get(i%2 ? other : root));
    assert.deepEqual((await Promise.all(requests)).map(x=>Buffer.from(x,'base64').toString()),Array.from({length:20},(_,i)=>i%2 ? 'other' : 'before'));
    renameSync(root,root+'.old');mkdirSync(root);writeFileSync(join(root,'value'),'after');
    assert.equal(Buffer.from(await get(root),'base64').toString(),'after');
    rmSync(join(root,'value'));assert.equal(await get(root),null);
    symlinkSync(join(other,'value'),join(root,'value'));await assert.rejects(get(root),/symlink/);
    assert.equal(Buffer.from(await get(other),'base64').toString(),'other','a refused read must not poison the pipe');
    await assert.rejects(filesystemRead({op:'put',root,path:'value'}),/only accepts get/);
    writeFileSync(join(other,'large'),Buffer.alloc(1536*1024,65));
    const program=`import {filesystemRead} from ${JSON.stringify(new URL('./filesystem.mjs',import.meta.url).href)};
      for(let wave=0;wave<3;wave++) {
        const replies=await Promise.all(Array.from({length:20},()=>filesystemRead({op:'get',root:${JSON.stringify(other)},path:'large'})));
        if(replies.some(value=>Buffer.from(value,'base64').length!==1536*1024)) throw Error('incomplete read');
        if(wave<2) await new Promise(resolve=>setTimeout(resolve,20));
      }
      console.log('done');`;
    const child=spawn(process.execPath,['--input-type=module','-e',program],{stdio:['ignore','pipe','pipe']});
    let output='', stderr='';
    await new Promise((resolve,reject)=>{
      let timer=setTimeout(()=>{child.kill();reject(new Error('reader startup timed out'));},180000);
      child.stdout.on('data',data=>{
        output+=data;
        if(output.includes('done')) {clearTimeout(timer);timer=setTimeout(()=>{child.kill();reject(new Error('idle reader kept Bun alive'));},2000);}
      });
      child.stderr.on('data',data=>{stderr+=data;});
      child.on('error',error=>{clearTimeout(timer);reject(error);});
      child.on('close',code=>{clearTimeout(timer);code===0 ? resolve() : reject(new Error(stderr || `child exited ${code}`));});
    });
    assert.equal(output.trim(),'done');
  } finally {rmSync(dir,{recursive:true,force:true});}
}, 200_000);

test('filesystem callers retain their helper across rebuilds and report failed operations without retrying', () => {
  const dir=mkdtempSync(join(tmpdir(),'exact-helper-capture-'));
  try {
    mkdirSync(join(dir,'scripts'));mkdirSync(join(dir,'bin'));
    writeFileSync(join(dir,'scripts/filesystem.mjs'),readFileSync(new URL('./filesystem.mjs',import.meta.url)));
    // Stand in for Cargo's mutable public output, independently of the real
    // workspace cache. A second caller publishes a different executable.
    writeFileSync(join(dir,'bin/cargo'),`#!/bin/sh
set -e
test -z "\${CLIPPY_ARGS-}"
test "\${RUSTC_WORKSPACE_WRAPPER-}" != /toolchain/clippy-driver
mkdir -p target/exact-filesystem-tool/debug
cp helper-source target/exact-filesystem-tool/debug/exact-filesystem
chmod 755 target/exact-filesystem-tool/debug/exact-filesystem
`,{mode:0o755});
    const helper=value=>`#!/bin/sh\nprintf '%s\\n' '${JSON.stringify({value})}'\n`;
    writeFileSync(join(dir,'helper-source'),helper('first'));
    const program=`
      import assert from 'node:assert/strict';
      import {writeFileSync,readFileSync} from 'node:fs';
      import {spawnSync} from 'node:child_process';
      import {filesystem} from './scripts/filesystem.mjs';
      assert.equal(filesystem({op:'get'}),'first');
      writeFileSync('helper-source',${JSON.stringify(helper('second'))});
      const next=spawnSync(process.execPath,['--eval',"import {filesystem} from './scripts/filesystem.mjs';console.log(filesystem({op:'get'}));"],{encoding:'utf8'});
      assert.equal(next.status,0,next.stderr);assert.equal(next.stdout.trim(),'second');
      assert.equal(filesystem({op:'get'}),'first','a rebuild must not replace a selected helper');
      writeFileSync('helper-source','#!/bin/sh\\nprintf x >> attempts\\nexit 42\\n');
      const failed=spawnSync(process.execPath,['--eval',"import {filesystem} from './scripts/filesystem.mjs';try{filesystem({op:'put'});process.exit(3)}catch(e){console.log(e.message)}"],{encoding:'utf8'});
      assert.equal(failed.status,0,failed.stderr);assert.match(failed.stdout,/status 42, signal none/);
      assert.equal(readFileSync('attempts','utf8'),'x','failed operations must not be retried');
    `;
    const child=spawnSync(process.execPath,['--eval',program],{cwd:dir,encoding:'utf8',env:{...process.env,PATH:join(dir,'bin')+':'+process.env.PATH,RUSTC_WORKSPACE_WRAPPER:'/toolchain/clippy-driver',CLIPPY_ARGS:'-D warnings'}});
    assert.equal(child.status,0,child.stderr);
  } finally {rmSync(dir,{recursive:true,force:true});}
});

test('browser destinations respect listener binding and keep public URLs explicit', () => {
  const interfaces={lo0:[{address:'127.0.0.1',family:'IPv4',internal:true}],en0:[{address:'192.168.1.20',family:'IPv4',internal:false}],utun:[{address:'100.84.2.3',family:'IPv4',internal:false}]};
  const options={host:'0.0.0.0',port:8879,interfaces};
  assert.deepEqual(installBrowserOrigins({...options,host:'127.0.0.1'}).map(x=>x.origin),['http://127.0.0.1:8879']);
  assert.deepEqual(installBrowserOrigins(options).map(x=>x.origin),['http://127.0.0.1:8879','http://192.168.1.20:8879','http://100.84.2.3:8879']);
  assert.equal(installBrowserOrigins({...options,host:'192.168.1.20'}).length,1);
  const configured={...manifest,install:{web:{methods:[{kind:'browser',url:'/app?hello=1'}],urls:[{label:'Public',url:'https://interview.example/app'}]}}};
  assert.deepEqual(installProblems(configured),[]);
  const page=installNetworkPage(installPage(configured),options);
  assert.match(page,/Tailscale \/ VPN/);assert.match(page,/https:\/\/interview.example\/app/);
  assert.ok(installProblems({...manifest,install:{web:{urls:[{label:'Bad',url:'javascript:alert(1)'}]}}}).length);
  assert.ok(installProblems({...manifest,install:{ios:{methods:[],urls:[]}}}).length);
});

test('a development server answers only to its printed names; its token and phone URL are its own', () => {
  const interfaces={lo0:[{address:'127.0.0.1',family:'IPv4',internal:true}],en0:[{address:'192.168.1.20',family:'IPv4',internal:false}]};
  const request=(host,peer)=>({headers:host===undefined?{}:{host},socket:{remoteAddress:peer}});
  const loopback=installBrowserOrigins({host:'127.0.0.1',port:8879,interfaces}), lan=installBrowserOrigins({host:'0.0.0.0',port:8879,interfaces});
  const local=developmentGate(loopback,8879), shared=developmentGate(lan,8879);
  assert.deepEqual(local.check(request('127.0.0.1:8879','127.0.0.1')),{allowed:true,local:true});
  assert.deepEqual(local.check(request('LOCALHOST:8879','::1')),{allowed:true,local:true});
  // DNS rebinding: a loopback socket reached under another name.
  assert.deepEqual(local.check(request('rebound.example:8879','127.0.0.1')),{allowed:false,local:false});
  assert.deepEqual(local.check(request('192.168.1.20:8879','127.0.0.1')),{allowed:false,local:false});
  assert.deepEqual(local.check(request(undefined,'127.0.0.1')),{allowed:false,local:false});
  assert.deepEqual(shared.check(request('192.168.1.20:8879','192.168.1.33')),{allowed:true,local:false});
  // A peer naming the loopback host is still not local.
  assert.deepEqual(shared.check(request('127.0.0.1:8879','192.168.1.33')),{allowed:true,local:false});
  assert.deepEqual(shared.loopbackOrigins,['http://127.0.0.1:8879','http://localhost:8879']);
  assert.equal(developmentGate(installBrowserOrigins({host:'127.0.0.1',port:80,interfaces}),80).check(request('127.0.0.1','127.0.0.1')).allowed,true);
  assert.equal(localInstallURL('simulator',lan,8879),'http://127.0.0.1:8879/');
  assert.equal(localInstallURL('device',lan,8879),'http://192.168.1.20:8879/');
  assert.throws(()=>localInstallURL('device',loopback,8879),/--lan/);
});

test('the dev opening page offers only the admitted, token-bearing links, escaped', () => {
  const app = {id:'test.one', displayName:'<b>One</b>', crate:()=>'one-apple', manifest:{}};
  const page = 'http://127.0.0.1:8879/?q="x"', token = 'a'.repeat(64);
  const html = developmentOpenPage(app, [{destination:'macos', href:`exact2-x://open?url=${encodeURIComponent(page)}&token=${token}`}], page);
  assert.match(html, new RegExp(`class="native" href="exact2-x://open\\?url=${encodeURIComponent(page).replaceAll('%', '%')}&amp;token=a{64}">Open in the Mac client<`));
  assert.ok(!html.includes('<b>One') && html.includes('&lt;b&gt;One'));
  assert.doesNotMatch(html, /No development client/);
  const none = developmentOpenPage(app, [], page);
  assert.doesNotMatch(none, /class="native"/);
  assert.match(none, /No development client built on this Mac admits this server/);
  assert.match(none, /--bundle --url http:\/\/127\.0\.0\.1:8879\/\?q=&quot;x&quot;/);
});
