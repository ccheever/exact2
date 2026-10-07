// What an Apple app links: the capabilities its composition names, and the
// SDK its executable records.
import { spawnSync } from 'node:child_process';
import { basename } from 'node:path';

/** The capabilities with a Swift half, by name, and the product of
 * `Package.swift` (its `capabilities` table) each is. */
const SWIFT_CAPABILITIES = [['grouped_lists', 'ExactGroupedLists']];

/** What the archive links (LLP 1047.001 D2, D4, D6): a fixed plan's use-set,
 * as the bake's graph names it; every capability for every other build,
 * development's included. `link` is `EXACT_APPLE_LINK` for SwiftPM (the
 * names, `none`, or `all`); `products` the Swift capability products the
 * composition links and installs, and an embedder links beside ExactKit.
 * Cargo is told `plan` or `all` before the bake, and the entry derives the
 * same set from the same plan (`contract::apple_linked`). */
export function appleComposition(graph, fixedPlan) {
  const uses = graph?.uses;
  const link = fixedPlan && Array.isArray(uses) ? uses.join(',') || 'none' : 'all';
  const products = SWIFT_CAPABILITIES.filter(([name]) => link === 'all' || link.split(',').includes(name)).map(([, product]) => product);
  return { link, products };
}

/** The SDK the linker recorded in an executable (`LC_BUILD_VERSION`) is the
 * one asked for: AppKit draws its design by that number, so a link that
 * records another (as SwiftPM's did, LLP 1069.011 §7) changes every app's
 * look without a word. Compared to major.minor. */
export function assertLinkedSdk(executable, expected) {
  const loads = spawnSync('otool', ['-l', executable], { encoding: 'utf8' }).stdout ?? '';
  const recorded = /cmd LC_BUILD_VERSION[\s\S]*?\n\s*sdk (\S+)/.exec(loads)?.[1];
  const majorMinor = (v) => String(v).split('.').slice(0, 2).map(Number).join('.');
  if (!recorded || majorMinor(recorded) !== majorMinor(expected)) {
    throw new Error(`host/apple: ${basename(executable)} records SDK ${recorded ?? '(none)'}, not ${expected}: AppKit would draw it in another design`);
  }
}
