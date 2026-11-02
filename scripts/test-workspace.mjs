// What a throwaway Cargo workspace a test builds needs to be like the real one.
import { copyFileSync } from 'node:fs';
import { resolve } from 'node:path';

/** `dir` with the repository's toolchain pin, as an exact2 root or app has: Cargo
 *  outside this checkout otherwise needs a rustup default, which a machine need not
 *  configure. Returns `dir`. */
export function pinned(dir) {
  copyFileSync(resolve(import.meta.dir, '../rust-toolchain.toml'), resolve(dir, 'rust-toolchain.toml'));
  return dir;
}
