# Code review: LLP 1122 T2b, the JS target, step 2 (the inspect table and row keys), round 1, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `bd83cb81f`.
- **Method:** one brief (sha256 `1be2a7b0e86d243b60dbb36766c5de46b074141f92911948f323ebe377575165`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole output, unedited.
- **Verdict:** request changes (1 high, 2 medium, 1 low).
- **Disposition:** taken. A literal `none` is not own (`vm::is_literal`), and a one-sided class ternary is own only while set (the page checks). The registry leaves with its scope and skips a region still being built. Relative lengths are the static tree's, per tuple (limit: a pinned root). A boundary's implied rows are the runner's eight, `symbol_palette` included, listed in the table (`implied`).

---

I'll review HEAD as a read-only pass: the inspect table, the dev-only row keys, and whether release output stays byte-identical. First I'll read the commit and the runner paths it has to match.The commit adds a dev-only inspect table and row keys. I'll read the new table, the runner's `node` op, and the runtime path that has to stay identical in release.The table is built from the static tree, while `node` reads the live kernel. I'll compare those two paths on classes, headings, tokens, and instance chains.Auto-compacting conversation (81% full)...
Conversation compacted.
The review is still open on three checks: relative lengths versus `node.computed`, release byte identity, and the `each` registry lifetime. I'll close those from the code, then write the findings.The inspect table matches `node` on the theme fixture's boot tuple, and a non-dev `rt.js` is byte-identical to `HEAD~1`. A literal `none` is still reported as a dynamic own row, and relative lengths are provisional pixels, so the table does not yet say what `node` says of every view.

## High

**A literal `none` is reported as a dynamic own row — `host/web-js/src/inspect.rs:79`**

`color=none` (and a `style` row that is `none`, copied in by `class=Muted`) lowers to `Opcode::None` + `Return`. `vm::is_literal` is true for that, so `node`'s dynamic predicate is false. The runner then `ClearStyle`s it with `written: false`, which takes the row out of the mask (`kernel/src/shared_style.rs:215`). `node` reports it as inherited or initial. `style::literal` (`host/web-js/src/style.rs:52`) accepts only `Number` / `Bool` / `Str`, so `inspect.rs:80` sets `dynamic` and, because the mask check is skipped when `dynamic` is set, writes `{d: true, v: null}` into `r`. The static tree never applies this binding (`style.rs:185`), which is why the mask is empty. A one-sided class ternary (`cond ? "#fff" : none`, `contract/lower/src/class.rs:70`) is a separate case: both sides correctly call it dynamic, but the table lists it for every view, including a view whose live arm cleared it.

Fix: use `vm::is_literal`, the same predicate as `node`. A literal `none` then stays out of `r`. Do not teach `style::literal` to return `Some(None)` without a matching clear arm in `static_tree`, or `project` will start calling `set_binding_style` on it and release CSS changes. For the ternary, record that the binding can clear, and do not treat `r` as own for a view whose used value is the clear.

## Medium

**Disposed `each` regions stay in `Eaches`, and a disconnected one is dropped for good — `host/web-js/rt.js:1215`**

`each` registers at `rt.js:1231` and never calls `onEnd`. `dispose` only runs `onEnd` callbacks (`rt.js:42`). After a parent `when` clears, or `list.js` `retire` removes a row, the anchor is detached but the `Set` entry's closure still holds `order` (row nodes, scopes, signals) until some later `$exactRowOf`. A dev session that never opens inspect keeps every destroyed list. The same `live()` test deletes the entry whenever `a.isConnected` is false, including while `build` (`rt.js:1176`) is still filling a fragment, before `b.before(frag)`. That call never happens again, so the region stays invisible once it is inserted. Nested regions, bracketed ranges, adopted and lazy rows, and moved rows are matched correctly: the region id filters, a bracket is "after start and before end", and `r.raw` is the key the runner stores. While a region is alive the extra cost is one field and one `Set` entry; the retained detached graphs are the measurable cost. `$exactRowOf` walks every region and row only when inspect calls it.

Fix: `onEnd(() => Eaches.delete(entry))`, and do not delete merely because the anchor is not connected yet.

**Literal `rem` / `em` / text-style / hairline values are provisional pixels — `host/web-js/src/inspect.rs:102`**

For a literal binding the table builds a fresh `StyleProps` and prints `get()`. `set_dynamic` stores the provisional length at root 16, parent 16, and scale 1 (`kernel/src/style/relative.rs:410`) and keeps the unit beside the row. `node` prints `computed` after the commit resolves `rem` against the live root, `em` against the parent font size, a text style against that root, and a hairline against the display scale (`kernel/src/txn/relative.rs:109`). The web host does change the root (`host/web/src/abi.rs:721`), and the dev loop pins `rootFontSize`. `text "A" font-size="2em"` inside `stack font-size="20px"` is `32px` in the table and `40px` in `node`, at the default root. `apps/theme-fixture` has no relative length, so the test never compares them.

Fix: read the value from the committed static-tree node when the reference is the tree's own parent and root, and store the unit and factor (`1.5rem`, `2em`, the text-style id, hairline) so a later root or scale can be applied the way the commit does. Provisional `16px` pixels cannot track `set_root_font_size`.

## Low

**A boundary's implied rows are only a flag — `host/web-js/src/inspect.rs:155`**

`b: true` matches `theme::boundary` (a `mode` or `color-scheme` binding). The eight rows the runner writes (`runner/src/instance/theme.rs:13`, including `SymbolPalette`) are omitted on purpose, and the test requires them to be absent. The only list the JS target already has, `modes.rs` `CHANNELS`, is seven and drops `SymbolPalette`. A page that walks that list will not report the implied `symbol-palette` row `node` reports.

Fix: put the runner's eight names on the boundary entry.

## The rest

Type, arm indexes, and tuple numbering match. Tuple `0` is the first axis, most significant, the same fold as `modes_of` and as `pinned`. Token names and text for a literal follow `row_token`'s re-parse; names do not vary by tuple. `heading_implies` is the kernel predicate (mask set, not written, role `heading`). A headline's weight stays a style-source site index in `s`, which is what `node` calls `source: style`; the platform number is not in the table. `i` is `computed()` after `browser_kernel()` forces the Neutral column and installs no host control fonts, so it matches the web wasm frame. The test calls `table()` before `Runner::boot`, which sets that column first, so a Mac column cannot fail it. Virtualized rows stay `[region, null]` in the chain and are not registered; `data-listitemkey` is `list.js` `keyText` (`n:18`, `d1:n:18`), which decodes to the raw value `node` prints. `$vl` is unchanged.

`project` is the old kernel build cut into `static_tree` and called once; the folding loop is untouched. `table()` builds the tree again after `emit`, so `app.js` does not change. The region id is appended only when `dev_reload` (`regions.rs:167`). `withoutDev(rt.js)` is byte-identical to `HEAD~1:host/web-js/rt.js` (117314 bytes). `build.mjs` strips `rt.js` whenever `--dev` is absent, including `--production`, writes `dev-inspect.json` only in the `dev_reload` branch, and copies it only then, after wiping `dist`. It is not inlined.

The tests check the theme fixture and one `for`/`if` contract at the boot tuple: type, chain without keys, own-row flags, non-dynamic values, tokens, style-source site, initials, and the boundary flag. They skip dynamic values and implied-row values. They never switch modes, never use `none`, `rem`/`em`, or a non-Neutral column, and they never build the registry or compare a release bundle to `HEAD~1`.

**Verdict:** Release bytes hold, and the fixture's boot tuple matches, but literal `none` and unresolved relative lengths will make step 3 disagree with `node`.

