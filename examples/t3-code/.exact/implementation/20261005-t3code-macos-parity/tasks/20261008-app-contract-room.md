---
name: 20261008-app-contract-room
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: 'feat(example)/t3-code-app-contract-room'
pr_url: https://github.com/ccheever/exact2/pull/264
verified_commit: null
---

# Room in app.contract, the composer AppKit test recipe and a flaky local-backend test

## Outcome

Three problems the coordinator found on the base (`e91fcfc65`; merged forward to `b7761f556` (#262) and `e784c8fb1` (#261)):
`app.contract` at 1,499 of the 1,500 lines `bun scripts/caps.mjs` allows, with no same-file `use`
lines left to merge (`client.test.ts` at 1,486); the `composer` AppKit test binary failing to
compile; and local-backend's `testTurningOffStopsAServerThatIgnoresSIGTERMWithinTheBound` failing
about one run in five. No behavior changes.

## Scope and exclusions

Included: moving cohesive parts of `app.contract` to area files with the compiled plan unchanged
(or changed only in an unavoidable, equivalent and explained way), splitting `client.test.ts` by
area with its test names, the narrowest fix that makes the composer recipe compile, and a
deterministic SIGTERM test that proves what it proved. Excluded: framework edits, X9, live UI
sessions (no behavior change, coordinator).

## What was built

1. **The root's view moves to `T3Window` (`app-window.contract`).** `app.contract`'s `provide` and
   `view` move unchanged into `component T3Window`, which takes the 313 root names the view reads
   as props of the same names; `pending(providerAuthChanged)` becomes the bool prop
   `providerAuthPending`. The root's view is one `T3Window(...)` use, and the eight `use` lines only
   the view needed move with it. `app-window.contract`'s use lines keep the root's order, so every
   file loads where it did (the loader is depth first, `contract/cli/src/sources.rs`).
   `app.contract` 1,499 → 1,459.
2. **`client.test.ts` split by area.** The `Backend` double and fixtures move unchanged to
   `client-fixture.ts`; the project management, scoped defaults and settings-page tests move
   unchanged to `client-settings.test.ts`. The settings tests used to start after tests that had
   already made the sidebar's once-per-process favicon request (`r3-sidebar-glyph.ts`
   `syncFavicons`), which the double records in `committed`; alone, "rename validates title…" saw
   that request. `client-settings.test.ts` connects once in `beforeAll`, so they start as before.
3. **Composer recipe.** #256 put `extension T3Module { codexAuthOps }` at the end of
   `T3CodexAuth.swift`; the recipe leaves `T3Module*.swift` out of the composer, menus and r5-panels
   binaries, so all three failed. The extension moves unchanged to `T3Module+CodexAuth.swift`, the
   op file `AGENT-HANDOFF.md` names for module ops; the README says an `extension T3Module` belongs
   in such a file. The recipe itself is unchanged.
4. **SIGTERM test.** Readiness answers at once (`FakeProber`), so the switch-off could send SIGTERM
   before the fake `t3` shell had run `trap '' TERM`, and the shell died on it (`lastExit`
   `signal=15`, the stop answered in under 2 ms). The double writes a marker after its trap and the
   test waits for the marker before switching off; it also checks that SIGKILL ended the server
   (`lastExit` `signal=9`). The code under test is right and unchanged: a server that dies on
   SIGTERM is stopped at once.
5. Records: `AGENT-HANDOFF.md` ("Where a feature adds its code": the window prop row, the client
   test files, the 40-line budget) and the README's source paragraph.

## Why app.contract stops at 1,459, not about 1,200

Everything left in `app.contract` belongs to the root component, and the compiler keeps it there:

- `resource`, `mutation` and `task` are root-only (`type-child-resource`, LLP 1091 "resources/
  mutations/tasks stay root-only"; X9). There are 46 resources, 15 mutations and 16 tasks.
- A child cannot assign root state. Of the 166 root states, 92 are read by a resource, task or
  mutation (directly or through a derive), so they stay. The other 74 are written by actions that
  also write such state or send a root mutation (`changed`, `localChanged`, …) — every one of them
  but two (`welcomeHelpOpen`, `dismissedProviderBanner`: 6 lines with their toggles). Moving the rest
  means splitting about 37 root actions into a child half and a root half, which changes the plan
  structurally and the agent-visible state names, and needs a live drive to verify: not a move.
- A root `derive` moved into a child is inlined at each read (no derive slot), and a repeated
  statement block factored into a called action compiles differently from the inline block
  (checked on scratch apps): neither keeps the plan.

Paths to more room: X9's option A1 (child resources, built on `daehyeon/fw-x9-child-resources`,
awaiting Charlie's ruling), or the action split above as its own task with a live drive.

## Plan identity

The plan cannot stay byte-identical with any wrapper component: the inliner numbers every component
use in preorder (`contract/syntax/src/inline.rs`, `lifted(name, n)` and `@capture:{n}:…`), and
`T3Window` is use 1, so every other instance's number moves up by one. Proof that this is the only
difference: a scratch decoder (`exact_plan::Plan::decode`, not committed) renumbers the branch's
lifted names back (`name#N` → `name#(N-1)`, `@capture:N:…` → `@capture:(N-1):…`) and re-encodes it: the
bytes equal the base's.

| Plan | SHA-256 |
| --- | --- |
| base `e784c8fb1` (`git archive`, no moves) | `6e5739b447cf3884a97058867b5b4a7b339f577105798c8712616e0119ad67ef` |
| this branch (merged with `e784c8fb1`) | `5ef66f29f31f39628e20367c7e8b7c6a0be9505ecc6cb4179bb5381440066b34` |
| this branch, instance numbers renumbered back (21,105 of 32,528 strings) | `6e5739b447cf3884a97058867b5b4a7b339f577105798c8712616e0119ad67ef` (equal to the base) |
| earlier base `b7761f556` / branch on it / renumbered | `714abbdc…aff81` / `0145ad3d…12f6` / `714abbdc…aff81` (equal) |

Controls: without renumbering, every renumbered string differs; the base against itself is equal. Both plans
on `e784c8fb1`: 3807 slots, 20 derives, 46 resources, 4100 actions, 74209 nodes, 29714 regions, 14,964,344 bytes.
The moved view is the base's text with `pending(providerAuthChanged)` → `providerAuthPending` at its
three uses (`diff`). No script, test or record in the clone names a lifted instance name (searched
for every lifted name in the base plan). `contract fmt --check` fails on `app.contract` at the base
(long view lines) and on both files here, for the same lines.

## Acceptance and reproduction

| Criterion | Result | Proof |
| --- | --- | --- |
| `app.contract` around 1,200 lines | **not met: 1,459** (40 lines of room); the rest needs X9 or an action rewrite (above) | `wc -l` |
| Plan identical | equal to the base after renumbering instance numbers (an unavoidable, equivalent change); raw hashes differ | table above |
| `client.test.ts` split, names kept | 1,486 → 865 + 477 (`client-settings.test.ts`) + 160 (`client-fixture.ts`); the 100 test names are the same (junit report, sorted, `diff` empty); 480 expect calls before and after | runner reports |
| Composer binary compiles and passes | before: `T3CodexAuth.swift:236:11: error: cannot find type 'T3Module' in scope` (menus and r5-panels the same); after: composer 51 tests, 0 failures; menus 45, 0; r5-panels 6, 0; codex-auth 6, 0 | build and run logs |
| SIGTERM test deterministic, 20 in a row | before: 5 of 30 full-binary runs failed (elapsed 0.0003–0.0016 s < 1.9), plus 1 of 6 earlier; a scratch copy with `sleep 0.3` before the trap failed 3 of 3 (`lastExit` `signal=15`). After: 30 of 30 full-binary runs pass (61 tests, 0 failures each); the `sleep 0.3` copy passes 3 of 3 (`signal=9`, 2.15–2.19 s) | loop logs |
| Clone checks | pass: `bun test examples/t3-code` 2984 pass, 1 skip, 0 fail (2985 tests; base `e784c8fb1` the same in 234 files, here 235; on `b7761f556` 2922/1/0 both); strict `tsc` on `app.ts` clean; `contract build` OK; `cargo test -p t3-code-macos --lib` 11 passed | logs |
| AppKit binaries (README recipe; every one compiles `T3Module+CodexAuth.swift` or leaves it out) | 32 of 33 pass: activity 9, app-control 25, attach 3, codex-auth 6, composer-files 4, composer 51, contextmenu 19, fleet 9, intent 4, local-backend 61, media-actions 7, menus 45, notifications 4, r10-connect 4, r10-device 4, r11-device 3, r11-upstream 3, r12-sidebar 3, r5-composer 3, r5-panels 6, r6-device 3, r6-media 5, r7-device 14, r8-keys 4, r8-pointer 2, r9-device 13, r9-input 13, sidebar 5, snapshot 12 checks, ssh 15 (1 skipped), terminal 38 (1 skipped; after `terminal-host/build.mjs`, which the recipe assumes), transport 57, all 0 failures. mermaid compiles; not run: needs a running T3 server (`T3_SERVER`) | logs |
| Repository checks | pass: `cargo build --all-targets --keep-going` 0; `cargo test --lib --bins --tests --no-fail-fast` 0 (3383 passed, 0 failed, 33 ignored); `cargo clippy --all-targets --keep-going -- -D warnings` 0; `cargo fmt --all -- --check` 0; `bun scripts/caps.mjs` 0 (after `git add -A`); `bun scripts/boot.mjs` 0 | logs |
| macOS app builds | pass: `bun host/apple/build.mjs t3-code-macos` (development, no launch; cargo 126.3 s) | build log |
| timeline-keyboard regression | not run: its README recipe no longer compiles against ExactKit (`Session.swift:373: 'Runtime' is inaccessible due to 'package' protection level`); ExactKit is the same on the base, so the base fails the same way. Not this task's change | build log |

Not done or not verified, each with its blocker:
- `app.contract` at about 1,200 lines: X9 ([#108](https://github.com/ccheever/exact2/issues/108); local fix A1 awaiting Charlie's ruling), or a separate action-split task verified with a live drive.
- mermaid binary: needs a running T3 server (`T3_SERVER`); it compiles with this change.
- timeline-keyboard: its recipe is broken against current ExactKit (above), independent of this change.

## Attempts and evidence

| Attempt | Revision | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (2026-10-08) | `e91fcfc65` | Base reproduced: composer, menus and r5-panels fail to compile (`T3CodexAuth.swift:236:11`); the SIGTERM test fails 5 of 30 full-binary runs, 0 of 20 alone; a scratch copy with the trap delayed 0.3 s fails 3 of 3 | build and loop logs (`target/room`, not committed) | — |
| 2 (2026-10-08) | `c2c68d287`, `8d9ef03a0`, `b827e78cc` | Composer recipe fixed (composer 51, menus 45, r5-panels 6, codex-auth 6); SIGTERM 30 of 30; `client.test.ts` split. The split's first run failed 1 test (the settings file started without the favicon request made): `beforeAll` connection added, then 100 of 100 | logs | — |
| 3 (2026-10-08) | merge of `b7761f556` (#262), then the window move | Plan comparisons: identical after renumbering instance numbers (the only way any wrapper can differ); every check row above passes | plan hashes, `plancmp` output | `app.contract` ≈ 1,200 needs X9 |
| 4 (2026-10-08) | merge of `e784c8fb1` (#261; no `app.contract`, Swift or Rust change) | Plan identical to `e784c8fb1`'s after renumbering; `bun test` 2984/1/0; strict `tsc` clean; caps 0. The Rust checks and AppKit binaries are unaffected by #261 and were not rerun | plan hashes | as above |

## Next action

Review the draft PR. Sibling PRs that edited the root's view lines re-apply those edits in
`app-window.contract` and add any new root name as a `T3Window` prop.
