# Code review: web links dates (main red), round 1, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `0221c618c`.
- **Method:** one brief (sha256 `e9f424a26945c6e3faf195188869ed7b21be18ac75ddba1e3d8e89cd29643b6c`), a quick review under the main-red standing permission. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Dates is correct; Numerals has the same gap (1 high).
- **Disposition:** taken. The web core's set holds Numerals too, the runtime's own rolling text, which the CLI's `web_linked` also counts as linking nothing. Its test: a plan with `-exact-content-transition="numeric"` uses Numerals and boots on the core.

---

- **High — [host/web/src/link.rs:183](/tmp/rv-dt-dates/host/web/src/link.rs:183): Numerals still prevents wasm boot.** A plan containing `text "1,234" -exact-content-transition="numeric"` requires `Capability::Numerals`, but [web_linked](/tmp/rv-dt-dates/contract/cli/src/logic.rs:148) deliberately links nothing for it, and `Linked::uses()` never admits it. Boot therefore returns `Unlinked("numerals")`, even with every optional capability linked. This predates HEAD but remains the same unresolved bug. **Fix:** add `.with(Capability::Numerals)` beside Dates and a core-only boot regression test.

The Dates change itself is correct. The browser renders the input and supplies its picker; no optional Rust capability or export is needed. Both [ordinary boot](/tmp/rv-dt-dates/host/web/src/host.rs:428) and [checkpoint adoption](/tmp/rv-dt-dates/host/web/src/page.rs:504) use the amended admission function; reloads also pass through it.

No equivalent Dates omission appeared in the other requested hosts:

| Host | Admission/linkage |
|---|---|
| Apple | [Production derives its set from the plan; development includes every capability](/tmp/rv-dt-dates/bake/src/link.rs:26), so Dates reaches admission automatically. |
| Linux | [Boots the runner directly](/tmp/rv-dt-dates/host/linux/src/host.rs:243), without this capability allowlist. |
| JS target | [Projects browser elements directly](/tmp/rv-dt-dates/host/web-js/src/style.rs:1), without wasm admission. |
| Android Canvas | Uses the Linux host; [Dates selects Compose during packaging](/tmp/rv-dt-dates/scripts/agent-android.mjs:279), with [framework pickers as fallback](/tmp/rv-dt-dates/host/android/app/src/main/java/dev/exact/app/MainActivity.kt:13). |

The [new test](/tmp/rv-dt-dates/host/web/src/link.rs:359) is a valid regression test: it explicitly registers `Linked::CORE`, verifies the compiled plan requires Dates, and calls real host boot. It would fail before this change. Existing [runner tests](/tmp/rv-dt-dates/runner/tests/it/uses.rs:374) cover all three input types. It proves admission, not browser interaction, and does not cover Numerals. Tests were not run under the read-only constraint.

**Verdict: Correct Dates hotfix; the broader repair remains incomplete because Numerals still blocks wasm boot.**