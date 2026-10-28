// The web host's file groups (scripts/app.mjs WEB_HOST_GROUPS): a file's static imports ship with it. A build ships
// base always and another group only with its capability, so a group's file may import statically only from its own
// group or base (or a group that always ships with it). documents-glue.js (base) importing storage-environment.js (then storage only) left an app with no
// storage capability without its drops and pastes on the wasm page (the module 404'd; conform's synthetic-file-drag).
import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { webHostFiles } from './app.mjs';

const ROOT = new URL('..', import.meta.url).pathname;
const GROUPS = ['base', 'module', 'storage', 'rust', 'gpu', 'gpuModules', 'native', 'development'];
// Groups that always ship with another: storage ships whenever module does (an app with app.ts; scripts/app.mjs).
const WITH = { module: ['storage'] };

/** A source's static relative imports (`import … from './x.js'`, `import './x.js'`, `export … from './x.js'`), by file name. */
export function staticImports(source) {
  const names = [...source.matchAll(/(?:^|[;\s])(?:import|export)\s*(?:[\w*{}\s,$]+\s*from\s*)?["']\.\/([\w.-]+\.m?js)["']/g)].map(m => m[1]);
  return [...new Set(names)];
}

test('static imports: import, export and side-effect forms, not dynamic import()', () => {
  expect(staticImports(`import { a } from './a.js';\nimport './b.js';\nexport { c } from "./c.js";\nconst d = await import('./d.js');`)).toEqual(['a.js', 'b.js', 'c.js']);
});

test("each group's files import statically only from their own group or base", () => {
  const base = new Set(Object.keys(webHostFiles('base')));
  const strays = [];
  for (const group of GROUPS) {
    const files = webHostFiles(group), own = new Set(Object.keys(webHostFiles(group, ...(WITH[group] ?? []))));
    for (const [name, path] of Object.entries(files)) {
      if (!/\.m?js$/.test(name) || !path.startsWith('host/web/')) continue;
      for (const dep of staticImports(readFileSync(join(ROOT, path), 'utf8')))
        if (!own.has(dep) && !base.has(dep)) strays.push(`${group}/${name} imports ${dep}`);
    }
  }
  expect(strays).toEqual([]);
});
