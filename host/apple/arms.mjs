// The Apple build's compiled arms, cached by key in a directory every app's build shares.
import { readdirSync, rmSync, statSync } from 'node:fs';
import { resolve } from 'node:path';

/** An arm's cached copies (`<key>-<name>`) beyond the three last used for
 * `name`, removed: different arguments (a development build, a production
 * one) keep a copy each, as every app's build shares the directory. Only an
 * exact `<16 hex>-<name>` is this name's; `probe-<name>` is another arm's.
 * A copy a crashed build left half-written (`.<pid>.tmp`) goes after a day. */
export function evictArms(dir, name) {
  // A copy another build removed meanwhile is the oldest
  const at = (file) => { try { return statSync(resolve(dir, file)).mtimeMs; } catch { return 0; } };
  const entries = readdirSync(dir);
  const copies = entries.filter((f) => /^[0-9a-f]{16}-/.test(f) && f.slice(17) === name).sort((a, b) => at(b) - at(a));
  for (const old of copies.slice(3)) rmSync(resolve(dir, old), { force: true });
  for (const tmp of entries.filter((f) => f.endsWith('.tmp'))) {
    if (Date.now() - at(tmp) > 86_400_000) rmSync(resolve(dir, tmp), { force: true });
  }
}
