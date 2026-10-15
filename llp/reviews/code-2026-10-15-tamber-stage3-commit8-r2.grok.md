# Code review: LLP 1121 stage 3, commit 8, round 2, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `da314db09820144a403716ee5d6e06c3c05e6dbcc957797bcb410ba54ab15d43`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: thirteen medium, one low; Grok: three high, three medium.
- **Disposition:** fixed in round 2

---

## High

### 1. A spec `$ref` alias is dropped, and the count never sees it
`tamber/src/import/dtcg.rs:157` and `:199`

Format 2025.10’s required alias is the token object itself:

```json
"alias": { "$ref": "#/base" }
```

and `{ "$ref": "#/colors/blue/$value", "$type": "color" }`. `tokens()` only treats an object as a token when it has `$value` or a legacy `value`. A bare `$ref` is read as a group extend. `group()` then lands on the target token, every `$…` key (including `$value`) is skipped, and the alias emits no `Def`. It is neither mapped nor reported, so `counted` still balances.

The form the tests use, `$value: { "$ref": "#/…/$value" }`, is preserved. Group `$ref` / `$extends` that point at a real group and have children must stay inheritance.

**Fix:** if `$ref` points at a token, at a `$value`, or at a non-group, record a token whose value is that `$ref` and whose type is the sibling `$type`. Leave a `$ref` that points at a group as inheritance.

### 2. Axis names and mode names that sanitize together are merged
`tamber/src/import/dtcg.rs:115`, `:315`, `:410`, `:767`

`ident()` lowercases and turns every non-alphanumeric run into one `-`. Distinct Figma collections (`Brand` / `brand`, `My Brand` / `my-brand`) and Tokens Studio groups (including `""` and a group literally named `theme`, both becoming `theme`) become one axis. `axes` then unions their values. Two independent dimensions become one list, so a tuple can no longer combine them, and `active()` turns on every layer of that collapsed value at once.

Two themes or modes that sanitize to the same value (`Compact` / `compact`, `Light Mode` / `light-mode`) hit `Some(_) => {}` and the second value is discarded. Both layers stay. With only one value left, every tuple activates both, the later layer wins, and no `when` is emitted. The mode is gone and the count still balances. Token-path collisions are reported; these are not.

**Fix:** when two raw group, collection, theme, or mode names `ident()` to one string, refuse with `tamber-import-axis` and name both sources. Do not union values or drop the later mode.

### 3. Tokens Studio applies sets per theme layer, not in set-list order
`tamber/src/import/dtcg.rs:386`–`:425`

Each theme becomes one layer: all of its `source` sets, then its `enabled` sets, each by `tokenSetOrder`. Layers are then ordered “base group, then `$themes` order”, and the later layer wins.

Tokens Studio’s rule is that the active themes’ sets are one list, and the lowest set in `tokenSetOrder` wins. Two consequences:

- A set early in the list that sits on a later group beats a set lower in the list on an earlier group. Forcing `source` before `enabled` does the same inside one theme when a source set is deliberately below an enabled set.
- The same set enabled on two themes (the documented way to keep primitives in every option) is parsed once per theme. On two groups that are active together, the earlier copy never wins a tuple and is reported as “declared again by a later layer”, and `source` counts both copies. On light and dark, both copies win, the colours happen to match, and the count is doubled. `counted` still adds up.

The lion fixture never shares a path across groups, so the gate stays green.

**Fix:** parse each set once. For a tuple, take the union of the sets selected by the active themes and apply that list in `tokenSetOrder` (unlisted sets after the listed ones, in file order). Stop reordering `source` above `enabled`, and stop stacking whole themes as layers.

## Medium

### 4. Token `$ref` pointers are not unescaped
`tamber/src/import/dtcg.rs:662`

