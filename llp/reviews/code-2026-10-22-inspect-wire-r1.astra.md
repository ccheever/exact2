# Code review: tamber/1.16 inherited rows' fromAt and inspect select, round 1 of 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort xhigh, read-only sandbox, a throwaway worktree.
- **Method:** round 1 of 3; HEAD `079ae5324b`. The author (Claude) is not a reviewer.
- **Verdict:** both: one BLOCKER (Apple hosts read a select frame as inspect off); with seven SHOULD-FIXes and NITs, all taken in a0fd4a2ae4.

1. **BLOCKER — [scripts/tamber-builds.mjs:558](/tmp/rv-dt-inspwire/scripts/tamber-builds.mjs:558):** The new frame also reaches Apple inspectors. `Inspect.swift:131` interprets its missing `on` as false, silently disabling inspection while the server retains ownership. Apple must handle `select` or report `unsupported`.

2. **SHOULD-FIX — [scripts/tamber-inspect.mjs:469](/tmp/rv-dt-inspwire/scripts/tamber-inspect.mjs:469):** Removing `fromSite` here misses `published()` → the raw `selected` event. Every inherited row still leaks it through that event. `heldOf()` also preserves any supplied `fromSite`, including without a map.

3. **SHOULD-FIX — [scripts/tamber-server.mjs:796](/tmp/rv-dt-inspwire/scripts/tamber-server.mjs:796):** With a registered, mapped build but no selection, `s.site` throws. The outer handler returns **400**, defeating the intended **409** “select a node first.” Guard `s` before dereferencing.

4. **SHOULD-FIX — [host/web/dev.js:906](/tmp/rv-dt-inspwire/host/web/dev.js:906):** Unlike `refresh()`, select-up never checks the remembered generation. After a rebuild, the server combines an old selection’s site with the current map, and wasm walks a potentially reused view ID. Bind the request to the selection’s plan/generation or refuse it.

5. **SHOULD-FIX — [scripts/tamber-inspect.mjs:648](/tmp/rv-dt-inspwire/scripts/tamber-inspect.mjs:648):** Exact chain equality fails for components that only render another component: they own no matching nodes. `Wrap → Label` in `contract/corpus/provide.contract` demonstrates this shape. A similarly delegated app root cannot satisfy the promised past-root clamp.

6. **SHOULD-FIX — [scripts/tamber-server.mjs:796](/tmp/rv-dt-inspwire/scripts/tamber-server.mjs:796):** Combining `refresh:true` with `select` makes `needs()` classify the request as read, then this branch bypasses the read-only handler. Ownership is checked; write scope is not. Require write whenever `select` is present.

7. **SHOULD-FIX — [host/web/dev.js:910](/tmp/rv-dt-inspwire/host/web/dev.js:910):** Each ancestor invokes full node serialization. The new `runner.site_of(from)` also scans the instance tree for every inherited row. Use a parent/site lookup without full reports; the iteration guard does not bound that work.

8. **NIT — [host/apple/Sources/ExactKit/Inspect.swift:64](/tmp/rv-dt-inspwire/host/apple/Sources/ExactKit/Inspect.swift:64):** Native trimming drops `fromSite`, losing source attribution for retained rows. Include it alongside `from`.

Chain ordering is innermost first; `slice(n)` and outermost matching work for ancestors with owned nodes. Recursive path naming covers `fromAt.file`. The 4096 guard exceeds the kernel’s 128-depth limit. No changes, builds, or tests performed.
