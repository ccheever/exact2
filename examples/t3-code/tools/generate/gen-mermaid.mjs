// The Mermaid page's vendored builds (assets/mermaid/, timeline-mermaid.ts): Mermaid's minified ES module build with its
// lazily loaded chunks, and DOMPurify's ES module, as the pinned packages ship them (the versions the reference locks;
// mermaid MIT, dompurify MPL-2.0 or Apache-2.0, their licences beside them). The page and its script are committed.
import { readdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const vendor = join(here, '..', '..', 'assets', 'mermaid', 'vendor');
const mermaid = join(here, 'node_modules', 'mermaid');
const dompurify = join(here, 'node_modules', 'dompurify');
const chunks = join('chunks', 'mermaid.esm.min');

/** Each vendored file: its source in the package and its place under assets/mermaid/vendor/. */
function sources() {
  const files = [
    [join(mermaid, 'dist', 'mermaid.esm.min.mjs'), 'mermaid.esm.min.mjs'],
    [join(mermaid, 'LICENSE'), 'LICENSE-mermaid'],
    [join(dompurify, 'dist', 'purify.es.mjs'), 'purify.es.mjs'],
    [join(dompurify, 'LICENSE'), 'LICENSE-dompurify'],
  ];
  for (const name of readdirSync(join(mermaid, 'dist', chunks)).filter(name => name.endsWith('.mjs')).sort()) {
    files.push([join(mermaid, 'dist', chunks, name), join(chunks, name)]);
  }
  return files;
}

export function generate() {
  return sources().map(([from, to]) => ({ file: join(vendor, to), text: readFileSync(from, 'utf8') }));
}

/** A chunk an older Mermaid build had and this one does not. */
export function stale(outputs) {
  const written = new Set(outputs.map(({ file }) => file));
  const directory = join(vendor, chunks);
  let present = [];
  try { present = readdirSync(directory).map(name => join(directory, name)); } catch { return []; }
  return present.filter(file => !written.has(file));
}
