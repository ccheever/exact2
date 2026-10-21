// Stand-ins for the JS runtime's modules a test does not load. Each exports, as a no-op function, every name the
// test's real modules take from it, read from their source, so a name the runtime comes to import cannot leave a
// stand-in short ("export 'svgGeometry' not found") and fail every test in the file.
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const WEB_JS = resolve(import.meta.dirname, '../../web-js');

/** Each module the `real` files of host/web-js import from and `real` does not hold, as a stand-in's source. */
export function standIns(real) {
  const taken = new Map();
  for (const file of real) {
    for (const [, names, from] of readFileSync(resolve(WEB_JS, file), 'utf8').matchAll(/(?:import|export)\s*\{([^}]*)\}\s*from\s*['"]\.\/([\w-]+\.js)['"]/g)) {
      if (real.includes(from)) continue;
      const set = taken.get(from) ?? new Set();
      taken.set(from, set);
      for (const name of names.split(',').map(n => n.trim().split(/\s+as\s+/)[0]).filter(Boolean)) set.add(name);
    }
    // A module imported for its side effects alone (rt.js's `import "./said.js"`): an empty stand-in.
    for (const [, from] of readFileSync(resolve(WEB_JS, file), 'utf8').matchAll(/import\s*['"]\.\/([\w-]+\.js)['"]/g)) if (!real.includes(from) && !taken.has(from)) taken.set(from, new Set());
  }
  return new Map([...taken].map(([file, names]) => [file, [...names].map(n => `export const ${n} = () => {};`).join('\n')]));
}
