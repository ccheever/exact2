surfaces: ios
agent: Codex gpt-6-astra
task: keep the Home header's native scroll owner during filtering

### Rough

The Expo Home screen keeps its native list mounted when a search or environment
filter has no matching threads. The Exact Home contract did the same, but the
route column cleared `navigationScroll` whenever the item projection was empty.
That left the header and the visible empty list with different scroll ownership.

### Lean in

The route now uses `home.fullPageEmpty` to choose its owner: a filtered empty
Home names `home-list`, while the true unfiltered empty state names no scroll
surface. The existing Home branches remain unchanged, so the filtered empty
card still lives inside its list and the full-page state remains non-scrollable.

### Needed

- native header and scroll coupling: provided by the existing `navigationScroll`
  route property and the named Home list.
- native API additions: none; this change uses no unsupported UIKit or Expo API.
- device acceptance: a populated and filtered Duo drive remains useful for
  checking the native header material after a live filter transition.

### Checkpoints

The focused scroll-contract suite passes 3 tests with 41 assertions. The full
mobile suite passes 4,243 tests with 2 intentional skips and 37,716 assertions.
Contract compilation succeeds with the app's existing warnings, generated
inputs match all 527 sources, and strict TypeScript passes with the ES2024
library required by the app's existing Promise API. Caps and boot pass. A
no-install iOS simulator build completes and signs the app; the existing Swift
deprecation and concurrency warnings are outside this unit. A live filtered
Home drive remains useful for native header material, but this source change
introduces no native API risk.
