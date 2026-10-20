# Code review: LLP 1121 stage 9 G1, a look snaps on the web, round 3, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max (round 4: high), sandbox read-only, a throwaway worktree.
- **Scope:** the change and rounds 1-2.
- **Method:** one brief (sha256 `d8837278ed2e15e6cbc72eea86aae7c07e88e570e261b1fb9c6b7979e732d5de`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (BLOCKING: focusSeen not seeded or scoped, a shortcut swallowing the key; stale frame re-check, dedupe, tests).
- **Disposition:** fixed in round 3's commit.

---

**Two blocking issues remain.**

1. **Blocking — [host/web/input-glue.js:474](/tmp/rv-g1-3/host/web/input-glue.js:474): `focusSeen` is neither initialized nor scoped to a focus session.** An autofocused node can already match `:focus-visible` when input glue loads. Its first pointer/script blur then compares `false` with `false`, so the look eases away. Conversely, removing a visibly focused node without delivering `focusout` to the root leaves `true` cached; reinsert it, render it unfocused, then focus it by script, and the incoming edge is skipped. **Fix:** seed the current deep focus at installation and treat a new `focusin` as a new focus session, rather than trusting a value retained from an earlier attachment.

2. **Blocking — [host/web/input-glue.js:491](/tmp/rv-g1-3/host/web/input-glue.js:491): shortcuts can bypass the key snap entirely.** Click a focus-look node, then press a plain key bound through `aria-keyshortcuts`, such as `x`. The existing document-capture shortcut handler calls `stopImmediatePropagation()` before this root listener runs. In the early browser ordering, `:focus-visible` has changed, but neither snapping nor the frame check happens. **Fix:** observe the focus-visible edge before shortcut dispatch can consume the event.

3. **Should-fix — [host/web/input-glue.js:495](/tmp/rv-g1-3/host/web/input-glue.js:495): the deferred check uses stale eligibility.** In the late-flip ordering, a key handler can set `disabled` on a focused, focusable `div`. The div remains focused, and `turned()` subsequently snaps it despite its look being disabled. The callback also retains the original element if focus moves. **Fix:** resolve the current deep focus again in the frame callback and repeat containment, disabled, look, and state checks.

4. **Should-fix — [host/web/input-glue.js:478](/tmp/rv-g1-3/host/web/input-glue.js:478): the microtask does not deduplicate normal browser events.** A microtask checkpoint can run when the pointer listener returns, before its compatibility mouse event—even within the same task. The marker is then gone, producing two snap pulses and two style reads. This follows [HTML’s script-cleanup rules](https://html.spec.whatwg.org/multipage/webappapis.html#clean-up-after-running-script). **Fix:** deduplicate actual hover-state changes or explicitly correlate and consume compatibility events while preserving mouse-only edges. I found duplication, rather than confirmed suppression of legitimate native mouse edges.

5. **Should-fix — [host/web/tests/looks.test.mjs:88](/tmp/rv-g1-3/host/web/tests/looks.test.mjs:88): the assertion cannot establish “no transition started.”** `getAnimations()` also returns no transition after one was started and canceled—the round-2 failure. The fixture additionally installs input handlers directly, so removing the `rt.js` loading fix would remain undetected. **Fix:** observe transition lifecycle events, count snap pulses on a real crossing, and add a look-only runtime fixture covering adoption and initially focused nodes.

6. **Nit — [host/web/input-glue.js:480](/tmp/rv-g1-3/host/web/input-glue.js:480): every boundary event creates another media-query object.** This happens even over nodes without looks, and the dedupe defect doubles it for ordinary mouse crossings. **Fix:** create one `MediaQueryList` during setup and read its live `.matches` value.

`inPage` terminates through the host chain. Calling `input()` before `Adopt` is correct; loading remains deferred past first paint. Look-only pages now incur the full input module, listeners, stylesheet, and observer setup afterward.

Validation used read-only Bun probes and `git diff --check`. The browser suite was not run because it creates a temporary Chrome profile.
