import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { prepareRustBundle, rustBundle, rustCard, rustCards, rustFiles, rustReceipt } from './rust.mjs';
import { rustRuntime } from '../host/web/rust-glue.js';

test('trapping module cleanup cannot throw through the resident Wasm import', () => {
  const original=globalThis.WebAssembly, log=console.error;
  const memory=new original.Memory({initial:1});
  const trap=()=>{throw new original.RuntimeError('unreachable');};
  try {
    globalThis.WebAssembly={...original,Memory:original.Memory,
      Module:class {static imports(){return [];}},
      Instance:class {constructor(){this.exports={memory,exact_logic_abi:()=>2,
        exact_logic_create:()=>1,exact_logic_destroy:trap,exact_logic_alloc:()=>16,
        exact_logic_dealloc:trap,exact_logic_call:()=>0,exact_logic_output:()=>32,
        exact_logic_output_len:()=>1};}},
    };
    console.error=()=>{};
    const runtime=rustRuntime(()=>memory),handle=runtime.load(0,1,2);
    assert.ok(handle);
    assert.equal(runtime.load(0,1,3),0,'a module of another ABI than the host passes is refused');
    assert.equal(runtime.call(handle,0,1),0xffffffff);
    assert.doesNotThrow(()=>runtime.drop(handle));
    assert.ok(runtime.load(0,1,2),'another module can still be instantiated');
  } finally {globalThis.WebAssembly=original;console.error=log;}
});

test('one produced Rust pair becomes exact platform assets and requirements', () => {
  const app={id:'com.exact.test'}, plan=Buffer.from('plan'), bytes=Buffer.from('module');
  const compat={target:'aarch64-apple-ios',inputs:{rustMode:'wasm',grantCeiling:'fs:notes',rustGrants:'fs:notes'}};
  const receipt=rustReceipt(app,compat,plan,bytes,'wasm32-unknown-unknown','wasm');
  const produced={plan,variants:{wasm:{receipt,bytes}}};
  const cards=rustCards(rustFiles(produced));
  assert.equal(cards.wasm.module.sha256,receipt.module.sha256);
  const base={plan:{...rustCard('app.plan',plan),bytes:plan},assets:[],platforms:{}};
  const build={compat,graph:{artifacts:[{name:'app.plan',requires:{sources:'embedded',plan:1}}]}};
  const result=rustBundle(app,base,build,produced);
  base.platforms.ios=result.bundle;
  assert.doesNotThrow(()=>JSON.stringify(base));
  assert.deepEqual(result.bundle.assets.map(a=>a.name),['rust/app.module.json','rust/app.module.wasm']);
  assert.deepEqual(result.build.graph.artifacts[0].requires,{plan:1,rustMode:'wasm',rustAbi:3,rustTarget:'wasm32-unknown-unknown',grantCeiling:'fs:notes',rustGrants:'fs:notes'});
  assert.equal(result.build.graph.artifacts[2].sha256,receipt.module.sha256);
  const off=rustBundle(app,base,{...build,compat:{...compat,inputs:{rustMode:'off'}}},produced);
  assert.deepEqual(off.bundle.assets,[]);
  assert.equal(off.bundle.platforms,undefined);
  const corrupt=rustFiles(produced);corrupt.set('rust/wasm/app.module.wasm',Buffer.from('changed'));
  assert.throws(()=>rustCards(corrupt),/does not match/);
});

test('mixed Rust receipts use source grants while artifact requirements retain the union ceiling', () => {
  const app={id:'com.exact.mixed'},plan=Buffer.from('plan'),bytes=Buffer.from('module');
  const javascript='net.fetch https://example.test/',rust='fs.read app:/data/rust';
  const compat={inputs:{rustMode:'wasm',javascriptGrants:javascript,rustGrants:rust,grantCeiling:rust+'\n'+javascript}};
  const receipt=rustReceipt(app,compat,plan,bytes,'wasm32-unknown-unknown','wasm');
  assert.equal(receipt.grants,rust);
  const result=rustBundle(app,{plan:{...rustCard('app.plan',plan),bytes:plan},assets:[]},{compat,graph:{artifacts:[{name:'app.plan',requires:{}}]}},{plan,variants:{wasm:{receipt,bytes}}});
  assert.equal(result.build.graph.artifacts[0].requires.grantCeiling,compat.inputs.grantCeiling);
  assert.equal(result.build.graph.artifacts[0].requires.rustGrants,rust);
  assert.throws(()=>rustReceipt(app,{inputs:{...compat.inputs,grantCeiling:javascript}},plan,bytes,'wasm32-unknown-unknown','wasm'),/exceed/);
  assert.throws(()=>rustReceipt(app,{inputs:{grantCeiling:rust}},plan,bytes,'wasm32-unknown-unknown','wasm'),/source grants/);
});

