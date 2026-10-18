# Code review: LLP 1122 T2b, the JS target, step 3 (inspect on the JS page), round 1, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `26dd75647`.
- **Method:** one brief (sha256 `75613a682b11d4722324979e5c0fad55d6492f32e66fb4abeabb9bf23b0f7220`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 high, 8 medium, 2 low).
- **Disposition:** taken in most part.
  - The gate is the dev client's: a classic script that listens on the window in the capture phase before the app's deferred module runs (and before `capture.js` queues a replay). Events go to the module's `gate` once it loads. Before that, a page reloaded while inspecting takes every input.
  - A token's row takes its value from the page's current design when a live edit moved it from the table's (theme2's ruling): the dev loop's snapshot gives the one token's value in the tuple, else the computed CSS. This was in the tree before this round.
  - Each row's CSS property is the emitter's (`css_property`, in the table), for set-now, computed values and the token scan.
  - An expression's row is set only when it isn't a clear (`inherit`, `unset`, …). A pin counts only when the design has it (modes.js `$xbad`). On a boundary, a colour counts only when its binding wrote it (`$xi`), not modes.js's implied one.
  - A boundary below an expression's row that names a token gets that token as its implied row.
  - A style's row takes its value from the table, in the tuple, with `from` the node itself when its own style gives it.
  - `focusin` stops before an app handler hears it. A guest frame's focus returns to the close control inside the gate.
  - The selection, with its click session, is kept across the dev loop's reload and found again by site and instance path.
  - The trim keeps `computed`.
  - Limits, for §10: a control start below a row only an expression sets; an expression's token text when it mixes text with tokens (role spellings too); a dynamic circle's `r` written as an attribute. The test still compares value, source and token, not `from` (view ids differ by page).

---

1. **High — Input gating installs too late.** [host/web-js/dev.mjs:466](/tmp/rv-dt-js3/host/web-js/dev.mjs:466)  
   The asynchronous import does not precede app startup. An app/module’s window capture listener registered first receives keys or pointers before `swallow`, even while inspecting. Reloaded pages also remain ungated until the import finishes. Install the gate synchronously before app scripts, restore its inspect state immediately, and delegate to the asynchronously loaded inspector.

2. **High — Live token edits produce stale values labelled with the current design.** [host/web-js/inspect-dev.js:71](/tmp/rv-dt-js3/host/web-js/inspect-dev.js:71)  
   Select the card, change `surface.card` through Tamber, then refresh. The page paints the new value, but `ownRow` still returns the build table’s value; that table is cached by plan, which live edits preserve. The envelope names the new design, so the server accepts the stale report. Resolve token-backed values against the current live design, including implied and inherited rows.

3. **Medium — The row-to-CSS mapping drops valid dynamic rows.** [host/web-js/inspect-dev.js:61](/tmp/rv-dt-js3/host/web-js/inspect-dev.js:61)  
   `border_width_top` becomes nonexistent `border-width-top`; `position_type` becomes `position-type`. Custom properties and prefixed properties are also missed. Numeric SVG geometry is written as attributes, so a dynamic circle’s `r` fails the inline-style check entirely. Generate DOM readers from the emitter’s actual property/attribute mappings instead of replacing underscores.

4. **Medium — Clears and refused values remain reported as own dynamic rows.** [host/web-js/inspect-dev.js:86](/tmp/rv-dt-js3/host/web-js/inspect-dev.js:86)  
   An expression yielding `color: inherit` leaves a nonempty CSS declaration, although the runner clears its own row and reports inheritance. Refused CSS is written as `unset`, causing the same error. Likewise, a pin containing `unset` still has `data-x-pin` and incorrectly reports a mode row. Determine ownership from whether the binding successfully sets a kernel row, including its clear/refusal semantics.

