// What an Apple app links: the capabilities its composition names, and the
// SDK its executable records.
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { basename } from 'node:path';

/** The capabilities with a Swift half, by name, and the product of
 * `Package.swift` (its `capabilities` table) each is. */
const SWIFT_CAPABILITIES = [['grouped_lists', 'ExactGroupedLists'], ['markdown', 'ExactMarkdown'], ['surfaces', 'ExactSurfaces'], ['drag', 'ExactDrag'], ['media_picker', 'ExactMediaPicker'], ['auth_session', 'ExactAuthSession'], ['numerals', 'ExactNumerals']];

/** The capabilities a development build links only as production would
 * (LLP 1047.001.000 F): each links a framework that costs every iOS launch
 * its images (PhotosUI, AuthenticationServices, ~300 each; SwiftUI), and the
 * agent's substitutes answer without the first two. */
const BY_USE_IN_DEVELOPMENT = ['media_picker', 'auth_session', 'numerals'];

/** What the archive links (LLP 1047.001 D2, D4, D6; LLP 1047 D8): for a
 * production build, the plan's use-set, as the bake's graph names it, the
 * manifest's `link` (capabilities linked ahead of use) and, for an app
 * whose grants name `auth.session` (`granted`), the sign-in sheet; every
 * capability for every other build, but those of `BY_USE_IN_DEVELOPMENT`,
 * which it links as production would. `link` is `EXACT_APPLE_LINK` for
 * SwiftPM (the names, `none`, or `all`); `products` the Swift capability
 * products the composition links and installs, and an embedder links beside
 * ExactKit. Cargo is told `plan` or `all` before the bake, and the entry
 * derives the same set from the same plan, grants and manifest
 * (`exact_bake::apple_link`). */
export function appleComposition(graph, production, ahead = [], granted = false) {
  const uses = graph?.uses;
  const byUse = Array.isArray(uses) ? [...uses, ...ahead, ...(granted ? ['auth_session'] : [])] : null;
  const link = !byUse ? 'all'
    : production ? [...new Set(byUse)].join(',') || 'none'
    : SWIFT_CAPABILITIES.map(([name]) => name).filter((name) => !BY_USE_IN_DEVELOPMENT.includes(name) || byUse.includes(name)).join(',');
  const products = SWIFT_CAPABILITIES.filter(([name]) => link === 'all' || link.split(',').includes(name)).map(([, product]) => product);
  return { link, products };
}

/** The system frameworks only a capability's Swift half brings, and so
 * absent from an executable whose composition does not link it (LLP
 * 1047.001.000 §6, the check's first form). */
const CAPABILITY_FRAMEWORKS = [['ExactMediaPicker', 'PhotosUI'], ['ExactAuthSession', 'AuthenticationServices'], ['ExactNumerals', 'SwiftUI']];

/** The dylibs an executable's load commands name (`LC_LOAD_DYLIB` and its
 * weak, re-export, lazy and upward forms), every slice of a universal one.
 * Read from the file, since `otool` takes a path ending in `name(member)`
 * for an archive's member (#234). */
export function loadedDylibs(executable) {
  const bytes = readFileSync(executable), names = new Set();
  const thin = (at) => {
    if (bytes.readUInt32LE(at) !== 0xfeedfacf) throw new Error(`host/apple: ${basename(executable)} is not a 64-bit Mach-O`);
    let cursor = at + 32;
    for (let i = 0, n = bytes.readUInt32LE(at + 16); i < n; i++) {
      const cmd = bytes.readUInt32LE(cursor), size = bytes.readUInt32LE(cursor + 4);
      if ([0xc, 0x80000018, 0x8000001f, 0x20, 0x80000023].includes(cmd)) {
        const name = cursor + bytes.readUInt32LE(cursor + 8);
        names.add(bytes.toString('utf8', name, bytes.indexOf(0, name)));
      }
      cursor += size;
    }
  };
  if (bytes.readUInt32BE(0) === 0xcafebabe) {
    for (let i = 0, n = bytes.readUInt32BE(4); i < n; i++) thin(bytes.readUInt32BE(8 + i * 20 + 8));
  } else thin(0);
  return [...names];
}

/** An executable links no framework that only an unlinked capability
 * brings: the next import of one into the core fails the build by name
 * instead of costing every launch silently. */
export function assertCapabilityFrameworks(executable, products, allowedFrameworks = []) {
  const loaded = loadedDylibs(executable);
  const stray = CAPABILITY_FRAMEWORKS.filter(([product, framework]) => !products.includes(product) && !allowedFrameworks.includes(framework) && loaded.some((path) => path.endsWith(`/${framework}.framework/${framework}`)));
  if (stray.length) throw new Error(`host/apple: ${basename(executable)} links ${stray.map(([product, framework]) => `${framework} (only ${product} may)`).join(', ')}, which its composition does not link: an import outside the capability's module brings it to every launch`);
}

/** Whether a build links by its plan (LLP 1047.001 D6): production always;
 * a development build with `--link plan`, so a smoke runs what ships
 * (development otherwise links everything, for the agent's whole vocabulary
 * and plan restarts). */
export function linksByPlan(production, args) {
  return production || (args.includes('--link') && args[args.indexOf('--link') + 1] === 'plan');
}

/** The SDK the linker recorded in an executable (`LC_BUILD_VERSION`) is the
 * one asked for: AppKit draws its design by that number, so a link that
 * records another (as SwiftPM's did, LLP 1069.011 §7) changes every app's
 * look without a word. Compared to major.minor. Read by `vtool`, which
 * takes any path as a file: the executable is named for its app, and
 * `otool` takes a path ending in `name(member)`, such as "T3 Code (Exact)",
 * for an archive's member (#234). */
export function assertLinkedSdk(executable, expected) {
  const shown = spawnSync('xcrun', ['vtool', '-show-build', executable], { encoding: 'utf8' });
  if (shown.status !== 0) throw new Error(`host/apple: xcrun vtool -show-build failed (${shown.status ?? shown.signal ?? shown.error?.message}) on ${executable}${shown.stderr?.trim() ? ': ' + shown.stderr.trim() : ''}`);
  const recorded = /cmd LC_BUILD_VERSION[\s\S]*?\n\s*sdk (\S+)/.exec(shown.stdout)?.[1];
  const majorMinor = (v) => String(v).split('.').slice(0, 2).map(Number).join('.');
  if (!recorded || majorMinor(recorded) !== majorMinor(expected)) {
    throw new Error(`host/apple: ${basename(executable)} records SDK ${recorded ?? '(none)'}, not ${expected}: AppKit would draw it in another design`);
  }
}
