surfaces: ios
agent: Codex gpt-6-astra
task: keep Home scroll ownership aligned with the reference empty states

### Rough

Home used `home-list` as the route scroll owner for every state. The filtered-empty card belongs to that list, but the true unfiltered empty surface spans the route and stays stationary under the native header.

### Lean in

The route now selects `home-list` only when `home.fullPageEmpty` is false. The Home contract already distinguishes filtered empty content from the full-page empty surface, so the route and its native scroll owner now make the same choice.

### Needed

- native header and list coupling: the existing named `home-list` scroll surface.
- native API additions: none.
- device acceptance: compare a populated Home, a filtered-empty result, and an unfiltered empty environment after launch.

### Checkpoints

Focused scroll-contract tests pass (5 tests, 71 assertions). The full mobile suite passes 4,316 tests with 2 intentional skips and 38,050 assertions. Contract build, `git diff --check`, caps, boot, and an iOS simulator build/launch pass; caps reports the existing repository violations in `host/web/navigation.js`, `docs/recipes/foldables.md`, and the unavailable Cargo executable.
