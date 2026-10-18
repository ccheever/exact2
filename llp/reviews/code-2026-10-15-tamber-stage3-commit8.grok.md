# Code review: LLP 1121 stage 3, commit 8, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `d1f51cb335565354075447ea957a1aa4e24a2e3c9461e757203624313cd32de0`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: eight high, twelve medium, three low; Grok: three high, ten medium, five low.
- **Disposition:** fixed in round 1 by a rewrite: each mode tuple resolved on its own in layer order, declarations' fates decided after assembly, the format and the resolver read as written; a gradient's reference stops stay in its report row; our own export's round trip is stage 5's

---

## High

**1. A token set by both the scheme and an axis resolves to the axis in every tuple.** `tamber/src/import/dtcg.rs:659-684`

`when` lines are emitted for every axis value, including the default. The checker treats the default tuple as that first value, so `when brand=casual` holds at rest and replaces `light-dark(...)`. If every axis value defines the path, no tuple selects the scheme expression, but those colours were still counted as mapped. `resolutionOrder` and `tokenSetOrder` never decide between the scheme layer and the axis layer; the axis always wins. Two axes are reported, as the commit says; scheme plus an axis is composed silently. The fixtures avoid this by using different paths (Figma) or aliases (Tokens Studio).

Fix: merge each permutation in resolver / `tokenSetOrder` order. The token’s default is the default permutation. Emit a `when` only for a non-default axis value. When that value replaces the scheme, report the scheme rows as superseded.

**2. An alias is mapped to a token the import never emits.** `tamber/src/import/dtcg.rs:529-535` and `:601-603`

`colour_paths` is every colour-typed path, taken before values are converted. `{bad}` becomes the sanitized name of `bad` even when `bad` failed `colour()`, was itself a broken alias, or never entered `values`. `import` returns `Ok`. `check` then fails with `tamber-unknown-token`. The shadcn importer drops a reference whose target was dropped.

Fix: after conversion and the second-axis drop, point an alias only at a path that is actually in `values`. Otherwise report it and do not map it.

**3. `$root` and `$extends` never enter the count.** `tamber/src/import/dtcg.rs:119-121`

Every key starting with `$` is skipped. That is right for `$type`, `$description`, and `$extensions`, and wrong for a 2025.10 `$root` token (`color.brand.$root` in the format spec). `$extends` and a group `$ref` (format §6.4, same merge as JSON Schema `$ref`, sibling keys override) are skipped too, so inherited tokens are neither mapped nor reported. The counted check still passes.

Fix: walk `$root` as a token whose segment is `$root`. Resolve `$extends` / group `$ref` by merging the target group first, and report a missing target or a cycle.

## Medium

**4. A `$ref` alias is reported as a bad colour.** `tamber/src/import/dtcg.rs:474-479` and `:601`

`alias()` accepts only a whole `{path}` string. Format §7.1.2 requires `{ "$ref": "#/colors/blue/$value" }` (equivalent to `{colors.blue}`) and property pointers such as `#/colors/blue/$value/components/0`. Those objects have no `hex` or `colorSpace`, so they are reported as “not a colour Tamber reads” and are not references. A real cycle is reported with that same sentence (`:606-611`); `cyclic` (`:582-598`) follows the first scope in `BTree` order (base, then light, then dark, then axes), so a dark-only cycle is imported as mutual references and a base cycle poisons a valid alias of the same path on another scope.

Fix: treat a `$value` object whose only meaningful key is `$ref` as an alias. A pointer that ends at a token’s `$value` becomes that token’s name. Walk cycles in the scope being resolved and report them as cycles. Report a property-level pointer instead of calling it a colour.

**5. One JSON file that is not `Collection.Mode` turns Light and Dark into the base.** `tamber/src/import/dtcg.rs:203-228`

Mode pairing runs only when every file’s stem contains a dot. An extra `manifest.json` makes both mode files base layers; the later path wins and the other is “declared again”. A collection name that contains dots is fine: `rsplit_once` keeps the last segment as the mode (`Foo.Bar.Light.json`). A mode name that contains a dot is not (`High.Contrast` becomes mode `Contrast`).

Fix: classify each file on its own. Pair the stems that are `Collection.Mode`. Report the others, and leave the mode files paired.

**6. An inline `resolutionOrder` entry is an empty layer.** `tamber/src/import/dtcg.rs:358-361`

No `$ref` yields the pointer `""`, and `Value::pointer("")` is the whole document, so `unwrap_or(entry)` never uses the entry. An inline set’s `sources` or an inline modifier’s `contexts` (resolver §4.1.6.1) contribute no tokens and no error. A dangling `#/missing` becomes an empty layer the same way (the spec says to throw). Keys beside `$ref` are ignored; §4.2.2 says they override the referenced tree. Inline token trees inside a set reached by `$ref` do work; `resolver.json` is that form.

Fix: if the entry has no `$ref`, use the entry and its `name`. Error when a pointer does not resolve. Merge sibling keys over the referenced tree.

**7. `hex` replaces the colour space.** `tamber/src/import/dtcg.rs:434-443`

When `hex` is present, `colorSpace` and `components` are ignored. In the 2025.10 colour object those two are the value; `hex` is a 6-digit sRGB fallback. An oklch or display-p3 token that also carries `hex` imports as sRGB. The resolver fixture does not show this: its sRGB hex matches the components (`#1a1a1a`). Appending alpha onto a 6-digit hex is fine (`#RRGGBBAA` parses). A 3-digit hex plus alpha becomes 5 digits and fails.

Fix: use `colorSpace`, `components`, and `alpha` when they are present. Use `hex` only when they are absent.

