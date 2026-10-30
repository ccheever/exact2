# Code review: gallery: distinct colour samples (D4 batch 2 #5; #4 taken back), round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `d9ffe61393`.
- **Method:** one brief (sha256 `8109d7026c4a95eebd34e29f608f962a502c09bb2f5404d3e46aa9df98af9ee5`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

- **SHOULD — [contract/cli/src/gallery.rs:942](contract/cli/src/gallery.rs:942): Preserve stretching for width-dependent components.** PostCard and PersonRow have width-auto roots, so this changes them to fit-content width. PersonRow’s trailing slot no longer reaches the cell’s right edge; an empty width-auto list can collapse horizontally. Root `flex=1` controls height in this column, so it does not restore width. Explicit `width="100%"` or `align-self="stretch"` still works. The new source-string assertion does not catch this regression.

- **SHOULD — [contract/cli/src/gallery.rs:1033](contract/cli/src/gallery.rs:1033): The second foreground colour becomes invisible.** A component with two props feeding separate text colours—directly or through children—now receives `-exact-label` and `-exact-background`. The second text becomes white on the default white gallery background. Declaration order does not establish that the second colour paints over the first, as VerifiedBadge’s check does.

- **SHOULD — [contract/cli/src/gallery.rs:1045](contract/cli/src/gallery.rs:1045): Sample allocation ignores fixed variant colours.** If the first colour prop’s selected variant supplies `-exact-background`, the second placeholder still receives `-exact-background`: `rank` counts the first prop, although `args` bypasses sampling for it. A foreground/background pair becomes identical where the previous label placeholder contrasted. Allocation needs the cell’s actual colour values.

No role-admission issue found for caret, accent or press colours, or for the four names across the requested hosts. Rust/UI tests were not run in this read-only checkout.

VERDICT: findings
