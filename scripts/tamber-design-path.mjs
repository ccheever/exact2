// The app's design file, as the compiler finds it (LLP 1121 D2), read as the
// wire reads it: `app.json` only as a regular file of at most 1 MiB, so a
// FIFO or a giant there never stalls the server (stage 10b's review).

import { existsSync, readFileSync, statSync } from 'node:fs';
import { resolve } from 'node:path';

/** `app.json`'s `"tamber"`, read only as a regular file of at most 1 MiB (a
 * FIFO or a giant there never stalls the server); null without one. */
export function manifestTamber(appDir) {
  try { const m = resolve(appDir, 'app.json'), st = statSync(m); return st.isFile() && st.size <= 1 << 20 ? JSON.parse(readFileSync(m, 'utf8')).tamber ?? null : null; } catch { return null; }
}

/** The app's design, as the compiler finds it: the file `app.json`'s
 * `"tamber"` names, else `app.tamber`; null when there is none. */
export function designPath(appDir) {
  const named = manifestTamber(appDir);
  const path = resolve(appDir, typeof named === 'string' ? named : 'app.tamber');
  return existsSync(path) ? path : null;
}
