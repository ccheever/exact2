# Code review: LLP 1081 stage 1b, text-decoration-line (astra)

**Reviewer:** Astra (codex gpt-6-astra, high), 2026-10-06, commit 5ff665c0f over 463acda68, read-only sandbox, brief sha256 `da202b0b4d1d4e138c626ca3f039700fcdc25c5488e3e2ada68dc070109a3cd5`. Mutually blind to Grok.

**Disposition (author, after the review):** all taken. The schema-digest snapshot is updated; `from_css` lowercases; the longhand and the shorthand refuse `underline-line-through` with an explicit rename; a computed longhand (either order, then a value no host draws) and a class switch are tested.

---

**Verdict: LAND WITH FIXES.** The lowercase spelling change is correctly connected through the production paths, and I found no remaining production constant using the old token. Two fixes are needed: refresh the schema-digest snapshot and make the new CSS parser case-insensitive. Rendering parity remains unverified.

Audited `5ff665c0f`, parent `463acda68`; HEAD matches the requested commit. Nothing was changed.

1. **High — the schema-digest test snapshot was not updated.**

   The changed enum string is part of the canonical schema hashed by [kernel/build.rs:1478](/Users/ccheever/projects/exact2-wt-names/kernel/build.rs:1478), using the digest function at [kernel/build.rs:112](/Users/ccheever/projects/exact2-wt-names/kernel/build.rs:112). However, [kernel/src/wire/codec.rs:702](/Users/ccheever/projects/exact2-wt-names/kernel/src/wire/codec.rs:702) still asserts `0x4430_c91f_3ec2_e2f8`, identically in the parent and this commit.

   **Impact:** the schema change should make `wire_codec_bytes_and_schema_digest_move_as_one_snapshot` fail. This is a static finding; I did not run the test or calculate the replacement digest.

   **Fix:** regenerate normally, update the handwritten snapshot to the resulting digest, and run the kernel tests. Do not edit generated code.

2. **Medium — the new parser does not fully follow CSS keyword casing, creating native/JS disagreement.**

   [kernel/src/style.rs:1475](/Users/ccheever/projects/exact2-wt-names/kernel/src/style.rs:1475) splits and sorts words but matches them case-sensitively. Consequently, `"UNDERLINE line-through"` is rejected, although it denotes the same CSS pair.

   This disagrees with the shorthand’s explicit lowercase normalization at [contract/lower/src/shorthands.rs:274](/Users/ccheever/projects/exact2-wt-names/contract/lower/src/shorthands.rs:274). It also disagrees with JS bound values, which reach the browser through [host/web-js/rt.js:829](/Users/ccheever/projects/exact2-wt-names/host/web-js/rt.js:829).

   **Impact:** a mixed-case longhand literal fails compilation; a mixed-case native bound value clears the row; the same JS bound value can render both lines. The old enum parser already had case-sensitive single-keyword lookup, but this new pair parser carries that limitation into the CSS correction.

   **Fix:** normalize ASCII keyword case in `from_css`, preserve duplicate/invalid-combination rejection, and test mixed-case values through both literal and bound paths.

3. **Medium — the tests cover parsing, but do not establish the requested cross-target result.**

   The new [kernel test:4](/Users/ccheever/projects/exact2-wt-names/kernel/src/style/decoration_tests.rs:4) checks both orders, whitespace, canonical `name()`, and several refusals. The new [Contract test:1414](/Users/ccheever/projects/exact2-wt-names/contract/cli/tests/it/styles.rs:1414) checks two longhand literals and rejection of the old token. An existing [shorthand test:1337](/Users/ccheever/projects/exact2-wt-names/contract/cli/tests/it/styles.rs:1337) switches a literal-choice expression to both lines and asserts the resulting native enum.

   Missing coverage includes:
   
   - A genuinely computed longhand string, updated through both orders and then an invalid value.
   - Decoration classes, including switching to a class without the row.
   - JS dynamic shorthand output and browser-computed decoration.
   - Exact web CSS emission after deleting the special case.
   - Decoration wire/agent round trips and native before/after pixels.

   **Fix:** add focused tests for those paths and perform the browser/native verification required by [LLP 1081:257](/Users/ccheever/projects/exact2-wt-names/llp/1081-names-exact-invents.rfc.md:257). Existing Swift tests that directly construct the spaced string do not exercise this schema/parser change.

