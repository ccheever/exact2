# Code review: stage 4 commit 6: uses, resolve and scrub sessions, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** HEAD, the change commit.
- **Method:** one brief (sha256 `354c2c9bb40f661dca87e3052f6f426d1d3a2e99c28d3b64bfbde13f41beeead`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (a release committing another client's scrub; an opening scrub not cancellable; a return to the starting value leaving the preview; wasm answers published out of order; the JS loop's stale baseline; idle from preview end; `uses` false and missed sites; a shared scratch file).
- **Disposition:** applied in the first-round commit, with theme2's conditions.

---

Blocking issues found.

- **P1 — Release can commit another client’s scrub.** [scripts/tamber-scrub.mjs:107](/tmp/rv-s4-10/scripts/tamber-scrub.mjs:107): ownership is checked before waiting, but only existence afterward. If Alice releases during a preview, her scrub is cancelled, and Bob opens one, Alice’s pending release commits Bob’s operations under Alice’s identity. Reproduced.

- **P1 — Cancellation misses an opening scrub.** [scripts/tamber-scrub.mjs:35](/tmp/rv-s4-10/scripts/tamber-scrub.mjs:35): `open` remains null through initial planning and preview. Consequently, competing writes, disconnects and server loss do nothing during that window; the delayed preview subsequently opens the scrub. Reproduced for all three events.

- **P1 — Returning to the starting value leaves the previous preview active.** [host/web-js/dev.mjs:271](/tmp/rv-s4-10/host/web-js/dev.mjs:271), also [host/web/dev.mjs:297](/tmp/rv-s4-10/host/web/dev.mjs:297): a proposal that changes nothing returns empty `texts`, which preview treats as success without restoring. Release then succeeds without writing, so no watcher repairs it. Existing and reconnecting builds retain the abandoned preview. Reproduced.

- **P1 — Wasm responses can publish batches backward.** [host/web/dev.mjs:471](/tmp/rv-s4-10/host/web/dev.mjs:471): when one stdout chunk contains `scrubbed` followed by `design`, the normal design publishes synchronously before the preview’s promise resumes. The older preview then replaces both batch and snapshot. The intervening build already cleared `Session.scrub`, so restoration cannot repair it. Reproduced.

- **P1 — JS rebuild captures a stale preview baseline.** [host/web-js/dev.mjs:137](/tmp/rv-s4-10/host/web-js/dev.mjs:137): hand-save the currently previewed value, then cancel while the build awaits completion. Cancellation restores the old value, but the build compares against the captured preview and sends no batch. Connected builds remain on the old value despite the newly committed snapshot. Reproduced.

- **P2 — Active updates can expire as idle.** [scripts/tamber-scrub.mjs:67](/tmp/rv-s4-10/scripts/tamber-scrub.mjs:67): the timer resets after preview completion, not update arrival. An update arriving at 29 seconds is cancelled at 30 seconds if its preview takes longer than one second. Reproduced with a controlled clock.

- **P2 — `uses` produces false positives and misses indirect reads.** [contract/cli/src/tamber_uses.rs:93](/tmp/rv-s4-10/contract/cli/src/tamber_uses.rs:93): scanning string operands reports `width=(mode == "brand" ? 100 : 200)` as using token `brand`. Conversely, `state ink = "brand"` followed by `color=ink` is missed because the binding contains `LoadSlot`, not the token string.

- **P2 — Preview scratch files collide across apps.** [host/web-js/dev.mjs:256](/tmp/rv-s4-10/host/web-js/dev.mjs:256): apps sharing a workspace share `app.target/web-js-scrub.tamber`. Concurrent dev loops can overwrite that file before either compiler reads it, displaying another app’s preview. Every preview also persists text on disk, contrary to the no-file-write requirement.

The [wire tests](/tmp/rv-s4-10/scripts/tamber-scrub.test.mjs:131) stub preview/restoration; they do not verify final build state. Validation above used in-memory Bun probes, not full suites. Worktree and index unchanged.
