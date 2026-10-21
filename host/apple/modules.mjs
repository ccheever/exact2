/**
 * The host's Rust modules, kept for the machine (LLP 1036.000 §10).
 *
 * `libexact_canvas_vello` and `libexact_svg_raster` hold nothing of an app,
 * so every checkout whose sources agree builds the same module. A first build
 * in a fresh checkout compiled both from nothing beside the bake: 190 CPU
 * seconds, 27 s of a 106 s build on a ten-core M4. A development build keeps
 * each module it compiles in `~/.cache/exact/apple-modules` (beside Hermes),
 * and a build in any checkout whose files are those bytes takes the kept
 * module instead of compiling it (scripts/kept.mjs says what "those bytes"
 * covers). Beside the crate, the group is the target and profile and the SDK
 * and Metal toolchain the caller names. A production build never reads a
 * kept module.
 */
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { keptProducts } from '../../scripts/kept.mjs';

/**
 * The kept modules for one build: `find(crate)` is a kept dylib built from
 * this checkout's bytes, or `null`; `keep(crate, started)` keeps the one
 * Cargo linked since `started`. `sdk` is the SDK's path and `metal` what the
 * Metal toolchain says of its version: with Xcode's own version they are the
 * tools a module's link and its shaders' compile run.
 */
export function keptModules({ root, moduleTarget, target, profile, env, sdk, metal }) {
  const text = (path) => { try { return readFileSync(path, 'utf8'); } catch { return null; } };
  const settings = text(resolve(sdk, 'SDKSettings.json')), xcode = /^(.*\.app\/Contents)\//.exec(sdk)?.[1];
  const sha = settings && createHash('sha256').update(settings).digest('hex');
  const tools = [sdk, sha, xcode ? text(resolve(xcode, 'version.plist')) : null, metal];
  const kept = keptProducts({ root, cache: 'apple-modules', env, tools, skip: [moduleTarget], label: 'host/apple' });
  const libDir = resolve(moduleTarget, target, profile);
  const spec = (crate) => {
    const product = resolve(libDir, `lib${crate.replaceAll('-', '_')}.dylib`);
    return { crate, product, depInfo: product.replace(/dylib$/, 'd'), group: [target, profile],
      buildDirs: [resolve(libDir, 'build'), resolve(moduleTarget, profile, 'build')], stamp: resolve(moduleTarget, 'kept', `${crate}-${target}-${profile}.json`) };
  };
  return { find: (crate) => kept(spec(crate)).find(), keep: (crate, started) => kept(spec(crate)).keep(started) };
}
