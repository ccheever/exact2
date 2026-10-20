# Code review: Stage 11 C2: the terminal held to its column (exact-terminal layout and plan), round 1, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `d7b83957df`.
- **Method:** one brief (sha256 `ec68f0fd4097918f4f96c93bf31d127aec5dcafe38062a68895ff5e91c2301cb`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking; two P2s (the same step split and percentage radii)
- **Disposition:** taken in 6b9d8def04

## Findings

1. **[P2] Keep a step’s text as one argument — `scripts/scale-check.mjs:322`**
   The terminal drive splits every step on whitespace before it becomes argv. `type`, `paste`, and `until` each take their text as one argument (`host/terminal/src/cli.rs:412`, `:436`, `:469`), and the agent’s step form is `type <target> <text…>` with the rest of the line as the text. `type field hello world` becomes `type`, `field`, `hello`, then an unknown operation `world`, and the drive exits. This expect file’s only steps are `tap flip`, so this run’s taps stay whole.

2. **[P2] Resolve a percent or calc radius against the box — `host/terminal/src/cli.rs:176`**
   `layout` prints a corner when `Dimension::resolve` yields `Points` and prints `0` for every other kind. `resolve` leaves `Percent` and `Calc` unchanged. `at_basis` is the call a host uses to size a radius (a percent of the basis, a calc, auto as 0), and the Linux border painter calls it (`host/linux/src/paint/border.rs:20`). The schema marks the four `border_radius_*` rows `terminal: quantize`, so a percentage is admitted on a terminal entry. A `50%` corner is therefore reported as a square corner. This scale’s terminal radii are the numeric 0, so the 20 passing checks stay as they are. The painter’s round-corner test (`host/terminal/src/paint.rs:400`) uses the same Points-only gate; that code is older than this commit, and the new JSON repeats it.

Nothing is blocking. The recorded 20/21 on the terminal is what this commit’s checks produce, and the one failure is the `even` assertion the ruling asked to see.

## Assessment

Reviewed `d7b83957` only, from the sources. No build and no test run.

`layout`’s walk matches `tree()` for boxes. Roots come from `kernel.roots()` in attach order; the stack is reversed so they pop in that order; children are pushed reversed and pop left to right. `display: none` and inline runs are skipped together with their descendants. A `Text` node’s children are not pushed. `tree()` does the same for boxes, then additionally prints named inline runs under a paragraph. `layout` omits those runs.

The JSON for finite numbers is valid. `format!("{n}")` is Rust `{}` on an `f64`. This repo’s `num` printer documents that form as plain decimal (`Layout::Decimal` in `num/src/text.rs`), including a value such as `1e21`, so the line does not switch to exponent text. Negative zero prints as `-0`, which is a JSON number. `NaN`, `inf`, and `-inf` are not JSON; `JSON.parse` throws and the drive fails closed. `json_string` escapes quotes, backslashes, and every code point below U+0020, and writes other scalars raw, so a Unicode `testId` stays inside one JSON string. A line split on `\n` does not cut it.

`plan` prints `inspection_digest`: SHA-256 of `plan.encode()` after the runner has stored the plan (`runner/src/runner.rs:1201`, `host/terminal/src/host.rs:345`). scale-check passes the `.plan` from `contract build`, which writes `plan.encode()`. The terminal decodes that file and does not compile it. Decode rejects trailing bytes and reads the same fields encode writes, so the digest is the hash of those file bytes. A `.contract` argument is a different entry: `compile_path_terminal` runs `terminal::check` and `Profile::Terminal`. The lowering difference found is block-link colour (`contract/lower/src/tags.rs:1438`). This scale app’s bare px widths fail terminal admission, and scale-check never takes that path. The running-plan check holds for the file it passes.

`baseLength` multiplies a terminal `{cells: n}` by `grid.w`. That matches the kernel: `ScaleValue::Cells` is emitted as `n ch` on both axes (`kernel/src/style/scale.rs:67`). A vertical `space.sm` is 8 px. Using the row height would expect 16 px and fail the padding checks that pass. `cells_of` still snaps x/width to the 8 px column and y/height to the 16 px row.

The `even` check compares opposite edges of the painted cell insets, and only when both nodes carry `cells`. Other hosts have no `cells`, so their extra check is the px padding and they stay at 21/21. For 8 px of padding on a 16 px row, the top and bottom insets sum to 1 and differ at every vertical phase: row-aligned is 1 above and 0 below, which is the geometry `layout_and_plan_answer_as_json` locks and the geometry the RFC and QUEUE record. Leave it as a real failure. The script exits 1 while any row fails, which is the signal stage 11 E is meant to see. Marking it known would make `--host terminal` exit 0 with the half-row drift still there. The count is 19 boxes plus two `then` phases.

The QUEUE entry’s column numbers match the schema and the cell constants: width 8, row 16, `space.sm` and `space.md` one cell (8 px), `space.lg` and `space.xl` two cells (16 px, one row). `14/16`, `12/16`, and `10/16` are 0.875, 0.75, and 0.625. Those three clone literals were not re-read from the clone tree in this review.

`--size 64x64` is 512×1024 px. The root is 420 wide, and the header, paddings, and gaps in this fixture sit inside 1024 of height. The checks read kernel frames. Three `cargo run` launches are one fresh process per phase, which is what the bound-corner taps require. The layout line is the last stdout line that starts with `{` and contains `"nodes"`. The plan line has no `"nodes"`, verb errors go to stderr, and a `testId` cannot insert a raw newline.

Tests lock the half-row geometry and, in `scripts/scale-check.test.mjs`, `baseLength` plus a synthetic 1-above-0-below failure. They do not `JSON.parse` the layout line, do not compare the digest to `contract build` bytes, do not drive `cargo`, and do not cover a percent radius or a step whose text contains a space.
