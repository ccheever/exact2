I'll review the exact2 test-harness diff read-only, starting with the review instructions and the change itself.The review skill wants a defect-first pass on this harness change. I'll read the surrounding test code and the guide it claims to assemble.**LAND WITH CHANGES.** Marker attachment and `fill_route` are correct for `docs/start-here.md`. The new opt-out fails open, so a marked block can skip both compiles and the test still passes.

## Findings

[P1] Fail the docs test on an unrecognized check marker — `contract/cli/tests/it/docs.rs:113`

`every_contract_example_in_the_guides_compiles_and_every_test_parses` skips every `contract` fence whose `check` is `Some`, while `every_split_app_in_the_guides_compiles_assembled` only concatenates `app`, inserts `route <name>`, and appends `file`. Any other value (`routes notes`, `route`, `Route notes`, `file NoteRow`, `<!-- check: -->`) is in neither path, and nothing records a failure. A dropped route leaves the skeleton arm, a `column` whose only line is a comment. The lexer ignores that comment, and `lower-zero-size` applies to `button` and `link` only, so the stub arm still lowers. The assembled app compiles, and diagnostics in that screen never run.

[P2] Reject two route blocks with the same name — `contract/cli/tests/it/docs.rs:226`

`fill_route` keeps the `when e.name == "…"` line and replaces the lines under it. The `when` line stays, so a second block with the same name finds that same line and overwrites the first body. The first block is never compiled, and the test does not report the duplicate.

## Assessment

On the current guide the parser and the splice do the right thing. Each `<!-- check: … -->` sits on the line immediately before its fence, and every non-marker line clears `marker`, so the eight fences pick up `app`, `route notes`, `file`, `route note`, `route add`, and `route feed`. App blocks join in order: shapes, then `component App`, then `view` at the same indent as the actions. Each route replace starts from a `when` at 14 spaces, deletes exactly the stub `column` and its comment (both deeper than 14), and stops on the next line at indent ≤ 14 — the next `when`, or the tablist `row` at 6. The inserted column is padded to 16, and every line of the snippet keeps its own indent, so the feed screen’s `else when` and `else` stay on the same column as their `when`. `note` does not match `notes`. `NoteRow` is appended at column 0, after `App` has closed. Type-checking walks both arms of a `when`, so a screen that was actually inserted is checked.

What this run does not protect against is a marker the allowlist misses. That is a one-line hole in the gate this change adds. Close it by rejecting any `check` other than `app`, `file`, or `route` plus a non-empty name, and by requiring each route name to be filled once. There is no direct test for a bad marker, a duplicate name, or an end-detection miss; the only coverage is this one guide compiling.

Residual: a syntax error in the assembled file still suppresses later passes for every screen, and the failure text cites the assembled line. The `app.ts` sample is still unpaired, because `### Persistence and fetch` clears `block.contract` before that fence. The per-block sound-file staging is not repeated on the assembled path; nothing in these marked blocks references a `sound`.
