# `font-family` from a string at run time, so a font picker can apply any installed family

**Status:** Open
**Systems:** Contract, runner, text, GUI hosts
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/318

## Current scope

Decision needed under LLP 1019/1053: runtime installed-family strings versus a declared allow-list. Preserve fallback, weights/styles and measurement invalidation. Reporter recommendations are not accepted amendments. Bundled fonts remain compile-time; no runtime font-file loading/parsing.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

### Request and background

A font picker lets a person choose any font installed on their machine for an app's interface, prompt or code text. T3 Code's Appearance settings work this way, and so does any editor with a font preference:

- the reference lists installed families with the Local Font Access API (`queryLocalFonts()`, `apps/web/src/appearanceFonts.ts:369-395` at `1e2ecbd975`);
- it writes the chosen name into a CSS variable at run time: `root.style.setProperty("--font-mono", "<name>, <fallback stack>")` (`:99-116`);
- every text that uses the variable re-resolves its face and re-lays out.

Exact accepts installed families, but only as **literals**. `font-family="Arial, sans-serif"` resolves on every host: on Apple, a family without a source is resolved to its real installed faces (`host/apple/Sources/ExactKit/Text.swift:630-645`, `FontFaces.swift:34`). A finite choice also compiles, as `?:` or `match`, when every arm is a literal family (LLP 1053 G7). A family name held in state or returned by a data source does not compile, because LLP 1019 interns every family stack at compile time: "**`font-family` is literal-only.** A `derive` evaluating to `"Menlo"` has nowhere to intern" (`llp/1019-fonts.rfc.md:206-211`).

So a picker can offer only the families the developer wrote into the source, never the families actually installed on the user's machine. Listing the families is not the gap: the clone enumerates 247 families through its own Swift module (CoreText). Applying the chosen one to ordinary Contract text is.

The consumer is T3 Code's Code-font picker, which lists installed families such as Arial, Menlo, Monaco and Georgia. The clone can list only a fixed set and answers "No fonts found." for Arial.

### Current and expected behavior

- **Current (every host):** `text … font-family=family`, where `family` is a `state` string, is refused at build:

  > `[lower-font-family-literal]` `font-family` takes a family name, or a choice (`?:` or `match`) whose every arm is one; a family computed at runtime cannot be resolved, because fonts are declared and resolved at compile time

  Both `bun exact.mjs mac` and the web build fail on it.
- **Control:** the same text with `font-family=(family == "Menlo" ? "Menlo" : "Arial")` builds. Switching it on macOS changes the node's `font_family` stack id from 9 to 8, and its height from 23 to 24 pt.
- **Expected:** a `font-family` bound to a string, including one that came from a data source, applies that CSS family list at run time:
  - the web keeps CSS's family-list semantics;
  - native hosts resolve the named installed family's real faces, with the same weight and style matching and fallback as a literal;
  - affected text is re-measured, with no stale metrics.

### Reproduction and evidence

A one-file app made with `bun scripts/exact.mjs new <dir>`, with no data sources:

```text
component X48app
  state family = "Arial"
  action pick(name: string)
    family = name
  view
    main testId="root" width="100%" height="100%" box-sizing="border-box" padding=24 gap=12 background-color="#ffffff" color="#111111"
      row gap=8
        button press=pick("Arial") testId="arial"
          text "Arial"
        button press=pick("Menlo") testId="menlo"
          text "Menlo"
      text `selected: ${family}` testId="label"
      text "The quick brown fox 0123456789" font-family=family font-size=20 testId="sample"
```

