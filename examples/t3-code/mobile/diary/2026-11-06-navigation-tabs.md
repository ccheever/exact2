# 2026-11-06 — persistent tab navigation

surfaces: ios
agent: Codex
task: keep tab roots mounted across Home, PRs, Usage and Settings

## Rough

The root declared four tab routes but rendered only the selected `stack(nav)`. Selecting PRs or Settings therefore ended Home and its manually attached tab bar; Settings close also called `back` at a root tab, which could not pop. The native chrome had placeholder tab controllers that did not own the route stacks.

## Lean in

The compact root now renders every `nav.tabs` stack as a direct root tab panel, with a stable app tablist below the route viewport. The bar remains on each index screen, including Settings, and is hidden with `display="none"` on pushed child routes so the stack and scroll state stay mounted. Settings is a normal compact tab root rather than a form sheet: its native header owns Close on the root and Back on child routes, while the Contract closes the root to Chats and pops children.

The Home chrome no longer creates a competing placeholder tab controller. On split layouts the selected app panel reuses the `workspace-panel` marker id, so the existing three-pane native container still receives direct route children and remains the first native tab owner; compact layouts expose the app tablist as the root owner.

## Needed

The first device-agent attempt returned `Agent tools did not start within 30000 ms`, but a later native run completed on the booted iPhone and Duo simulators. The device drive used the current signed build and `--chrome platform`; it verified the horizontal iPhone tab bar, the Duo vertical rail, and Settings selection. The raw captures are `/tmp/t3-nav-phone.png`, `/tmp/t3-nav-platform.png`, and `/tmp/t3-settings-root-phone.png`.

## Checkpoints

- Contract build: passed.
- Focused app tests: 31 passed; mobile workspace structure 21 passed; New Task presentation 97 passed.
- Earlier web drive: Settings tab stayed visible and selected; Settings close returned to Chats; Settings → Appearance → Back returned to the Settings index; all four tab stacks remained present.
- iOS no-install build: passed; signed simulator app produced at `examples/t3-code/target/clients/82a2c967afe4c72ea79cd67c/com.exact.t3code.ios/ios-simulator/aarch64-apple-ios-sim/embedded/development/standalone/T3 Code.app`.
- Native platform screenshots: iPhone shows Chats, PRs, Usage and Settings in one horizontal UITabBarController; Settings selection shows its native X and scope controls with the tab bar retained. Duo shows the same four destinations in the native vertical rail.
