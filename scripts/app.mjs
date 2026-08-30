// Where an app lives. Inside this repo an app is `apps/<name>` — its crates
// are workspace members and build into `target/`. Outside it (weird-castle:
// its own repo, its own cargo workspace, depending on this repo's crates by
// path so the two iterate together), `EXACT_APP_DIR` names the directory and
// everything else follows from it: the workspace cargo runs in, the target
// directory the artifacts land in, `app.contract`, `assets/`, `gpu/`. Every
// script that builds, serves, or drives an app resolves it here, so nothing
// else knows the difference.
//
//   node host/web/build.mjs weird-castle-web          (EXACT_APP_DIR set)
//   node host/apple/build.mjs --ios weird-castle-apple --run
//   node host/web/dev.mjs --app weird-castle
//   node scripts/agent.mjs --app weird-castle macos tree
import { existsSync } from 'node:fs';
import { basename, resolve } from 'node:path';

const ROOT = resolve(new URL('..', import.meta.url).pathname);

/** The app `nameOrCrate` names (`caltrain`, `caltrain-web`, …; `EXACT_APP_DIR`'s basename when unset): its directory, cargo workspace, target directory, and crate names. */
export function resolveApp(nameOrCrate) {
  const outside = process.env.EXACT_APP_DIR ? resolve(process.env.EXACT_APP_DIR) : null;
  const name = nameOrCrate ? String(nameOrCrate).replace(/-(web|apple|linux|gpu)$/, '') : outside ? basename(outside) : 'caltrain';
  const dir = outside ?? resolve(ROOT, 'apps', name);
  if (!existsSync(resolve(dir, 'app.contract'))) throw new Error(`no app at ${dir} (no app.contract)${outside ? '' : '; set EXACT_APP_DIR for an app outside this repo'}`);
  const workspace = outside ? dir : ROOT;
  const target = process.env.CARGO_TARGET_DIR ? resolve(process.env.CARGO_TARGET_DIR) : resolve(workspace, 'target');
  return { name, dir, workspace, target, crate: (kind) => `${name}-${kind}` };
}
