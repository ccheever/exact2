# Code review: contract gallery, the variant gallery's G2, round 2 of 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree.
- **Method:** one brief (sha256 `362cf2d950ef1419e65e771cf0ff9da1a747571dee95db0ac877a7c1953670f3`), shared by both reviewers, each blind to the other's review; HEAD `2c681530f3`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** 2 BLOCKER, 6 SHOULD-FIX; round 1's partly closed.
- **Disposition:**
  All of Astra's taken in 12e5a2fcb7 (round 2's commit):
  - 1 (destination): under a node_modules or target anywhere in its path; neither app.json nor app.contract written over unless ours; never a link.
  - 2 (design assets): the gallery's `assets` links to the app's (tested with an icon SVG).
  - 3 (exported shape names): read from the file the specifier reaches (`use Info as PublicInfo`, tested).
  - 4 (app inside its package): the loader's recorded origin (tested).
  - 5 (aliases bypass the root check): every root the gallery reads must be one the app reads.
  - 6 (relative uses): refused in place.
  - 7 (lowercase exports): brought under a capital.
  - 8 (compiler records): placeholders from the type checker's own declarations (tested with TextDecorations).
  Grok: no review. Three runs each ended after the CLI's automatic context compaction, before any finding; the last run's narration is kept below. Round 3 (both reviewers) covers this round's change.

This round still has blockers. I cannot mark every round 1 finding closed.

Round 1 disposition, using your A/B numbering:

| Finding | Status at HEAD |
|---|---|
| A1: destination safety | **Partial.** Canonical directory checks exist at [gallery.rs:272](/tmp/rv-dt-g2r2/contract/cli/src/gallery.rs:272); finding 1 below remains. |
| A2 / B1–B2: declaration identity | **Reported cases closed.** Resolved imports select declarations at [gallery.rs:369](/tmp/rv-dt-g2r2/contract/cli/src/gallery.rs:369). Additional discovery cases appear below. |
| A3 / B5: library-span refusals | **Reported case closed.** Individual-cell compilation handles these at [gallery.rs:179](/tmp/rv-dt-g2r2/contract/cli/src/gallery.rs:179). |
| A4: recursive records | **Code fix present.** Refused cells no longer retain component imports at [gallery.rs:525](/tmp/rv-dt-g2r2/contract/cli/src/gallery.rs:525). The recursion test still exercises only `args`, not generation. |
| A5 / B4: shape names | **Partial.** Internal renaming is handled, but public export aliases remain broken; finding 3. |
| A6 / B3: generated-name collisions | **Closed.** Reserved names and actual uniqueness checks at [gallery.rs:100](/tmp/rv-dt-g2r2/contract/cli/src/gallery.rs:100) and [gallery.rs:344](/tmp/rv-dt-g2r2/contract/cli/src/gallery.rs:344). |
| A7: package resolution | **Partial.** The probe checks resolution, but findings 5–6 remain. |
| A8: manifest identity | **Closed.** `Manifest::read` supplies defaults, errors and `.shells` handling at [gallery.rs:97](/tmp/rv-dt-g2r2/contract/cli/src/gallery.rs:97). |
| B6: test coverage | **Partial.** Rename/subpath and library-span fixtures were added at [tests/it/gallery.rs:279](/tmp/rv-dt-g2r2/contract/cli/tests/it/gallery.rs:279) and [346](/tmp/rv-dt-g2r2/contract/cli/tests/it/gallery.rs:346). Git exclusion, installed aliases and recursive generation remain uncovered. |

Findings from reviewing both commits together:

1. **BLOCKER — [gallery.rs:284](/tmp/rv-dt-g2r2/contract/cli/src/gallery.rs:284): Destination protection still permits source-tree output and overwrites.** The cache restriction applies only beneath the input app. For `apps/library-gate`, `-o docs/gallery-output` passes and can resolve the repository’s dependencies, leaving generated files visible to git. Separately, the guard checks only `app.contract`; an existing `app.json` is overwritten, including a symlink back to the original app’s manifest, by [line 105](/tmp/rv-dt-g2r2/contract/cli/src/gallery.rs:105). Validate git exclusion and both output files before writing. The destination test asserts neither protection.

