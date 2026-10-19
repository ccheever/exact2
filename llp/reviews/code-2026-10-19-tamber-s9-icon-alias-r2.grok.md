# Review round 2, LLP 1121 stage 9 icon-alias fix, Grok (xhigh)

Reviewed: 4ed1b22ed4.

No findings. The round-1 issues are fixed, and the alias fold is consistent on every path that checks a design.

Round 1:
- `refuse-icons.tamber:22` is `icon.bare = sparkle`. The golden `refuse-icons.err:10` is still `22:15: tamber-kind: an icon is …`, which does not quote the word, so the column stays the start of the value. `star = icon(sf "star")` remains `tamber-icon-name` on line 23. Leaving `star` would have dropped line 22, because `sources` accepts that literal and `aliased` would copy it.
- `icon::aliased` copies a target only when `sources` succeeds (`tamber/src/icon.rs:126-131`). `icon.home = icon.bad` with `icon.bad = icon(png "x")` keeps its own text: line 4 says `` `png` ``, line 5 says `an icon is`.
- `Folding.design` documents that those icons hold the resolved `icon(…)` (`tamber/src/lib.rs:141-142`). Explain still reads the alias off the file (`icon.alias` stays `"icon.home"`, `sf` is `"house"`).
- The role-axis write is refused for a plain design, the same design plus an alias, and that alias in an app with params (`contract/cli/tests/it/tamber_serve.rs:768-794`). `write_checks` calls `exact_tamber::check`, which now resolves the alias, so a params build no longer treats the design as unread and lets the write through.
- The unit test covers a two-step chain, an unknown name, a cycle, a theme token (`icon.home = ink`), and the bad literal (`tamber/src/icon.rs:231-255`).

What checks out:
- `check` and `check_design` resolve aliases before `icon::check`. `params::laid` still calls the same function after icon params are inserted, and the second pass is a no-op once a value contains `(`. A chain resolves within `icons.len()` passes, including a param default of `icon.home` processed after the design icons.
- A target whose sources do not read, a theme token, or a cycle is left as written and refused with `tamber-kind` at the alias value's own line and column. Name, kind, and `when` errors stay on the target; spans are not rewritten.
- Build, CLI check/eval/ir/export, server export (`design` → `check`), and describe/resolve/explain/uses (`laid` then `check_design`) all go through that resolution. IR and the plan use the resolved `Icon` sources, not the raw alias text.
