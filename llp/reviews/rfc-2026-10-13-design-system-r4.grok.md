I'll review r5 against the architecture it cites, starting with the new decisions and the inherited-colour blocker from round 3.**RIGHT WITH CHANGES**

The round-3 blocker is fixed. A mode boundary writes each inherited colour token as a row on that node, holding the token's value for the boundary's tuple (`llp/1121-design-system.rfc.md:527-537`). `copy_inherited` copies the nearest ancestor whose mask has the row, so a text node whose parent has no `color` still finds the boundary (`kernel/src/arena.rs:598-614`). `css_style_of` emits specified style (`host/web/src/element.rs:604-633`), which is why the row has to sit on the boundary and not in a computed snapshot. `text_color` is inherited (`kernel/tables/schema.json` bit 75). The same rule is what the JS target writes (`:573-576`) and what stage 2's gate requires on the web, the wasm host, and iOS. That is the case round 3 named.

## BLOCKER

**1. D13 / §5 stage 4. The live table never reaches a running build, and the skip is on the wrong process.** `llp/1121-design-system.rfc.md:786-790`, `:827-835`, `:1084-1091`

The server is placed in `host/web-js/dev.mjs` and told to skip "the watcher's reload … in-process." The process that turns a source save into a plan is `exact_web::dev::Session` (`host/web/src/dev.rs:1-11`), run from `host/web/dev.mjs`. It watches the sources it compiled, writes a plan, and the page boots that plan with `exact_boot_plan` (`llp/1007-web-host-v1.spec.md:473-500`). iOS and Mac do the same when `EXACT_DEV_PLAN` changes (`llp/1008-apple-host-v1.spec.md:535-537`). The JS loop separately reloads the page on any app-dir change (`host/web-js/dev.mjs:7-8`, `:109-127`). A digest flag inside one of those processes does not stop the others. The background plan rebuild the RFC wants (`:834-835`) is itself the native reload signal, because publishing the plan file is how those hosts restart.

That restart rebuilds the tree. Slots, resources, and the clock carry; scroll, focus, and a draft do not (`llp/1007-web-host-v1.spec.md:497-500`). Stage 4's gate requires a dial drag to hit the simulator, the Mac, and the web with the list still scrolled and no reload. There is no other entrance: nothing in the kernel ABI applies a row batch short of `boot_plan` (`host/web/src/abi.rs`).

**Fix:** a design-only save (structure digest unchanged) must not publish a new plan. Put that rule in `Session` and in `host/web-js/dev.mjs`'s watcher. Carry the batch on the channel those hosts already listen to (the dev SSE / plan connection) as colour and length wire values, applied in the running kernel in one commit. Publishing a plan stays the path for a structure change.

## MAJOR

**2. D13. A live colour batch cannot be `Wide` ids, and the cap has no reclamation rule.** `llp/1121-design-system.rfc.md:827-832`, `:1084-1090`

`ColorValue::Wide` is an index into a process-global table (`kernel/src/style.rs:706-707`, `kernel/src/style/wide.rs:43`, `:131-139`). The dev server, the wasm host, and each native app have different tables, so an id folded in the server is meaningless in the app. `intern` returns `None` at 4,096 (`wide.rs:61`, `:135-136`, test at `:147-161`) and refuses the new colour. Identical CSS text reuses an id; a scrub that produces a new `oklab` mix each frame does not. The ids are dense indexes, so "reclaim" means a free list of holes that an in-flight transition can still hold. "The policy says what a running transition keeps alive" (`:1086`) does not state that.

The stage 4 example hits this: `accent.fill-pressed` folds to `Wide` (`:500-508`, `:1032`).

**Fix:** the batch carries each colour's CSS text (LLP 1095's wire). Each process interns it. An id stays while a row, a fallback, or a transition endpoint holds it; a full table refuses the edit with a diagnostic. The gate's "drop none" is this rule.

**3. D13 / D14. The token test is thinner than the one it cites, and an Exact app cannot speak the socket.** `llp/1121-design-system.rfc.md:313-314`, `:837-850`, `:869-884`

`access.local` is `admitted && !forwarded &&` loopback host and loopback peer (`scripts/install-page.mjs:269-292`). The forwarding-header clause is what rejects a tunnel that rewrites `Host` to `127.0.0.1`. The RFC's parenthetical keeps only the two loopback checks. The install token is memory-only and presented as `x-exact-install-token` (`host/web/local-install.mjs:20`, `:98`). This token is a file in `.exact/`. The root `.gitignore` does not ignore `.exact/` (`docs/diary.md:21` says it does).

The client is an Exact app (`:871`). On every host the admitted socket is receive-only and cannot set headers or send frames (`llp/1016.000-answers-that-keep-coming.rfc.md:176-180`). Outbound frames exist for a web JS TypeScript data module only (`docs/contract-for-agents.md:816-825`). D2's wasm page cannot send `set-token`. Pairing (`:845-849`) needs an unauthenticated endpoint on a server whose reads otherwise require the token, and the short code has no entropy or attempt limit.

**Fix:** use the install predicate whole, including `!forwarded`, and return the same not-found the install endpoint returns. Keep the token in memory. The design tool's writes go through the web JS TypeScript socket (first message is the token, not a query string) or through `bun exact.mjs facet op`'s HTTP POST with that header. Native and wasm sessions do not open `facet/1`. Pairing stays behind §9 Q5 with a rate-limited code of stated entropy.

**4. D15. The Contract param block is not legal Contract, and the neutral form is incomplete.** `llp/1121-design-system.rfc.md:920-935`, `:948-967`, `:1003-1007`; `docs/contract-grammar.md:197-205`

