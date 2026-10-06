---
name: 20261005-shiki-residuals
plan: 20261005-t3code-macos-parity
implementation: planned
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: daehyeon/t3-code
branch: null
pr_url: null
verified_commit: null
---

# Code colours match Shiki for more languages and long texts

## Outcome

Code in chat fences, diffs, Files, attachment code previews and content-search lines has the colours the reference paints (Shiki 4.2 with the pierre light and dark themes):

- for languages beyond the sixteen the clone covers today,
- for texts longer than 40,000 characters, without a visible stall,
- with italics where a pierre theme sets `fontStyle`.

The sixteen existing grammars stay identical to Shiki. The grammar generator no longer reads the reference checkout: its input is a set of pinned published packages, and the app build never runs it.

## Scope and exclusions

Clone paths are `examples/t3-code/<file>`; the tables write `<file>`. Reference paths are repo-relative at T3 Code `1e2ecbd975` (`W/` is `apps/web/src/components/`). IDs are from [research](../research.md).

| Part | Reference behavior | Reference evidence | Clone state and work |
| --- | --- | --- | --- |
| Languages without a grammar | The reference colours every language Shiki bundles. A fence's language is the fence word (`gitignore` maps to `ini`); a language Shiki does not know falls back to plain text, and so does a language that fails to load. | `W/ChatMarkdown.tsx:561-563` (`extractFenceLanguage`), `:1275-1298` (highlight, fall back to `text` on error); `apps/web/src/lib/syntaxHighlighting.ts:25-43` (`getSyntaxHighlighterPromise`); tests `lib/syntaxHighlighting.test.ts`, `lib/incrementalHighlighting.test.ts`. | **Partial.** 16 grammars: typescript, tsx, javascript, jsx, json, jsonc, shellscript, python, css, html, markdown, yaml, toml, rust, go, swift (`r12-render-grammar.ts`, generated, 852,737 bytes; engine `r12-render-textmate.ts`; entry `r12-render-highlight.ts`, `shikiLanguage`). Other languages (ruby, java, kotlin, c/cpp, php and so on) use the heuristic tokenizer in `timeline-highlight.ts`. Adding every Shiki grammar grows the data from about 0.7 MB to over 2.3 MB (r12-render lane report, 2026-10-05). |
| Long texts | No size limit; the highlighter also resumes a streaming block from its last unchanged line. | `lib/incrementalHighlighting.ts` and its test. | **Partial.** `SHIKI_MAX_CHARS` = 40,000 (`r12-render-highlight.ts:16,86`): a longer text returns `null` and uses the heuristic tokenizer, so a long file changes colour model at the limit. Hermes tokenizes about 60,000 characters a second for the JavaScript-family grammars, and 5 to 50 times faster for the others (lane report). The streaming resume exists (`tokenizeDocument`, six cached documents). |
| Italics | A theme rule's `fontStyle` (for example italic Markdown emphasis) styles the token. | pierre themes in `@pierre/theme` (`themes/pierre-light.json`, `pierre-dark.json`). | **Missing.** Code runs carry a colour class only (`Cls` in `timeline-highlight.ts:13`, `Token` :17). Chat prose runs already use `font-style` (`markdown.contract:324`); code token render sites do not (`markdown.contract:577,582`, the diff, Files and search contracts). |
| Closed gap re-check | The r11 TypeScript corpus differed in 12,368 of 131,756 characters before round 12 and in 0 after. | — | **Done in round 12, not re-run by the plan.** Keep it closed: the re-check is a regression guard in this ticket. |
| Grammar generator input | — | — | **Wrong input today.** `gen-grammar.mjs` (lane tool, `target/t3-ui-parity/lanes/r12-render/tools/gen-grammar.mjs`) imports the packages from the reference build copy `target/t3-ref/src-f870c41/node_modules/.pnpm/…`. The reference checkout is verification-only, so the generator must read published packages instead (see Implementation notes). |

Not in this ticket: code-line wrap points and weight (X10, integrated acceptance); rendered HTML loading from non-IP `http` hosts (X7, `20261005-media-actions`); the `html` grammar's embedded languages beyond what the 16 give; the heuristic tokenizer for the Mermaid or diagram code (unchanged).

States: loading (a long text paints with the heuristic colours first and upgrades when its slice finishes; no spinner), error (a grammar that fails to load or throws paints as plain text, as the reference falls back to `text`), empty (an empty fence paints nothing), disabled and permission (none). No dialog, popover or menu. Reduced motion: the colour upgrade has no animation.

## Context and guidance

