# Code review: LLP 1122 T5 step 2, the tool's set-row editor, round 2, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `61bc295299`.
- **Method:** one brief (sha256 `fc6b755db6f8d8bb657abb5e43f6e049b8a4ea78072d91599679c7fd21a8270b`), shared by both reviewers, blind to the other review; commits 82d98133ff. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 medium; smaller points).
- **Disposition:** taken in `54d7784b9f`: a held look's row that set-row writes has its editor (`heldrow-row-`), the token beside; the tests assert set-row's address as the tool sends it (`editTarget`, template and style). Not taken: a reload with the same build facts doesn't stale a draft (same plan, design, pin and files: the draft is still the same copy's).

**Request changes: one T4/T5 integration gap blocks sign-off.**

1. **[P2] Held inline state rows have no set-row editor.** The held-row loop at [app.contract:1287](/tmp/rv-dt-t5r2/apps/design-tool/app.contract:1287) renders token links and `r.state` buttons, but ignores `r.edit`. The corrected join now gives inline state rows the template rung.

   Concrete scenario: select a button’s text child and hold Pressed, with `:active opacity=0.8` authored on the button. The held row has a valid set-row address, but both `state` and `token` are empty. It displays without an edit button or refusal reason; the selected child’s base rows offer no alternative. I reproduced this through the actual join and `selectionView`. Render the same `editRow(r.edit, r.editWords, r.editText)` control in the held-row loop.

Two smaller points remain:

- **Draft invalidation still revives after a same-facts reload.** [inspector.ts:95](/tmp/rv-dt-t5r2/apps/design-tool/inspector.ts:95) includes a facts fingerprint, not a reload generation. Open a draft, reload without changing source, then reselect the same node: the key and file digests match the original, so [app.contract:706](/tmp/rv-dt-t5r2/apps/design-tool/app.contract:706) clears `stateStale` without reopening the row. Leaving and re-entering Inspect also reproduces this. The specific pin case is fixed.
- **The reason for declining JSON assertions is partly incorrect.** `expect text` supports whole-text equality through [steps.rs:809](/tmp/rv-dt-t5r2/contract/syntax/src/parser/steps.rs:809), and its string lexer accepts escaped quotes at [lexer.rs:509](/tmp/rv-dt-t5r2/contract/syntax/src/lexer.rs:509). An exact JSON expectation is therefore possible. Join tests alone do not protect client serialization.

The other taken fixes check out:

- [app.ts:832](/tmp/rv-dt-t5r2/apps/design-tool/app.ts:832) rejects the original delayed-poll overwrite scenario for set-state, both set-row forms, both promotions, and detach.
- The heading names the authored attribute, including a shorthand and state.
- Style undo words include the file; the fixture now uses a paint row in a distinct file.
- T3 proposals and T4 hold addressing remain unaffected by the key change.

Keeping real compilation outside the canned stub is reasonable; it belongs in the real-server gate.

Verification used read-only Bun probes with in-memory replies. I did not run the browser/Contract suite or server gate. The checkout is unchanged.
