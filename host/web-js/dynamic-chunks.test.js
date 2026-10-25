// No module the runtime loads lazily (a dynamic `import()` of a sibling: a chunk) imports rt.js, the entry's own
// module, directly or through another module: a chunk that imports the module that loads it is shared code a
// single-file bundle can't make (the paint harness's IIFE build failed so on 642157ccba; fa737ea8ca). A chunk takes
// the runtime's objects from rt-link.js instead. Static imports of `./x.js` and dynamic ones of `./x.js` or
// `new URL('./x.js', import.meta.url)` are read from the sources, the web host's glue and this runtime's together, as a
// build lays them side by side (this runtime's taking precedence).
import { expect, test } from 'bun:test';
import { readdirSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const here = new URL('.', import.meta.url).pathname, web = resolve(here, '../web');
const ENTRY = new Set(['rt.js', 'app.js']); // the entry itself, which some modules import lazily by name
const at = new Map();
for (const dir of [web, here]) for (const f of readdirSync(dir)) if (f.endsWith('.js') && !/\.test\.js$/.test(f)) at.set(f, resolve(dir, f));
const files = [...at.keys()];
const statics = new Map(), dynamics = new Map();
for (const f of files) {
  const src = readFileSync(at.get(f), 'utf8');
  statics.set(f, [...src.matchAll(/(?:^|[;\n])\s*(?:import|export)\s[^;]*?from\s*["']\.\/([^"'/]+\.js)["']|(?:^|[;\n])\s*import\s*["']\.\/([^"'/]+\.js)["']/g)].map((m) => m[1] ?? m[2]));
  dynamics.set(f, [...src.matchAll(/import\(\s*(?:["']\.\/([^"'/]+\.js)["']|new URL\(\s*["']\.\/([^"'/]+\.js)["']\s*,\s*import\.meta\.url\s*\)(?:\.href)?)\s*\)/g)].map((m) => m[1] ?? m[2]));
}
/** The chain from `root` to rt.js by static imports, or null. */
function toEntry(root) {
  const from = new Map([[root, null]]), queue = [root];
  while (queue.length) {
    const f = queue.shift();
    if (f === 'rt.js') { const chain = []; for (let x = f; x; x = from.get(x)) chain.unshift(x); return chain; }
    for (const g of statics.get(f) ?? []) if (!from.has(g)) { from.set(g, f); queue.push(g); }
  }
  return null;
}

test('no lazily loaded chunk of the JS runtime imports rt.js, directly or through another module', () => {
  const chunks = [...new Set([...dynamics.values()].flat())].filter((f) => !ENTRY.has(f) && files.includes(f)).sort();
  expect(chunks.length).toBeGreaterThan(20); // the reader sees them (frames, motion, native, the glues…)
  expect(chunks).toContain('motion.js');
  expect(chunks).toContain('native.js');
  const reached = chunks.map((c) => [c, toEntry(c)]).filter(([, chain]) => chain).map(([c, chain]) => `${c}: ${chain.join(' → ')}`);
  expect(reached).toEqual([]);
});