2. **BLOCKER — [gallery.rs:104](/tmp/rv-dt-g2r2/contract/cli/src/gallery.rs:104): Reusing the design path loses its asset context and can abort the whole gallery.** A valid app design containing `icon(svg "icons/reply.svg")` reads that file beneath the original app’s `assets/`. Gallery compilation instead supplies `out` as the asset root; [icons.rs:104](/tmp/rv-dt-g2r2/contract/lower/src/icons.rs:104) therefore reads the nonexistent cache asset. Every individual cell fails, and the final gallery containing only refusal labels still fails because the design’s icons are checked independently of their use. Preserve the design’s asset context.

3. **SHOULD-FIX — [gallery.rs:443](/tmp/rv-dt-g2r2/contract/cli/src/gallery.rs:443): A shape’s declaration name is still mistaken for its public export name.** If the package entry says `use Card, Info as PublicInfo from "./card.contract"`, the app can construct `PublicInfo`, but the gallery emits `use Info from "<package>"`. It incorrectly refuses every `Card` cell. Recovering the original declaration spelling fixes `Info__index`, but an export-to-declaration mapping is still needed.

4. **SHOULD-FIX — [gallery.rs:372](/tmp/rv-dt-g2r2/contract/cli/src/gallery.rs:372): An app inside its library package silently loses all its imports.** For a library’s own example app, such as `pkg/test/fixture/app.contract`, the linked package root contains the app. This containment check consequently skips the app’s own `use` lines and produces zero cells. The loader explicitly supports this layout; [tamber_params_tools.rs:461](/tmp/rv-dt-g2r2/contract/cli/tests/it/tamber_params_tools.rs:461) covers it. Classify sources using their recorded origin rather than package-root containment.

5. **SHOULD-FIX — [gallery.rs:331](/tmp/rv-dt-g2r2/contract/cli/src/gallery.rs:331): Installed aliases can bypass the “same package installation” check.** `roots()` calls `packages()`, which [deduplicates aliases by canonical root](/tmp/rv-dt-g2r2/contract/cli/src/sources/packages.rs:53). If the app first imports a shape through alias A, then a component through alias B of the same package, its map retains only A. The gallery imports B; `app.get(B)` returns `None`, so a different installation shadowing B beneath the cache is accepted. Preserve every resolution name or compare the actual resolved declarations.

6. **SHOULD-FIX — [gallery.rs:402](/tmp/rv-dt-g2r2/contract/cli/src/gallery.rs:402): Rebased relative imports cannot escape the gallery’s new root.** A valid app can import `Card` through `./node_modules/pkg/index.contract` and another export through the package name, making `Card` a discovered library component. The generated relative path points outside `out`, but [resolve.rs:161](/tmp/rv-dt-g2r2/contract/cli/src/resolve.rs:161) rejects exactly that. `reaches()` aborts instead of generating the gallery. Rebase through a valid package export or preserve the original resolution context.

7. **SHOULD-FIX — [gallery.rs:409](/tmp/rv-dt-g2r2/contract/cli/src/gallery.rs:409): Public component aliases need not be valid gallery call names.** A package may re-export `Card as card`, with the app importing `card as Tile`. The gallery chooses lowercase `card` as its local name and emits `card(...)`; the parser recognizes component calls only when the name starts uppercase ([parser.rs:1167](/tmp/rv-dt-g2r2/contract/syntax/src/parser.rs:1167)). This valid component is incorrectly refused. Generate a valid local component identifier independently of the public spelling.

8. **SHOULD-FIX — [gallery.rs:674](/tmp/rv-dt-g2r2/contract/cli/src/gallery.rs:674): Constructible compiler records receive false “no shape” refusals.** `MediaMetadata`, `TextDecorations` and `TextDecorationSpan` are valid record constructors explicitly admitted by [records.rs:12](/tmp/rv-dt-g2r2/contract/types/src/records.rs:12), but they are absent from `file.shapes`. For example, `TextDecorations(value="value", spans=[])` is a valid placeholder, while this code refuses its prop before compilation. Include these constructible types when synthesizing records.

The shared cells comparison is present at [library-gate.test.mjs:65](/tmp/rv-dt-g2r2/scripts/library-gate.test.mjs:65). I found no additional issue with literal escaping, the empty `noop`, warning handling or refusal-loop termination. The commit messages report 46-app byte parity; I did not rerun that gate.

Static review only: no files changed, builds run or test suites executed.
