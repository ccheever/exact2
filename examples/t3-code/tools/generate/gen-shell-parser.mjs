// The pinned unbash parser and declarations used by embedded shell highlighting (ISC, LICENSE-T3).
// Runtime sources live beside the highlighter because the bake excludes generator node_modules.
import { readdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
export function generate() {
  const source = join(here, 'node_modules/unbash/dist');
  return readdirSync(source).filter(name => name.endsWith('.js') || name.endsWith('.d.ts')).map(name => ({
    file: join(here, '../../src/highlight/shell-parser', name),
    text: readFileSync(join(source, name), 'utf8'),
  }));
}
