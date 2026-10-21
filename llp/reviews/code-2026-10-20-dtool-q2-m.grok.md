# Code review: a folded :disabled or :checked row read as its base (LLP 1122 queue item (2)), the second narrow verification round, 2026-10-20 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `cc1e2b922b`.
- **Method:** one brief (sha256 `2f85d7629846492f39a831f6e281d610426e2bda5f953ca847059c54f3137f44`) quoting the first narrow round's two reviews verbatim, asking of each open finding "closed? broke anything?"; commit cc1e2b922b. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks (1 NIT).
- **Disposition:** taken in the commit named in the sign-off request: (5)'s NIT, as Astra's 1 (the case-fold reverted to exact names and `-exact-` role spellings); (1)-(4) closed or held, as the review says.

Nothing blocks.

1. **A1 — closed.** A folded param base is named exactly `<home>#<component>.<name>` at `scripts/tamber-inspect.mjs:498-500`, and that string has to equal `p.param`. With only `b.contract#Card.fill` cached, the app row is refused as unread. With both cached, it selects `app.contract#Card.fill`. Suffix matching is gone.

   `home` is the same string the compiler already stores on the component (`contract/cli/src/sources.rs:737-749`), and the map copies it onto each source (`contract/cli/src/map.rs:352-355`). The join reads that field for the base attribute's file (`scripts/tamber-inspect.mjs:481-482`). The component is the declaration above that attribute: the scan stops at the first top-level line, and a component body is indented, so a slot fill written in the caller's view resolves to the caller. That is the component `param.<name>` was rewritten in (`contract/lower/src/params/rewrite.rs:527-544`); the fill stays in the use site's scope (`contract/syntax/src/inline/use_node.rs:319-324`).

   Every `.contract` source the loader parses gets a `home`, including one that declares no components — the store sits outside the component loop. `relocate` rewrites paths only (`contract/cli/src/sources.rs:63-99`); identities were built from that same `home`, and the node's `file` and `sources[].file` are both the relocated path, so the lookup still finds it. A package module declaration (`exact.json`) is listed without `home` (`contract/cli/src/sources.rs:821-833`). It has no component body, and a missing home fails closed (unread), so it does not pick another param.

2. **A7 — closed.** Two parts of one state are labelled with the full file and line (`apps/design-tool/inspector.ts:111`). `styles/card.contract:6` and `ui/card.contract:6` no longer share a name. One part of a state keeps the old label. The edit address is still the rung.

3. **B1 — closed.** Same change as A1. Two components of one name are separated by `home`, for an app file (`a.contract#Card.fill` versus `b.contract#Card.fill`) and for a package (`@acme/ui/card.contract#Card.fill`, from `home()` at `contract/cli/src/sources.rs:271-294`). A shared name whose other branch is showing is not a suffix hit, and it is not written onto the other component.

4. **B2 — the decline holds.** `contract/lower/src/states.rs` is unchanged. A later literal on one side still leaves the earlier part listed (`states.rs:518-520`); `over` still returns a literal as itself (`states.rs:211`). The review's reason still matches the code: each part stays, labelled by its place, and an edit writes that part's line.

5. **B3 — the reported case is closed; one residual NIT.** `canvastext` matches a reported `CanvasText`, and `-Exact-Secondary-Label` still matches `secondary-label` (`scripts/tamber-inspect.mjs:502-504`). The token text becomes that reported name, so the template edit keeps the source spelling.

   **NIT.** The compare is any reported name, case-folded, after one leading `-exact-` is removed. The kernel folds only a role, and only in the spelling it accepts (`kernel/src/style/roles.rs:38-50`, `kernel/src/style/theme.rs:357-359`): bare `canvastext` is `CanvasText`, while `-exact-canvastext` is not (`roles.rs:936-938`), and a bare Exact role is not a colour (`roles.rs:930-933`). On a folded row the reported names are the branch now showing (`host/web-js/inspect-dev.js:109-112`). A base written `-exact-canvastext`, or `primary` while the shown branch's token is `Primary`, is offered that token (`tamber-inspect.mjs:503`, then `tamber-inspect.mjs:358-366`). The template rung is still the base's own line. No param is retargeted.

Whitespace handling, the package param, both parts of one state, and a literal part under an unread host param are unchanged. The compiler change stores `home` beside identities that were already computed; it does not alter plan bytes. That was not remeasured. No files were changed.
