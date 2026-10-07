import {test, expect, spyOn} from 'bun:test';
import * as crypto from 'node:crypto';
import assert from 'node:assert/strict';
import {spawn, spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {EventEmitter} from 'node:events';
import {createServer, get} from 'node:http';
import {existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, symlinkSync, writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {parse, relative, resolve} from 'node:path';
import {PassThrough} from 'node:stream';
import {closeFilesystemReader, filesystem, filesystemErrorCode} from './filesystem.mjs';
import {Cdp, captureCdpRequest, cdpFailureContext, copyCdpFailureContext, chromium, closeWindowsBrowser, retainCleanupError, packagedBuildChanges, removeBrowserProfile} from './agent-launch.mjs';
import {browserKey, open, typeCommand} from './agent.mjs';
import {cdpKey, deliverClipboard, pasteChord, withHeldModifiers} from './agent-keys.mjs';
import {focusForKey, playwrightPointer, withHeldKeys} from './agent-playwright.mjs';
import {runCaps} from './caps.mjs';
import {binaryenArchive, binaryenVersion} from './exact.mjs';
import {buildTreeFile, serveBuildTree, listPublicFiles, publicFileCards, readStaticFile, readStaticFileAsync, staticFile} from '../host/web/serve.mjs';
import {gameShells} from '../game/app/shells.mjs';
import {formatProofError, proofFailureRow} from '../game/proof.mjs';
import {readManifest} from './app.mjs';

test('JS build trees serve native paths without admitting private or escaping files', async () => {
  const owned=realpathSync(mkdtempSync(resolve(tmpdir(),'exact JS tree café ')));
  const root=resolve(owned,'dist'), outside=resolve(owned,'outside');
  let server;
  try {
    mkdirSync(resolve(root,'nested café'),{recursive:true}); mkdirSync(outside);
    for(const [name,body] of [['index.html','shell'],['app.js','entry'],['nested café/data.js','unicode'],['nested café/index.html','nested'],['.private.js','private']])
      writeFileSync(resolve(root,name),body);
    writeFileSync(resolve(outside,'secret.js'),'outside');
    symlinkSync(outside,resolve(root,'outside'),process.platform==='win32'?'junction':'dir');
    server=createServer((req,res)=>serveBuildTree(root,req,res));
    await new Promise(done=>server.listen(0,'127.0.0.1',done));
    // A raw HTTP path preserves traversal spellings a URL constructor normalizes.
    const request=path=>new Promise((done,fail)=>{
      get({host:'127.0.0.1',port:server.address().port,path},res=>{
        const chunks=[];res.on('data',chunk=>chunks.push(chunk));res.on('error',fail);
        res.on('end',()=>done({status:res.statusCode,body:Buffer.concat(chunks).toString()}));
      }).on('error',fail);
    });
    for(const [path,body] of [['/','shell'],['/index.html','shell'],['/app.js','entry'],['/nested%20caf%C3%A9/data.js','unicode'],['/nested%20caf%C3%A9/','nested'],['/app-route','shell']])
      expect(await request(path)).toEqual({status:200,body});
    for(const path of ['/.private.js','/%2e%2e/outside/secret.js','/nested%20caf%C3%A9/../../outside/secret.js','/nested%5cdata.js','/%00','/outside/secret.js'])
      expect(await request(path)).toEqual({status:404,body:''});
    // An explicitly selected volume root already ends in its native separator.
    const volume=parse(root).root, file=resolve(root,'app.js');
    const route='/'+relative(volume,file).replaceAll('\\','/');
    expect(buildTreeFile(volume,route)?.path).toBe(file);
  } finally {
    if(server)await new Promise(done=>server.close(done));
    rmSync(owned,{recursive:true,force:true});
  }
});

test('ordinary Windows host manifests admit only the empty host settings object', () => {
  const root=mkdtempSync(resolve(tmpdir(),'exact-windows-manifest-'));
  const manifest={name:'Windows fixture',app:{id:'test.exact.windows',name:'Windows fixture'},host:{windows:{}},deploy:{store:{windows:'0'}}};
  const write=host=>writeFileSync(resolve(root,'app.json'),JSON.stringify({...manifest,host}));
  try {
    write({windows:{}});
    const accepted=readManifest(root,'fixture');
    expect(accepted.host.windows).toEqual({});
    expect(accepted.deploy.store.windows).toBe('0');
    write({windows:{title:'unexpected'}});
    expect(()=>readManifest(root,'fixture')).toThrow('host.windows.title: not a known key');
    for(const value of [null,[],true,'windows']) {
      write({windows:value});
      expect(()=>readManifest(root,'fixture')).toThrow('host.windows: expected object');
    }
    write({});
    expect(readManifest(root,'fixture').host.windows).toBeUndefined();
  } finally { rmSync(root,{recursive:true,force:true}); }
});

test('storage error numbers retain platform meaning and require an exact typed suffix', () => {
  const source=readFileSync(new URL('../js/src/prelude.js',import.meta.url),'utf8');
  const mapping=source.slice(source.indexOf('  var windowsStorage ='),source.indexOf('  function storageError('));
  const make=marker=>{
    const global={__exact_windows_storage:marker};
    const code=new Function('global',`${mapping}\nreturn storageCode;`)(global);
    expect(global).not.toHaveProperty('__exact_windows_storage');
    return code;
  };
  const windows=make(true), unix=make(undefined);
  for(const [number,code] of [[2,'ENOENT'],[3,'ENOENT'],[32,'EBUSY'],[33,'EBUSY'],[80,'EEXIST'],[145,'ENOTEMPTY'],[170,'EBUSY'],[183,'EEXIST'],[267,'ENOTDIR']])
    expect(windows(`filesystem: unavailable (os error ${number})`)).toBe(code);
  for(const number of [5,17,21,39]) expect(windows(`filesystem: unavailable (os error ${number})`)).toBe('failed');
  expect(unix('unavailable (os error 17)')).toBe('EEXIST');
  expect(unix('unavailable (os error 39)')).toBe('ENOTEMPTY');
  expect(unix('unavailable (os error 267)')).toBe('failed');
  expect(windows('fs.readFile doc:/1/folder: cannot read a directory (filesystem code EISDIR)')).toBe('EISDIR');
  expect(windows('(filesystem code EISDIR) then access refused (os error 5)')).toBe('failed');
  expect(windows('denied: fs.read (filesystem code EISDIR)')).toBe('denied');
  expect(unix('(filesystem code EISDIR)')).toBe('failed');
});

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
    expect(formatProofError(failure)).toBe(`${String(failure)}\n${failure.stack}\nCDP ${JSON.stringify(cdpFailureContext(failure))}`);
    expect(failure.message).toBe('Runtime.evaluate did not answer within 10 ms');
    expect(formatProofError('plain refusal')).toBe('plain refusal');
  } finally { input.destroy(); output.destroy(); }
});

