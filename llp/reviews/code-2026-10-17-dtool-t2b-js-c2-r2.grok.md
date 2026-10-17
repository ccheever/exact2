# Code review: LLP 1122 T2b, the JS target, step 2 (the inspect table and row keys), round 2, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `a6855fe7d`.
- **Method:** one brief (sha256 `1df2a917e0e11f486e868f9c6f28dc35c7834ab4e3d985ec914e73ff5cf47f2e`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole output, unedited.
- **Verdict:** request changes (2 medium, 1 low).
- **Disposition:** taken. A literal `em` beside a dynamic font-size, and a hairline at any display scale, are read from the element (`u`). The mixed-text expression token limit is in the commit's limits.

---

I'll review this commit read-only: the diff, round 1's records, and whether the inspect table matches what the runner's `node` op actually reports.Round 1 left written dispositions. I'll read those next to the inspect table, the runner's `node` path, and the dev-only strip.The table and the round-1 fixes are in view. Next I’ll compare them to `node`, `row_token`, and the static tree the values come from.Auto-compacting conversation (81% full)...
Conversation compacted.
The remaining check is whether a non-dev strip of `rt.js` is byte-identical to the parent, and whether the relative-length and hairline claims still match the code. I'll confirm those against the current tree, then write the review.Round 1's taken fixes hold, and a non-dev `rt.js` matches its parent byte for byte. Two value paths still disagree with `node`.

## High

**A literal `em` is baked against a font-size the static tree dropped — `host/web-js/src/inspect.rs:126`**

`static_tree` applies a style binding only when `style::literal` returns a value (`host/web-js/src/style.rs:185`). An expression is omitted, so the kernels built at `inspect.rs:36` have no dynamic `font-size`. Resolution then multiplies `em` by whatever font is left (`kernel/src/txn/relative.rs:103`): the parent's computed size, or 16 px when that size was the expression. `node` multiplies by the live size.

`column font-size={n}` with `n` equal to 20, and a child `padding-left="1em"`, is 16 px in the table and 20 px in `node`. The same split happens on one node, `text font-size={n} padding-left="1em"`: the commit resolves the padding against the element's own resolved size once `font-size` is in the mask, and the table resolves it against the parent. The new test is `font-size=20 padding-left="1em"`, which is the literal case this tree does get right. The named limit (16 px root, a pinned root not applied) covers `rem` and a text style. It does not cover this. A literal varying font-size token cannot reach here: the compiler refuses it (`tamber-inherited-axis`).

Fix: keep the unit and the factor for a relative row, and which font an `em` refers to, and let the page multiply by the live font-size. Bake pixels only when that font is a literal in the same tree.

## Medium

**A hairline is baked at display scale 1 — `host/web-js/src/inspect.rs:36`**

Those kernels never call `set_display_scale`. A hairline is `1/scale` (`kernel/src/style/relative.rs:62`), and the scale stays 1 (`kernel/src/txn/relative.rs:72`). The web host sets it from `devicePixelRatio` (`host/web/src/abi.rs:731`, `host/web/navigation.js:936`).

`border-width="-exact-hairline"` on a 2× display is 1 px in the table and 0.5 px in `node`. The 16 px limit does not mention scale.

Fix: store the hairline factor and let the page apply the scale, or name scale 1 next to the root-font limit.

## Low

**An expression that mixes a token with other text has no `row_token` text — `host/web-js/src/inspect.rs:151`**

A dynamic row leaves `values` and `names` empty, so `k` and `x` are null (`inspect.rs:175`). `props` maps one custom property back to one token name, which recovers a lone `var(--x-…)`. It cannot recover the text `row_token` recorded when the expression mixes that token with other text. The round-1 disposition states this limit. The commit message's Limits line does not.

Fix: put that sentence in the limits, or keep the evaluated text for a development build.

## What holds

Literal `none` stays out of `r`: `vm::is_literal` is true, `style::literal` does not apply it, and the empty mask matches `node` after the clear. Widening `style::literal` to `Some(None)` would need a clear arm in `static_tree` or release CSS would change. A one-sided class ternary stays `d: true` for every view; the page treats it as own only while the element has it set. That matches the disposition.

A control start is recorded in `i` when an ancestor mask stops the walk, including when the start's value equals the global initial. The button under `text-transform="uppercase"` covers that. `b` matches a `mode` or `color-scheme` binding, and `implied` is the runner's eight colour rows, `symbol-palette` included. A literal heading's implied size and weight are `h`. A text style or heading chosen by an expression is still absent from `s`, which is the limit the commit names.

The registry does not leak or mis-find. `each` runs inside `scope` (`rt.js:124`, mount at `rt.js:1433`), `onEnd` (`rt.js:44`) runs from `dispose` (`rt.js:42`), and `live` only skips a disconnected region. Effect re-runs dispose children and do not register again. Inlined components give nested `each`s distinct region ids; sibling instances of one id do not overlap. Bracketed rows use document order, and adopted, lazy, and moved rows share the runtime's `start` and `end`. `list.js` `retire` ends the row scope (`list.js:444`), so an `each` inside a virtualized row is deleted. Region id 0 is registered (`arguments[5] !== undefined`). Cost while alive is one `Set` entry, one `raw` field, and one `onEnd`; `$exactRowOf` scans only when inspect calls it. Virtualized rows stay `[region, null]` and keep their key on `data-listitemkey` (`list.js` `keyText`).

`project` is the previous kernel build moved into `static_tree` and called once; the folding loop after that call is unchanged. `table()` runs after `emit`, and only under `dev_reload` (`main.rs:283`), so it cannot change `app.js`. The region id is appended only when `dev_reload` (`regions.rs:167`).

Release bytes: `withoutDev` on the current `rt.js` equals `HEAD~1:host/web-js/rt.js` (117278 bytes, four dev-only blocks removed). `build.mjs:177` strips `rt.js` whenever `devReload` is false, which is both `--production` and a plain non-dev build, after the copy into `gen` and before the bundle. `dev-inspect.json` is removed from the output and copied only for `--dev` (`build.mjs:698`). The table is a separate file. I did not compile a bundle; `app.js` stays the same because emit never sees the table and the only emit change is that dev-only argument.

The Rust tests prove, at the boot tuple of the theme fixture, a `for`/`if` contract, and the literal-em plus button contract: type, arm chain with keys dropped, own-row flags, non-dynamic values, token names and text, style-source site, control-start initials, and the boundary flag. Tuple numbering matches `modes_of` (first axis most significant). They do not prove a mode switch, a dynamic font-size beside `em`, a hairline, an off class, the registry, or byte equality. `inspect.test.mjs` checks that the stripped file is a subsequence of the source and contains none of the dev words. The "every `r` row is currently own" assertion holds for these fixtures; a fixture with an unset dynamic row would fail it, so it does not encode the page-decides contract.

**Verdict:** Request changes — the taken round-1 fixes and the release `rt.js` hold, but a literal `em` beside a dynamic font-size, and a hairline at a non-1 display scale, still disagree with `node`.

