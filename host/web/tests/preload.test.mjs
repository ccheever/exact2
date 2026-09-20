import {test} from 'bun:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

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
  const before=readFileSync(new URL('./fixtures/ordinary-page.html',import.meta.url));
  assert.ok(Buffer.from(page(ordinary)).equals(before));
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

test('dev response drops static hints before its verified byte loader, leaving ordinary pages unchanged',()=>{
  const source=readFileSync(new URL('../dev.mjs',import.meta.url),'utf8');
  const start=source.indexOf('    if (index) body = body.toString()');
  const transform=new Function('body','index',source.slice(start,source.indexOf('    if (INSTALL_FILES.includes(found.route))',start))+'return body;');
  const before=page(ordinary);
  assert.equal(transform(before,true),before.replace('<script type="module" src="./glue.js"></script>','<script type="module" src="./glue.js"></script>\n<script type="module" src="./dev.js"></script>'));
  const withModule=page({...ordinary,game:{world:true}});
  assert.equal(transform(withModule,true).includes('rel="preload"'),false);
  assert.equal(transform(withModule,true).includes('rel="modulepreload"'),false);
  assert.equal(transform(withModule,false),withModule);
});