**8. A Tokens Studio group `type` is not inherited.** `tamber/src/import/dtcg.rs:99-104`

Legacy `type` is read only on an object that also has `value`. `{ "type": "color", "red": { "value": "#f00" } }` leaves `red` with no type, reported as “no `$type`”. A DTCG `$type` on a group does inherit. A token’s own `value`/`type` pair is read, which is what the lion sample uses.

Fix: if the object is a group, take a string `type` as the type children inherit.

**9. `tokenSetOrder` matches the wrong set.** `tamber/src/import/dtcg.rs:261-268`

An entry matches the full path or any set’s last segment. In this fixture `core` matches nothing (`primitive/colors`), and `button` matches `component/button` only by the last segment. Two sets that end with the same segment share one rank. Unmatched sets share `order.len()` and then follow the caller’s path sort. Later-wins is wrong as soon as two sets in one theme define the same path. This sample’s sets do not overlap, so the gate still comes out 44 + 5.

Fix: match the set id the theme writes (`primitive/colors`, or the single-file key). Report an order name that matched nothing or matched two sets.

**10. Axis names collapse, and some cannot be accepted.** `tamber/src/import/dtcg.rs:84-86` and `:638-645`

`ident` lowercases and sanitizes. `Light Mode` and `light-mode` become one value; the second layer shares that scope and overwrites on overlap, with no collision report. Two modifiers that sanitize to one name become one axis. `scheme`, `contrast`, and `color-scheme` are still emitted; `check` rejects them (`tamber/src/check.rs:275`, `tamber-axis-host`). `reconcile` (`tamber/src/import/mod.rs:110-125`) appends values onto an existing axis of the same name. Defaults themselves are right: Figma sorts mode names and takes the first, the resolver honours `default`, Tokens Studio keeps file order.

Fix: report a sanitized collision and skip that value. Refuse a host axis name. If the design already has the axis and these values are not the same axis being re-imported, report the clash.

**11. Gradient stops that are references stay inside the reported blob.** `tamber/src/import/dtcg.rs:420`

D14 says a gradient is reported and its stops become colour tokens when they are references. The importer reports one row, “no Tamber kind: a gradient”, and leaves `{color.red}` in the JSON. Borders, transitions, and stroke styles are reported as the table says.

Fix: keep the gradient in the report. For each stop colour that is an alias, map or report that reference as its own colour row so the count sees it.

**12. A set no theme selects is invisible to the count.** `tamber/src/import/dtcg.rs:293-298` and `:503`

`enabled` and `source` are included; `disabled` is not. A JSON file that no theme selects never becomes a layer, so its tokens are not in `source`. The counted check cannot see them. Filtering `disabled` out of the active merge is right; dropping them from the report is the gap.

Fix: report each token in a set that no layer included, with why “disabled” or “not selected by a theme”.

**13. A dark-only colour becomes the light value too.** `tamber/src/import/dtcg.rs:647-651`

`pair(None, Some(dark))` with no base returns that colour as the token’s only value, so light mode paints it. If a base exists, `pair(base, dark)` is a real `light-dark()`, which is the right fallback. A token that exists only in the dark set hits the first path.

Fix: keep a one-sided scheme value as `light-dark` and report the missing side, or report the token, instead of promoting the dark colour to the unscoped value.

## Low

**14. `$deprecated` is not surfaced.** `tamber/src/import/dtcg.rs:119`. The token is imported with no mark. Keep it, and put the deprecation string on the report or the token comment.

**15. The second axis’s report replaces the colour with `axis=value`.** `tamber/src/import/dtcg.rs:698-704`. Report the value that was dropped. First-axis-wins is what the commit specifies.

**16. Math and Tokens Studio `modify`.** `tamber/src/import/dtcg.rs:474`. `{spacing.sm} * 2` is not an alias. A pending length keeps that text in the report, which is enough until stage 6. A colour that is not a pure `{path}` is “not a colour”. A `modify` block next to an alias is ignored, so the unmodified reference is mapped. Report the modifier (and a math expression on a colour) as unmapped instead of mapping the bare alias.

**17. The gate tests do not lock the cases above.** `tamber/tests/import.rs:709` and `:735`. Figma asserts 104 and 40 mapped. Tokens Studio does not assert 49 = 44 + 5, though a count of the fixture files matches that split and the sample’s aliases do resolve (`button.bg` → `colors.bg.default.color` → `colors.blue.600`). Nothing covers `$ref`, `$root`, scheme-plus-axis, a cycle, a dangling alias, an extra non-mode file, an inline `resolutionOrder` entry, or an axis named `scheme`. The 64-tuple cap is implemented (`dtcg.rs:726`) and untested for DTCG.

**18. Directory reading has no cycle or size cap.** `contract/cli/src/tamber_import.rs:120-134`. Relative `.json` paths are sorted and separators normalized, which is what the importer needs. A symlink cycle does not terminate (`is_dir` follows links, and the stack has no visited set). The extension compare is case-sensitive, so `Tokens.JSON` is skipped. Track canonical directories, cap the file count, and accept `.json` in any case.

Checked and holding on this commit: DTCG `$type` inheritance; `{a.b}` aliases in the fixtures, including a target that exists only on an axis; legacy `value`/`type` on tokens; `enabled`/`source`/`disabled` as the active-set filter; single-file `$themes` versus `$themes.json` plus set files; Figma collection names that contain dots; the resolver fixture’s inline sources, `default`, and light/dark `light-dark()`; name collisions reported for token paths; a per-occurrence count that does not both map and report the same row; re-import of these three fixtures proposing nothing after accept.
