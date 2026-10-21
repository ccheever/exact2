# Code review: a bound autofocus that is false is no attribute (web JS target), 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `4e725bb59049cf57dc80db8d0568df4789a1c4a9d62bdbfd326d7e0abb864368`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** READY.
- **Disposition:** No findings beyond Astra's NIT (more transitions in the plan, and conform's comparison omits props.autofocus), left: both reviewers traced the transitions and the agent's data attribute by hand.

---

No findings.

The bound `autofocus=false` path now matches the static one. `P` stringifies the boolean, `toggleAttribute` leaves the attribute absent unless the value is `"true"`, and a non-true value writes `data-autofocus="false"`. `focus.js` only scans `[autofocus]`, so a false binding is not offered and does not take focus. `agent.js` still reports `autofocus: false` from that data attribute.

A later change is the same rule as the wasm host. False to true adds the attribute before `autofocus()` runs (`flush` is earlier in the commit), and the element is not in `offered`, so it can take focus. True to false removes the attribute and does not blur; the element stays in `offered`, so it will not take focus again. A carried restart still stubs `focus`, then `offerAll` marks every `[autofocus]` element. A false binding is not in that set, so a later true can still focus, which is the same as the wasm host marking only `exactAutofocus` nodes.

No other boolean prop written through `P` has this bug. The HTML boolean attributes (`disabled`, `readonly`, `inert`, `checked`, `multiple`, and the media ones) were already in `BOOL`. The rest are ARIA enumerated values or custom attributes compared to the string `"true"` (`retainFocus`, `emojiPicker`, `swipeIndicator`, `scrollFollowEnd`), where `attribute="false"` means false. `rt.js` stays 1499 lines; the edit is on the two existing lines.

The new plan only locks the mount case (focused flag and the `log` text). `conform.mjs` tree comparison does not include `props.autofocus`, so the `data-autofocus` write is untested. That write matches `rows.rs` and `agent.js`.

READY
