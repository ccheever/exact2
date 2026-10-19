# Review round 2, LLP 1121 stage 9 H: an empty symbol: name is no symbol, Grok (xhigh)

Reviewed: 963e889b00.

No findings.

Round 1 is closed. `symbol:` and `symbol:sf/` now take the same path everywhere that path branches on an empty name.

- Lowering accepts a folded `symbol:` in `contract/lower/src/symbols.rs`. Dead `if` arms are still checked. The `QUEUE.md` note matches that.
- `kernel/src/grouped.rs` `apple_symbol` returns `Some("")` for an empty role, so a grouped row led by `image "symbol:"` is a standard cell, same as `symbol:sf/`.
- `contract/cli/src/snapshot.rs` `local_image` treats every `symbol:` source as local, including the empty one.
- The agent reason is `"empty"` for both spellings in `host/web/glue.js`, `host/web-js/agent.js`, `AgentIOS.swift`, `AgentMac.swift`, and `host/linux/src/presenter.rs`. The unknown-role log is skipped for `symbol:` the same way `symbol:sf/` already was, on the web, the JS target, iOS, and the Mac.
- Draw paths already went through `apple_symbol_name` / `symbol_path`, which are `""` / none for both. Linux paint, the web prerender, Apple `symbolName`, the widget snapshot leaf, and the terminal's missing-image text do not branch between them. A native button still rejects both spellings (`contract/lower/src/controls.rs`).

Tests cover the behavior this sha changes. `an_empty_symbol_name_is_no_symbol` builds the theme2 repro, keeps only `symbol:close`, and asserts grouped-row parity `(custom: false, symbol: Some(""), title: "Reply")`. Snapshot admits both literals. The web prerender counts `symbol:` with the empty `sf/` image (no fetch, empty `data-symbol-path`). Linux rasters clear and refuse nothing for `symbol:`, same as `symbol:sf/`. `corpus.rs` no longer lists `""` as a refused role.

The agent reason and the "unknown symbol role" log are only checked by reading the predicates. No host test asserts `reason: "empty"` or a silent log for `symbol:`.
