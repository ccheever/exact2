// The Apple build's compiled arms, cached by key in a directory every app's build shares.
import { copyFileSync, mkdirSync, readdirSync, realpathSync, rmSync, statSync } from 'node:fs';
import { basename, resolve } from 'node:path';

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

/** The app's own sources copied into `dir` as they are when the build begins: the
 * arms compile the copies, so an edit made while the build runs lands in the next
 * build instead of failing this one (swiftc: "input file … was modified during the
 * build"). `key` names a copy by its author's path, so the arm cache keys as before;
 * `authored` puts the author's paths back into a compiler's output. */
export function snapshotSources(files, dir) {
  mkdirSync(dir, { recursive: true });
  const real = realpathSync(dir), authored = new Map();
  const copies = files.map((file) => {
    const copy = resolve(dir, basename(file));
    copyFileSync(file, copy);
    authored.set(resolve(real, basename(file)), file).set(copy, file); // the longer /private/var… first
    return copy;
  });
  return {
    copies,
    key: (arg) => authored.get(arg) ?? arg,
    authored: (text) => [...authored].reduce((out, [copy, file]) => out.replaceAll(copy, file), text),
  };
}
