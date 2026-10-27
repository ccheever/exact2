// D2's labels are written five times: the interface's `tables.labels`
// (the substitute's), and the web, Rust, Swift and Kotlin implementations';
// the Apple symbols twice, `tables.apple` and the Swift's for a drive's pad.
// This checks each copy is the interface's, family by family.
//
// usage: bun packages/gamepad/test/labels.mjs
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { LABELS } from '../web/index.js';

const pkg = join(import.meta.dir, '..');
const declared = JSON.parse(readFileSync(join(pkg, 'exact.json'), 'utf8')).tables.labels;
const rust = Object.fromEntries(
  [...readFileSync(join(pkg, 'linux/lib.rs'), 'utf8').matchAll(/\(\s*"(\w+)",\s*\[([^\]]*)\],?\s*\)/g)]
    .map(([, family, list]) => [family, [...list.matchAll(/"([^"]*)"/g)].map(m => m[1])]),
);
const lists = (text, pattern) => Object.fromEntries([...text.matchAll(pattern)]
  .map(([, family, list]) => [family, [...list.matchAll(/"([^"]*)"/g)].map(m => m[1])]));
const swift = readFileSync(join(pkg, 'apple/Gamepad.swift'), 'utf8');
const between = (text, from) => text.slice(text.indexOf(from), text.indexOf(']\n\n', text.indexOf(from)));
const kotlin = readFileSync(join(pkg, 'android/Gamepad.kt'), 'utf8');
const symbols = Object.fromEntries(Object.entries(JSON.parse(readFileSync(join(pkg, 'exact.json'), 'utf8')).tables.apple).map(([k, v]) => [k, v.symbols]));
let failed = 0;
for (const [name, table, want] of [
  ['web', LABELS, declared], ['rust', rust, declared],
  ['swift', lists(between(swift, 'familyLabels'), /"(\w+)": \[([^\]]*)\]/g), declared],
  ['kotlin', lists(kotlin, /"(\w+)" to listOf\(([^)]*)\)/g), declared],
  ['swift symbols', lists(between(swift, 'familySymbols'), /"(\w+)": \[([^\]]*)\]/g), symbols],
]) {
  const same = JSON.stringify(table) === JSON.stringify(want);
  if (!same) failed++;
  console.log(`${same ? 'PASS' : 'FAIL'} ${name}: the interface's table${same ? '' : `: ${JSON.stringify(table)}`}`);
}
process.exit(failed ? 1 : 0);
