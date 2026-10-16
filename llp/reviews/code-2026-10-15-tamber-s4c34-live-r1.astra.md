# Code review: stage 4 commits 3 and 4: the live table, round 1, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Method:** one brief (sha256 `230ec6b094bdf6799c769e0b4c8300a3ee18f6ecb7f17535381f1745d0114afa`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (code-referenced strings left out of the structure; the TypeScript receipt compared; snapshots refused with a role override; batches lost before a generation; snapshots racing batches; line caps; and seven more).
- **Disposition:** fixed in the first-round commit.

---

I found blocking issues in HEAD `41dad6f48`.

1. **P1 — `structure()` excludes strings that bytecode uses.** [plan/build.rs:734](/tmp/rv-s4-3/plan/build.rs:734)  
   Only table fields rescue strings marked as design values; `Opcode::Str` operands are never examined. Changing a literal `"#111111ff"` and a token sharing that string to `"#222222ff"` can leave the structure unchanged despite changed executable content. After a failed combined edit, a `.tamber`-only repair can consequently publish a batch while leaving changed Contract code unpublished.

2. **P1 — TypeScript value-only edits still publish generations.** [host/web/dev.mjs:590](/tmp/rv-s4-3/host/web/dev.mjs:590)  
   `sameModule` compares every `MODULE_FILES` entry, including `app.module.json`. That receipt includes the plan’s SHA-256, which changes with every retained token-value edit. The live branch therefore fails its equality check even when the TypeScript executable is unchanged.

3. **P1 — Snapshots fail whenever the plan contains a role override.** [runner/src/runner/live.rs:75](/tmp/rv-s4-3/runner/src/runner/live.rs:75)  
   Snapshots contain every token, including unchanged overrides such as `Canvas`. Passing them all to `set_tokens` rejects the entire snapshot because role tokens are forbidden. Ordinary deltas can work, while reconnect recovery—and Linux’s snapshot-based delivery—cannot apply the same edit.

4. **P1 — The wasm page drops batches arriving before their generation finishes loading.** [host/web/dev.js:266](/tmp/rv-s4-3/host/web/dev.js:266)  
   Such a batch receives `live: "structure"` against the old runner and is discarded. Generation completion neither retries it nor fetches the snapshot. An in-memory replay finished on the new plan’s original design instead of the newer live design.

5. **P1 — Older snapshot responses can overwrite newer edits.** [host/web-js/dev.mjs:205](/tmp/rv-s4-3/host/web-js/dev.mjs:205), [PlanURL.swift:859](/tmp/rv-s4-3/host/apple/Sources/ExactKit/PlanURL.swift:859)  
   Snapshot requests run concurrently with subsequent batches and other snapshot requests. Responses have no design-order check, and `base: null` permits unconditional application. I reproduced a newer blue snapshot being applied, then overwritten by an older red response. Apple’s resolution check only distinguishes generations, so it does not prevent this within one generation.

6. **P1 — The mandatory source-size check fails.** [Session.swift:250](/tmp/rv-s4-3/host/apple/Sources/ExactKit/Session.swift:250), [glue.js:1416](/tmp/rv-s4-3/host/web/glue.js:1416), [host.rs:448](/tmp/rv-s4-3/host/web/src/host.rs:448)  
   `bun scripts/caps.mjs` reports these files at **1,517**, **1,510**, and **1,501** lines respectively, exceeding the 1,500-line cap.

7. **P2 — Newly booted Apple sessions miss the current live design.** [Session.swift:252](/tmp/rv-s4-3/host/apple/Sources/ExactKit/Session.swift:252)  
   `design` updates only currently booted sessions and retains no snapshot. Later sessions boot from `app.lastPlan`, which still contains the published plan’s original values. A new window—or a snapshot received before any session boots—therefore shows old colors until another update triggers recovery.

8. **P2 — The JavaScript target accepts snapshots for another structure.** [host/web-js/modes.js:189](/tmp/rv-s4-3/host/web-js/modes.js:189)  
   `design()` never checks `frame.structure`; initialization supplies only the design digest. A snapshot fetched during a structural rebuild can apply another plan’s properties and tuple ordering to the current page. An in-memory probe confirmed that a foreign-structure snapshot returns `applied` and changes the rules.

9. **P2 — Deleted assets are mistaken for temporary files.** [host/web-js/dev.mjs:114](/tmp/rv-s4-3/host/web-js/dev.mjs:114)  
   Every missing path qualifies as `designOnly`, including a deleted asset. When plan structure and executable hashes remain unchanged, `keep()` discards the rebuilt output and retains the deleted asset in the served build. This can happen without any `.tamber` edit.

10. **P2 — Partial acceptance drops `requires` guards.** [scripts/tamber-server.mjs:525](/tmp/rv-s4-3/scripts/tamber-server.mjs:525)  
    Recomputing a non-revert proposal calls `hold()` without forwarding `p.guards`. The replacement proposal can subsequently be accepted without checking the original dependencies. I reproduced a guarded proposal becoming an unguarded proposal after accepting only one item.

11. **P2 — Live edits leave accent companion rules unchanged.** [host/web-js/modes.js:174](/tmp/rv-s4-3/host/web-js/modes.js:174)  
    Rules containing `data-x-accent` are excluded, although `modes.rs` embeds literal token colors in their `--exact-accent` declarations. For a token varying between `AccentColor` and a literal, changing the literal updates its custom property while descendants using the accent retain the old color. The replay produced a red token property alongside a black accent companion.

12. **P2 — Snapshot CSS loses custom profile colors.** [host/web-js/src/modes.rs:460](/tmp/rv-s4-3/host/web-js/src/modes.rs:460)  
    Unlike `rules()`, `design()` does not bind `plan.profiles` before calling `css_of()`. Custom `color(--brand …)` values consequently fail resolution and become `transparent` in `dev-design.json`. Applying a snapshot after an unrelated token edit overwrites those otherwise unchanged profile-color properties.

Validation: the existing modes tests passed (**2 tests**), Rust formatting passed, and caps failed as described. In-memory probes reproduced the JavaScript ordering, classifier, accent, and proposal-guard failures. Rust/Swift builds and integration suites were not run under the read-only constraint. No files changed.