`alias()` splits the pointer on `/` and joins with `.`. It never turns `~1` into `/` or `~0` into `~`. Names may contain both (`/` and `~` are legal; `.` `{` `}` are not). `#/group/a~1b/$value` looks up `group.a~1b` while the def path is `group.a/b`, so the alias is reported as missing. `group()` and resolver `deref` go through `Value::pointer`, which does unescape, so `$extends` and resolver refs of the same name work.

**Fix:** unescape each segment before joining, the same way `serde_json` does.

### 5. `$extends` replaces a child group instead of merging it, and a cycle can hang
`tamber/src/import/dtcg.rs:203` and `:211`

Local keys replace inherited keys wholesale. A local `background.default` throws away an inherited `background.hover`. The spec copies the target, then overrides at the same path, so `hover` stays. Those inherited tokens are never defs, so they are not reported.

The walk also has no cycle set. Depth 32 is per path. This document branches by two at every level and does not finish:

```json
{ "a": { "$extends": "{a}",
  "p": { "$extends": "{a}" },
  "q": { "$extends": "{a}" } } }
```

The spec requires a cycle to be reported.

**Fix:** merge child groups recursively, with the local leaf winning. Remember groups already on the extend stack; on re-entry, emit a diagnostic and stop.

### 6. A symlink to a `.json` file is skipped
`contract/cli/src/tamber_import.rs:127`

`file_type()` does not follow links, and only `is_dir` / `is_file` are kept. A symlink to a token file is neither, so it is omitted with no error and never enters the count. Directory links stay unfollowed, which avoids a cycle. `Path::is_dir` on the root does follow a link, so only nested file links are dropped.

**Fix:** if `file_type()` is a symlink, use `metadata()` on the path. Include the target when it is a file; still do not recurse into a directory link.

## Checked, no finding

**When-lines** (`dtcg.rs:952`). Conditions are the axes whose value is not `vs[0]`, which is the value Tamber treats as the default (resolver `default` is sorted first; Figma modes are sorted by name before that, matching the comment and the Brutal/Modern fixture). Combos run from fewer differences to more. A line is added only when the most specific already-held line is missing, ambiguous, or has the wrong value. A tie adds the more specific line, including when the two values are equal, which is what `select` in `tamber/src/check.rs:592` requires. Scheme stays inside `light-dark()`; dark tuples are paired with the light combo, not given their own conditions. Multi-axis corners and three-value axes follow from that.

**Aliases across tuples** (`dtcg.rs:710` and `:914`). The emitted text is the target token’s name whenever that token is made. Tamber resolves a reference in the current tuple, so a target whose colour changes per tuple stays faithful without a `when` on the alias. A target that differs by tuple is a different name or a literal, and the when pass sees that string. A target that is not made is replaced by the colour `seen` already resolved in that tuple. `light-dark(ref, other)` takes the referenced token’s light side and the other’s dark side; imported whens do not use `under scheme=dark`, so the value string does not change with scheme.

**Counting** (`dtcg.rs:1017`). A def that wins only tuples of a path that was not made is absent from `values` and is reported, not mapped. Unused sets are not layered and are reported first. Every def that `tokens()` actually emits hits one branch. The holes are the ones above: `$ref` aliases that never become defs, and one Tokens Studio set copied into several themes.

**Figma order** (`dtcg.rs:311` and `:325`). Modes are ordered by name, the first is the default, Light/Dark is the scheme, a single mode is base, and layers run base, then scheme, then axes. That matches the fixture (`brutal-theme | modern-theme`, light-dark text tokens).

**Tuple cap** (`dtcg.rs:798`). The product of the other design axes and the new axis sizes uses `checked_mul` and refuses past `MAX_TUPLES` (64) before the cross product is built. Scheme is not an axis, so the walk is at most twice that. The when pass stays small at that bound. The unbounded walk is the `$extends` fan-out in finding 5.

Resolver `deref` (`dtcg.rs:440`) matches the resolver spec: follow `$ref`, then shallow-replace sibling keys, including objects and arrays. No finding there.
