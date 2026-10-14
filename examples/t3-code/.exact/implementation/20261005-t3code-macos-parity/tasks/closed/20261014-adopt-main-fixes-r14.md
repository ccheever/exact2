---
name: 20261014-adopt-main-fixes-r14
plan: 20261005-t3code-macos-parity
implementation: verified
verification: passed
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-adopt-main-fixes-r14
pr_url: null
verified_commit: b40d3c08dd748bf05ef44a5c124b5232cb45f1ab
---

# Main adoption, round 14

## Outcome

Bring main `178ae7237edaf7b3b07aa419f0d32b98b7e5ee80` into the T3 feature branch while
preserving main's ancestry. Adapt the example to Contract's new view keywords, receiver
methods and array types. Keep the framework identical to main and verify the combined
first-wave repairs on that framework.

## Scope and exclusions

The pin contains 144 commits after round 13's `958523176`. This is an application
adoption; it changes no framework implementation. The app's `Cargo.toml` member and
`Cargo.lock` package remain the only differences outside `examples/t3-code`.

No existing workaround is removed merely because a related capability landed. The
composer's native atomic chips, shell menu adapter, timeline registry, window geometry
owner and transport remain separate consumers. The eight follow-up application tasks
start from the feature branch after this adoption merges. PR #498 and the deferred
framework decisions are outside this task.

## Context and guidance

- Parent: [plan](../../plan.md), [specification](../../spec.md).
- First wave: [continuation review](../../reviews/20261014-first-wave-resume.md).
- Previous adoption: [round 13](../20261014-adopt-main-fixes-r13.md).
- Kept workarounds: [round 12 inventory](../20261013-r12-kept-workarounds.md).
- Guidance: the parent's `20261005-platforms-v3` snapshot, with foundations,
  application state, accessibility, macOS, and testing guidance. Current public syntax
  and API behavior were checked against the pinned main source and `docs/testing.md`.

## Dependencies

| Dependency | Kind | Resolution |
| --- | --- | --- |
| First-wave application repairs | Merged task PRs | All eleven PRs merged remotely at 03:04 UTC; feature checkpoint `1e31522094c2587ce6e7a30bdd8ca0e87bc8310a` |
| Main fixes | Merged framework work | Present in pinned main `178ae7237`; local ancestry-preserving merge `1d8c7d5f1` |
| Publication and bug-fix merges | Recorded user decision | Existing continuation authorization; no new scope decision |

## Implementation notes

`2fe43b6bd` ran main's `scripts/migrate-1116-015.mjs` against the application before
merging main. View `when`/`each` become `if`/`for`; subject-first standard calls become
receiver methods; `length(value)` becomes `value.length`; `list<T>` becomes `T[]`.
Task and keybinding `when` remain unchanged. Existing test literals, fixture parsers,
conditional/repeated-view walkers and the depth estimator follow the same syntax.
Obsolete method shims were deleted. The depth budget is unchanged.

The first full migrated Bun run exposed three old-spelling assertions in two files.
`92f970fad` updates those assertions without changing behavior or expected results.

The root source sweep then found main's new refusal of `transform` on non-SVG elements
(`099941dce`). Four disclosure chevrons now use the supported `rotate=90/0` property with
their original conditions. The monogram drops an anisotropic transform that Apple had
ignored, preserving its previous Mac glyphs. Its reference's SVG `textLength` adjustment
remains unimplemented; a conversion to SVG is a separate visual change. SVG transforms
elsewhere remain unchanged. Independent review checked these three changed files.

`1d8c7d5f1350d8a55ebd1fad42a5ce435cd1ffc5` merges main with no conflict. Independent
review by `repair_markdown` compared all 187 application Contract files with a fresh
official migration of the pre-adoption source: 162 changed, zero mismatches. It also
verified the framework tree and both Cargo exceptions byte-for-byte. Relevant field,
selection, scroll, font-size and accessibility interfaces remain source-compatible.

| Main change | Adoption consequence |
| --- | --- |
| D1/D2 syntax, `366b9f0eb` and `031dfa3ec` | Migrate authored views, methods, types and their executable test helpers |
| Refused box transforms, `099941dce` | Use individual rotation for four chevrons; remove the monogram's previously ignored transform |
| Hidden/inert selection, #525 `63ceeeff8` | Shared host fix; no clone workaround to remove |
| Ancestor context menus, #526 `72598699b` | Shared host fix; real VoiceOver qualification remains separate |
| macOS resize cursors, #527 `c46ed7431` | Shared host mapping; overflowing hit regions still require real-pointer proof |
| Minimized visibility, #528 `ad634ebf9` | Existing `exactPage()` consumer receives the corrected host state |
| Inline AXPress, #529 `fc0195641` | Fix applies to inline runs; the ordinary-button issue is not closed by it |
| Semantics, #530 `c8d809085` | Run the quick differential check on the combined tree |
| Linux cross C++, #532 `fbf1cb78e` | Adopt tooling; no native Linux behavior claim from a Mac build |
| Captured bake receipts, #538 `d13b0c6b4` | Adopt the capture fix; no new metrics claim from this app smoke |
| Field selection/beforeinput, `0a36b5c40` | Public plain-field capability; native chip editor remains required |
| AppKit scroll padding/margin and smooth scrolling, `c5d3b3f8e` | Available for the later timeline task; agent mode deliberately freezes smooth motion |
| Percentage root font size, `b00aad973` | Existing numeric pixel calls keep their meaning; no app change |
| Test reliability, #543 `1d9df1220` | Use current build/test entry points and compiled fixtures |

