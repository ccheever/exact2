// D2's labels are written three times: the interface's `tables.labels`
// (the substitute's), the web implementation's and the Rust one's. This
// checks the three agree, family by family.
//
// usage: bun packages/gamepad/test/labels.mjs
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { LABELS } from '../web/index.js';

const pkg = join(import.meta.dir, '..');
const declared = JSON.parse(readFileSync(join(pkg, 'exact.json'), 'utf8')).tables.labels;
const rust = Object.fromEntries(
  [...readFileSync(join(pkg, 'linux/lib.rs'), 'utf8').matchAll(/\("(\w+)", \[([^\]]*)\]\)/g)]
    .map(([, family, list]) => [family, [...list.matchAll(/"([^"]*)"/g)].map(m => m[1])]),
);
let failed = 0;
for (const [name, table] of [['web', LABELS], ['rust', rust]]) {
  const same = JSON.stringify(table) === JSON.stringify(declared);
  if (!same) failed++;
  console.log(`${same ? 'PASS' : 'FAIL'} ${name}'s labels are the interface's${same ? '' : `: ${JSON.stringify(table)}`}`);
}
process.exit(failed ? 1 : 0);
