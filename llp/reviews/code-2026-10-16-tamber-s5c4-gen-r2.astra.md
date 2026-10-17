# Code review: `tamber gen`, the TS Token union, Rust constants (Tamber stage 5, commit 4), round 2, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** HEAD, the first round over the change.
- **Method:** one brief (sha256 `4f710583304f4f26098af3b299d8368291632ece7875718bf0fab4df99c7ed99`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (`contract types` and baked web builds ignored exports; `__proto__`).
- **Disposition:** in the second-round commit.

---

Three **P2 blockers** remain:

- **`contract types` still ignores `exports`.** [typescript.rs:15](/tmp/rv-s5c4-2/contract/cli/src/typescript.rs:15): the CLI still calls this unfiltered wrapper. With only `badge.ink` exported and private `brand` retained for a view, `ColorToken` admits `"brand"`. `types --check` also disagrees with declarations refreshed by `contract build`. The new test exercises only the filtered helper.

- **Baked web builds also bypass `exports`.** [main.rs:287](/tmp/rv-s5c4-2/host/web-js/src/main.rs:287): `.plan` input explicitly receives no export filter. Production’s `--plan` path therefore regenerates the broader union, accepting private tokens that native builds reject.

- **`__proto__` becomes the object’s prototype.** [tamber_gen.rs:198](/tmp/rv-s5c4-2/contract/cli/src/tamber_gen.rs:198): this valid token name generates a prototype setter instead of an own property. `Object.keys(tokens)`, serialization, and object spreads silently omit it. Emit computed property keys.

The other round 1 fixes look correct on inspection. In-memory TypeScript/JavaScript checks confirmed the reported language behavior; Rust tests were not run because the sandbox forbids temporary/build writes. Worktree and index remain unchanged.
