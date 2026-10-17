# Code review: `tamber gen`, the TS Token union, Rust constants (Tamber stage 5, commit 4), round 2, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** HEAD, the first round over the change.
- **Method:** one brief (sha256 `4f710583304f4f26098af3b299d8368291632ece7875718bf0fab4df99c7ed99`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the CLI's output, unedited (its progress lines run into the first paragraph).
- **Verdict:** as transcribed below.
- **Disposition:** in the second-round commit.

---

I'll review the round-2 fixes against the stage 5 brief and the retention ruling, read-only, and check whether anything in that commit range is still blocking.The review target is the round-2 delta on commit 4. I'll read that diff and the retention ruling, then check each round-1 fix against the code.The diff is in. Next I’ll check role naming, retention, and the CSS fallback against the code those fixes call.Auto-compacting conversation (80% full)...
Conversation compacted.
The remaining open questions are tint-role resolution and whether the `contract types` path still ignores `exports`. I'll confirm those against the current code before reporting.Round 2 fixes the ordinary role constants, design-file retention (`app.ts`, `data/Cargo.toml`, modules and hatches, and runner-owned sources no longer count), declaration clashes, CSS names, standalone Exact colours, and the output directory. Two issues are still blocking.

## P1 — `ColorToken` still ignores `exports` on the paths that typecheck `app.ts`

`typescript_exporting` filters the union, and `contract build`, the native TypeScript bake, and a web build of `app.contract` call it. The command the data module is checked with does not.

`contract types` still generates with no export list:

```72:72:contract/cli/src/main.rs
        Some("types") => types(&args[1..], "types", "app.d.ts", contract::typescript),
```

```14:15:contract/cli/src/typescript.rs
pub fn typescript(plan: &Plan) -> Result<String, String> {
    typescript_exporting(plan, None)
```

`contract/cli/src/tamber_gen.rs:9` says that union is what `contract types` writes. After a successful `contract build` of `app.contract`, `scripts/exact.mjs:544` calls `refreshTypes`, which runs `types` and overwrites `app.contract.d.ts` (`scripts/exact.mjs:553-564`). `build.rs:137-139` may have just written the filtered file; this replaces it. The test in `contract/cli/tests/it/tamber_binding.rs:381` calls `typescript_exporting` directly, so it stays green.

Failure: `app.ts` exists, the design has `exports { badge.ink }`, and a view uses `brand` (a `CanvasText` override is kept too). The plan keeps `brand`, `badge.ink`, and `CanvasText`. `bun exact.mjs contract build app.contract` writes a `ColorToken` that includes `"brand"`. A data module returning `"brand"` passes `tsc`. `tamber gen` omits `brand`.

The same wider union is what a production web build typechecks. Production requires `--plan` (`host/web-js/build.mjs:60`), and `host/web-js/src/main.rs:287-289` skips `design_exports` for any `.plan`. `design_exports` would not find the design from `dist/app.plan` anyway (`sources.rs:137-146` only resolves a design from `app.contract` or from `app.json` beside that file). `build.mjs:222-226` then runs `tsc` against that file.

## P2 — a bare tint role’s generated constant resolves to nothing

`row_name` finds the canonical spelling, then returns the name as written:

```113:124:runner/src/runner/tamber.rs
    fn row_name(&self, name: &str) -> Option<String> {
        let plan = &self.runner.plan;
        let row = plan
            .theme
            .iter()
            .find(|t| plan.str(t.name) == name)
            .or_else(|| {
                plan.theme
                    .iter()
                    .find(|t| !plan.str(t.role).is_empty() && plan.str(t.role) == name)
            })?;
        Some(plan.str(row.name).to_string())
    }
```

Tint rows are omitted from the theme table (`runner/src/bridge.rs:406-411`). Non-system tints are `link` and `symbol` (`kernel/src/style/roles.rs:993-1007`: iOS `@tint` at full strength). A bare Exact name is not a colour (`roles.rs:17`).

Failure: the design says `link = #ff00ff`. The generator emits `Token::new("-exact-link")` (`check.rs:126-128` stores the row spelling). `get` rewrites that to `"link"`. The theme table has no row, `roles::role("link")` is `None`, and `parse_light_dark` fails (`kernel/src/style.rs:797-805`), so `color` returns `None` and journals that the value is no colour. `secondary-label` and `CanvasText` are unaffected: they stay in the table, and the roles test only covers those. `AccentColor`, `LinkText`, and `ButtonText` still parse as roles in any case, so they return the role rather than `None`.
