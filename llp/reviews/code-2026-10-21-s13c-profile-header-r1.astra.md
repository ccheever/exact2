# Code review: @exact/cards ProfileHeader (stage 13 C piece 2) and the gallery's typed call arguments, round 1, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `80fd041776`.
- **Method:** one brief (sha256 `aa492c3b0079dd825e4ee5b8d8d759a07510731b5186bbf30af5302cb9b725e9`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** all six taken in e4a06e5ce3 except two declined with reasons (the fade's 20px, then solved in round 2; the banner shade fixed over a picture)

**VERDICT: WRONG**

1. **should-fix — [contract/cli/src/gallery/inert.rs:96](/tmp/iosfix-rv-ph1/contract/cli/src/gallery/inert.rs:96):** Five explicit arguments generate parameters `a, b, c, d, d`. The parser rejects the duplicate, and the malformed declaration remains even after cells are refused, breaking the whole gallery. Generate unique names and add a regression with five arguments alongside another valid component.

2. **should-fix — [contract/cli/src/gallery/inert.rs:182](/tmp/iosfix-rv-ph1/contract/cli/src/gallery/inert.rs:182):** List inference ignores lexical shadowing when looking up component props. If `items: NumberRow[]` is shadowed by `for items in [stringRows]`, an inner `for x in items` incorrectly types `x.id` from `NumberRow`. Compiler validation prevents unsound output, but valid components are falsely refused. Respect the existing bindings and test this case.

3. **should-fix — [packages/cards/profile-header.contract:130](/tmp/iosfix-rv-ph1/packages/cards/profile-header.contract:130):** The badge lacks the required tint params. `VerifiedBadge` reads `cards.badge` and `cards.badge-mark` directly; changing those also changes post-card badges. Expose profile badge colour params and pass them into the helper.

4. **should-fix — [packages/cards/profile-header.contract:76](/tmp/iosfix-rv-ph1/packages/cards/profile-header.contract:76):** The fade’s extra `20px` is a literal size knob, contrary to D1’s stated requirement. Put that dimension behind a package token.

5. **should-fix — [packages/cards/cards.tamber:31](/tmp/iosfix-rv-ph1/packages/cards/cards.tamber:31):** `cards.banner-shade = #00000080 scheme=fixed` gives a neutral colour a fixed literal default. The brand exception covers the blue banner, not this black fade. Use a role-backed default; the clone can retain its `black_50` override.

6. **should-fix — [scripts/stage12-gate.mjs:518](/tmp/iosfix-rv-ph1/scripts/stage12-gate.mjs:518):** The rendered-slot gate selects only `PostCard`, so every new `ProfileHeader` marker is skipped. Extend selection to all exported slotted components. The source-marker assertions do not prove that the slot remains visible.

Both enum deviations are appropriate: the four icons preserve the clone’s distinctions, and `verifier` means its trusted-verifier badge. RFC D0 itself does not prohibit supporting shape exports.

The shell’s placement, unconditional slot, empty-alert hiding, blocked/compact behavior and fixture cells check out. All eight saved capture pairs have zero differing pixels. No normal compilation path changed; builds and the 46-app identity check were not rerun. No files changed.