## Acceptance and reproduction

Run from the checkout with Bun 1.4.2 first on `PATH` and Lean available through
`~/.elan/bin`. Native verification uses an isolated local fixture, never the user's
normal T3 home or a provider message.

```sh
bun install --frozen-lockfile
bun test examples/t3-code --timeout 60000
bun node_modules/typescript/bin/tsc --noEmit --strict --target ES2023 --module ESNext --moduleResolution bundler --skipLibCheck --lib ES2023,DOM examples/t3-code/app.ts
cargo build --all-targets --keep-going
cargo test --lib --bins --tests --no-fail-fast
cargo clippy --all-targets --keep-going -- -D warnings
cargo fmt --all -- --check
bun scripts/caps.mjs
bun scripts/boot.mjs
cargo check --workspace --all-targets --keep-going
cargo run -p contract-difftest -- quick
cargo test -p t3-code-macos --lib
cargo run -q -p contract -- build examples/t3-code/app.contract -o target/t3-adoption.plan
EXACT_APP_DIR="$PWD/examples/t3-code" bun host/apple/build.mjs t3-code-macos --bundle
```

The native smoke opens a seeded thread, collapses and restores the sidebar, visits the
PR overview and Settings General/Appearance, types an unsent draft, opens a local browser
preview, and cancels a dirty branch-restore confirmation. Read back the draft, navigation,
process state and unchanged local Git branch/file. Inspect screenshots before calling
them visual evidence. This smoke does not prove authenticated GitHub content, smooth
scrolling, real IME, pointer cursors or VoiceOver speech.

## Progress

| Check | Result |
| --- | --- |
| Ancestry and framework identity | Pass at `1d8c7d5f1`; independent source review agrees |
| Existing migration/helper tests | 311 pass, 1,744 assertions |
| Full app Bun suite | 4,978 pass, one existing skip, zero failures; 118,555 assertions across 335 files |
| Strict application TypeScript | Pass |
| Root all-target build | Pass, 54.06 seconds |
| Clippy, formatting, caps and boot | Pass |
| Quick semantics differential check | Pass |
| Root tests | 4,408 passed, 47 ignored; the sole failed source sweep passed after repair (94 roots, 195 imports, zero failures) |
| Workspace | Fresh `--workspace --all-targets --keep-going` check passed in 198.67 seconds |
| App Rust | 27 passed |
| Final Contract plan | Pass: 28,103,415 bytes, 15.6% below the original feature baseline |
| Native bundle/smoke | Build passed in 309.85 seconds; all ten connected smoke captures completed, with inspected screenshots and unchanged dirty Git state |
| Remote first-wave merge | All eleven PRs confirmed merged by GitHub; feature checkpoint `1e3152209` |
| Remote adoption merge | Pending publication |

## Attempts and evidence

The full app suite before adoption passed the same 4,978 tests at `128463953`.
First migrated run: 4,976 pass, one skip, two failed cases; both were stale Contract
spelling assertions. The three assertion edits preserve the original expectations;
the rerun passes. No application behavior repair was needed for migration.

The root test run's one failure was the newly refused box transform. The concurrent
workspace check correctly rejected its capture when those app sources changed. Both
results are retained. The repaired source passed the affected source sweep, a fresh
Contract compile, all 27 app Rust tests, the full Bun suite and a fresh workspace check.

Local development output is under `target/fix-wave`: `app-tests-adoption-red.log`,
`app-tests-adoption.log`, `types-adoption.log`, and `<check>-adoption.log` with
`checks-adoption.json`. Raw verification logs are not committed to the application
branch. Durable proof is published in the [combined packet](https://github.com/ccheever/exact2/blob/144fe75ff80651336d1a7511f71dbb54923345d7/adopt-main-fixes-r14/20261014-integration/README.md), with exact source/binary identities, checks, earlier failures, reproduction and inspected screenshots.

## Next action

Publish this verified adoption as a PR against the first-wave feature checkpoint.
Merge by fast-forward or a real merge so main remains an ancestor. Keep the separate
real-input parity rows open while the desktop is locked.

## Final verification and independent review

The native smoke ran at `b40d3c08dd748bf05ef44a5c124b5232cb45f1ab`. It remained connected
through the seeded thread, sidebar collapse/return, PR empty state, General, Appearance,
unsent draft, local browser preview and dirty branch confirmation. Cancel preserved
`## main` and `?? dirty.txt`; the draft survived. The browser panel closed to its mini
preview. The unauthenticated provider warning and update toast are expected fixture
output. No sign-in, update, provider send or remote Git mutation was performed.

Independent reviewer `repair_markdown` approved the exact commit after reviewing all
54 changed TypeScript files, all 187 Contract files and the three transform exceptions.
The framework tree and both Cargo exceptions match the intended main pin. The reviewer
independently reproduced the full 1,241-file source fingerprint, including runtime PNG
assets: `b630e3a916d82fdc54d04e223ff6d5aa430b2ba1e4765eb6fb026e9165ce73f6`. The earlier
evidence fingerprint omitted two unchanged runtime PNGs; the published packet corrects
that provenance without changing application source or requiring a rebuild.

This closes the bounded adoption task. It does not close the first-wave task matrices or
claim real IME, VoiceOver, pointer, smooth-motion or authenticated-service acceptance.
