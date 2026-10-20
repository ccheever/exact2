# Code review: a folded :disabled or :checked row read as its base, its parts beside it (LLP 1122 queue item (2), c1 and c2), round 1, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree at `caed257141`.
- **Method:** one brief (sha256 `f6ed21b515a1daecd8fb945a0bd40a6fd1e6b0c192959823f6d70dbde35384aa`), shared by both reviewers, each blind to the other's review; commits 2db62df4e4 (c1) and caed257141 (c2). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** 2 BLOCKER, 3 SHOULD-FIX, 1 NIT.
- **Disposition:** all taken in the round's fix commit (named in round 2's records):
  - A param base is named from the base as written, its file, component and `param.<name>`, by the param's full name, never from the runtime's names (the branch it shows now's); a param not read yet is refused, never a template edit of `param.fill` as text; tested with the disabled branch showing (1) and a component name two files share (4).
  - A part's edit is that part's alone: a part naming a param is offered its own template or state edit, never the param (2).
  - `color` and the tint are named as written (`color`, `-exact-tint-color`) (3).
  - A role spelling keeps its token beside the base's edit (`-exact-` normalized, as `editable` does) (5).
  - One part a state: a later part of the same state (the node's over its class's) shadows the earlier in the fold, so the map and the inspector name one (6).

**c2 has two blockers. I found no blocking issue or plan-byte-changing path in c1.**

1. **BLOCKER — The base param disappears when a state branch wins.** [tamber-inspect.mjs:464](/tmp/rv-dt-q2a/scripts/tamber-inspect.mjs:464) requires the param in `row.token.names`. Both hosts report tokens read by the **current value**, not every branch: see [inspect-dev.js:109](/tmp/rv-dt-q2a/host/web-js/inspect-dev.js:109) and [theme.rs:268](/tmp/rv-dt-q2a/runner/src/instance/theme.rs:268). For PostButton’s `background-color="param.fill"` with disabled fill `"#999999"`, selecting it while disabled supplies no param token. The join returns a **template** rung, losing the param editor and detach. Resolve the base independently of the active branch. The new test masks this by supplying the inactive param in its mocked report.

2. **BLOCKER — A part’s default edit can change the base too.** [tamber-inspect.mjs:449](/tmp/rv-dt-q2a/scripts/tamber-inspect.mjs:449) passes parts through `editable`’s param-first logic. With both the base and `:disabled` part reading `"param.fill"`, the part receives `rung: "param"`. Its editor changes the shared param, affecting the base and other uses instead of editing that part alone. An own part needs the template rung with `state`; a param link can be offered separately.

3. **SHOULD-FIX — `color` and tint parts get incorrect edit names.** The new calls at [tamber-inspect.mjs:448](/tmp/rv-dt-q2a/scripts/tamber-inspect.mjs:448) use `writtenAs`, whose fallback converts underscores to hyphens: `text_color` becomes `text-color`, and `tint_color` becomes `tint-color`. For `:disabled color="#999999"`, an own part is incorrectly refused; a class part offers `set-state` for nonexistent `text-color`. Use `color` and `-exact-tint-color`, respectively.

4. **SHOULD-FIX — Ambiguous component names lose a valid param rung.** [tamber-inspect.mjs:464](/tmp/rv-dt-q2a/scripts/tamber-inspect.mjs:464) matches `p.short`, but [tamber_params.rs:283](/tmp/rv-dt-q2a/contract/cli/src/tamber_params.rs:283) deliberately makes that null when two files declare the same component name. Thus `a.contract#Card.fill` falls back to a template edit even while the base is active and its exact param token is reported. Match the declaring file, component, and param identity rather than the optional short name.

5. **SHOULD-FIX — Role spellings lose their token alternative.** [tamber-inspect.mjs:466](/tmp/rv-dt-q2a/scripts/tamber-inspect.mjs:466) requires exact equality with a reported token name. A base written as `"-exact-secondary-label"` reports `secondary-label`; the helper discards it, although ordinary `editable` handles that spelling. The folded row therefore loses “Edit the token … instead.” Preserve the existing role-name normalization.

6. **NIT — Multiple parts can share inspector keys and test IDs.** [inspector.ts:108](/tmp/rv-dt-q2a/apps/design-tool/inspector.ts:108) gives each part its state plus longhand name. A class’s `:disabled border-color` and the node’s `:disabled border-top-color` both produce `:disabled border-top-color`. [app.contract:1657](/tmp/rv-dt-q2a/apps/design-tool/app.contract:1657) uses that name as both key and test-ID suffix. The JS runtime tolerates repeated keys, but the identities and test IDs are ambiguous. Include the part’s source address in a separate key.

The remaining c1 tracing preserved shorthand bases, mixed class/own origins, checked-under-disabled ordering, and fold indexing before `pressed`. Stale-source handling closes both base and part edits.

Confirmed the join findings with in-memory probes. No files changed; no builds or test suites ran. I did not rerun c1’s recorded byte comparison.