test('CDP failure context preserves timeout identity and actual proof serialization', async () => {
  const input=new PassThrough(), output=new PassThrough(), cdp=new Cdp(input,output);
  const expression='exact.agentSettled({op:"tap",target:"play-frontier"})';
  try {
    const failure=await cdp.send('Runtime.evaluate',{expression},'attached-7',5).catch(error=>error);
    const context=cdpFailureContext(failure), originalStack=failure.stack;
    expect(context).toMatchObject({schema:1,method:'Runtime.evaluate',requestId:1,cdpSessionId:'attached-7',timeoutMs:5,category:'timeout',source:'captured-primitive-params',
      parameter:{field:'expression',characters:expression.length,utf8Bytes:Buffer.byteLength(expression),sha256:createHash('sha256').update(expression).digest('hex')}});
    expect(Object.isFrozen(context)).toBe(true); expect(Object.isFrozen(context.parameter)).toBe(true);
    const cause=new Error('cause'), helper={pid:123};
    failure.cause=cause; failure.ownedHelper=helper; Object.freeze(failure);
    const row=proofFailureRow(13,'tap',['play-frontier'],0,failure);
    expect(row).toMatchObject({session:13,method:'tap',args:['play-frontier'],clock:0,error:failure.message,cdp:context});
    const encoded=JSON.stringify(row);
    for(const secret of [expression,'ownedHelper','cause']) expect(encoded).not.toContain(secret);
    expect(formatProofError(failure)).toContain(originalStack);
    expect(failure.cause).toBe(cause); expect(failure.ownedHelper).toBe(helper);
    const wrapper=new Error('Chrome did not answer');
    expect(copyCdpFailureContext(failure,wrapper)).toBe(wrapper);
    expect(cdpFailureContext(wrapper)).toBe(context); expect(wrapper.cause).toBeUndefined();
    expect(copyCdpFailureContext(wrapper,failure)).toBe(failure); expect(cdpFailureContext(failure)).toBe(context);
    expect(proofFailureRow(1,'tap',[],0,new Error('plain'))).toEqual({session:1,method:'tap',args:[],clock:0,error:'plain',steps:undefined});
  } finally { input.destroy(); output.destroy(); }
});

