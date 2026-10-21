# Code review: Type and family tokens, and Expose converted to them (LLP 1121 stage 6 C), round 3, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** HEAD~3..HEAD, the change, Expose's conversion and the first two rounds.
- **Method:** one brief (sha256 `7704276c616e726fc2591a34ad1dc51e26009e423206125c186fa02228d1fe57`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (a P1: the runner's design shape missing type longhands, so a structural snapshot passes the live guard; P2s on quoted spaces, the node's own style in the face check, an undotted type token in `font-family`).
- **Disposition:** applied without re-review (the last round), in the third-round commit.

---

One blocking issue remains.

- **P1 — Structural snapshots pass the live-update guard.** [runner/src/design.rs:76](/tmp/rv-s6c-3/runner/src/design.rs:76) omits type longhands and families from the shape digest. Changing `type.body` from `700 1rem/1.4` to `1rem/1.4` leaves that digest unchanged. An old runner accepts the new snapshot; refreshing the removed weight fails and leaves **700** applied. Tamber’s new classification does not protect `apply_live`.

- **P2 — Quoted family names collide.** [kernel/src/style/theme/typeface.rs:204](/tmp/rv-s6c-3/kernel/src/style/theme/typeface.rs:204) collapses whitespace inside quoted names. Distinct declared aliases `"A B"` and `"A  B"` therefore share a key; a token naming the second resolves to the first stack.

- **P2 — Bundled-face checks ignore accompanying longhands.** [contract/lower/src/type_tokens.rs:146](/tmp/rv-s6c-3/contract/lower/src/type_tokens.rs:146) treats an omitted token style as normal, ignoring `FontUse.italic`. With a bundled family containing only 700 italic, `font="type.bold"` (`700 1rem`) plus `font-style="italic"` is wrongly refused. The advertised `type.emphasis` test only declares the token; it never exercises a bundled face.

- **P2 — Undotted type tokens escape the family refusal.** [contract/lower/src/type_tokens.rs:124](/tmp/rv-s6c-3/contract/lower/src/type_tokens.rs:124) returns “not a token” for other kinds. Given `heading = 700 1rem Georgia`, `font-family="heading"` consequently compiles as an installed family named **heading**, instead of reporting `tamber-kind`.

Other round-1/2 fixes look intact by inspection. Expose’s two files exactly reproduce their original contents after substituting token values. Two JS family tests passed; Rust and host execution were unavailable under the read-only sandbox. Worktree and index remain unchanged.
