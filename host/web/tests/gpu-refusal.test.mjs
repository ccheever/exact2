import {test, expect} from 'bun:test';
import {mkdtempSync, mkdirSync, readFileSync, rmSync} from 'node:fs';
import {resolve} from 'node:path';
import {fixture} from './surface.mjs';
import {open} from '../../../scripts/agent.mjs';
import {launchFacts} from '../../../scripts/agent-launch.mjs';

test('a wasm trap while the world is borrowed leaves host diagnostics readable without reentering it', async () => {
  const f = await fixture(); f.create(1); await f.exact.gpu.settled();
  const target = resolve(import.meta.dir,'../../../target');
  mkdirSync(target,{recursive:true});
  const output = mkdtempSync(resolve(target,'borrow-panic-'));
  let bytes;
  try {
    const child = Bun.spawn(['rustc',resolve(import.meta.dir,'../../../game/engine/tests/fixtures/gpu-borrow-panic.rs'),
      '--crate-type','cdylib','--target','wasm32-unknown-unknown','-C','panic=abort',
      '-o',resolve(output,'fixture.wasm')], {stdout:'pipe',stderr:'pipe'});
    const [code, diagnostic] = await Promise.all([child.exited,new Response(child.stderr).text()]);
    expect(code,diagnostic).toBe(0);
    bytes = readFileSync(resolve(output,'fixture.wasm'));
  } finally { rmSync(output,{recursive:true,force:true}); }
  let calls = 0, borrowed = false;
  const wasm = new WebAssembly.Instance(new WebAssembly.Module(bytes), {env:{
    panic_log(pointer,length) { f.diagnostics.push(new TextDecoder().decode(new Uint8Array(wasm.exports.memory.buffer,pointer,length))); },
    world_borrowed() { borrowed = true; },
  }});
  f.gpu.gpu_agent = () => { calls++; wasm.exports.trap_borrowed_world(); };
  const refused = f.exact.gpu.handle({op:'state',id:1,world:true}, null, x => x);
  expect(refused.refused.host).toBe('web');
  expect(refused.refused.subsystem).toBe('gpu');
  expect(borrowed).toBe(true);
  const logs = f.exact.gpu.decorate({op:'logs',all:true},{lines:[],from:0,next:0});
  expect(logs.world[0].lines.join(' ')).toMatch(/unreachable/i);
  expect(calls).toBe(1);
  const carrier = {host:'web',browser:'chrome',launchFacts:launchFacts({}),reset(){},hostLines:f.diagnostics,
    ask:async () => { throw new WebAssembly.RuntimeError('unreachable'); }};
  const session = await open({reuse:carrier});
  expect((await session.logs()).host.join('\n')).toContain('borrowed world fixture');
  expect((await session.logs({all:true})).host.join('\n')).toContain('borrowed world fixture');
  expect((await session.logs()).host).toEqual([]);
}, 60000);