4. **Low — the promised migration hint is only partially implemented.**

   Stage 1b explicitly requires refusal “with the hint, through the value pre-pass” at [LLP 1081:232](/Users/ccheever/projects/exact2-wt-names/llp/1081-names-exact-invents.rfc.md:232).

   Instead, the longhand gets the generic enum-options error from [contract/lower/src/values.rs:128](/Users/ccheever/projects/exact2-wt-names/contract/lower/src/values.rs:128). That includes the replacement among valid options, satisfying the new test’s substring assertion, but is not an explicit rename diagnostic. The shorthand’s old token reaches the generic unsupported-component error at [contract/lower/src/shorthands.rs:279](/Users/ccheever/projects/exact2-wt-names/contract/lower/src/shorthands.rs:279).

   **Fix:** provide the targeted replacement diagnostic in both contexts, or explicitly narrow the stage’s diagnostic requirement.

5. **Informational — production-path audit.**

   | Path | Result and evidence |
   |---|---|
   | Schema and generator | The authoritative value changes at [schema.json:1702](/Users/ccheever/projects/exact2-wt-names/kernel/tables/schema.json:1702). `pascal()` treats spaces and hyphens identically ([build.rs:82](/Users/ccheever/projects/exact2-wt-names/kernel/build.rs:82)); enum ordering is unchanged, so `UnderlineLineThrough` remains ordinal **3**. No generated file is edited. |
   | Kernel parsing and output | `set_dynamic` now calls `from_css` ([build.rs:1296](/Users/ccheever/projects/exact2-wt-names/kernel/build.rs:1296)); both lowercase orders work. Readback uses `.name()` ([build.rs:1367](/Users/ccheever/projects/exact2-wt-names/kernel/build.rs:1367)), producing the canonical spaced pair. |
   | Exact-name APIs | `from_name` remains an exact canonical-name lookup. `StyleId::enum_from_name` still delegates to it ([build.rs:671](/Users/ccheever/projects/exact2-wt-names/kernel/build.rs:671)), so those APIs reject reversed order. I found no production caller using them to parse this row; authored values use `set_dynamic`. |
   | Contract literals | Validation probes the kernel parser at [values.rs:726](/Users/ccheever/projects/exact2-wt-names/contract/lower/src/values.rs:726), so both lowercase orders work and the old token fails. |
   | Contract shorthand | The both-lines result is now canonical CSS ([shorthands.rs:283](/Users/ccheever/projects/exact2-wt-names/contract/lower/src/shorthands.rs:283)). Literal choices are recursively expanded. Arbitrary computed shorthand strings remain explicitly unsupported ([shorthands.rs:81](/Users/ccheever/projects/exact2-wt-names/contract/lower/src/shorthands.rs:81)); this is unchanged. |
   | Classes | Classes expand into ordinary attributes ([lib.rs:651](/Users/ccheever/projects/exact2-wt-names/contract/lower/src/lib.rs:651)); conditional classes produce per-row expressions ([class.rs:52](/Users/ccheever/projects/exact2-wt-names/contract/lower/src/class.rs:52)). There is no separate decoration-token parser. |
   | Native computed values | Runtime strings reach the same `set_dynamic` parser ([bridge.rs:148](/Users/ccheever/projects/exact2-wt-names/runner/src/bridge.rs:148)). Invalid values clear the row and produce a diagnostic ([instance.rs:915](/Users/ccheever/projects/exact2-wt-names/runner/src/instance.rs:915)). |
   | Web host | Generic enum emission writes the canonical string unchanged ([css.rs:606](/Users/ccheever/projects/exact2-wt-names/host/web/src/css.rs:606)). Removing the old-token special case is therefore correct. |
   | JS target | Static literals use the runner bridge and web projection ([style.rs:158](/Users/ccheever/projects/exact2-wt-names/host/web-js/src/style.rs:158)). Dynamic enum strings pass through without kernel parsing ([style.rs:573](/Users/ccheever/projects/exact2-wt-names/host/web-js/src/style.rs:573)). Both lowercase orders work through browser parsing; an old-token runtime value is attempted, rejected, and logged after removing the previous declaration ([rt.js:827](/Users/ccheever/projects/exact2-wt-names/host/web-js/rt.js:827)). |
   | Apple Rust and Swift | Generic Rust style JSON now contains the spaced name ([style.rs:298](/Users/ccheever/projects/exact2-wt-names/host/apple/src/style.rs:298)); content-region JSON already did ([wire.rs:95](/Users/ccheever/projects/exact2-wt-names/host/apple/src/content_region/wire.rs:95)). Both Swift painters test substrings ([Text.swift:789](/Users/ccheever/projects/exact2-wt-names/host/apple/Sources/ExactKit/Text.swift:789), [RegionTextSource.swift:75](/Users/ccheever/projects/exact2-wt-names/host/apple/Sources/ExactKit/RegionTextSource.swift:75)). Equivalent accepted values should retain their pixels. |
   | Linux | Contrary to the RFC’s stale statement, Linux **does paint decorations**. It matches the unchanged enum variant ([text_decoration.rs:32](/Users/ccheever/projects/exact2-wt-names/host/linux/src/paint/text_decoration.rs:32)); no spelling-dependent pixel change is apparent. |
   | Terminal | Matches the unchanged enum and sets underline/strike flags ([paint.rs:355](/Users/ccheever/projects/exact2-wt-names/host/terminal/src/paint.rs:355)). No text-token dependency. |
   | Wire | Enum decoding/encoding uses a byte ([codec.rs:283](/Users/ccheever/projects/exact2-wt-names/kernel/build/codec.rs:283), [codec.rs:312](/Users/ccheever/projects/exact2-wt-names/kernel/build/codec.rs:312)). The row payload remains ordinal 3; the schema digest changes, so this does **not** imply old/new frame compatibility. |
   | Agent | Runner inspection quotes the canonical enum name ([row.rs:109](/Users/ccheever/projects/exact2-wt-names/runner/src/agent/row.rs:109)); JS inspection reads computed browser CSS ([agent.js:113](/Users/ccheever/projects/exact2-wt-names/host/web-js/agent.js:113)). The field name remains `text_decoration_line`. |

   Thus, **literal and bound lowercase longhand values have the same result**, but not identical processing: native uses the kernel parser; JS bound values use browser parsing. Besides casing, JS can accept browser-supported values outside the native enum, such as `overline`. That broader mismatch predates this commit.

6. **Informational — remaining old spellings are intentional tests or historical prose.**

   The tracked-source sweep found **13 matching lines across five files**:

   - Negative tests: [decoration_tests.rs:25](/Users/ccheever/projects/exact2-wt-names/kernel/src/style/decoration_tests.rs:25) and [styles.rs:1423](/Users/ccheever/projects/exact2-wt-names/contract/cli/tests/it/styles.rs:1423).
   - RFC: `llp/1081-names-exact-invents.rfc.md`, lines 69, 117, 122, 187, 238, 320.
   - Historical reviews: `llp/reviews/1081-names-exact-invents.astra.md:40`; `llp/reviews/1081-names-exact-invents.grok.md`, lines 113, 137, 163, 167.

   No production emitter, matcher, app, or fixture retained the literal token in the searched tracked sources. Historical descriptions need not be mechanically renamed.

**Not checked:** compilation, test execution, generated artifacts, browser behavior in a running page, or native screenshots. These were outside your command restrictions. Dependency sources and external apps were not inspected. The digest failure and rendering conclusions above are source-based findings, not claimed execution results.
