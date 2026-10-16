#!/usr/bin/env node
// Callable by a qualification farm; this is not another blocking gate.
import {createHash} from 'node:crypto';
import {readFileSync} from 'node:fs';
import {dirname, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
export const receiptPath = 'snapback4/readme-tested.json';
export const testedFiles = ['snapback4/README.md', 'docs/recipes/snapback-auth.md',
  'snapback4/ts/snapback.ts', 'snapback4/online/snapback.ts', 'snapback4/ts/web.ts',
  'snapback4/ts/projection.ts', 'snapback4/online/projection.ts', 'snapback4/ts/release.ts', 'snapback4/online/release.ts',
  'game/new.mjs', 'js/bake/src/typescript.mjs', 'host/web-js/agent.js', 'scripts/agent-test.mjs', 'scripts/agent.mjs',
  'scripts/retest-snapback-readme.mjs', 'scripts/check-snapback-readme.mjs', 'snapback4/Cargo.toml', 'snapback4/Cargo.lock'];
export function digest(text) {return createHash('sha256').update(text).digest('hex');}
export function checkReadmeVersion(readme, pinned, wave=pinned) {
  const claims=[...readme.matchAll(/tested as written against\s+(\d+\.\d+\.\d+)/g)].map(m=>m[1]);
  if (!claims.length) throw new Error('Snapback README has no tested-against version claim');
  if (wave!==pinned) throw new Error(`Snapback wave ${wave} differs from Exact's pin ${pinned}`);
  const stale=claims.filter(version=>version!==pinned);
  if (stale.length) throw new Error(`Snapback README tested against ${[...new Set(stale)].join(', ')}; pinned snapback4 is ${pinned}. Re-run every README example with scripts/retest-snapback-readme.mjs before refreshing the claim.`);
  return {version:pinned,claims:claims.length};
}
export function checkReceipt(receipt, pinned, files) {
  const rerun = 'Run node scripts/retest-snapback-readme.mjs --version ' + pinned;
  if (receipt?.format!==1 || receipt.version!==pinned || !/^\d{4}-\d{2}-\d{2}$/.test(receipt.date ?? '') || !receipt.examples?.length)
    throw new Error(`Snapback README has no completed test receipt for ${pinned}. ${rerun}`);
  for (const name of testedFiles) {
    if (receipt.files?.[name]!==digest(files[name] ?? '')) throw new Error(`Snapback README tests are stale: ${name} changed. ${rerun}`);
  }
  const names=receipt.examples.map(example=>example.name);
  if (names.length!==5 || new Set(names).size!==5 || !['A complete app','A complete online app','A complete online password app','A complete mixed identity app','Auth recipe: reload'].every(name=>names.includes(name)))
    throw new Error(`Snapback README receipt omits a complete example. ${rerun}`);
  for (const example of receipt.examples) if (!(example.tests>0 && example.runs===2)) throw new Error(`Snapback README receipt has an incomplete example. ${rerun}`);
  return receipt;
}
export function checkRepository(root, wave) {
  const pin=JSON.parse(readFileSync(resolve(root,'package.json'),'utf8')).devDependencies.snapback4;
  const files=Object.fromEntries(testedFiles.map(name=>[name,readFileSync(resolve(root,name),'utf8')]));
  const result=checkReadmeVersion(files['snapback4/README.md'],pin,wave ?? pin);
  if (!readFileSync(resolve(root,'bun.lock'),'utf8').includes(`\"snapback4@${pin}\"`)) throw new Error('Snapback npm lock does not match the tested pin');
  let receipt; try {receipt=JSON.parse(readFileSync(resolve(root,receiptPath),'utf8'));} catch {throw new Error(`Snapback README has no test receipt. Run node scripts/retest-snapback-readme.mjs --version ${pin}`);}
  checkReceipt(receipt,pin,files);
  const revision=/snapback4-core = .*rev = "([a-f0-9]{40})", version = "=([^"]+)"/.exec(files['snapback4/Cargo.toml']);
  if (!revision || revision[2]!==pin || revision[1]!==receipt.revision) throw new Error('Snapback Cargo source does not match the README test receipt');
  const claims=[...files['snapback4/README.md'].matchAll(/tested as written against\s+\d+\.\d+\.\d+\s*\((\d{4}-\d{2}-\d{2})\)/g)];
  if (claims.length!==result.claims || claims.some(m=>m[1]!==receipt.date)) throw new Error('Snapback README tested-against dates do not match its test receipt');
  return {...result,examples:receipt.examples.length};
}
if (process.argv[1] && resolve(process.argv[1])===fileURLToPath(import.meta.url)) {
  try {
    const args=process.argv.slice(2), root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
    if (args.length && !(args.length===2 && args[0]==='--version' && /^\d+\.\d+\.\d+$/.test(args[1]))) throw new Error('usage: node scripts/check-snapback-readme.mjs [--version <wave-version>]');
    const result=checkRepository(root,args[1]);
    console.log(`Snapback README: ${result.examples} re-tested examples; ${result.claims} claims match pinned ${result.version}`);
  } catch(error) {console.error(error.message);process.exitCode=1;}
}
