# Code review: `tamber gen`, the TS Token union, Rust constants (Tamber stage 5, commit 4), round 1, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** HEAD, the change.
- **Method:** one brief (sha256 `a10690d4b9aa3e0a90f6a87f08042701fc1895cb581e3b067b000ecc497118d4`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the CLI's output, unedited (its progress lines run into the first paragraph).
- **Verdict:** as transcribed below.
- **Disposition:** in the first-round commit.

---

I'll review HEAD read-only: the stage 5 brief, the retention ruling, and the `tamber gen` commit, without changing the worktree.The commit message and D7/D12 are in hand. Next I’ll read the generator, the retention filter, and the tests against those rules.Auto-compacting conversation (81% full)...
Conversation compacted.
The review is read-only. I'll check the skill's report format and confirm the retention and name-mapping lines against the current sources before writing findings.The generator and the retention predicate are in place. Next I’ll check how role names are stored versus how `get` looks them up, and whether the tests actually cover that path.No P0. Two P1s: a generated role constant does not resolve, and a runner-owned source keeps tokens the app cannot name.

[P1] Emit the plan's author spelling in generated constants — `contract/cli/src/tamber_gen.rs:101`

`tokens()` copies `Checked.name`. For a role override the checker sets that to the row spelling (`tamber/src/check.rs:126`), while the plan stores the spelling the author wrote (`contract/lower/src/theme.rs:240`) and `Tamber::has` matches only that (`runner/src/runner/tamber.rs:106`). `secondary-label = #112233` generates `Token::new("-exact-secondary-label")` and a TS `name` of `"-exact-secondary-label"`, but the plan key is `secondary-label`, so `get` and `color` return `None`. The same split hits `canvastext` (constant `"CanvasText"`, plan `"canvastext"`). `contract types` builds `ColorToken` from the plan key, so the two unions disagree. Writing the row spelling in the design (`-exact-secondary-label`, `CanvasText`) happens to match, which is all the golden covers.

[P1] Do not treat a runner-owned source as a data module — `contract/lower/src/lib.rs:1137`

`names_tokens_outside_views` is true whenever `root.sources` is non-empty. `record_source` inserts `exactViewport`, `exactTime`, `exactPage`, `exactSurface`, `exactDelivery`, and `exactTransport` (`contract/types/src/lib.rs:536`). An app whose only source is `resource viewport = exactViewport()`, with no `app.ts` call and no native tag, keeps every token (or its `exports`). The web theme map emits every plan row (`host/web-js/src/style.rs:432`), so an unused `badge.ink` shows up in the release plan and JS. Theme-fixture has no `resource`, so the byte-identical claim can still hold.

[P2] Reject CSS custom properties that collide or are not idents — `contract/cli/src/tamber_gen.rs:210`

The property pass turns every non-ASCII-alphanumeric character except `-` into `-`, then strips leading hyphens. `_ink` and `ink` both become `--ink` (the later rule wins) while camel stays `_ink` / `ink` and screaming stays `_INK` / `INK`, so `admitted()` allows both. `_2foo` is a legal token name and becomes `--2foo`, which is not a dashed ident. `_` becomes `--` and `pub const _`, which Rust rejects.

[P2] Refuse the TypeScript bindings a module cannot declare — `contract/cli/src/tamber_gen.rs:254`

The reserved list stops at sloppy-mode keywords. A token named `await`, `public`, `implements`, `interface`, `package`, `private`, `protected`, `eval`, or `arguments` is emitted as `export const …` in a module, which is a syntax error. `tokens` and `tuples` are also legal token names and duplicate the generated `export const tuples` and `export const tokens` (`tamber_gen.rs:166`, `tamber_gen.rs:180`).

[P2] Keep design tokens when a data module exists but nothing calls it yet — `contract/lower/src/lib.rs:1137`

The predicate is "a source is already recorded," not "a module can name a token." An `app.ts` (or a data crate) that returns `(): ColorToken => "badge.ink"` before any `resource`, `send`, or `pages` source leaves `sources` empty, so an unused token is pruned and `ColorToken` omits a name the design declares. `tsc` then fails that return. A `send` does record its source. `~/x2voice/to-x2-perf4.md` and the stage-5 brief were not on disk; the commit text defines the test as the seam's sources, so this is the gap between that proxy and D7's "can name."

[P3] Accept the role export spellings the checker already accepts — `contract/cli/src/tamber_gen.rs:91`

`exports()` accepts any ASCII case of a role (`tamber/src/check.rs:933`). The generator keeps a token only when the export equals `Checked.name` (the row) or `Checked.role` (the schema name) exactly. `exports canvastext` with a `CanvasText` token, or `exports Secondary-Label` with `secondary-label`, checks cleanly and then drops the constant. The plan still keeps the role, so `color("CanvasText")` can work while the generated name is absent.

A design of ordinary dotted names, with no bare or differently cased role spellings and no runner-owned source as its only source, generates stable constants that resolve. Tuple order matches the checker's digits (first axis slowest, defaults included). Unicode cannot appear in a name. A native tag in a non-root component or in `when` / `each` / `match` is walked (`contract/lower/src/native.rs:171`). A `fn` body cannot hold a tag. A child `resource` is `type-child-resource`. CLI parsing, the default directory beside the design file, the unchanged-byte skip, and `exact_runner::tamber::{Token, Color, Kind}` match the generated Rust.

The tests do not cover these failures. `the_rust_constants_resolve_through_the_binding_core` uses only `brand`, `surface.card`, and schema-case `CanvasText`. `exports_bound_the_generated_names` (`contract/cli/tests/it/tamber_gen.rs:169`) says every constant resolves and only checks that the source contains `BADGE_INK`. `tamber_binding.rs` never uses a runner-owned source, a bare role, or an `app.ts` with zero sources. The `tsc` check is `#[ignore]`.