| Scenario | Setup / reset / exact commands | Platform / OS / device | Framework revision | Actual result | Expected result | Evidence |
|---|---|---|---|---|---|---|
| State-bound family, compiler | `contract build state.contract -o state.plan` (the three-line form: `state family = "Arial"` / `text "…" font-family=family`) | macOS 26.6.2, Apple Silicon (any host) | main `b896050d7` | exit 1, `state.contract:4:50 [lower-font-family-literal]` | exit 0 | [record](https://raw.githubusercontent.com/ccheever/exact2/f9d5d3e7be6a803d35a207726ac4a28408bbdf58/file-x48-x68/x48-record.txt) |
| The app above, both hosts | `bun exact.mjs mac`; `bun exact.mjs web-build` | macOS; web (JS target) | same | both fail: `app.contract:13:57 [lower-font-family-literal]` | both build; tapping Menlo draws the sample in Menlo | same |
| Literal choice (control) | `font-family=(family == "Menlo" ? "Menlo" : "Arial")`; `bun exact.mjs mac`; `bun exact.mjs agent macos --size 520x240 "layout sample" "tap menlo" "clock settle" "layout sample"` | macOS | same | builds; `font_family = 9` → `8 (dynamic, own)`; box 472×23 → 472×24 | (works today) | same |
| Literal list and literal choice, compiler | `font-family="Arial, sans-serif"`; `font-family=(useArial ? "Arial" : "Menlo")` | any | same | exit 0 for both | (works today) | same |

### Acceptance criteria

- The state-bound app builds on every host. Tapping Menlo, then Arial, changes the sample's resolved face and its measured box each time, without a rebuild.
- A name returned by a data source, which appears nowhere in the source, works the same way.
- Quoted names, fallback lists, generic keywords, a missing family (falling back as CSS does), bold and italic matching, and inheritance behave as they do for literal families.
- On macOS, the resolved face name and measured width show that Arial and Menlo select their own faces, not a silently substituted generic.
- Declared (bundled) fonts stay compile-time. This adds no runtime font files and no `loadFont`.

### Constraints and related work

- Workaround: a finite literal `match` over a fixed list of families. That list cannot include families installed after the build or unknown to the developer.
- Blocking rule: LLP 1019 D2's literal-only line (`llp/1019-fonts.rfc.md:206-211`) and its premise that "the family id is assigned by the compiler, never at runtime" (`:24-25`). LLP 1019 OQ3 (`:530-540`) already names installed-only faces as the open counter-case. Its "file-less declaration" is still compile-time, so it does not serve a picker.
- `rules/DEFERRED.md:130-131` (the text-flow entry) takes "no runtime font parsing". This request needs none: it names installed families, which the host's own font system resolves.
- Not tested: iOS, Linux (which picks the first installed family of a stack), and runtime font-family on `input`/`textarea`.
- Related: #266 (`text-wrap: balance`, placeholder colour, smoothing) and the closed #102 (root font size) are other text rows; neither covers family names.

## Discussion at transfer

### daehyeon-mun — 2026-10-08T09:42:18Z

## Decision needed

**What blocks it (main `b896050d7`).**
- `llp/1019-fonts.rfc.md:206-211`: "**`font-family` is literal-only.** A `derive` evaluating to `"Menlo"` has nowhere to intern. v1 accepts string literals and closed control flow over literals … it does not accept a data string." This rests on the premise at `:24-25`: "The family id is assigned by the compiler, never at runtime".
- `llp/1053-gaps-found-by-the-list-benchmarks.rfc.md:50` (G7, as built): choices over literal families are resolved to stack ids at compile time, and "runtime strings refused".
- `contract/lower/src/fonts.rs:264-285` raises `lower-font-family-literal`. The kernel's `font_family` row is a `u16` stack id into the plan's table (`kernel/tables/schema.json`).
- LLP 1019 OQ3 (`:530-540`) names an installed-only face as the honest counter-case. Its proposed form, a file-less `font "Menlo"` declaration, is still compile-time.

**Options.**
- **A. Runtime installed families.** A `font-family` bound to a string expression is admitted. The runner interns each new family list into a run-time stack id above the plan's ids and hands the hosts the list:
  - Apple resolves the installed family's real faces, as it already does for a literal installed family (`Text.swift:630-645`);
  - the web writes the CSS string;
  - Linux uses fontdb's family lookup.

  A name that resolves to no installed family falls back as CSS does. Declared, bundled fonts stay compile-time, with no runtime files and no `loadFont`.
- **B. Declared allow-list only.** OQ3's file-less declaration (`font "Menlo"`), plus expressions over declared and generic families (G7's original proposal, `llp/1053-gaps-found-by-the-list-benchmarks.rfc.md:146`). This is still compile-time, so a picker offers only families the developer listed.
- **C. No change.** Apps use a literal `match` over a fixed list, and a picker cannot offer the families that are actually installed.

**Recommendation.** A, limited to installed families named by string. It is what the web does, and the reference sets a CSS variable at run time. It keeps LLP 1019's real goal: no runtime font files, so no loading state and no cache coherence across file arrivals. Bundled faces keep their compile-time ids and checks. Only the "never at runtime" id rule changes, and only for installed names.

**Cost.**
- Runner: a run-time stack table (string → id), with ids stable for the session.
- Kernel: none beyond the existing `u16` row.
- Hosts: a "stack N = family list" op after boot, and invalidation of the text measurement and paint cached for nodes that change stack. Apple already re-lays out on a literal-choice switch.
- Web: the string goes straight to CSS.
- Tests: the repro on each host, plus face identity (Arial and Menlo) and a missing-family fallback case.
