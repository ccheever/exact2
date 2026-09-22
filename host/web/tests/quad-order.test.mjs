import {test,expect} from 'bun:test';
import {readFileSync,mkdtempSync,writeFileSync,rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {spawnSync} from 'node:child_process';

// Compile the actual comparator and its direct regression with both backends.
// No wgpu device is needed to verify an integer sort-key contract.
test('native and wasm execute the same translucent sort-key regression',()=>{
  const source=readFileSync(process.env.R7_QUADS_SOURCE ?? 'game/render/src/quads.rs','utf8');
  const types=source.slice(source.indexOf('#[derive(Clone, Copy)]'),source.indexOf('pub(crate) struct Draw'));
  const start=source.indexOf('    fn every_kind_and_owner_ordinal_has_the_same_total_order()');
  const body=source.slice(start,source.indexOf('    #[test]',start));
  const dir=mkdtempSync(join(tmpdir(),'exact-r7-order-'));
  const env={...process.env,DEVELOPER_DIR:'/Library/Developer/CommandLineTools',SDKROOT:'/Library/Developer/CommandLineTools/SDKs/MacOSX26.sdk',EXACT_UPDATE_TRUST:'development',EXACT_IDENTITY:'-'};
  try {
    const file=join(dir,'order.rs'), wasm=join(dir,'order.wasm'), native=join(dir,'order');
    // Child drawing is native-only; its portable rank is also exercised by wasm.
    const portable=(types+'\n'+body).replaceAll('#[cfg(not(target_arch = "wasm32"))]','');
    writeFileSync(file,`#![allow(dead_code,unused_mut)]\n${portable}\n#[no_mangle] pub extern "C" fn verify() { every_kind_and_owner_ordinal_has_the_same_total_order(); }\nfn main(){verify();}`);
    for(const args of [[file,'--edition=2021','-o',native],[file,'--edition=2021','--target','wasm32-unknown-unknown','--crate-type','cdylib','-C','panic=abort','-o',wasm]]) {
      const r=spawnSync('rustc',args,{env,encoding:'utf8',timeout:60000});expect(r.status,r.stderr).toBe(0);
    }
    const r=spawnSync(native,[],{encoding:'utf8',timeout:10000});expect(r.status,r.stderr).toBe(0);
    const instance=new WebAssembly.Instance(new WebAssembly.Module(readFileSync(wasm)),{});
    expect(()=>instance.exports.verify()).not.toThrow();
  } finally {rmSync(dir,{recursive:true,force:true});}
},65000);
