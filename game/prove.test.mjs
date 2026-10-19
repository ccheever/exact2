import {test, expect} from 'bun:test';
import {mkdirSync, mkdtempSync, readFileSync, writeFileSync, rmSync} from 'node:fs';
import {resolve} from 'node:path';

test('a signalled leg resumes, collecting exit 3, and publishes byte-identical first pins', async () => {
  mkdirSync(resolve(import.meta.dir,'../target'), {recursive:true});
  const parent = mkdtempSync(resolve(import.meta.dir,'../target/resume-'));
  const app = resolve(parent,'sample'); mkdirSync(app);
  mkdirSync(resolve(app,'logic/src'), {recursive:true});
  writeFileSync(resolve(app,'logic/src/lib.rs'), '1');
  const pinFile = resolve(app,'pins.json');
  writeFileSync(pinFile, '{"ticks":{},"saves":{}}');
  writeFileSync(resolve(app,'proof.mjs'), `
    import {mkdirSync,readFileSync,writeFileSync} from 'node:fs';
    const host = process.argv[2], mode = process.env.EXACT_GAME_PARANOID;
    if (process.env.KILL_PROOF_LEG === '1' && host === 'web' && mode === '1') process.kill(process.pid, 'SIGTERM');
    const value = readFileSync('logic/src/lib.rs','utf8');
    if (process.env.EDIT_SOURCE_APP) writeFileSync(process.env.EDIT_SOURCE_APP+'/logic/src/lib.rs','2');
    const out = process.env.EXACT_PROOF_OUT;
    mkdirSync(out,{recursive:true});
    const summary = {host, mode, status:'UNVERIFIED', failures:[], seconds:0, inputs:'a'.repeat(64), sourceInputs:'b'.repeat(64), pins:{ticks:{1:'0x'+BigInt(value).toString(16).padStart(16,'0')}, saves:{checkpoint:'c'.repeat(64)}}, worlds:[], saves:[]};
    writeFileSync(out+'/summary.json',JSON.stringify(summary));
    process.exit(3);
  `);
  const run = async (args = [], env = {}) => {
    const child = Bun.spawn([process.execPath, resolve(import.meta.dir,'prove.mjs'), app, ...args], {env:{...process.env,...env},stdout:'pipe',stderr:'pipe'});
    const [code, stdout, stderr] = await Promise.all([child.exited,new Response(child.stdout).text(),new Response(child.stderr).text()]);
    return {code, text:stdout+stderr, root:stdout.match(/^ARTIFACTS (.+)$/m)?.[1]};
  };
  try {
    const first = await run(); expect(first.code, first.text).toBe(0);
    expect(first.text).toContain('PROOF PASS sample: baseline published');
    const expected = readFileSync(pinFile,'utf8');
    writeFileSync(pinFile, '{"ticks":{},"saves":{}}');
    const interrupted = await run([], {KILL_PROOF_LEG:'1'});
    expect(interrupted.code).toBe(1);
    expect(JSON.parse(readFileSync(pinFile)).ticks).toEqual({});
    const resumed = await run(['--continue',interrupted.root]);
    expect(resumed.code, resumed.text).toBe(0);
    expect(resumed.text).toContain('REUSE linux-0-gpu-dev-1');
    expect(resumed.text).not.toContain('REUSE web-1-gpu-dev-1');
    expect(readFileSync(pinFile,'utf8')).toBe(expected);
    const ledger = JSON.parse(readFileSync(resolve(resumed.root,'legs.json')));
    expect(Object.values(ledger.legs).every(leg => leg.exitCode === 3 && leg.state === 'complete')).toBe(true);
    const matrix = await run(['--continue',interrupted.root,'--hosts','web,linux']);
    expect(matrix.code).toBe(1); expect(matrix.text).toContain('matrix');
    writeFileSync(pinFile, '{"ticks":{},"saves":{}}');
    const edited = await run([], {EDIT_SOURCE_APP:app});
    expect(edited.code, edited.text).toBe(0);
    expect(readFileSync(pinFile,'utf8')).toBe(expected);
    const changed = await run(['--continue',edited.root]);
    expect(changed.code).toBe(1);
    expect(changed.text).toContain('app/logic/src/lib.rs changed');
    writeFileSync(resolve(app,'logic/src/lib.rs'), '1');
    mkdirSync(resolve(app,'tools'));
    writeFileSync(resolve(app,'tools/probe.mjs'), 'an unrelated probe');
    const tools = await run(['--continue',edited.root]);
    expect(tools.code, tools.text).toBe(0);
    expect(readFileSync(pinFile,'utf8')).toBe(expected);
  } finally { rmSync(parent,{recursive:true,force:true}); }
}, 120000);


