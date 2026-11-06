// The vendored builds of the Mermaid and math pages (assets/mermaid/, assets/math/; timeline-mermaid.ts,
// markdown-math.ts), as the pinned packages ship them (the versions the reference locks, their licences beside
// them): pele's lazy ES module entry with its chunks and DOMPurify's ES module for the Mermaid page; KaTeX's ES
// module, its stylesheet and its WOFF2 fonts for the math page, which the Mermaid page also imports for `$$` labels
// (pele MIT, dompurify MPL-2.0 or Apache-2.0, katex MIT). The pages and their scripts are committed.
import { readdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const assets = join(here, '..', '..', 'assets');
const vendors = [join(assets, 'mermaid', 'vendor'), join(assets, 'math', 'vendor')];
const module = name => join(here, 'node_modules', name);

/** Each vendored file: its source in the package and its place under assets/. */
function sources() {
  const pele = join(module('pele'), 'dist', 'esm'), katex = join(module('katex'), 'dist');
  const files = [
    [join(pele, 'lazy.js'), join('mermaid', 'vendor', 'pele', 'lazy.js')],
    [join(module('pele'), 'LICENSE'), join('mermaid', 'vendor', 'LICENSE-pele')],
    [join(module('dompurify'), 'dist', 'purify.es.mjs'), join('mermaid', 'vendor', 'purify.es.mjs')],
    [join(module('dompurify'), 'LICENSE'), join('mermaid', 'vendor', 'LICENSE-dompurify')],
    [join(katex, 'katex.mjs'), join('math', 'vendor', 'katex.mjs')],
    [join(katex, 'katex.min.css'), join('math', 'vendor', 'katex.min.css')],
    [join(module('katex'), 'LICENSE'), join('math', 'vendor', 'LICENSE-katex')],
  ];
  for (const name of readdirSync(join(pele, 'chunks')).filter(name => name.endsWith('.js')).sort()) {
    files.push([join(pele, 'chunks', name), join('mermaid', 'vendor', 'pele', 'chunks', name)]);
  }
  // The stylesheet lists WOFF2 first, which WebKit loads; it never asks for the WOFF and TTF fallbacks.
  for (const name of readdirSync(join(katex, 'fonts')).filter(name => name.endsWith('.woff2')).sort()) {
    files.push([join(katex, 'fonts', name), join('math', 'vendor', 'fonts', name)]);
  }
  return files;
}

export function generate() {
  return sources().map(([from, to]) => ({ file: join(assets, to), text: to.endsWith('.woff2') ? readFileSync(from) : readFileSync(from, 'utf8') }));
}

/** A vendored file this build does not have: an older pele chunk or the Mermaid build it replaced. */
export function stale(outputs) {
  const written = new Set(outputs.map(({ file }) => file)), present = [];
  const walk = directory => {
    let entries = [];
    try { entries = readdirSync(directory, { withFileTypes: true }); } catch { return; }
    for (const entry of entries) entry.isDirectory() ? walk(join(directory, entry.name)) : present.push(join(directory, entry.name));
  };
  vendors.forEach(walk);
  return present.filter(file => !written.has(file));
}