5. **Medium — Boundaries lose tokens supplied by dynamic ancestors.** [host/web-js/inspect-dev.js:155](/tmp/rv-dt-js3/host/web-js/inspect-dev.js:155)  
   Put `color=(flag ? "text.primary" : "text.secondary")` above a pinned preview. Dynamic table rows have no `k`, so `impliedRow` returns null. The preview reports inheritance from the ancestor, using its tuple, instead of its own implied token row resolved in the preview’s tuple. Read the ancestor’s current binding provenance and resolve it again at the boundary.

6. **Medium — Dynamic inheritance crosses control reset boundaries.** [host/web-js/inspect-dev.js:138](/tmp/rv-dt-js3/host/web-js/inspect-dev.js:138)  
   A parent with dynamic `text-transform="uppercase"` above an ordinary button exposes this: the static table sees no parent value, so it need not record the button’s initial-valued reset. The runtime walk then inherits uppercase through the button, whereas the runner stops there. Emit unconditional control reset metadata and honor it while walking ancestors.

7. **Medium — A text style supplied by the selected node loses `from`.** [host/web-js/inspect-dev.js:132](/tmp/rv-dt-js3/host/web-js/inspect-dev.js:132)  
   Select a node with `font-size="-exact-headline"` and no explicit weight. Its style supplies its own weight and leading, so `entry.s[name]` can equal its own site. Searching only above the node cannot find that source. Start this lookup at `el`, allowing `from` to identify the selected view.

8. **Medium — `focusin` reaches app handlers while inspecting.** [host/web-js/inspect-dev.js:348](/tmp/rv-dt-js3/host/web-js/inspect-dev.js:348)  
   If an asynchronous update autofocuses a field inside a `focusin` handler’s subtree, the inspector blurs the field but lets the original event continue. The app action still runs. Stop propagation before blurring, including events targeting inspector controls inside app modals.

9. **Medium — The guest-frame recovery listener cannot run.** [host/web-js/inspect-dev.js:350](/tmp/rv-dt-js3/host/web-js/inspect-dev.js:350)  
   When a guest frame takes focus, the earlier capture listener for `blur` calls `stopImmediatePropagation`, preventing this recovery listener from focusing Close. Subsequent keys—including Escape—remain in the guest document. Perform recovery inside the capture gate before stopping the event. This and the preceding focus defect also exist in the wasm implementation.

10. **Medium — Selection refresh does not survive the JS dev reload.** [host/web-js/inspect-dev.js:193](/tmp/rv-dt-js3/host/web-js/inspect-dev.js:193)  
    Select a node, then make a logic edit that preserves its plan/site. The JS loop replaces the document, losing `inspecting.selected`; session storage restores only inspect mode. A refresh now silently returns instead of resolving the previous instance. Persist the selection identity and click-session information through the reload, then resolve it or explicitly report its disappearance.

11. **Low — Dynamic token text is transformed CSS rather than the runner’s text.** [host/web-js/inspect-dev.js:80](/tmp/rv-dt-js3/host/web-js/inspect-dev.js:80)  
    A dynamic gradient naming `accent.fill` and `surface.card` reports text containing generated `var(--…)` references. The runner reports the original evaluated expression; role aliases also lose their original spelling. Preserve evaluated binding text before CSS translation instead of reconstructing it from CSSOM.

12. **Low — Trimming removes the value-format discriminator.** [host/web-js/inspect-dev.js:174](/tmp/rv-dt-js3/host/web-js/inspect-dev.js:174)  
    Once a report exceeds 60 KiB, the first trimming pass removes `computed` from every row. Surviving browser-computed values consequently appear to be kernel-format values. Preserve `computed` when trimming.

Release exclusion looks correct by source inspection: the module’s route exists in the dev server, and `$exactViewId` is stripped through the dev-only block. The GET route is Host-gated, rather than authenticated to an individual page; it provides no cross-origin CORS permission.

The tests do **not** establish the broader guarantees. The seven-node comparison omits `from`, `computed`, and `testId`, and exercises neither live edits nor the dynamic/reset/reload cases above. The release test checks source strings rather than release artifacts. I did not run tests because they build and write files, contrary to the read-only restriction.

**Verdict: Request changes; report parity and complete input isolation are not established.**
