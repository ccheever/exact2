import {test, expect} from 'bun:test';
import assert from 'node:assert/strict';
import {spawn, spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {EventEmitter} from 'node:events';
import {existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {resolve} from 'node:path';
import {PassThrough} from 'node:stream';
import {closeFilesystemReader, filesystem, filesystemErrorCode} from './filesystem.mjs';
import {Cdp, chromium, closeWindowsBrowser, retainCleanupError, packagedBuildChanges, removeBrowserProfile} from './agent-launch.mjs';
import {browserKey, open} from './agent.mjs';
import {cdpKey, withHeldModifiers} from './agent-keys.mjs';
import {runCaps} from './caps.mjs';
import {binaryenArchive, binaryenVersion} from './exact.mjs';
import {listPublicFiles, publicFileCards, readStaticFile, readStaticFileAsync, staticFile} from '../host/web/serve.mjs';
import {gameShells} from '../game/app/shells.mjs';
import {artifactDigest, buildInputHash, formatProofError} from '../game/proof.mjs';

test('proof interruption preserves a real CDP timeout message when its stack omits it', async () => {
  const input=new PassThrough(), output=new PassThrough(), cdp=new Cdp(input,output);
  try {
    let failure;
    try { await cdp.send('Runtime.evaluate',{},undefined,10); } catch(error) { failure=error; }
    expect(failure).toBeInstanceOf(Error);
    expect(failure.message).toBe('Runtime.evaluate did not answer within 10 ms');
    const originalStack=failure.stack;
    expect(formatProofError(failure)).toContain(failure.message);
    expect(formatProofError(failure)).toContain(originalStack);
    // Preserve the exact message-free shape seen in the failed frozen proof;
    // an isolated timer on this Bun version does not always omit its message.
    failure.stack='Error\n    at <anonymous> (agent-launch.mjs:361:76)';
    expect(formatProofError(failure)).toBe(`${String(failure)}\n${failure.stack}`);
    expect(failure.message).toBe('Runtime.evaluate did not answer within 10 ms');
    expect(formatProofError('plain refusal')).toBe('plain refusal');
  } finally { input.destroy(); output.destroy(); }
});

test('startup failure retains the cleanup error and its owned helper handle', () => {
  const cause=new Error('original cause'), original=new Error('original operation',{cause});
  const helper={pid:123}, cleanup=Object.assign(new Error('cleanup refused'),{ownedHelper:helper});
  retainCleanupError(original,cleanup);
  expect(original.message).toBe('original operation; cleanup: cleanup refused');
  expect(original.cause).toBe(cause);
  expect(original.cleanupError).toBe(cleanup);
  expect(original.cleanupError.ownedHelper).toBe(helper);
});

test.skipIf(process.platform !== 'win32')('owned Chrome refusal preserves live process/profile and bounded shutdown evidence', async () => {
  const profile=mkdtempSync(resolve(tmpdir(),'exact-close-refusal-'));
  const marker=resolve(profile,'owned-marker'); writeFileSync(marker,'keep');
  const child=spawn(chromium().executable,['--headless=new','--remote-debugging-pipe',`--user-data-dir=${profile}`,
    '--no-sandbox','--no-first-run','--disable-background-networking','about:blank'],
    {detached:true,windowsHide:true,stdio:['ignore','ignore','pipe','pipe','pipe']});
  child.stderr.on('data',()=>{});
  const cdp=new Cdp(child.stdio[3],child.stdio[4]);
  const exited=new Promise(resolve=>child.once('exit',resolve));
  child.on('exit',()=>cdp.fail('owned Chrome exited'));
  try {
    await cdp.send('Browser.getVersion');
    let calls=0;
    const refused={send:async method=>{expect(method).toBe('Browser.close');throw new Error('fixture close rejected');}};
    const began=performance.now();
    await assert.rejects(closeWindowsBrowser(child,refused,exited,profile,pid=>{
      expect(pid).toBe(child.pid); calls++;
      return spawn(process.execPath,['-e','process.stdout.write("x".repeat(5000));process.stderr.write("permission denied");process.exit(5)'],
        {windowsHide:true,stdio:['ignore','pipe','pipe']});
    }),error=>{
      expect(error.message).toContain(`Chrome ${child.pid} did not exit; owned profile retained at ${profile}`);
      const detail=JSON.parse(error.message.split('; shutdown ')[1]);
      expect(detail.cdp.error).toBe('fixture close rejected');
      expect(detail.taskkill.status).toBe(5);
      expect(detail.taskkill.error).toBeNull();
      expect(detail.taskkill.pid).toBeGreaterThan(0);
      expect(detail.taskkill.deadline).toBe(false);
      expect(detail.taskkill.stdout.length).toBe(2048);
      expect(detail.taskkill.stderr).toBe('permission denied');
      expect(detail.exitCode).toBeNull(); expect(detail.signalCode).toBeNull();
      expect(detail.totalMs).toBeGreaterThanOrEqual(3900);
      return true;
    });
    expect(performance.now()-began).toBeLessThan(10000);
    expect(calls).toBe(1);
    expect(readFileSync(marker,'utf8')).toBe('keep');
    expect((await cdp.send('Browser.getVersion')).product).toContain('Chrome');
    // A failed helper is not a successful exit. A subsequent real graceful
    // close may clean this owned process, without erasing the refusal above.
    await closeWindowsBrowser(child,cdp,exited,profile);
    expect(child.exitCode !== null || child.signalCode !== null).toBe(true);
  } finally {
    if(child.exitCode===null && child.signalCode===null) await closeWindowsBrowser(child,cdp,exited,profile);
    await removeBrowserProfile(profile);
  }
},30000);

test.skipIf(process.platform !== 'win32')('owned Chrome forced close requires the recorded child exit', async () => {
  const profile=mkdtempSync(resolve(tmpdir(),'exact-close-forced-'));
  const child=spawn(chromium().executable,['--headless=new','--remote-debugging-pipe',`--user-data-dir=${profile}`,
    '--no-sandbox','--no-first-run','--disable-background-networking','about:blank'],
    {detached:true,windowsHide:true,stdio:['ignore','ignore','pipe','pipe','pipe']});
  child.stderr.on('data',()=>{});
  const cdp=new Cdp(child.stdio[3],child.stdio[4]);
  const exited=new Promise(resolve=>child.once('exit',resolve));
  child.on('exit',()=>cdp.fail('owned Chrome exited'));
  try {
    await cdp.send('Browser.getVersion');
    await closeWindowsBrowser(child,{send:async()=>{throw new Error('fixture withholds graceful close');}},exited,profile);
    expect(child.exitCode !== null || child.signalCode !== null).toBe(true);
  } finally {
    if(child.exitCode===null && child.signalCode===null) await closeWindowsBrowser(child,cdp,exited,profile);
    await removeBrowserProfile(profile);
  }
},30000);

for (const mode of ['delayed-exit','slow-helper','unconfirmed-helper']) {
  test.skipIf(process.platform !== 'win32')(`owned Chrome async termination: ${mode}`, async () => {
    const profile=mkdtempSync(resolve(tmpdir(),'exact-close-async-'));
    const marker=resolve(profile,'owned-marker'); writeFileSync(marker,'keep');
    const child=spawn(chromium().executable,['--headless=new','--remote-debugging-pipe',`--user-data-dir=${profile}`,
      '--no-sandbox','--no-first-run','--disable-background-networking','about:blank'],
      {detached:true,windowsHide:true,stdio:['ignore','ignore','pipe','pipe','pipe']});
    child.stderr.on('data',()=>{});
    const cdp=new Cdp(child.stdio[3],child.stdio[4]);
    const exited=new Promise(resolve=>child.once('exit',resolve));
    child.on('exit',()=>cdp.fail('owned Chrome exited'));
    let helper, helperExited, pulse, closeTimer, lateClose;
    let pulses=0, helperStarted, killedAt, exitedAt, killCalls=0;
    try {
      await cdp.send('Browser.getVersion');
      const closing=closeWindowsBrowser(child,{send:async()=>{throw new Error('fixture delays close');}},exited,profile,pid=>{
        expect(pid).toBe(child.pid);
        helperStarted=performance.now();
        helper=spawn(process.execPath,['-e',`setTimeout(()=>process.exit(7),${mode==='delayed-exit'?800:30000})`],
          {windowsHide:true,stdio:['ignore','pipe','pipe']});
        helperExited=new Promise(resolve=>helper.once('exit',()=>{exitedAt=performance.now();resolve();}));
        const kill=helper.kill.bind(helper);
        helper.kill=signal=>{killCalls++;killedAt=performance.now();return kill(signal);};
        pulse=setInterval(()=>pulses++,20);
        closeTimer=setTimeout(()=>{lateClose=cdp.send('Browser.close').catch(error=>error);},100);
        if(mode==='unconfirmed-helper') {
          // The real helper stays alive; the injected process boundary refuses
          // termination and supplies no exit. Cleanup below still owns its handle.
          return Object.assign(new EventEmitter(),{pid:helper.pid,exitCode:null,signalCode:null,
            stdout:helper.stdout,stderr:helper.stderr,kill:()=>false});
        }
        return helper;
      });
      if(mode==='unconfirmed-helper') {
        await assert.rejects(closing,error=>{
          expect(error.message).toContain(`Chrome termination helper ${helper.pid} did not exit`);
          expect(error.ownedHelper.pid).toBe(helper.pid);
          const detail=JSON.parse(error.message.split('; shutdown ')[1]);
          expect(detail.taskkill.deadline).toBe(true);
          expect(detail.taskkill.killSent).toBe(false);
          expect(detail.taskkill.status).toBeNull(); expect(detail.taskkill.signal).toBeNull();
          expect(detail.exitCode!==null || detail.signalCode!==null).toBe(true);
          return true;
        });
        expect(helper.exitCode).toBeNull(); expect(helper.signalCode).toBeNull();
        expect(readFileSync(marker,'utf8')).toBe('keep');
        expect(performance.now()-helperStarted).toBeLessThan(5500);
      } else {
        await closing;
        expect(child.exitCode!==null || child.signalCode!==null).toBe(true);
        expect(helper.exitCode!==null || helper.signalCode!==null).toBe(true);
        if(mode==='delayed-exit') {
          expect(killCalls).toBe(0); expect(helper.exitCode).toBe(7);
          expect(exitedAt-helperStarted).toBeGreaterThanOrEqual(750);
        } else {
          expect(killCalls).toBe(1);
          expect(killedAt-helperStarted).toBeGreaterThanOrEqual(1900);
          expect(killedAt-helperStarted).toBeLessThan(3500);
          expect(pulses).toBeGreaterThan(30);
        }
      }
    } finally {
      clearInterval(pulse); clearTimeout(closeTimer);
      await lateClose;
      if(helper && helper.exitCode===null && helper.signalCode===null) helper.kill('SIGKILL');
      if(helperExited) await helperExited;
      if(child.exitCode===null && child.signalCode===null) await closeWindowsBrowser(child,cdp,exited,profile);
      await removeBrowserProfile(profile);
    }
  },30000);
}

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

test('modifier codes preserve side identity and accepted holds survive later frame failure', async () => {
  for (const [key, bit, vk] of [['Shift',8,16],['Control',2,17],['Alt',1,18],['Meta',4,91]]) {
    for (const [side, location] of [['Left',1],['Right',2]]) {
      expect(cdpKey(key+side)).toMatchObject({code:key+side,key,location,modifiers:bit,vk:key==='Meta'&&side==='Right'?92:vk});
    }
    expect(cdpKey(key)).toMatchObject({code:key+'Left',key,location:1,modifiers:bit});
  }
  let focus = 0;
  await assert.rejects(browserKey({id:1,opts:{key:'ControlMiddle'},evaluate:async()=>{focus++;},ask:async()=>{},call:async()=>{},frame:async()=>{}}),/unsupported key/);
  expect(focus).toBe(0);
  const held = new Map([['ControlRight',{}]]), sent=[];
  const call=async(method,event)=>{
    sent.push(withHeldModifiers(method,event,held));
    if(event.type==='keyUp') held.delete(event.code); else held.set(event.code,event);
  };
  let failed;
  try {await browserKey({id:1,opts:{key:'ControlLeft',phase:'down'},evaluate:async()=>true,ask:async()=>({ok:true}),call,frame:async()=>{throw Error('frame failed');}});}
  catch(error){failed=error;}
  expect(failed.message).toBe('frame failed');
  expect(held.has('ControlLeft')).toBe(true);
  await assert.rejects(failed.release(),/frame failed/);
  expect(held.has('ControlLeft')).toBe(false);
  expect(sent.at(-1)).toMatchObject({type:'keyUp',code:'ControlLeft',modifiers:2,location:1});
  expect(withHeldModifiers('Input.dispatchMouseEvent',{type:'mouseMoved'},held).modifiers).toBe(2);
  held.clear();
  expect(withHeldModifiers('Input.dispatchMouseEvent',{type:'mouseMoved'},held).modifiers).toBe(0);
});

test('trusted browser modifier events retain both sides and reach following keys and pointers', async () => {
  const server=Bun.serve({port:0,fetch(){return new Response(`<div id="exact-root" data-boot-ms="1"><canvas id="world" tabindex="0" style="position:absolute;left:20px;top:30px;width:100px;height:100px"></canvas></div><script>
    const world=document.getElementById('world'),events=[];let focuses=0;
    for(const type of ['keydown','keyup','pointerdown','pointerup','contextmenu']) world.addEventListener(type,e=>{e.preventDefault();events.push({type,code:e.code,key:e.key,location:e.location,shift:e.shiftKey,ctrl:e.ctrlKey,alt:e.altKey,meta:e.metaKey,trusted:e.isTrusted});});
    const agent=async r=>r.op==='tags'?{clock:0}:r.op==='tree'?{nodes:[{id:1,type:'canvas',props:{testId:'world'}}]}:r.op==='layout'?{viewport:{w:420,h:900},nodes:[{id:1,x:20,y:30,w:100,h:100}]}:r.op==='focus'?(focuses++,world.focus(),{ok:true}):r.op==='state'?{events,focuses}:{};
    window.exact={ready:Promise.resolve(),views:new Map([[1,world]]),gpu:{wantsInput:()=>true},agent,agentSettled:agent};
    </script>`,{headers:{'content-type':'text/html'}});}});
  let s;
  try {
    s=await open({host:'web',url:server.url.href});
    const codes=['ShiftLeft','ShiftRight','ControlLeft','ControlRight','AltLeft','AltRight','MetaLeft','MetaRight'];
    for(const key of codes) await s.type('world',{key});
    let state=await s.carrier.ask({op:'state'});
    expect(state.events).toHaveLength(16);
    for(let i=0;i<codes.length;i++) for(const [offset,type] of [[0,'keydown'],[1,'keyup']]) {
      const code=codes[i], key=code.replace(/Left|Right/g,''), flag={Shift:'shift',Control:'ctrl',Alt:'alt',Meta:'meta'}[key];
      expect(state.events[i*2+offset]).toEqual({type,code,key,location:code.endsWith('Left')?1:2,shift:false,ctrl:false,alt:false,meta:false,trusted:true,[flag]:offset===0});
    }
    const focuses=state.focuses;
    await assert.rejects(s.type('world',{key:'ControlMiddle'}),/unsupported key/);
    expect((await s.carrier.ask({op:'state'})).focuses).toBe(focuses);
    for(const [key,phase] of [['ControlLeft','down'],['ControlRight','down'],['ControlLeft','up']]) await s.type('world',{key,phase});
    await s.type('world',{key:'Digit1'});
    await s.tap('world',{contextmenu:true,at:[50,50]});
    await s.type('world',{key:'ControlRight',phase:'up'});
    await s.type('world',{key:'Digit2'});
    state=await s.carrier.ask({op:'state'});
    const tail=state.events.slice(16);
    // Browsers place contextmenu on either side of pointerup; the held chord
    // must reach it in both cases, before ControlRight is released.
    expect(tail.filter(e=>e.type!=='contextmenu').map(e=>[e.type,e.code??null,e.ctrl])).toEqual([
      ['keydown','ControlLeft',true],['keydown','ControlRight',true],['keyup','ControlLeft',true],
      ['keydown','Digit1',true],['keyup','Digit1',true],['pointerdown',null,true],['pointerup',null,true],
      ['keyup','ControlRight',false],['keydown','Digit2',false],['keyup','Digit2',false],
    ]);
    expect(tail.filter(e=>e.type==='contextmenu').map(e=>[e.type,e.code??null,e.ctrl])).toEqual([['contextmenu',null,true]]);
    const menu=tail.findIndex(e=>e.type==='contextmenu');
    expect(menu).toBeGreaterThan(tail.findIndex(e=>e.type==='pointerdown'));
    expect(menu).toBeLessThan(tail.findIndex(e=>e.type==='keyup'&&e.code==='ControlRight'));
    expect(tail.every(e=>e.trusted)).toBe(true);
  } finally {await s?.close();server.stop(true);}
},60000);

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


test('packaged native freshness verifies executable-relative shader bytes', () => {
  const root=mkdtempSync(resolve(tmpdir(),'exact-package-shaders-'));
  const receipt=resolve(root,'build.json'), binary=resolve(root,'game.exe'), shader=resolve(root,'shaders/fog.wgsl');
  const digest=bytes=>createHash('sha256').update(bytes).digest('hex');
  try {
    mkdirSync(resolve(root,'shaders')); writeFileSync(binary,'exe'); writeFileSync(shader,'shader');
    writeFileSync(receipt,JSON.stringify({version:1,binary:{inputs:[],missing:[],directories:[]},products:[{path:binary,sha256:digest('exe')}]}));
    writeFileSync(resolve(root,'compat.json'),JSON.stringify({embedded:{assets:[{name:'shaders/fog.wgsl',bytes:6,sha256:digest('shader')}]}}));
    expect(packagedBuildChanges(receipt,root)).toEqual([]);
    writeFileSync(shader,'edited'); expect(packagedBuildChanges(receipt,root)).toEqual([shader]);
    rmSync(shader); expect(packagedBuildChanges(receipt,root)).toEqual([shader]);
  } finally { rmSync(root,{recursive:true,force:true}); }
});

test('Windows receipts bind the executable, GPU DLL, packaged assets and build profile', () => {
  const dir=mkdtempSync(resolve(tmpdir(),'game-windows-receipt-'));
  const artifacts={binary:resolve(dir,'game.exe'),module:resolve(dir,'game_gpu.dll')};
  try {
    expect(artifactDigest('windows',dir,artifacts)).toBe(null);
    writeFileSync(artifacts.binary,'exe'); writeFileSync(artifacts.module,'gpu');
    const first=artifactDigest('windows',dir,artifacts);
    mkdirSync(resolve(dir,'assets')); writeFileSync(resolve(dir,'assets/terrain.tex'),'terrain');
    expect(artifactDigest('windows',dir,artifacts)).not.toBe(first);
    rmSync(artifacts.module);
    expect(artifactDigest('windows',dir,artifacts)).toBe(null);
    expect(buildInputHash('windows','target','0','release').digest('hex'))
      .not.toBe(buildInputHash('windows','target','0','gpu-dev').digest('hex'));
  } finally {rmSync(dir,{recursive:true,force:true});}
});