The prose says a Contract value is a string and the metadata are ordinary attributes (`:1005-1007`). The example mixes `"accent.fill"`, unquoted `12px`, and `in 0..40`. The Contract lexer has no `..` (`contract/syntax/src/lexer.rs:54-57`); the `.` punctuator is one dot. The `.facet` grammar (`:968-969`) is a different language from the Contract sentence.

exact1 kept the value and the slider apart: `radius = $radius.md` in `params`, `min`/`max` on the `design` control (`packages/exact-facet-contract/src/components/button.contract:47-58`). Here `in 0..40` is a checked range (`:948-956`) and the default `"radius.full"` folds to `999px` (`:1036`), so the example fails its own check. `control=select` has no option list. `control=toggle` has no bool kind (D3). `set-param` updates "the live table" (`:809`) and does not say it re-folds params that read it; `pressed` is `color-mix(… param.fill …)` (`:928`). The row rewrite is specified (`:940`); the same `param.` names inside a default expression are not.

The IR "`components` block" has no fields. D12's Swift surface is `color`, `font`, and `symbol` (`:755`). A length param has no accessor, but a non-Contract component is told to read params through those bindings (`:957-967`). exact1's `control prop variant as select` (`button.contract:54-55`) has no successor, so a variant prop is not something the tool can surface.

**Fix:** one grammar. In Contract, the value is a quoted string and `in`, `step`, and `control` are attributes the Contract lexer already accepts; give the range attribute a quoted form. In `.facet`, bare CSS, with the design lexer's `..`. A range constrains a literal override; a token default stays a token, and a scrub that replaces it is a pinned override marked the way D14 marks a dial (`:892-895`). `select` takes an explicit list. `set-param` re-folds every param and token that reads it. The IR block lists name, kind, default expression, range, step, control, options, label, help, group, priority. Bindings grow `length` and `number`, or params of those kinds stay in the tool until stage 6. State that prop controls are not params.

**5. D5 / D7. A Light preview does not re-specify inherited colour.** `llp/1121-design-system.rfc.md:449-454`, `:573-576`, `:1069-1072`

`color` inherits as a computed colour, which is why a mode boundary writes the row again. A Light preview is only `color-scheme="light"` (`:451-453`). It is not a mode boundary, so the write does not run. On the web, `light-dark()` resolves on the element that specifies it (`llp/1034-scheme-aware-colour.rfc.md:382-383`, `host/web/src/css.rs:650-652`); descendants inherit the used colour. A dark page's `text.primary` stays dark inside the Light card. iOS re-resolves a stored `LightDark` per view trait (`llp/1034:373-379`), so the same preview can be right on iOS and wrong on the web and the wasm host. Stage 2's gate requires all three (`:1069-1072`).

**Fix:** a `color-scheme` boundary re-specifies inherited colour token rows by the same rule as a mode boundary, so the browser resolves the pair against the subtree's scheme.

**6. §5. The new stages don't match D14 and D15.** `llp/1121-design-system.rfc.md:1077-1083`, `:1102-1106`, `:1129-1130`, `:877-881`

`facet-param-unused` is a stage 2 check (`:1129`). Params are stage 7. The D15 example's `pressed` is read only by `:active`, and state parts are stage 8 (`:1107`), so stage 7's unused-param check refuses the example it gates. D14's workbench (state parts, side-by-side modes, embedded phone and Mac sessions, `:877-881`) is not in any gate. Stage 3 checks file edits of dials and tokens only (`:1077-1083`).

**Fix:** `facet-param-unused` starts at stage 7. A param whose only reader is a state part lands in stage 8. Give stage 3 the file tool, stage 4 the embedded sessions and live dials, stage 7 the param controls, and stage 8 the state-part workbench, each with a gate line.

**7. D13. `prefer mode` is refused by `prefer`.** `llp/1121-design-system.rfc.md:866-867`

`prefer` accepts `media`, `page`, and `fold` only. An unknown name is an error (`scripts/agent-prefer.mjs:9-28`, `host/linux/src/agent.rs:568-572`). `mode` is not a media feature. LLP 1095 D7 pins traits; an app axis is `setMode` (`:445-448`).

**Fix:** a fourth `prefer` group, `mode`, that runs `setMode` and does not change `prefers-color-scheme`. Add it to `preferGroups`.

## MINOR

**8. D7. `-exact-tint-color` is not an inherited kernel row.** `llp/1121-design-system.rfc.md:530`

`tint_color` has no `inherited` flag (`kernel/tables/schema.json` bit 86). `caret_color`, `text_color`, `fill`, and `stroke` do. The re-specify set is the schema's inherited colour rows. UIKit tint inheritance is the Apple host's, and it only sees a tint the host applied from a specified row.

**9. D13. A transaction is not an atomic multi-file write, and `preview` names no build.** `llp/1121-design-system.rfc.md:803-823`, `:813`

A transaction can touch `app.facet` and a `.contract` (`set-token` and `set-state`), checked then "written through `facet fmt` and `contract fmt`" (`:821`). A crash between the two fmt runs leaves the design half-applied. The inverse is not stamped with the post-write digest it must precondition on. `preview` "pins an app mode on one build" (`:813`) with several builds connected.

**Fix:** write every file to a temp, fsync, and rename as a set; the inverse's precondition is the digest the write just returned. `preview` takes a build id from the subscribe hello, or it is a `-exact-mode` pin inside the tool's own session and not a protocol write.
