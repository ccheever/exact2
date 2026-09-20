import {test} from 'bun:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {createHash} from 'node:crypto';

// Execute the actual bake's HTML write without compiling either wasm.
const build=readFileSync(new URL('../build.mjs',import.meta.url),'utf8');
const html=readFileSync(new URL('../index.html',import.meta.url),'utf8');
const fragment=build.slice(build.indexOf('const icon = webManifest.icons?.[0];'),build.indexOf('// The deep-link association file'));
function page(manifest) {
  let written;
  new Function('app','webManifest','stage','resolve','readFileSync','writeFileSync',fragment)(
    {manifest},manifest,'fixture',(...parts)=>parts.join('/'),()=>html,(_,text)=>written=text);
  return written;
}
const ordinary={name:'A & B',theme_color:'#123456',icons:[{src:'assets/icon.png',type:'image/png'}]};
test('ordinary app HTML is byte-identical to the K3 bake, including metadata escaping',()=>{
  const digest=createHash('sha256').update(page(ordinary)).digest('hex');
  assert.equal(digest,'3b1b280afd496b3634c934493705736a0cc8faf0997fd4a57b89739abe6b4ba4');
  assert.equal(page(ordinary).includes('modulepreload'),false);
});
test('only manifest-declared surface modules preload the JS and credential-matched wasm fetch',()=>{
  for(const world of [true,false]) {
    const document=page({...ordinary,game:{crate:'fixture-logic',type:'Fixture',world}});
    assert.equal((document.match(/rel="modulepreload" href="gpu.js"/g)??[]).length,1);
    assert.equal((document.match(/rel="preload" as="fetch" crossorigin href="gpu_bg.wasm"/g)??[]).length,1);
    assert.ok(document.indexOf('modulepreload')<document.indexOf('<script type="module"'));
    assert.ok(document.includes('href="app.wasm" fetchpriority="high"'));
    assert.equal(document.includes('data-device-free-surfaces'),world);
  }
});
