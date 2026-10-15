# Code review: LLP 1121 stage 2, commit 4's pin spelling (axis=value), 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, a throwaway worktree at the reviewed commit, read-only by instruction.
- **Method:** one brief (sha256 `ed10d6a88373a5da490aa484781439864bc9755d64d846f7f45a428ac5e9d86d`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra fix (MINOR); Grok ship.
- **Disposition:** the test fixed in the next commit; the RFC's examples are the RFC owner's §10 amendment

---

No findings.

The flip is consistent on every path that writes or reads a mode pin. `axis=value` is the only spelling that is accepted, and `axis:value` is refused on each of those paths.

- A literal `-exact-mode` pin is refused at compile time by `check_pins` in `contract/lower/src/theme.rs`. Any pin containing `:` is a `tamber-mode` error that names the `=` spelling (`dark-style:dim` becomes `` `dark-style=dim` ``). A pin with no `=` is told to write `axis=value`.
- A computed pin is the runner's. `pinned` in `runner/src/bridge.rs` splits only on `=`, so `dark-style:dim` yields no tuple. The mode row is cleared and the journal gets `invalid mode value …; unset`, and the surrounding tuple stays. `.tamber` `when` conditions already split on `=`; the colon there is the clause separator before the value.
- `prefer mode` in `scripts/agent-prefer.mjs` accepts only `axis=value` pins. `dark-style:dim` throws with `mode <axis>=<value>`.
- The wasm glue and the Apple bridge join pins as `axis=value` with commas, and both refuse an axis or value that itself holds `,` or `=`. The ABI parsers (`host/web/src/abi/mode.rs`, `host/apple/src/abi/preferences.rs`) split on `=` and return an error that names `axis=value` when a pin has none. Linux `prefer` already takes split pairs, so it has no string separator to update.

The new tests match that contract: a colon literal is `tamber-mode` and the message contains `` `dark-style=dim` ``; a computed colon pin leaves `ink` on the default slice `light-dark(#111111, #eeeeee)` and journals `invalid mode value`; the prefer group refuses `dark-style:dim`. Existing pins in the same file were rewritten to `=`. The Linux prefer-mode test's `()` data source matches the other tests in that file.

VERDICT: ship
