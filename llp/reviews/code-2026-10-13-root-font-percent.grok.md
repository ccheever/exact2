# Code review: setRootFontSize("125%"), round 1, 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `bbb66f0c659100ad0ef6b737ac8993a75331665df6aacc7b6563fad72781d066`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: fix (three MINORs); Grok: ship (the same three as MINORs).
- **Disposition:** the underflow and the JS share check fixed with tests; tests added for an overflowing host size and bad input under a percentage; no failed-commit rollback test (no plan makes a commit fail on demand; the snapshot covers it)

---

I'll review commit `503953765` read-only, starting with the review skill and the diff itself.Reviewing commit `503953765` as a single-commit diff. I'll load the reviewer persona and collect that commit before launching the review.Collected the commit diff: 13 files, 191 insertions and 35 deletions. HEAD is that commit. Launching the reviewer.I'll review the commit against the LLP decision and the state machine, rollback, grammar, CSS, and host paths before writing notes.The diff is in. Next I’ll trace the font-size state machine, commit rollback, and each host’s parser against the LLP text.The web host emits a calc rule. I’ll compare the JS target and the native hosts to see whether percentage mode is actually honored.The reviewer is still reading the commit. I'll inspect the state machine, rollback snapshot, and host paths in parallel so I can check its findings against the code.The state machine, rollback snapshot, CSS rule, and host paths check out. I’ll record the one untested rollback edge and a ship verdict.Review notes: `/var/folders/mv/pmcqbrn10cs3bpj4mxnm6xvc0000gn/T/grok-admin/grok-review-28e14416.md`

**VERDICT: ship**

- suggestion MINOR `runner/src/runner/commit.rs:182` — A failed commit restores percentage mode via the `RootFont` snapshot, but no test covers rollback while a percentage is active.`setRootFontSize("125%")` stores a ratio (`AppSize::Of`) and recomputes it when the host root size changes. An absolute px still stands over later host updates, and `"medium"` clears the app size. Percentage mode follows Dynamic Type, the Linux font scale, and the browser setting through the existing host-size path. No host that already pushes a px size needs a new parser.

The refused-commit snapshot is the whole `RootFont` plus the kernel px (`runner/src/runner/commit.rs:112`). Restore at `commit.rs:182` puts `AppSize::Of` back. A failed host-size commit also rewinds `root_font.host` (`runner/src/runner/root_font.rs:94`). That path is untested.

The web rule is `:root{font-size:calc(var(--exact-host-font-size, 100%) * 1.25)!important}`. The multiplier is a unitless number, so the product is a valid length or percentage, and `!important` is the declaration flag. The agent's `prefer` writes `--exact-host-font-size` as a px length; otherwise the `100%` fallback is the initial font size and follows the browser setting. Apple (`Session.swift` `tellPage` on `UIContentSizeCategory.didChangeNotification`), Linux (`font_scale` → `set_root_font_size`), `glue.js`, and `host/web-js/commands.js` already deliver a host px or call `appRootFontSize`. They do not parse the percentage token.

Rust `css_percent` and the JS `PERCENT` regex agree on ordinary forms (`125%`, `87.5%`, `.5%`, `.5e2%`, `+125%`, `1E+2%`) and both reject `125.%`, spaces, `0%`, `calc()`, and a bare `125`. Two edges do not.

Review: `/var/folders/mv/pmcqbrn10cs3bpj4mxnm6xvc0000gn/T/grok-admin/grok-review-28e14416.md`
Summary: `/var/folders/mv/pmcqbrn10cs3bpj4mxnm6xvc0000gn/T/grok-admin/grok-review-summary-28e14416.md`

13 files, +191/−35. 2 bugs, 1 suggestion, 0 nits. No MAJOR findings.

- MINOR `num/src/lib.rs:92` — `css_percent` divides after the `> 0` check, so `5e-324%` becomes `Some(0.0)`. The compiler accepts it (`contract/types/src/checks.rs:1010`); the runner and JS reject a ratio of 0.
- MINOR `host/web/navigation.js:874` — JS accepts any positive finite ratio. The runner also requires `fits(host * ratio)` as an f32, so `1e300%` and `1e-100%` are refused natively and applied on the JS target.
- MINOR `runner/src/runner/commit.rs:182` — rollback of an active percentage is correct and has no test.

VERDICT: ship
