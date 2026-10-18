import {test, expect} from 'bun:test';
import {fixture} from './surface.mjs';
import {open} from '../../../scripts/agent.mjs';
import {launchFacts} from '../../../scripts/agent-launch.mjs';

test('a wasm trap while the world is borrowed leaves host diagnostics readable without reentering it', async () => {
  const f = await fixture(); f.create(1); await f.exact.gpu.settled();
  const wasm = new WebAssembly.Instance(new WebAssembly.Module(new Uint8Array([
    0,97,115,109,1,0,0,0,1,4,1,96,0,0,3,2,1,0,7,8,1,4,116,114,97,112,0,0,10,5,1,3,0,0,11,
  ])));
  let calls = 0, borrowed = false;
  f.gpu.gpu_agent = () => { calls++; borrowed = true; f.diagnostics.push('GPU module panicked: borrowed world fixture'); wasm.exports.trap(); };
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
});