Parent specification: [spec](../spec.md). Research: [research](../research.md). The 16-grammar engine and its numbers come from round 12 (lane `r12-render`): against real Shiki 4.2 the r11 TypeScript corpus went from 12,368 to 0 differing characters, real files in the 15 other languages gave 0, and the 2,208 regex variants compile in the pinned Hermes (one Hermes lookbehind bug is rewritten at generation time). The plan did not re-run these numbers.

Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol. Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23).

Library revision: `20261005-platforms-v3`. Selected topics: performance (same-scenario before and after for the long-text workload; measure, do not assume), state-and-data (a late slice for an old text must not overwrite a newer one), design (complete states), testing-and-debugging (record source and build identity with each measurement). Unknown in the library: the data module's Hermes regex behaviour and bytecode size limits; the clone's runtime evidence on the pinned main is the basis.

Consumer framework revision and toolchain: the pin chosen by `20261005-clone-on-exact2-main`; pinned Bun 1.4.2 and Hermes. Verification-only tools may read the read-only reference checkout (token comparison corpus). The app build and the generator must not.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` | pending |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged (oracle shots for the pixel pairs) | pending |
| recorded decision | U2: apparatus: the grammar generator moves into the example (`tools/grammar/`) and a token-comparison harness is added | none | User approves | pending |
| recorded decision | U15: languages beyond the 16 grammars, and the long-text limit | none | User chooses the list and the limit | pending |

## Issue assessment at preparation

Checked sources and time: local issue drafts in [issues](../issues/README.md), `EXACT2-GAPS.md`, the library; no upstream search (planning). Re-check at `prepare`.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X10](../issues/20261005-x10-text-rendering-parity.md) | Text rendering parity | Code wrap points and weight differ in pixel pairs | nonblocking (declared) | List in `EXACT2-GAPS.md` with the pair |
| [X22](../issues/20261005-x22-reactive-layout-facts.md) | Layout facts | Not needed: tokens are computed in the data module | none | — |
| none found | More grammars, slicing, italics | `font-style` is admitted on `text` nodes (`markdown.contract:324`); confirm with `contract vocab` at `prepare` | none | — |

## Implementation notes

**Generator input (pinned published packages, with hashes).** Install these exact versions from the npm registry into `tools/grammar/` from a committed `package.json` and `bun.lock`, with `bun install --frozen-lockfile`. The integrity values are those the reference's `pnpm-lock.yaml` records at `1e2ecbd975` (they are the registry tarball hashes; the reference lockfile is the source of the values, not an input):

| Package | Version | Integrity (sha512) |
| --- | --- | --- |
| `@shikijs/langs` | 4.2.0 | `bwrVRlJ0wUhZxAbVdvBbv2TTC9yLsh4C/IO5Ofz0T8MQntgDvyVnkbjw9vi50r1kx7RCIJdnJnjZAwmAsXFLZQ==` |
| `@pierre/theme` | 1.1.0 | `GC2OWTAfTIIWWYhPCygwG8t2EtePQkRfON4MI2rwIkJylmiyqIttJID2dCL8sUD8cNdEvYkEyfEHHKMeCiDLoQ==` |
| `oniguruma-to-es` | 4.3.6 | `csuQ9x3Yr0cEIs/Zgx/OEt9iBw9vqIunAPQkx19R/fiMq2oGVTgcMqO/V3Ybqefr1TBvosI6jU539ksaBULJyA==` |
| `oniguruma-parser` (dependency) | 0.12.2 | `6HVa5oIrgMC6aA6WF6XyyqbhRPJrKR02L20+2+zpDtO5QAzGHAUGw5TKQvwi5vctNnRHkJYmjAhRVQF2EKdTQw==` |
| `regex` (dependency) | 6.1.0 | `6VwtthbV4o/7+OaAF9I5L5V3llLEsoPyq9P1JVXkedTP33c7MfCG0/5NOPcSJn0TzXcG9YUrR0gQSWioew3LDg==` |
| `regex-recursion` (dependency) | 6.0.2 | `0YCaSCq2VRIebiaUviZNs0cBz1kg5kVS2UKUfNIx8YVs1cN3AV7NTctO5FOKBA+UT2BPJIWZauYHPqJODG50cg==` |

These are the versions `gen-grammar.mjs` imported from the reference build copy (`@shikijs+langs@4.2.0`, `@pierre+theme@1.1.0`, `oniguruma-to-es@4.3.6`). The reference still pins the same versions at `1e2ecbd975`. Re-check them at `prepare`: if the reference bumps them, the pin changes in the same PR as a new generated file, never silently.

- **The build never runs the generator.** The generator is a developer command. Its output (`r12-render-grammar.ts` and any added per-language data files) is committed, with a header that lists the package pins above and the generator's own hash. Nothing in `app.json` commands, build scripts or tests runs it except the reproducibility test below. The app build needs neither `tools/grammar/node_modules` nor the reference checkout.
- **Port the generator.** Replace the two absolute imports of `gen-grammar.mjs` with imports from `tools/grammar/node_modules`. Keep its options (Shiki's JavaScript-engine options, ES2018 target, the four `\A`/`\G` variants, the Hermes lookbehind rewrite). Per language, check the grammar's licence in the package metadata and keep it in the generated header (the sixteen are "MIT and other permissive licences"); omit a language whose terms are unclear and list it for U15.
- **File cap.** The generated file has very long lines; the cap is on lines (1,500). One data file per language group keeps each file reviewable.
- **Long texts.** Proposal: slice tokenization across data-module turns so one turn never runs longer than a budget (50 ms is a starting value, to be measured), paint the heuristic tokens first, then upgrade; keep the streaming resume. The limit is a decision (U15) and the measurements decide the slice size.
- **Italics.** Add a style field to code tokens (for example `Cls` plus an italic flag) and set `font-style` at every code render site. Which `fontStyle` values the pierre themes use is unknown: read them from `@pierre/theme` at `prepare`.
- **Comparison harness (verification apparatus).** The earlier harness (`cmp-engine.mjs`, a 40-file TypeScript corpus taken from the reference's server sources, per-language samples) lived in a session scratchpad and is not preserved. Rebuild it as `target/t3-ui-parity/shiki-compare.mjs` (committed under `examples/t3-code/tools/` with the same relative paths, decision U23): it runs real Shiki (`shiki@4.2.0`, integrity `hjNax6o/ylDy9lefQEaSDtzaT3iVNtZ3WmpQnbuQNoG4xvnSKf2kSKbihZVO4JRG1TTMejs7CmNRYlWgAL66pQ==`, with its dependency closure from a committed lockfile) and the app's engine on the same text, and compares each character's light and dark colour. It may read the reference checkout for the corpus; the corpus is copied to `target/` and never into the build.

## Acceptance and reproduction

All rows: macOS 26.6.2, 1280×840 and 840×620, light and dark. Pixel pairs use the reference desktop oracle. There are no attended rows. Isolated fixture backends use ports 16000–16999 and isolated HOME, CODEX_HOME, CLAUDE_CONFIG_DIR, XDG_* and T3CODE_HOME.

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Generator reproduces the committed data | Clean checkout; `bun install --frozen-lockfile` in `tools/grammar/` | Run the generator for the 16 languages | Output is byte-identical to the committed `r12-render-grammar.ts` | macOS | diff log |
| Pins verified | Same | Compare `bun.lock` integrity values with the table above | All six match | macOS | log |
| Build is self-contained | Rename the reference checkout path and delete `tools/grammar/node_modules` | Build the app bundle and run the clone checks | Succeeds; `git grep gen-grammar` finds only `tools/grammar`, its test and docs | macOS | build log, grep log |
| Closed gap stays closed | The 40-file TypeScript corpus (131,756 characters in the earlier run) copied to `target/` | `shiki-compare.mjs` | 0 differing characters | macOS | harness log with file list and hashes |
| Each added language | At least three real files per language, 10 KB or more in total | `shiki-compare.mjs` per language | 0 differing characters, light and dark | macOS | harness log |
| Long text | A 200 KB source file in Files, a chat fence and a diff; same file before the change | Open, scroll, type in the composer meanwhile | Heuristic paint first, then Shiki colours; no data-turn longer than the budget; the UI keeps responding | macOS | per-turn timings, `perf` before and after, shots |
| Language outside the list | A fence with an unknown language word; a grammar forced to throw | Render | Plain text, no error shown | macOS | shot, unit test |
| Italics | A Markdown file and fence with emphasis; a theme rule with italic | Render | Italic where the oracle is italic | macOS | pixel pairs |
| Pixel pairs | Files, chat fence and diff for three added languages | Compare with the oracle | Colours equal; wrap-point differences listed under X10 | macOS | pairs |
| Checks | `git add -A` | clone checks (bun test, strict tsc, contract build, `cargo test -p t3-code-macos --lib`, AppKit binaries), `bun scripts/caps.mjs`, the five checks | Green; every moved matrix cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

Task-owned source paths: `r12-render-*.ts` and new per-language data files, `timeline-highlight.ts`, `timeline-diff-syntax.ts`, `r4-surfaces-files.ts`, `markdown.contract` and the diff, Files and search Contract files that draw code tokens, `tools/grammar/` (generator, `package.json`, `bun.lock`), tests, `AGENT-HANDOFF.md`.
Required environment: network access for the pinned package install (once); the read-only reference checkout and real Shiki for the comparison harness only.

## Progress

Planned.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

`prepare` after the three dependencies merge: confirm U2 and U15 with the user, then `implement`.
