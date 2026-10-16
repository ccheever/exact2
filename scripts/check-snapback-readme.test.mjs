import {expect,test} from 'bun:test';
import {mkdtempSync,readFileSync,rmSync,writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {checkReadmeVersion,checkReceipt,checkRepository,digest,testedFiles} from './check-snapback-readme.mjs';
import {commitFiles,extractExamples,refreshClaims} from './retest-snapback-readme.mjs';

test('qualification refuses stale version claims and a different wave pin',()=>{
  expect(checkReadmeVersion('tested as written against 0.4.29\ntested as written against\n0.4.29','0.4.29')).toEqual({version:'0.4.29',claims:2});
  expect(()=>checkReadmeVersion('tested as written against 0.4.28','0.4.29')).toThrow(/Re-run every/);
  expect(()=>checkReadmeVersion('tested as written against 0.4.29\ntested as written against 0.4.28','0.4.29')).toThrow(/0.4.28/);
  expect(()=>checkReadmeVersion('no claim','0.4.29')).toThrow(/no tested-against/);
  expect(()=>checkReadmeVersion('tested as written against 0.4.29','0.4.29','0.4.30')).toThrow(/differs/);
});
test('a text-only repin, edited example, or incomplete run cannot refresh the receipt',()=>{
  const files=Object.fromEntries(testedFiles.map(name=>[name,`contents of ${name}`]));
  const examples=['A complete app','A complete online app','A complete online password app','Auth recipe: mixed identity'].map(name=>({name,tests:2,runs:2}));
  const receipt={format:1,version:'0.4.29',date:'2026-10-16',examples,files:Object.fromEntries(testedFiles.map(name=>[name,digest(files[name])]))};
  expect(checkReceipt(receipt,'0.4.29',files)).toBe(receipt);
  expect(()=>checkReceipt(receipt,'0.4.30',files)).toThrow(/no completed test receipt/);
  expect(()=>checkReceipt({...receipt,version:'0.4.30'},'0.4.30',{...files,'snapback4/README.md':'claims updated without running'})).toThrow(/README.md changed/);
  expect(()=>checkReceipt(receipt,'0.4.29',{...files,'snapback4/online/snapback.ts':'new adapter'})).toThrow(/snapback.ts changed/);
  expect(()=>checkReceipt({...receipt,examples:examples.map(example=>({...example,runs:1}))},'0.4.29',files)).toThrow(/incomplete example/);
  expect(()=>checkReceipt({...receipt,examples:examples.slice(1)},'0.4.29',files)).toThrow(/omits/);
});
test('failed publication restores existing files and removes new ones',()=>{
  const dir=mkdtempSync(join(tmpdir(),'readme-retest-')),first=join(dir,'first'),second=join(dir,'second');
  writeFileSync(first,'old pin');let writes=0;
  try {
    expect(()=>commitFiles(new Map([[first,'new pin'],[second,'new claim']]),{write:(path,bytes)=>{if(++writes===2)throw new Error('disk full');writeFileSync(path,bytes);}})).toThrow(/disk full/);
    expect(readFileSync(first,'utf8')).toBe('old pin');
    expect(()=>readFileSync(second)).toThrow();
  } finally {rmSync(dir,{recursive:true,force:true});}
});
test('the retest extracts every complete authored app including its exact test commands',()=>{
  const readme=readFileSync(new URL('../snapback4/README.md',import.meta.url),'utf8');
  const recipe=readFileSync(new URL('../docs/recipes/snapback-auth.md',import.meta.url),'utf8');
  const examples=extractExamples(readme,recipe);
  expect(examples).toHaveLength(4);
  expect(examples[0].online).toBe(false);
  expect(examples[2].blocks[0]).toContain('use identity password');
  expect(examples[3].blocks[2]).toContain('.guest(');
  expect(examples[3].blocks[3]).toContain('reload');
  expect(()=>extractExamples(readme,recipe.replace('```quarry','```text'))).toThrow(/expected schema/);
  expect(refreshClaims('tested as written against\n0.4.29\n(2026-10-15)','0.4.30','2026-10-16')).toBe('tested as written against 0.4.30 (2026-10-16)');
});

test('the committed pins, authored sources and claims have a complete execution receipt',()=>{
  expect(checkRepository(new URL('..',import.meta.url).pathname).examples).toBe(4);
});
