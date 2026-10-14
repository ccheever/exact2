# Code review: LLP 1121 stage 2, commit 7, round 3, 2026-10-14 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `5ef1e8b8d2173e54d0879e0ba0116bac7c67b65f8afd5116cda1b9eb88d4af59`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: two medium, two low; Grok: one high, one medium, three low.
- **Disposition:** applied without re-review: admission isolated and failing closed, implied after a clear, inline theme names; named and left: profile colours in contract tamber (a note says contract build checks them), a guide's tamber fence checked by the crate only

---

**High: none. Medium: two findings. Low: two findings.**

1. **Medium — Admission can succeed without checking the design.** [contract/cli/src/tamber.rs:153](/tmp/rv-s2c7r3/contract/cli/src/tamber.rs:153)  
   For an app declaring `"strings":{"base":"en"}`, the temporary app receives its manifest but no `strings/` directory. Compilation stops with `strings-unreadable` before design lowering; the filename filter discards that error. Consequently, the reserved name `top` or a varying role can still pass. Temporary-file failures likewise return an empty refusal list. **Fix:** isolate design admission from unrelated app loading and propagate setup failures. Add localized-app and failed-setup regressions.

2. **Medium — All three CLI verbs reject supported profiled colours.** [contract/cli/src/tamber.rs:74](/tmp/rv-s2c7r3/contract/cli/src/tamber.rs:74)  
   A token such as `ink = color(--brand 0.1 0.2 0.3 0.4)` is supported when Contract declares its `color-profile`—the existing [compiler fixture](/tmp/rv-s2c7r3/contract/cli/tests/it/tamber.rs:384) covers this. The initial raw crate check instead rejects `--brand` as an unknown colour space, before `admission` runs. Copying the manifest also cannot preserve profile declarations from Contract. **Fix:** share the compiler’s design preparation and profile context, preserving profiled values through `check`, `eval`, and `ir`; exercise that fixture through the CLI.

3. **Low — Clearing a bound colour produces incorrect `implied` metadata.** [host/web-js/modes.js:182](/tmp/rv-s2c7r3/host/web-js/modes.js:182)  
   When a boundary’s colour binding becomes `unset`, the runtime caches `$css.color = null`, then restores the inherited token in `$xs`. The new predicate requires `undefined`, so it reports `implied:false` despite the boundary supplying the row. This affects the existing conformance fixture’s `own-box` after `tap clear`. **Fix:** derive implication from the active boundary row, rather than the binding cache’s presence. Add a runtime metadata assertion after clearing the binding.

4. **Low — Inline theme tokens still produce no-tells false positives.** [scripts/no-tells.mjs:225](/tmp/rv-s2c7r3/scripts/no-tells.mjs:225)  
   With inline `theme` entry `brand="#c8102e"`, `color="rgb(from brand r g b / 0.5)"` is flagged. The identical expression passes when `brand` comes from `app.tamber`. The inline block is stripped without collecting its names, leaving D2’s two supported source forms inconsistent. **Fix:** collect inline-theme names alongside external-design names before masking the block; test equivalent inputs.

Caps, `git diff --check`, six read-only no-tells tests, and three help tests passed. In-memory probes reproduced both low findings, using a mocked DOM for the boundary case. Rust findings were source-reviewed: no built artifacts were available, and building or running the CLI integration tests would write files. No files changed.
