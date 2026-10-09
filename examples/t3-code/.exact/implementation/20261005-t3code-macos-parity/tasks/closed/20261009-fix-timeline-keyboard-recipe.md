---
name: 20261009-fix-timeline-keyboard-recipe
plan: 20261005-t3code-macos-parity
implementation: done
verification: passed
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-fix-timeline-keyboard-recipe
pr_url: https://github.com/ccheever/exact2/pull/333
verified_commit: 2526ef4e2
---

# The `timeline-keyboard` AppKit regression builds and passes again

## Outcome

`macos/tests/timeline-keyboard/main.swift` builds with the README recipe and passes on the current base. It is the
regression test of the timeline's keyboard handling through ExactKit's real key loop and scroll views
(timeline-work.contract's ToolOutput and the mounted WorkRow): Tab and Shift-Tab between the disclosure, the output and
the next control, the output's `focus`, `blur` and scroll keys reaching the Contract action, and the row's hatches and
timestamp staying out of the Tab order.

Found by [app-contract-room](20261008-app-contract-room.md) and
[app-contract-root-rewrite](20261008-app-contract-root-rewrite.md) ("Not done / not verified"): the recipe stopped
compiling on the base. The coordinator made this follow-up task (2026-10-09).

## Scope and exclusions

Included:
1. The README recipe's `swiftc` flags.
2. `main.swift` on the current ExactKit key API, with the test's intent and every assertion kept.

Excluded: anything under `host/` (framework code is main-side work); the other AppKit test directories (their own
recipe, which passes).

## Context and guidance

- Parent: [spec](../../spec.md), [plan](../../plan.md).
- The test came from `5a355f704` (fix(t3-code): keep hidden timeline hooks out of the Tab order).

## Dependencies

| Kind | Item | State | Effect |
| --- | --- | --- | --- |
| framework issue | none | — | no framework wait; nothing under `host/` changes |

## Acceptance and reproduction

| Criterion | Check |
| --- | --- |
| Fails before | the README recipe, run from the repository root on the base after building the macOS app, does not compile |
| Passes after | the same recipe block, taken from the README verbatim, compiles and the binary exits 0 with its PASS lines |
| Intent kept | every `precondition` of the base test is still there; none is loosened |
| Clone checks | `bun test examples/t3-code --timeout 60000`, `cargo test -p t3-code-macos --lib` |
| Repository | `git add -A && bun scripts/caps.mjs` |

## Cause and fix

Two changes from main, both adopted in round 5 (#236), broke the recipe; the test was not run by any task since then
(each recorded it "not run").
1. ExactKit's declarations became `package` (main `c043bc7f4`). SwiftPM compiles them with a package name; a bare
   `swiftc` has none, so `Session.swift` fails: "'Runtime' is inaccessible due to 'package' protection level" and the
   same for `Presenter`. Fix: the recipe passes `-package-name apple`, the name SwiftPM itself passes for
   `host/apple/Package.swift` (read from the macOS build's own Swift build data: every ExactKit compile there has
   `-package-name apple`). The test and ExactKit are one module, so the name only has to be present; using SwiftPM's
   keeps the two compiles alike.
2. `Presenter.onKey` takes a `KeyPress` (chord, `code`, `repeat`, `up`; main `5a20af1cf`, #220) instead of a `String`:
   "cannot convert value of type 'KeyPress' to expected argument type 'String'" at `main.swift:31`. Fix: the hook records
   `press.chord` (the string the test compared before) and refuses a keyup (`press.up`), so a keydown that also reached
   the `keyup` handlers would fail the test instead of being counted.

The README also names the build that writes the archive the recipe links (`bun host/apple/build.mjs t3-code-macos`,
`target/aarch64-apple-darwin/host-dev/libt3_code_macos.a`).

## Acceptance results

| Criterion | Result | Proof |
| --- | --- | --- |
| Fails before | pass (fails on the base) | recipe as written: `Session.swift:359:26` and `:367:28`, "'Runtime' / 'Presenter' is inaccessible due to 'package' protection level", exit 1; with only `-package-name apple` added: `main.swift:31:17` "cannot convert value of type 'KeyPress' to expected argument type 'String'", exit 1 |
| Passes after | pass | the README block run verbatim (`zsh -e`), exit 0, on `5b6df4841` and again on the merged tree with #332: "PASS: Tab and Shift-Tab traverse disclosure/output/next control; focus, blur and scroll keys delivered", "Base row: 3 invisible or non-reference Tab stop(s) before the output", "PASS: mounted row: Tab goes disclosure -> output -> next control; hooks and timestamp are not Tab stops" |
| Intent kept | pass | the diff keeps every assertion and adds one (`!press.up`); the keys are still `["ArrowDown", "ArrowUp", " "]` through `Presenter.routeKey`, the session's monitor path |
| Clone checks | pass | on the merged tree `2526ef4e2`: `bun test examples/t3-code --timeout 60000` 3,468 pass / 1 skip / 0 fail (3,469 tests, 254 files); `cargo test -p t3-code-macos --lib` 13 pass; the same on `5b6df4841` before the merge |
| Repository | pass | `git add -A && bun scripts/caps.mjs`: all budgets within cap. The diff changes no Rust, Contract or `host/` file (a Swift test the README compiles, the README and records), so the five Cargo checks are not re-run here |

No test case failed for a behaviour reason: once it compiled, every assertion held.

## Real-input batch steps

None: the test drives AppKit's key-view loop and `NSEvent`s through the real `Presenter` in-process; no real input.

## Progress

2026-10-09: built the macOS app (`bun host/apple/build.mjs t3-code-macos`), reproduced both base failures, fixed the
recipe and the test, and ran the README block verbatim (pass). Merged the feature branch at `f633671b7` (#332, the root
rewrite; no conflict), rebuilt and re-ran everything. Delivered as draft [PR #333](https://github.com/ccheever/exact2/pull/333).

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| base recipe | `b53cd7da7` | compile fails: `package` access (2 errors); with `-package-name apple`: `KeyPress` vs `String` (1 error) | PR body; local `target/fix-timeline-keyboard-recipe/run-base.txt` | none |
| branch recipe | `5b6df4841` | README block verbatim: compiles (64 s), 2 PASS lines, exit 0; `bun test` 3,468 / 1 / 0; `cargo test -p t3-code-macos --lib` 13 pass | PR body; local `run-recipe.txt` | none |
| merged tree | `2526ef4e2` (`f633671b7` merged) | macOS app rebuilt; README block verbatim exit 0 with the same PASS lines; `bun test` 3,468 / 1 / 0; `cargo test -p t3-code-macos --lib` 13 pass; caps within | PR body; local `run-recipe-merged.txt` | none |

## Next action

2026-10-09 (records sync, `t3-code-records-337`): merged into `feat(example)/t3-code` as #333 (`3c7b35984`); the record moved to `tasks/closed/`.

Coordinator: review and merge draft PR #333.
