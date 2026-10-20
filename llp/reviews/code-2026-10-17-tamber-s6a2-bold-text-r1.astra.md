# Code review: Bold Text weights the system font (LLP 1115.000 D12.3, Tamber stage 6 A2), round 1, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** HEAD, the change.
- **Method:** one brief (sha256 `82035845d066b7f5bcec097178d16f276d41996389ac9bf8ac63e899bdbece25`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (P1s: a default weight measured bold and painted regular, a preference change left unapplied; P2s on host-painted buttons, generic lists, SVG's cascade and struts).
- **Disposition:** in the first-round commit.

---

HEAD `f3fd07247` has two P1 blockers and four P2 findings:

- **P1 — Default weight is measured bold but painted regular.** [defaults.rs:214](/tmp/rv-s6a2-1/kernel/src/arena/defaults.rs:214) changes `font_weight` without setting its mask bit. A bare text with no inherited weight measures at 600, but Apple’s serialization omits the weight and its painter falls back to 400. Custom bar faces also lose their previous bolding.

- **P1 — Preference changes can remain unapplied.** [viewport.rs:47](/tmp/rv-s6a2-1/runner/src/runner/viewport.rs:47) forces a commit only when the viewport is unchanged. Changing `bold_text` makes it unequal; without an `exactViewport()` resource, `replace_viewport()` commits nothing. Android toggles can therefore leave text unchanged until an unrelated app commit.

- **P2 — Host-painted buttons are incorrectly exempted.** [text_style.rs:113](/tmp/rv-s6a2-1/kernel/src/arena/text_style.rs:113) exempts native buttons on every host. Android’s buttons use the Linux painter, which applies no accessibility weight adjustment. Their titles remain regular while surrounding text becomes bold.

- **P2 — Generic font lists and runtime generics are missed.** [text_style.rs:115](/tmp/rv-s6a2-1/kernel/src/arena/text_style.rs:115) treats only IDs below eight as system fonts. `font-family="system-ui, sans-serif"` and a runtime string resolving to `"system-ui"` receive higher IDs, so the same system face stops responding to Bold Text.

- **P2 — SVG applies bolding before its final cascade.** [scene.rs:285](/tmp/rv-s6a2-1/kernel/src/svg/scene.rs:285) gets the newly bolded root style, but [cascade:319](/tmp/rv-s6a2-1/kernel/src/svg/scene.rs:319) overlays raw descendant rows. An SVG text’s explicit 600 stays 600 instead of 800; conversely, a descendant selecting a bundled family can inherit the root’s already-bolded weight.

- **P2 — Paragraph struts retain the original weight.** [arena.rs:568](/tmp/rv-s6a2-1/kernel/src/arena.rs:568) leaves the strut unbolded while the new run path steps it. For `white-space="pre" font-weight=400`, Apple measures tab stops using a 400 strut but paints using 600, producing different tab spacing and potentially wrapping.

No preference-bit collision found in the web/wasm remapping. No files changed; repository tests were not run.
