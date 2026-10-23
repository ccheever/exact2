# Code review: stage 13 B, the pen importer, round 3 re-check 2 (24f53824c2) (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `857b44d885`.
- **Method:** one brief (sha256 `e51f6ecc79fe08689b5a209b9f71eebd3ec5a2b7ae47e70a69a668e9094b07e5`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

- [dtcg.rs:1159](tamber/src/import/dtcg.rs:1159): **Long-name memory amplification remains through collisions.** Rejected names still enter `taken`; every collision clones the full name. One 256 KiB all-dash name followed by 5,000 distinct eight-character names using `!/?/^` can normalize to `x`. A constructed request is 527,304 bytes; `collided` alone retains **1,250 MiB**, without budget charging.

- [pen.rs:72](tamber/src/import/dtcg/pen.rs:72): **The allowlist omits valid bindings.** `innerRadius`, gradient-stop `position`, `fontStyle`, and `content` are examples. A number used by both `gap` and `innerRadius` incorrectly becomes a length; a string used by both `fontFamily` and `content` incorrectly becomes a family. These properties accept variables in the [Pencil schema](https://docs.pencil.dev/for-developers/the-pen-format#typescript-schema).

- [pen.rs:215](tamber/src/import/dtcg/pen.rs:215): **Theme values still influence kinds when axis names match allowlisted properties.** Declare `themes:{"opacity":["$n"]}` and a frame with `gap:"$n", theme:{"opacity":"$n"}`: the theme value becomes a non-pixel use, changing the token to a number. Theme entries are literal axis values in the [schema](https://docs.pencil.dev/for-developers/the-pen-format#typescript-schema).

**Should-fix:** None.  
**Nits:** None.

By inspection, long-name alias errors are bounded, `reaches` charges each step, and opaque-map `id` bindings now count. Tests could not run: the read-only sandbox denied Cargo’s temporary-file creation. No files changed.
