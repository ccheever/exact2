// What an Apple app links: the capabilities its composition names, and the
// SDK its executable records.
import { spawnSync } from 'node:child_process';
import { basename } from 'node:path';

/** The capabilities with a Swift half, by name, and the product of
 * `Package.swift` (its `capabilities` table) each is. */
const SWIFT_CAPABILITIES = [['grouped_lists', 'ExactGroupedLists'], ['markdown', 'ExactMarkdown'], ['surfaces', 'ExactSurfaces'], ['drag', 'ExactDrag']];

/** What the archive links (LLP 1047.001 D2, D4, D6; LLP 1047 D8): for a
 * production build, the plan's use-set, as the bake's graph names it, and the
 * manifest's `link` (capabilities linked ahead of use); every capability for
 * every other build. `link` is `EXACT_APPLE_LINK` for SwiftPM (the names,
 * `none`, or `all`); `products` the Swift capability products the composition
 * links and installs, and an embedder links beside ExactKit. Cargo is told
 * `plan` or `all` before the bake, and the entry derives the same set from the
 * same plan and manifest (`exact_bake::apple_link`). */
export function appleComposition(graph, production, ahead = []) {
  const uses = graph?.uses;
  const link = production && Array.isArray(uses) ? [...new Set([...uses, ...ahead])].join(',') || 'none' : 'all';
  const products = SWIFT_CAPABILITIES.filter(([name]) => link === 'all' || link.split(',').includes(name)).map(([, product]) => product);
  return { link, products };
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
