# Code review: the crossing skip's Excord half (render.ts frozen rows), round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox (run by x2-perf4, the Excord half's reviewer).
- **Method:** one brief (sha256 `a13b4683aa950a6bb8a3ef17aa10a7100b9dc515a1923391eac55e6dce7d7598`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

Reviewed only `aed74f34cd`. No BLOCKER or MAJOR found. Files unchanged.

- **Freeze coverage:** Rows and all nested runs, blocks, embeds, media, files, reactions, polls, replies, stickers, and forwarded notes are frozen. Rendered authors are strings; source user and attachment objects remain unfrozen. The `{...row, isNew}` copy correctly remains unfrozen, shares frozen children, and serializes correctly.
- **Mutation audit:** No writes to rendered objects after freezing. Construction writes in [render.ts](apps/excord/render.ts:78)—run insertion/newline merging, block assembly, syntax highlighting, edited markers, and sticker additions—all precede the freeze. Subsequent updates use these separate targets:

| Operation | Write target |
|---|---|
| Edits, late embeds, send status | Replacement store records: `discord/store.ts:344,363,927,936` |
| Reactions and poll votes | Replacement records/arrays: `discord/store.ts:630,656` |
| Author/role changes, read state | Store directories/read-state map: `discord/store.ts:275,486,506` |
| Translation and spoiler reveal | Separate UI state: `app.contract:887,899,901,939,942` |
| Highlight and jump decorations | Separate UI state: `app.contract:628`; `routing.contract:182` |
| Image loading and geometry | Viewer state: `viewer.contract:49,62,68`; natural sizing uses layout |
| Renderer cache | Map insertion/removal, without modifying cached rows: `render.ts:406` |

- **Cost and frequency:** Freeze runs once per newly rendered row; cache hits bypass it. A 50-row page shift retained **249 of 299 identities**. Repeating the cached page made **zero freeze calls**. Isolated traversal measured approximately **0.09 ms per 50 mixed rows in Bun**, **0.25 ms in the available Hermes debug binary**; these are local microbenchmarks.
- **Tests and output:** All **35 renderer tests pass**. The added test recursively checks its 20 fixture rows and the unfrozen marker exception; it does not itself exercise runtime caching or compare complete output. Independent in-memory checks covered **11,690 fixture rows / 183,067 objects**, plus populated optional branches: no unfrozen descendants, input-object aliases, or JSON differences. Exercising the parent’s `keptReply` produced identical bytes and cached-row serializations of **299 → 50 → 0** across initial, shifted, and repeated windows.
- **RFC 0001:** No request changes. The broader suite recorded **565 passes**; two test files could not load because `fflate` is missing.

VERDICT: APPROVE

1. **NIT — [apps/excord/render.ts:379](apps/excord/render.ts:379):** The comment points to nonexistent `prelude.js` function `keptToken`; the implementation is `binary.js`’s `keptReply`. Also move the helper above the existing renderer JSDoc so that documentation still attaches to `createRenderer`.