test('a failed save comparison cannot continue under a weaker comparison contract', async () => {
  const parent = mkdtempSync(resolve(import.meta.dir,'../target/compare-')), app=resolve(parent,'sample'); mkdirSync(app);
  writeFileSync(resolve(app,'pins.json'),'{"ticks":{"1":"x"},"saves":{"save":"x"}}');
  writeFileSync(resolve(app,'proof.mjs'),`
    import {writeFileSync} from 'node:fs';
    const host=process.argv[2];
    if (import.meta.main) writeFileSync(process.env.EXACT_PROOF_OUT+'/summary.json',JSON.stringify({host,seconds:0,status:'PASS',worlds:[{session:1,tick:1,hash:'x'}],saves:[host]}));
  `);
  const run = async args => {
    const p=Bun.spawn([process.execPath,resolve(import.meta.dir,'prove.mjs'),app,...args],{stdout:'pipe',stderr:'pipe'});
    const [code,out,err]=await Promise.all([p.exited,new Response(p.stdout).text(),new Response(p.stderr).text()]);
    return {code,text:out+err,root:out.match(/^ARTIFACTS (.+)$/m)?.[1]};
  };
  try {
    const failed=await run(['--hosts','linux,web','--compare-saves']);
    expect(failed.code,failed.text).toBe(1); expect(failed.text).toContain('PROOF FAIL');
    const continued=await run(['--hosts','linux,web','--continue',failed.root]);
    expect(continued.code).toBe(1); expect(continued.text).toContain('continue refused');
  } finally {rmSync(parent,{recursive:true,force:true});}
},120000);

test('parent SIGINT stops a baseline without starting the next mode', async () => {
  const parent=mkdtempSync(resolve(import.meta.dir,'../target/interrupt-')), app=resolve(parent,'sample'); mkdirSync(app);
  writeFileSync(resolve(app,'pins.json'),'{"ticks":{},"saves":{}}');
  writeFileSync(resolve(app,'proof.mjs'),`console.error('LEG_STARTED');setInterval(()=>{},1000);`);
  const p=Bun.spawn([process.execPath,resolve(import.meta.dir,'prove.mjs'),app],{stdout:'pipe',stderr:'pipe'});
  const out=new Response(p.stdout).text(); let err='';
  try {
    for await (const chunk of p.stderr) { err+=new TextDecoder().decode(chunk); if(err.includes('LEG_STARTED')) {p.kill('SIGINT');break;} }
    expect(await p.exited).not.toBe(0);
    const stdout=await out;
    const root=stdout.match(/^ARTIFACTS (.+)$/m)[1];
    expect(Object.keys(JSON.parse(readFileSync(resolve(root,'legs.json'))).legs)).toHaveLength(1);
    expect(JSON.parse(readFileSync(resolve(app,'pins.json'))).ticks).toEqual({});
  } finally {p.kill();await p.exited;rmSync(parent,{recursive:true,force:true});}
},120000);

test('proof legs share both Cargo directories across snapshots', async () => {
  const parent=mkdtempSync(resolve(import.meta.dir,'../target/proof-cache-')), app=resolve(parent,'sample');mkdirSync(app);
  writeFileSync(resolve(app,'pins.json'),'{"ticks":{"1":"x"},"saves":{"save":"x"}}');
  writeFileSync(resolve(app,'proof.mjs'),`
    import {writeFileSync} from 'node:fs';
    const expected=${JSON.stringify(resolve(app,'target'))};
    if (process.env.CARGO_TARGET_DIR !== expected || process.env.CARGO_BUILD_BUILD_DIR !== expected)
      throw new Error('proof lost the shared Cargo target or intermediate directory');
    writeFileSync(process.env.EXACT_PROOF_OUT+'/summary.json',JSON.stringify({host:'linux',seconds:0,status:'PASS',worlds:[{session:1,tick:1,hash:'x'}],saves:['x']}));
  `);
  try {
    for(let i=0;i<2;i++) {
      const child=Bun.spawn([process.execPath,resolve(import.meta.dir,'prove.mjs'),app],{stdout:'pipe',stderr:'pipe'});
      const [code,out,err]=await Promise.all([child.exited,new Response(child.stdout).text(),new Response(child.stderr).text()]);
      expect(code,out+err).toBe(0);
    }
  } finally {rmSync(parent,{recursive:true,force:true});}
},120000);
