# Code review: `:active` as the press paint; looks on the runner and the web (LLP 1121 stage 8 B1, B1'), round 3, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree.
- **Scope:** B1's commits through `674f09af9` (`cd2778f28` and `674f09af9` new since round 2).
- **Method:** one brief (sha256 `90c0215b56e19912ad396b85ebb46f70f8235c03820b1db48219b1a61215e153`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (should-fix: Space on a disabled box, the JS target's stale press base; nits: inspect's node span for an `:active` row, the snap assertion, the pointer gate's `closest`). The recorded transition deviation judged acceptable.
- **Disposition:** post-round fixes, as Astra's 2, 4, 1 and 6. Declined: 5, the pointer gate keeps `closest` (a pressable inside a disabled box took no press before B, and still takes none).

---

**RIGHT WITH CHANGES.** Round 2’s blockers are fixed. Two should-fix holes from that round are only half closed, and the recorded transition deviation is acceptable.

The transition item is an acceptable recorded deviation. `QUEUE.md:24` and the stage note at `llp/1121-tamber.rfc.md:3408` describe the real limit: on the web, an authored `transition` times a `:hover` or `:focus-visible` change both ways, because CSS transitions any computed change and a `transition: none !important` on the look would also kill base transitions while the look matches. The fix belongs in the glue. Apple and Linux are B2/B3. Do not treat this as an open defect.

Round 2, each finding:

- JS look specificity over the press paint (both reviews, blocking): fixed. `host/web-js/src/emit.rs:770` emits unscoped `[data-exact-looks="N"]`, specificity `(0, 4, 0)`, under the shell’s `#exact-root` press rules `(1, 2, 0)`. `modes_tests.rs` asserts the old prefix is gone.
- Disabled box still paints (Astra, blocking): pointer half fixed and covered by `host/web/tests/looks.test.mjs:111-119` (`disabled=""` plus mouse). Keyboard half still open (finding 1). The stage note’s “the glue takes no press on a box” overstates this.
- Docs still said Contract has no `:active` (Grok, should-fix): fixed. `docs/contract-for-agents.md:2386` and `:3098` say `:active` is that press paint, opacity is the pressed target, and each paint is written one way.
- Press mix freezes the base under a live edit (Grok, should-fix): wasm half fixed. `#exact-looks` / `#exact-theme` text in `document.head` retriggers paint, and the Chrome test replaces that sheet mid-press. JS-target tokens and nested modes do not (finding 2).
- Text with only a look folds to `display: contents` (Astra, should-fix): fixed. `host/web/src/element.rs:112` and `:136` require `looks().is_empty()` in the shared fold used by both web targets. Not re-run.
- Internal `-exact-press-opacity-to` leaked (both reviews): fixed. A `style` block blanks it and emits `lower-unknown-attr` (`contract/lower/src/lib.rs:347`), `similar_attr` skips it (`contract/lower/src/tags.rs:988`), named `contract vocab` skips it (`contract/cli/src/vocab.rs:183`), and a node still refuses it (`contract/lower/src/nodes.rs:288`). It stays in the style table the kernel needs.

Map and `uses` name an `:active` row’s own span (`"state":"active"` and `"at"` in `contract/cli/src/map.rs:228`, copied in `contract/cli/src/tamber_uses.rs:390-400`). Extra fields do not break `tamber_serve` or the design-tool readers: origins stay `own` / `tag` / `class:Name`, and unknown keys are ignored. `explain` still returns the token chain, not sites; the line is on `uses`, which is what the stage note means. The sheets observer does not loop: it connects only while `paints` is non-empty, disconnects when the last paint ends, and paint toggles `data-exact-painting` plus a Web Animation, which that observer does not watch. `--exact-press-opacity:` is not a substring of `--exact-press-opacity-to:`, so the share rule and the absolute rule do not both match.

1. **SHOULD-FIX** — Space still paints a disabled non-button. `host/web/input-glue.js:167-180`. Pointer down bails on `el.closest(":disabled,[disabled]:not([disabled='false'])")` (`:334`). The key path does not. A `column` with `press` is a `div` (`tabindex="0"`, `disabled=""` — `host/web/glue.js:460` and `:541`; the Chrome fixture is that div at `host/web/tests/looks.test.mjs:31`). It is not `button:enabled`, and `pressesByKey` (`input-glue.js:6`) is true, so Space calls `click()` and `keyed()` → `showPress` / `showPaint`. A real `<button disabled>` is already excluded. Before `preventDefault` / `click` / `keyed`, return on the same predicate as pointer down. Extend the disabled Chrome test with a Space keydown and assert neither `data-pressed` nor `data-exact-painting`.

2. **SHOULD-FIX** — A held press on the JS target keeps a stale base. `host/web/input-glue.js:239-245` snapshots opacity, background, and color into the animation keyframes. Repaint on a sheet or root change is only the observer at `:269-281` (head `childList` / `characterData`, and attributes on `document.documentElement`). That sees wasm `#exact-looks` text and a window `data-x-modes` change. It does not see `host/web-js/modes.js:216-222`, where a live token does `CSSStyleRule.style.setProperty` on tuple and `:root` rules, nor a nested boundary whose window key is unchanged (`modes.js:74-90`: `set` / `write` on the boundary, not on `<html>`). For the rest of the hold the mix eases from the colors captured at press. Call the same `repaint` from `design()` and from `apply()` after those writes. Attribute observation cannot see a CSSOM update.

3. **NIT** — Inspect still reports the node span for an `:active` row. `scripts/tamber-inspect.mjs:138-141` maps `binding.row` → `binding.origin` and sets `source` from the node. `scripts/agent-inspect.mjs:108-113` checks that origin regex and copies it onto the style; `sourceMap` is `at(entry)` for the node. Neither reads `"state"` / `"at"`. `uses` already does. When a binding has `at`, put that file and span on the row the inspector shows.

4. **NIT** — `host/web/src/host/looks.rs:80` treats “the rule text has no `transition`” as “a look snaps.” The QUEUE entry says that is not enough when the node has its own `transition`. Drop the snap claim from that assert, or cover both edges in the glue once the deviation is fixed.

5. **NIT** — The pointer gate uses `closest` (`host/web/input-glue.js:334`), so an enabled pressable inside a `div` with `disabled=""` takes no press, while the look rule only excludes the disabled element itself. Gate with `el.matches(":disabled,[disabled]:not([disabled='false'])")`. A control inside a disabled `fieldset` already matches `:disabled` on itself.

Cargo and the Chrome test were not re-run. Digest pins were not recomputed.
