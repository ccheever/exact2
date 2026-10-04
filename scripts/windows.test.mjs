import {test, expect} from 'bun:test';
import assert from 'node:assert/strict';
import {spawn, spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {resolve} from 'node:path';
import {closeFilesystemReader, filesystem, filesystemErrorCode} from './filesystem.mjs';
import {packagedBuildChanges, removeBrowserProfile} from './agent-launch.mjs';
import {browserKey, open} from './agent.mjs';
import {runCaps} from './caps.mjs';
import {binaryenArchive, binaryenVersion} from './exact.mjs';
import {listPublicFiles, publicFileCards, readStaticFile, readStaticFileAsync, staticFile} from '../host/web/serve.mjs';
import {gameShells} from '../game/app/shells.mjs';

test.skipIf(process.platform !== 'win32')('owned browser profile cleanup retries a real sharing lock and refuses a persistent one', async () => {
  const root=mkdtempSync(resolve(tmpdir(),'exact-browser-cleanup-'));
  const script=resolve(root,'hold.ps1');
  writeFileSync(script, `param([string]$Path, [int]$Delay)
$held=[System.IO.File]::Open($Path,[System.IO.FileMode]::Open,[System.IO.FileAccess]::ReadWrite,[System.IO.FileShare]::None)
Write-Output READY
Start-Sleep -Milliseconds $Delay
$held.Dispose()
`);
  try {
    for (const delay of [800, 2200]) {
      const profile=resolve(root, String(delay)); mkdirSync(profile);
      const file=resolve(profile,'held'); writeFileSync(file,'owned');
      const child=spawn('powershell.exe',['-NoProfile','-File',script,file,String(delay)],{windowsHide:true,stdio:['ignore','pipe','pipe']});
      const exited=new Promise((ok,fail)=>{child.once('error',fail);child.once('exit',code=>code===0?ok():fail(new Error(`holder exited ${code}`)));});
      await new Promise((ok,fail)=>{child.stdout.on('data',data=>{if(String(data).includes('READY'))ok();});child.once('error',fail);});
      try {
        if (delay === 800) { await removeBrowserProfile(profile); expect(existsSync(profile)).toBe(false); }
        else { await assert.rejects(removeBrowserProfile(profile),{code:'EBUSY'}); expect(existsSync(file)).toBe(true); }
      } finally { await exited; }
      if (delay === 2200) expect(readFileSync(file,'utf8')).toBe('owned');
    }
  } finally { rmSync(root,{recursive:true,force:true}); }
}, 10000);

test('browser contextmenu reaches an off-center point and refuses invalid or covered points', async () => {
  const server = Bun.serve({port:0, fetch() { return new Response(`
    <div id="exact-root" data-boot-ms="1"><canvas id="world" style="position:absolute;left:20px;top:30px;width:100px;height:100px"></canvas>
    <button style="position:absolute;left:20px;top:30px;width:20px;height:20px">HUD</button></div>
    <script>
    const world=document.getElementById('world'), events=[];
    for (const type of ['pointerdown','pointerup','contextmenu']) world.addEventListener(type,e=>{e.preventDefault();events.push([type,e.clientX,e.clientY,e.button,e.isTrusted]);});
    const node={id:1,type:'canvas',props:{testId:'world'}}, box={id:1,x:20,y:30,w:100,h:100};
    const agent=async r=>r.op==='tags'?{clock:0}:r.op==='tree'?{nodes:[node]}:r.op==='layout'?{viewport:{w:420,h:900},nodes:[box]}:r.op==='state'?{events}:{};
    window.exact={ready:Promise.resolve(),views:new Map([[1,world]]),agent,agentSettled:agent};
    </script>`, {headers:{'content-type':'text/html'}}); }});
  let session;
  try {
    session=await open({host:'web',url:server.url.href});
    const reply=await session.tap('world',{contextmenu:true,at:[25,75]});
    expect(reply.at).toEqual([45,105]);
    const {events}=await session.carrier.ask({op:'state'});
    expect(events.map(event=>event[0]).sort()).toEqual(['contextmenu','pointerdown','pointerup']);
    expect(events.every(event=>event[1]===45 && event[2]===105 && event[3]===2 && event[4]===true)).toBe(true);
    for(const at of [null,[],[25],[25,75,0],['25',75],[NaN,75],[25,Infinity],[-1,75],[100,75],[25,100],[5,5]]) {
      await assert.rejects(session.tap('world',{contextmenu:true,at}), /contextmenu|covers/);
    }
    expect((await session.carrier.ask({op:'state'})).events).toEqual(events);
    await session.tap('world',{down:true,at:[25,75]});
    await assert.rejects(session.tap('world',{contextmenu:true,at:[25,75]}), /held contact/);
    await session.pointer('up');
  } finally { await session?.close(); server.stop(true); }
},60000);

test('explicit primary mouse replaces a touch history and preserves real mouse identity', async () => {
  const server=Bun.serve({port:0,fetch(){return new Response(`<div id="exact-root" data-boot-ms="1"><canvas id="world" style="position:absolute;left:20px;top:30px;width:100px;height:100px"></canvas><button style="position:absolute;left:20px;top:30px;width:20px;height:20px">HUD</button></div><script>
    const world=document.getElementById('world'), events=[];
    for(const type of ['pointerdown','pointerup']) world.addEventListener(type,e=>{e.preventDefault();events.push([e.pointerType,e.pointerId,e.type,e.clientX,e.clientY,e.buttons,e.isTrusted]);});
    world.addEventListener('contextmenu',e=>e.preventDefault());
    const agent=async r=>r.op==='tags'?{clock:0}:r.op==='tree'?{nodes:[{id:1,type:'canvas',props:{testId:'world'}}]}:r.op==='layout'?{viewport:{w:420,h:900},nodes:[{id:1,x:20,y:30,w:100,h:100}]}:r.op==='state'?{events}:{};
    window.exact={ready:Promise.resolve(),views:new Map([[1,world]]),agent,agentSettled:agent};
    </script>`,{headers:{'content-type':'text/html'}});}});
  let s;
  try {
    s=await open({host:'web',url:server.url.href});
    await s.tap('world',{down:true,at:[25,75]}); await s.pointer('up');
    await s.tap('world',{mouse:true,at:[25,75]});
    await s.tap('world',{contextmenu:true,at:[25,75]});
    const {events}=await s.carrier.ask({op:'state'});
    expect(events.slice(0,2).every(e=>e[0]==='touch' && e[1]>1)).toBe(true);
    expect(events.slice(2)).toEqual([1,0,2,0].map((buttons,i)=>['mouse',1,i%2?'pointerup':'pointerdown',45,105,buttons,true]));
    for(const at of [null,[],[25],[25,75,0],['25',75],[NaN,75],[25,Infinity],[-1,75],[100,75],[25,100],[5,5]]) await assert.rejects(s.tap('world',{mouse:true,at}), /mouse|covers/);
    for(const opts of [{down:true},{contextmenu:true},{wheel:[0,1]},{drag:{dx:1,dy:1}}]) await assert.rejects(s.tap('world',{mouse:true,...opts}), /another input mode/);
    expect((await s.carrier.ask({op:'state'})).events).toEqual(events);
    await s.tap('world',{down:true,at:[25,75]});
    await assert.rejects(s.tap('world',{mouse:true,at:[25,75]}), /held contact/); await s.pointer('up');
  } finally {await s?.close();server.stop(true);}
},60000);

test.skipIf(process.platform !== 'win32')('release Windows game shells use GUI executables and preserve agent pipes', () => {
  const root=mkdtempSync(resolve(tmpdir(),'exact GUI shell '));
  try {
    mkdirSync(resolve(root,'logic/src'),{recursive:true});
    writeFileSync(resolve(root,'logic/src/lib.rs'),'pub struct Probe;');
    const game={crate:'gui-probe-logic',type:'Probe'};
    writeFileSync(resolve(root,'app.json'),JSON.stringify({app:{id:'com.exact.gui-probe',name:'Signal 夜'},game}));
    gameShells(root,game,resolve(import.meta.dir,'../game'));
    // Compile the real generated shell with a tiny included entry: subsystem
    // selection must preserve explicitly inherited stdin/stdout, as agent mode does.
    writeFileSync(resolve(root,'entry.rs'),'fn main() { let mut line = String::new(); std::io::stdin().read_line(&mut line).unwrap(); print!("received:{}", line); }');
    for(const [assertions,subsystem] of [['no',2],['yes',3]]) {
      const binary=resolve(root,`probe-${assertions}.exe`);
      const compile=spawnSync('rustc',[resolve(root,'.shells/windows/src/main.rs'),'--edition=2021','--crate-name','gui_probe','-C',`debug-assertions=${assertions}`,'-o',binary],{cwd:root,env:{...process.env,OUT_DIR:root},encoding:'utf8',timeout:60000,windowsHide:true});
      expect(compile.status,compile.stderr).toBe(0);
      const bytes=readFileSync(binary), pe=bytes.readUInt32LE(0x3c);
      expect(bytes.toString('ascii',pe,pe+4)).toBe('PE\0\0');
      expect(bytes.readUInt16LE(pe+24+68)).toBe(subsystem);
      const agent=spawnSync(binary,[],{input:'{"op":"tags"}\n',encoding:'utf8',timeout:10000,windowsHide:true});
      expect(agent.status,agent.stderr).toBe(0);
      expect(agent.stdout).toBe('received:{"op":"tags"}\n');
    }
  } finally { rmSync(root,{recursive:true,force:true}); }
},60000);

test('browser function keys reach the focused game with their platform key identity', async () => {
  for(const [key,vk] of [['F1',112],['F2',113],['F12',123],['F24',135]]) {
    const calls=[];
    await browserKey({id:17,opts:{key},evaluate:async()=>true,ask:async()=>({ok:true}),
      call:async(method,args)=>calls.push([method,args]),frame:async()=>{}});
    expect(calls.map(([method,args])=>[method,args.type,args.code,args.key,args.windowsVirtualKeyCode]))
      .toEqual(['keyDown','keyUp'].map(type=>['Input.dispatchKeyEvent',type,key,key,vk]));
  }
});

test('public web inventory and owned reads agree on native Windows paths', async () => {
  const root=mkdtempSync(resolve(tmpdir(),'exact static paths '));
  try {
    mkdirSync(resolve(root,'stages'));
    writeFileSync(resolve(root,'index.html'),'game');
    writeFileSync(resolve(root,'stages/inspection.wasm'),'inspection');
    expect(listPublicFiles(root)).toEqual(['index.html','stages/inspection.wasm']);
    expect(readStaticFile(root,'/').body.toString()).toBe('game');
    expect((await readStaticFileAsync(root,'/stages/inspection.wasm')).body.toString()).toBe('inspection');
    expect((await publicFileCards(root)).map(file=>[file.name,file.bytes])).toEqual([['index.html',4],['stages/inspection.wasm',10]]);
    for (const path of ['/stages/../../secret','/stages/%2e%2e/%2e%2e/secret','/stages\\inspection.wasm']) expect(staticFile(root,path)).toBeNull();
  } finally { closeFilesystemReader(); rmSync(root,{recursive:true,force:true}); }
});

test('setup selects supported pinned Binaryen archives on Windows and Unix', () => {
  expect(binaryenArchive('version_132','win32','x64')).toBe('binaryen-version_132-x86_64-windows.tar.gz');
  expect(binaryenArchive('version_132','darwin','arm64')).toBe('binaryen-version_132-arm64-macos.tar.gz');
  expect(binaryenArchive('version_132','linux','arm64')).toBe('binaryen-version_132-aarch64-linux.tar.gz');
  expect(()=>binaryenArchive('version_132','win32','arm64')).toThrow('no Binaryen setup');
  expect(binaryenVersion('wasm-opt version 132 (version_132)')).toBe('version 132');
});

test('SDK CLI entrypoint executes with spaces in its real file path', () => {
  const result=spawnSync(process.execPath,[resolve(import.meta.dir,'exact.mjs'),'--help'],{encoding:'utf8'});
  expect(result.status).toBe(0);
  expect(result.stdout).toContain('exact setup [--check]');
});

test('caps command really runs from a checkout path containing spaces', () => {
  const root=resolve(import.meta.dir,'..');
  const result=spawnSync(process.execPath,[resolve(import.meta.dir,'caps.mjs')],{cwd:root,encoding:'utf8'});
  expect(result.stdout).toContain('caps — budgets declared');
  expect(result.status).toBe(runCaps(root).problems.length ? 1 : 0);
});

test('filesystem errors distinguish Windows access denial from Unix IO failure', () => {
  expect(filesystemErrorCode(5,'win32')).toBe('EACCES');
  expect(filesystemErrorCode(5,'linux')).toBe('EIO');
  expect(filesystemErrorCode(3,'win32')).toBe('ENOENT');
  expect(filesystemErrorCode(32,'win32')).toBe('EBUSY');
  expect(filesystemErrorCode(null,'win32')).toBe('EXACT_FS_REFUSED');
});

test('a real filesystem helper missing-parent response preserves ENOENT', () => {
  const root=mkdtempSync(resolve(tmpdir(),'exact-fs-code-'));
  try { expect(()=>filesystem({root,op:'read',path:'missing/file'})).toThrow(expect.objectContaining({code:'ENOENT'})); }
  finally { rmSync(root,{recursive:true,force:true}); }
});

test('packaged native freshness rejects changed source and copied binary bytes', () => {
  const root=mkdtempSync(resolve(tmpdir(),'exact-package-receipt-'));
  const receipt=resolve(root,'build.json'), source=resolve(root,'source.rs'), binary=resolve(root,'game.exe'), gpu=resolve(root,'game.dll');
  const digest=path=>createHash('sha256').update(readFileSync(path)).digest('hex');
  try {
    expect(packagedBuildChanges(receipt,root)).toEqual(['missing compiler build receipt']);
    for(const path of [source,binary,gpu]) writeFileSync(path,path);
    writeFileSync(receipt,JSON.stringify({version:1,binary:{inputs:[{path:source,name:'source',sha256:digest(source)}],missing:[],directories:[]},products:[binary,gpu].map(path=>({path,sha256:digest(path)}))}));
    expect(packagedBuildChanges(receipt,root)).toEqual([]);
    writeFileSync(source,'changed'); expect(packagedBuildChanges(receipt,root)).toEqual(['source']);
    writeFileSync(source,source); writeFileSync(gpu,'changed'); expect(packagedBuildChanges(receipt,root)).toEqual([gpu]);
    rmSync(binary); expect(packagedBuildChanges(receipt,root)).toEqual([binary,gpu]);
  } finally { rmSync(root,{recursive:true,force:true}); }
});
