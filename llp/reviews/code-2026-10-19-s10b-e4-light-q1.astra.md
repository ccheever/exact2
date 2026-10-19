# Code review: LLP 1121 stage 10b E4, the one light mode (the live Figma test), quick check 1, 2026-10-19 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `b199702127`.
- **Method:** one brief (sha256 `1b1674c981f2ee53078506dca7d2d6eb3472fb76138583c7f4ffdc36cf9276af`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

1. **Note — collapse is correct for valid exports.** Light contexts collapse when all remaining axes match after removing every axis marked `under: "dark"`. The first context survives; sorting the export default first preserves it as representative and first mode. Labels omit those axes (`light`, or e.g. `density=tight, light`). Dark contexts and headerless exports remain separate. Multiple axes work; labels retain the existing 40-character truncation and collision suffixes. Split layout remains uncollapsed.

2. **Note — normal round trips agree.** `holding` reads the representative’s mode ID; push copies its edited value to every `also` context; `contextsMoved` counts all twins. Re-pulling an old four-mode file checks both old light modes independently, so an edited Tamber value on the removed twin normally prompts before removal. The representative’s mode ID survives.

3. **Two blockers prevent a “nothing blocks” conclusion:**

   - **Blocker — conflict detection ignores `also`.** [mapping.ts:295](/tmp/rv-e4-light/tools/figma-tamber/src/mapping.ts:295) compares only the representative. Example: edit shared light blue→red; the next export makes the axis ordinary, with representative red and twin blue. `conflicts` returns nothing, and unchanged context names mean no context warning. Pull expands the modes and silently replaces the twin’s intended red with blue. Conflict detection must cover every represented context.

   - **Blocker — old four-mode collapse can silently delete personal values.** [code.ts:157](/tmp/rv-e4-light/tools/figma-tamber/src/code.ts:157) removes the redundant light mode, including that mode’s values on user-created variables. `check` inspects tracked Tamber variables, while `contextsMoved` reports no removal. With unchanged Tamber values, there is no confirmation before those personal values disappear. Mode removal needs its own protection.

The permitted tests passed: **49 passed, 0 failed**; neither blocker is covered.