test('CDP failure context success keeps one Promise, value identity and retires raw metadata without hashing', async () => {
  const input=new PassThrough(), output=new PassThrough(), cdp=new Cdp(input,output);
  const hash=spyOn(crypto,'createHash'), NativePromise=globalThis.Promise, created=[];
  let promise;
  try {
    globalThis.Promise=class extends NativePromise { constructor(executor) { super(executor); created.push(this); } };
    try { promise=cdp.send('Runtime.evaluate',{expression:'successful secret'},'session',1000); }
    finally { globalThis.Promise=NativePromise; }
    expect(created).toEqual([promise]);
    const pending=cdp.pending.get(1), value={same:true}; pending.resolve(value);
    expect(await promise).toBe(value); expect(hash).not.toHaveBeenCalled();
    // Invoke the real settled closure: its diagnostic string is already retired.
    const late=new Error('late rejection'); pending.reject(late);
    expect(cdpFailureContext(late).parameter).toEqual({field:'expression',characters:17});
    expect(hash).not.toHaveBeenCalled();
    // Ensure the spy observes the actual production failure hash too.
    const failed=cdp.send('Runtime.evaluate',{expression:'failure'},'session',1000).catch(error=>error);
    output.write(JSON.stringify({id:2,error:{message:'refused'}})+'\0'); await failed;
    expect(hash).toHaveBeenCalledTimes(1);
  } finally { globalThis.Promise=NativePromise; hash.mockRestore(); cdp.fail('test cleanup'); input.destroy(); output.destroy(); }
});

test('CDP failure context distinguishes concurrent errors and ignores late replies', async () => {
  const input=new PassThrough(), output=new PassThrough(), cdp=new Cdp(input,output);
  try {
    const params={expression:'before'};
    const one=cdp.send('Runtime.evaluate',params,'one',1000).catch(error=>error);
    const two=cdp.send('Page.navigate',{url:'https://user:secret@example.test/private?token=secret'},'two',1000).catch(error=>error);
    params.expression='after'; output.write(JSON.stringify({id:1,error:{message:'protocol refused'}})+'\0'); cdp.fail('pipe closed');
    const a=await one,b=await two;
    expect(a).not.toBe(b);
    expect(cdpFailureContext(a)).toMatchObject({requestId:1,cdpSessionId:'one',category:'protocol',parameter:{sha256:createHash('sha256').update('before').digest('hex')}});
    expect(cdpFailureContext(b)).toMatchObject({requestId:2,cdpSessionId:'two',category:'transport',parameter:{field:'url'}});
    expect(JSON.stringify(cdpFailureContext(b))).not.toContain('secret');
    output.write(JSON.stringify({id:1,result:{late:true}})+'\0'); expect(cdp.pending.size).toBe(0);
    const closed=await cdp.send('Runtime.evaluate',{expression:'closed'},'three',1000).catch(error=>error);
    expect(cdpFailureContext(closed)).toMatchObject({requestId:null,category:'closed',cdpSessionId:'three'}); expect(cdp.next).toBe(3);
  } finally { input.destroy(); output.destroy(); }
});

