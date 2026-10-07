// Runs exact release's own signingOrder() (taken verbatim from the given scripts/exact.mjs) over a
// bundle and signs each entry the way release() does, ad hoc (`-`) in place of a Developer ID.
import { readFileSync, readdirSync, openSync, readSync, closeSync } from 'node:fs';
import { resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
const [exactMjs, bundle] = process.argv.slice(2);
const src = readFileSync(exactMjs, 'utf8');
const fns = [];
for (const name of ['function machO(', 'function signingOrder(', 'function bundleExecutable(']) {
  const start = src.indexOf(name); if (start < 0) continue;
  let depth = 0, end = src.indexOf('{', start);
  for (let i = end; i < src.length; i++) { if (src[i] === '{') depth++; if (src[i] === '}' && --depth === 0) { end = i + 1; break; } }
  fns.push(src.slice(start, end));
}
const pre = (src.match(/^const NESTED_BUNDLE[^\n]*\n/m) ?? [''])[0];
const signingOrder = new Function('readdirSync', 'resolve', 'openSync', 'readSync', 'closeSync', 'readFileSync', `${pre}${fns.join('\n')}; return signingOrder;`)(readdirSync, resolve, openSync, readSync, closeSync, readFileSync);
const order = signingOrder(bundle);
console.log('signingOrder:');
for (const p of order) console.log('  ' + p.replace(bundle, 'Fixture.app'));
for (const path of order) {
  const r = spawnSync('codesign', ['--force', '--sign', '-', '--options', 'runtime',
    ...(path === bundle ? ['--identifier', 'com.example.fixture'] : []), path], { encoding: 'utf8' });
  console.log(`$ codesign --force --sign - --options runtime ${path.replace(bundle, 'Fixture.app')}  -> exit ${r.status}`);
  if (r.status !== 0) process.stdout.write(r.stderr.replaceAll(bundle, 'Fixture.app'));
}
