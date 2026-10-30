# Code review: Harmony production archive (development-only grant lines), round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `f2696d6d5b`.
- **Method:** one brief (sha256 `e870094c2e89a00bf9d89c52f895340adb0ee1f4ca38b5f247b3cb7c99150f62`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. **Backend rewriting remains incomplete.** [js/bake/src/typescript.mjs:241](js/bake/src/typescript.mjs:241) evaluates the entire grants expression before backend selection whenever any line is marked. For example, an unmarked `net.fetch ${origin}/api` beside `development device.hid` becomes a literal containing the old origin—even in production. The subsequent rewrite only matches exact-origin lines. Aliased exports also remain stuck because injection searches for a variable named `grants`. Requests to the selected backend then lack permission. Preserve origin references until backend selection and use the exported binding consistently in both transformations.

2. **The keyed rewrite bypasses validation.** [js/bake/src/typescript.mjs:236](js/bake/src/typescript.mjs:236) accepts computed literal keys, and `Object.fromEntries` silently collapses duplicate keys. Whenever an entry contains a marker, these invalid authored forms become ordinary, unique properties before `keyedGrants` checks them. A duplicate permission entry therefore silently wins instead of failing the build. Apply the original structural and duplicate-key checks before rewriting entries.

**Non-blocking**

1. **“No CallKit” overstates the change.** [scripts/harmony-production.test.mjs:3](scripts/harmony-production.test.mjs:3) claims binary exclusion, but Harmony still imports `@exact/calls`, and [host/apple/build.mjs:1098](host/apple/build.mjs:1098) includes its implementation independently of grants. Reword this as removing the call grant and its derived background modes.

The other round-one fixes look resolved by inspection: the [leading marker](js/bake/src/typescript.mjs:221), [Rust diagnostic order](bake/src/reach.rs:414), [production web trust](host/web-js/build.mjs:68), [complete test-result assertions](scripts/harmony-production.test.mjs:62), and [documentation](docs/contract-for-agents.md:1192). I found no additional problem with whole-initializer wrapper replacement, authored `staticFacts`, the [lint’s whitespace-aware scan](contract/cli/src/grants/literals.rs:345), incremental configure, or Android/Linux’s shared bake path.

Harmony’s intended production behavior looks sound: [its source reads the filtered grants](apps/harmony/app/app.ts:219), [the start action is guarded](apps/harmony/app/app.contract:309), and [the row is conditional](apps/harmony/app/app.contract:761).

Static review only; no files edited, network calls, tests, or builds.
