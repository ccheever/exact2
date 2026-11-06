// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/terminal-host/src/shims/utils.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// The one function the terminal needs from T3 Code's lib/utils.ts (MIT reference, see
// LICENSE-T3: apps/web/src/lib/utils.ts:15-17). The reference module also imports contracts,
// effect, tailwind-merge and the composer draft store; the page bundle must not.
export function isMacPlatform(platform: string): boolean {
  return /mac|iphone|ipad|ipod/i.test(platform);
}