test('publisher companion acquisition binds exact local bytes and isolates concurrent bakes', () => {
  const root=mkdtempSync(resolve(tmpdir(),'exact-rust-record-'));
  try {
    const releases=resolve(root,'.exact/streams/test/releases'), blobs=resolve(root,'.exact/blobs');
    mkdirSync(releases,{recursive:true});mkdirSync(blobs,{recursive:true});
    const assets=[['rust/app.module.json',Buffer.from('{}')],['rust/app.module.wasm',Buffer.from('wasm')]].map(([name,bytes])=>{
      const card=rustCard(name,bytes);writeFileSync(resolve(blobs,card.sha256),bytes);
      return {name,bytes:card.bytes,sha256:card.sha256,url:`../../blobs/${card.sha256}`};
    });
    const record=resolve(releases,'1.json');writeFileSync(record,JSON.stringify({envelope:{assets}}));
    const app={manifest:{rust:true}};
    const env={EXACT_UPDATE_RECEIPT:record,EXACT_UPDATE_TRUST:'production',EXACT_BAKE_OUTPUT:resolve(root,'bake')};
    const a=prepareRustBundle(app,'ios','aarch64-apple-ios',env);
    const b=prepareRustBundle(app,'ios','aarch64-apple-ios',env);
    assert.notEqual(a,b);
    assert.equal(readFileSync(resolve(a,'app.module.wasm'),'utf8'),'wasm');
    assert.throws(()=>prepareRustBundle({manifest:{rust:false}},'ios','aarch64-apple-ios',env),/incompatible/);
    writeFileSync(resolve(blobs,assets[1].sha256),'bad');
    assert.throws(()=>prepareRustBundle(app,'ios','aarch64-apple-ios',env),/differ from signed receipt/);
  } finally {rmSync(root,{recursive:true,force:true});}
});

test('tiered publication binds both executors to one target, plan and module digest', () => {
  const app={id:'com.exact.tiered'},plan=Buffer.from('plan');
  const wasm=Buffer.from([0,97,115,109,1,0,0,0]),native=Buffer.from('native image');
  const compat={target:'aarch64-apple-darwin',inputs:{rustMode:'tiered',rustGrants:'',grantCeiling:''}};
  const produced={plan,variants:{wasm:{receipt:rustReceipt(app,compat,plan,wasm,'wasm32-unknown-unknown','wasm'),bytes:wasm},native:{receipt:rustReceipt(app,compat,plan,native,compat.target,'native'),bytes:native}}};
  const result=rustBundle(app,{plan:{...rustCard('app.plan',plan),bytes:plan},assets:[]},{compat,graph:{artifacts:[{name:'app.plan',requires:{sources:'embedded'}}]}},produced);
  const [meta,image]=result.bundle.assets,receipt=JSON.parse(meta.bytes);
  assert.equal(receipt.executor,'tiered');assert.equal(receipt.target,compat.target);
  assert.equal(image.name,'rust/app.module.bin');
  assert.equal(image.bytes.readUInt32LE(8),wasm.length);
  assert.deepEqual(image.bytes.subarray(12,12+wasm.length),wasm);
  assert.deepEqual(image.bytes.subarray(12+wasm.length),native);
  assert.equal(receipt.module.sha256,image.sha256);
  assert.equal(result.build.graph.artifacts[0].requires.rustMode,'tiered');
  const files=rustFiles({plan,variants:{...produced.variants,tiered:{receipt,bytes:image.bytes}}});
  assert.equal(rustCards(files).tiered.target,compat.target);
  files.set(`rust/tiered/${compat.target}/app.module.bin`,Buffer.from('changed native'));
  assert.throws(()=>rustCards(files),/does not match/);
});

test('tiered mode inherits ordinary environment and platform overrides', async () => {
  const {rustPolicy}=await import('./app.mjs');
  const manifest={rust:{platforms:{macos:{dev:'tiered',prod:'native'},linux:{mode:'tiered',prod:false}}}};
  assert.equal(rustPolicy(manifest,'macos'),'tiered');
  assert.equal(rustPolicy(manifest,'macos','prod'),'native');
  assert.equal(rustPolicy(manifest,'linux'),'tiered');
  assert.equal(rustPolicy(manifest,'linux','prod'),'off');
  assert.equal(rustPolicy(manifest,'ios'),'wasm');
  assert.equal(rustPolicy(manifest,'web'),'browser');
  for(const platform of ['web','ios'])assert.throws(()=>rustPolicy({rust:'tiered'},platform),/unavailable/);
});
