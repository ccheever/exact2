import {test, expect} from 'bun:test';
import {mkdirSync, mkdtempSync, readFileSync, writeFileSync, rmSync} from 'node:fs';
import {resolve} from 'node:path';

test('a signalled leg resumes, collecting exit 3, and publishes byte-identical first pins', async () => {
  mkdirSync(resolve(import.meta.dir,'../target'), {recursive:true});
  const parent = mkdtempSync(resolve(import.meta.dir,'../target/resume-'));
  const app = resolve(parent,'sample'); mkdirSync(app);
  const pinFile = resolve(app,'pins.json');
  writeFileSync(pinFile, '{"ticks":{},"saves":{}}');
  writeFileSync(resolve(app,'proof.mjs'), `
    import {mkdirSync,writeFileSync} from 'node:fs';
    const host = process.argv[2], mode = process.env.EXACT_GAME_PARANOID;
    if (process.env.KILL_PROOF_LEG === '1' && host === 'web' && mode === '1') process.kill(process.pid, 'SIGTERM');
    const out = process.env.EXACT_PROOF_OUT;
    mkdirSync(out,{recursive:true});
    const summary = {host, mode, status:'UNVERIFIED', failures:[], seconds:0, inputs:'a'.repeat(64), sourceInputs:'b'.repeat(64), pins:{ticks:{1:'0x0000000000000001'}, saves:{checkpoint:'c'.repeat(64)}}, worlds:[], saves:[]};
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
  } finally { rmSync(parent,{recursive:true,force:true}); }
}, 120000);