test('CDP failure context preserves synchronous throws and their existing pending timers/late replies', async () => {
  const input=new PassThrough(), output=new PassThrough(), cdp=new Cdp(input,output);
  const cause=new Error('cause'), helper={pid:123}, thrown=Object.freeze(Object.assign(new Error('write refused',{cause}),{ownedHelper:helper}));
  input.write=()=>{throw thrown;};
  try {
    const failure=await cdp.send('Runtime.evaluate',{expression:'retire me'},'one',10).catch(error=>error);
    expect(failure).toBe(thrown); expect(failure.cause).toBe(cause); expect(failure.ownedHelper).toBe(helper);
    expect(cdpFailureContext(failure)).toMatchObject({requestId:1,category:'send-refusal'}); expect(cdp.pending.has(1)).toBe(true);
    const pending=cdp.pending.get(1); output.write(JSON.stringify({id:1,result:{late:true}})+'\0'); expect(cdp.pending.has(1)).toBe(false);
    const late=new Error('late closure'); pending.reject(late);
    expect(cdpFailureContext(late).parameter).toEqual({field:'expression',characters:9});
    const serializeError=new Error('serialization refused');
    const serialized=await cdp.send('Runtime.evaluate',{expression:'not serialized',toJSON(){throw serializeError;}},'two',5).catch(error=>error);
    expect(serialized).toBe(serializeError); expect(cdpFailureContext(serialized).parameter.omitted).toBe('serialization-hook');
    expect(cdp.pending.has(2)).toBe(true); await new Promise(resolve=>setTimeout(resolve,15)); expect(cdp.pending.has(2)).toBe(false);
  } finally { cdp.fail('test cleanup'); input.destroy(); output.destroy(); }
});

test('CDP failure context marks a shared thrown Error ambiguous without unbounded history', async () => {
  const input=new PassThrough(), output=new PassThrough(), cdp=new Cdp(input,output), shared=new Error('shared'); input.write=()=>{throw shared;};
  try {
    const one=await cdp.send('Runtime.evaluate',{expression:'first'},'one',1000).catch(error=>error); expect(cdpFailureContext(one).requestId).toBe(1);
    const two=await cdp.send('Runtime.evaluate',{expression:'second'},'two',1000).catch(error=>error);
    expect(one).toBe(shared); expect(two).toBe(shared); expect(cdpFailureContext(shared)).toEqual({schema:1,omitted:'shared-error',ambiguous:true});
  } finally { cdp.fail('test cleanup'); input.destroy(); output.destroy(); }
});

test('CDP failure context capture refuses accessors, hooks and Proxies without extra evaluation', () => {
  let touched=0;
  const capture=params=>captureCdpRequest('Runtime.evaluate',params,'session',1,15).parameter;
  expect(capture({get expression(){touched++;return 'secret';}}).omitted).toBe('not-string-data');
  expect(capture({expression:{toString(){touched++;return 'secret';}}}).omitted).toBe('not-string-data');
  expect(capture(new Proxy({expression:'secret'},{get(){touched++;throw Error('get');},getPrototypeOf(){touched++;throw Error('prototype');},getOwnPropertyDescriptor(){touched++;throw Error('descriptor');}})).omitted).toBe('not-plain-data');
  expect(capture(Object.create(new Proxy({},{get(){touched++;throw Error('get');}}))).omitted).toBe('not-plain-data');
  expect(capture({expression:'secret',get toJSON(){touched++;throw Error('hook');}}).omitted).toBe('serialization-hook');
  try {
    Object.defineProperty(Object.prototype,'toJSON',{configurable:true,get(){touched++;throw Error('prototype hook');}});
    expect(capture({expression:'secret'}).omitted).toBe('serialization-hook');
  } finally { delete Object.prototype.toJSON; }
  expect(touched).toBe(0); expect(capture(Object.assign(Object.create(null),{expression:'ok'}))).toEqual({field:'expression',characters:2,value:'ok'});
});

test('CDP failure context caps retention before dispatch and hashes only the complete admitted string', async () => {
  const input=new PassThrough(), output=new PassThrough(), cdp=new Cdp(input,output), limit=256*1024;
  const exact='😀'.repeat(limit/2), excessive='x'.repeat(limit+1), hash=spyOn(crypto,'createHash');
  try {
    const snapshot=captureCdpRequest('Runtime.evaluate',{expression:excessive},'one',1,10);
    expect(snapshot.parameter).toEqual({field:'expression',characters:limit+1,omitted:'length-limit'}); expect(JSON.stringify(snapshot)).not.toContain(excessive);
    const over=cdp.send('Runtime.evaluate',{expression:excessive},'one',1000).catch(error=>error); output.write(JSON.stringify({id:1,error:{message:'refused'}})+'\0');
    expect(cdpFailureContext(await over).parameter).toEqual(snapshot.parameter); expect(hash).not.toHaveBeenCalled();
    const at=cdp.send('Runtime.evaluate',{expression:exact},'two',1000).catch(error=>error); expect(hash).not.toHaveBeenCalled();
    output.write(JSON.stringify({id:2,error:{message:'refused'}})+'\0'); const context=cdpFailureContext(await at);
    expect(hash).toHaveBeenCalledTimes(1); expect(context.parameter).toMatchObject({characters:limit,utf8Bytes:limit*2});
    expect(JSON.stringify(context)).not.toContain('😀'); expect(Buffer.byteLength(JSON.stringify(context))).toBeLessThanOrEqual(8192);
  } finally { hash.mockRestore(); cdp.fail('test cleanup'); input.destroy(); output.destroy(); }
});

