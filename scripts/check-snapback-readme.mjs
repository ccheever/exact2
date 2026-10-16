#!/usr/bin/env bun
// Callable by a qualification farm; this is not a sixth blocking gate.
import {readFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
export function checkReadmeVersion(readme, pinned, wave=pinned) {
  const claims=[...readme.matchAll(/tested as written against\s+(\d+\.\d+\.\d+)/g)].map(m=>m[1]);
  if (!claims.length) throw new Error('Snapback README has no tested-against version claim');
  if (wave!==pinned) throw new Error(`Snapback wave ${wave} differs from Exact's pin ${pinned}`);
  const stale=claims.filter(version=>version!==pinned);
  if (stale.length) throw new Error(`Snapback README tested against ${[...new Set(stale)].join(', ')}; pinned snapback4 is ${pinned}. Re-run both README tests before refreshing the claim.`);
  return {version:pinned,claims:claims.length};
}
if (process.argv[1] && resolve(process.argv[1])===fileURLToPath(import.meta.url)) {
  try {
    const args=process.argv.slice(2), root=resolve(import.meta.dirname,'..');
    if (args.length && !(args.length===2 && args[0]==='--version' && /^\d+\.\d+\.\d+$/.test(args[1]))) throw new Error('usage: bun scripts/check-snapback-readme.mjs [--version <wave-version>]');
    const pin=JSON.parse(readFileSync(resolve(root,'package.json'),'utf8')).devDependencies.snapback4;
    const result=checkReadmeVersion(readFileSync(resolve(root,'snapback4/README.md'),'utf8'),pin,args[1] ?? pin);
    console.log(`Snapback README: ${result.claims} tested-against claims match pinned ${result.version}`);
  } catch(error) {console.error(error.message);process.exitCode=1;}
}