test('CDP failure context metadata failure preserves the Error and adds no unhandled rejection', async () => {
  const input=new PassThrough(), output=new PassThrough(), cdp=new Cdp(input,output), observed=[], original=Object.freeze(new Error('original'));
  const hash=spyOn(crypto,'createHash').mockImplementation(()=>{throw new Error('metadata failed');});
  const unhandled=error=>observed.push(error); process.on('unhandledRejection',unhandled); input.write=()=>{throw original;};
  try {
    const result=await cdp.send('Runtime.evaluate',{expression:'secret'},'session',5).catch(error=>error);
    expect(result).toBe(original); expect(cdpFailureContext(result)).toEqual({schema:1,omitted:'metadata-unavailable'});
    await new Promise(resolve=>setTimeout(resolve,15)); expect(observed).toEqual([]);
  } finally { hash.mockRestore(); process.off('unhandledRejection',unhandled); cdp.fail('test cleanup'); input.destroy(); output.destroy(); }
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

test('paste sends the platform chord and a prevented keydown skips the clipboard', async () => {
  const chord = pasteChord();
  expect(chord).toBe(process.platform === 'darwin' ? 'Meta+v' : 'Control+v');
  const mapped = cdpKey(chord);
  expect(mapped).toMatchObject({key:'v', code:'KeyV', text:undefined, modifiers:process.platform === 'darwin' ? 4 : 2, vk:86});

  const drive = ({clipboard, text, prevented, editable = true, failAt}) => {
    const trace = [];
    let listening = false, flag = null;
    const evaluate = async (expression) => {
      if (expression.includes('removeEventListener')) { listening = false; flag = null; trace.push('unlisten'); return; }
      if (expression.includes('addEventListener')) { listening = true; flag = null; trace.push('listen'); return; }
      if (expression === 'window.__exactPasteKey?.defaultPrevented !== true') { trace.push('flag'); return flag !== true; }
      if (expression.includes('ClipboardEvent')) {
        trace.push('event');
        if (failAt === 'event') throw new Error('send failed');
        return {editable, prevented:false};
      }
      throw new Error(`unexpected evaluate: ${expression}`);
    };
    const ask = async (req) => { trace.push('focus'); expect(req).toEqual({op:'focus', id:7, select:false}); return {}; };
    const call = async (method, args) => {
      if (method === 'Input.insertText') { trace.push(['insert', args.text]); return; }
      trace.push([args.type, args.key, args.code, args.modifiers, args.text]);
      if (args.type === 'keyDown' && listening) flag = prevented;
      if (failAt === 'down' && args.type === 'keyDown') throw new Error('down failed');
    };
    return deliverClipboard({id:7, opts:{clipboard, ...(text == null ? {} : {text})}, evaluate, ask, call}).then(() => trace);
  };

  expect(await drive({clipboard:'paste', text:'secret', prevented:true})).toEqual([
    'focus', 'listen', ['keyDown', 'v', 'KeyV', mapped.modifiers, undefined], 'flag', ['keyUp', 'v', 'KeyV', mapped.modifiers, undefined], 'unlisten',
  ]);
  expect(await drive({clipboard:'paste', text:'secret', prevented:false})).toEqual([
    'focus', 'listen', ['keyDown', 'v', 'KeyV', mapped.modifiers, undefined], 'flag', 'event', ['insert', 'secret'], ['keyUp', 'v', 'KeyV', mapped.modifiers, undefined], 'unlisten',
  ]);
  expect(await drive({clipboard:'paste', text:'secret', prevented:false, editable:false})).toEqual([
    'focus', 'listen', ['keyDown', 'v', 'KeyV', mapped.modifiers, undefined], 'flag', 'event', ['keyUp', 'v', 'KeyV', mapped.modifiers, undefined], 'unlisten',
  ]);
  expect(await drive({clipboard:'copy'})).toEqual(['focus', 'event']);
  await assert.rejects(drive({clipboard:'paste', text:'secret', prevented:false, failAt:'event'}), /send failed/);
  await assert.rejects(drive({clipboard:'paste', text:'secret', prevented:false, failAt:'down'}), /down failed/);
  // The chord is released when the paste event throws, and not pressed again when the keydown throws.
  const released = [];
  let listening = false;
  await assert.rejects(deliverClipboard({
    id:7, opts:{clipboard:'paste', text:'secret'},
    ask:async () => ({}),
    evaluate:async (expression) => {
      if (expression.includes('removeEventListener')) return;
      if (expression.includes('addEventListener')) { listening = true; return; }
      if (expression === 'window.__exactPasteKey?.defaultPrevented !== true') return true;
      if (expression.includes('ClipboardEvent')) throw new Error('send failed');
    },
    call:async (_method, args) => { released.push(args.type); if (args.type === 'keyDown' && !listening) throw new Error('keydown before the listener'); },
  }), /send failed/);
  expect(released).toEqual(['keyDown', 'keyUp']);
  released.length = 0;
  await assert.rejects(deliverClipboard({
    id:7, opts:{clipboard:'paste', text:'x'},
    ask:async () => ({}),
    evaluate:async (expression) => {
      if (expression.includes('removeEventListener') || expression.includes('addEventListener')) return;
      throw new Error(`unexpected ${expression}`);
    },
    call:async (_method, args) => { released.push(args.type); if (args.type === 'keyDown') throw new Error('down failed'); },
  }), /down failed/);
  expect(released).toEqual(['keyDown']);
});

test('a firefox or webkit drag is the mouse, a click keeps its modifiers, and an unfocusable key still presses', async () => {
  const moves = [], waits = [];
  let down = 0, up = 0;
  const pointer = playwrightPointer({
    name: 'firefox',
    move: async (x, y) => { moves.push([x, y]); },
    down: async () => { down++; },
    up: async () => { up++; },
    wait: async (ms) => { waits.push(ms); },
  });
  await assert.rejects(pointer('down', {}, { x: 1, y: 2 }), /firefox down unsupported: Playwright cannot produce trusted phased touches/);
  expect(down).toBe(0);
  expect(await pointer('down', { id: 4, mouse: true, x: 10, y: 20 }, { x: 0, y: 0 })).toMatchObject({ phase: 'down', at: [10, 20], delivery: 'platform', pointer: 'mouse', contact: 4 });
  expect(await pointer('move', { dx: 32, dy: 0, ms: 32 }, {})).toMatchObject({ phase: 'move', at: [42, 20] });
  expect(moves).toEqual([[10, 20], [26, 20], [42, 20]]);
  expect(waits).toEqual([16, 16]);
  waits.length = 0;
  expect(await pointer('hold', { ms: 50, virtual: true }, {})).toMatchObject({ phase: 'hold', at: [42, 20] });
  expect(waits).toEqual([]);
  expect(await pointer('hold', { ms: 40 }, {})).toMatchObject({ phase: 'hold' });
  expect(waits).toEqual([40]);
  expect(await pointer('cancel', {}, {})).toMatchObject({ phase: 'cancel', at: [42, 20], delivery: 'platform' });
  expect(up).toBe(1);
  await assert.rejects(pointer('up', {}, {}), /firefox up unsupported/);

  const keys = [];
  const keyboard = { down: async (k) => { keys.push(['down', k]); }, up: async (k) => { keys.push(['up', k]); } };
  await withHeldKeys(keyboard, 'Shift+Meta', async () => { keys.push('act'); });
  expect(keys).toEqual([['down', 'Shift'], ['down', 'Meta'], 'act', ['up', 'Meta'], ['up', 'Shift']]);
  keys.length = 0;
  await assert.rejects(withHeldKeys(keyboard, 'Shift', async () => { throw new Error('click failed'); }), /click failed/);
  expect(keys).toEqual([['down', 'Shift'], ['up', 'Shift']]);
  keys.length = 0;
  await withHeldKeys(keyboard, '', async () => { keys.push('plain'); });
  expect(keys).toEqual(['plain']);

  await focusForKey(1, false, async () => ({ ok: false }));
  await assert.rejects(focusForKey(1, true, async () => ({ ok: false })), /view 1 could not take focus/);
  await focusForKey(1, true, async () => ({ ok: true }));
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
    expect(tail.map(e=>[e.type,e.code??null,e.ctrl])).toEqual([
      ['keydown','ControlLeft',true],['keydown','ControlRight',true],['keyup','ControlLeft',true],
      // Chrome opens a context menu at the press on macOS and at the release elsewhere.
      ['keydown','Digit1',true],['keyup','Digit1',true],['pointerdown',null,true],
      ...(process.platform==='darwin'?[['contextmenu',null,true],['pointerup',null,true]]:[['pointerup',null,true],['contextmenu',null,true]]),
      ['keyup','ControlRight',false],['keydown','Digit2',false],['keyup','Digit2',false],
    ]);
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
  // Cargo's executable is the crate's; the package names it for the app, as Visual Studio would.
  const app={id:'com.exact.game',displayName:'Game: Two',crate:()=>'game-windows'};
  const receipt=resolve(root,'build.json'), source=resolve(root,'source.rs'), built=resolve(root,'cargo/game-windows.exe'), binary=resolve(root,'Game- Two.exe'), gpu=resolve(root,'game.dll');
  const digest=path=>createHash('sha256').update(readFileSync(path)).digest('hex');
  try {
    expect(packagedBuildChanges(receipt,root,app)).toEqual(['missing compiler build receipt']);
    mkdirSync(resolve(root,'cargo')); writeFileSync(built,'exe');
    for(const path of [source,binary,gpu]) writeFileSync(path,path===binary?'exe':path);
    writeFileSync(receipt,JSON.stringify({version:1,binary:{inputs:[{path:source,name:'source',sha256:digest(source)}],missing:[],directories:[]},products:[built,gpu].map(path=>({path,sha256:digest(path)}))}));
    expect(packagedBuildChanges(receipt,root,app)).toEqual([]);
    writeFileSync(source,'changed'); expect(packagedBuildChanges(receipt,root,app)).toEqual(['source']);
    writeFileSync(source,source); writeFileSync(gpu,'changed'); expect(packagedBuildChanges(receipt,root,app)).toEqual([gpu]);
    rmSync(binary); expect(packagedBuildChanges(receipt,root,app)).toEqual([binary,gpu]);
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
    const app={id:'com.exact.game',displayName:'game',crate:()=>'game-windows'};
    expect(packagedBuildChanges(receipt,root,app)).toEqual([]);
    writeFileSync(shader,'edited'); expect(packagedBuildChanges(receipt,root,app)).toEqual([shader]);
    rmSync(shader); expect(packagedBuildChanges(receipt,root,app)).toEqual([shader]);
  } finally { rmSync(root,{recursive:true,force:true}); }
});

test('a type step keeps its newlines, quotes and clipboard words', () => {
  expect(typeCommand('type editor hello for 100')).toEqual(['editor','hello for 100']);
  expect(typeCommand('type editor key End for 40')).toEqual(['editor',{key:'End',for:40}]);
  expect(typeCommand('type editor hello\\n\\nbody')).toEqual(['editor','hello\n\nbody']);
  expect(typeCommand('type editor # A\n\nbody')).toEqual(['editor','# A\n\nbody']);
  expect(typeCommand('type editor "a\\nb"')).toEqual(['editor','a\nb']);
  expect(typeCommand('type "the note" hello')).toEqual(['the note','hello']);
  expect([typeCommand('type editor paste a\\nb c'),typeCommand('type editor copy')]).toEqual([['editor',{clipboard:'paste',text:'a\nb c'}],['editor',{clipboard:'copy'}]]);
});

test('shifted punctuation is the physical key under it, as on Linux and Apple (b6 review C2)', () => {
  for (const [key, code] of [['!', 'Digit1'], ['@', 'Digit2'], ['+', 'Equal'], ['?', 'Slash'], ['~', 'Backquote'], [')', 'Digit0'], ['-', 'Minus']])
    expect(cdpKey(key)).toMatchObject({ key, code });
  expect(cdpKey('Shift++')).toMatchObject({ key: '+', code: 'Equal', modifiers: 8 });
});
