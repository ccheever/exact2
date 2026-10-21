**RIGHT WITH CHANGES. No new BLOCKER.**

Round 3’s inherited-colour blocker is **fixed in the design**. [D7:527](/Users/admin/x2theme/exact2/llp/1121-design-system.rfc.md:527) now materializes an implied token-colour row on the boundary. That stops the kernel’s [ancestor walk](/Users/admin/x2theme/exact2/kernel/src/arena.rs:598) there and gives the wasm [CSS emitter](/Users/admin/x2theme/exact2/host/web/src/element.rs:633) an actual row to emit. The nested-pin gate covers the reported case. This is a specification fix; nothing is implemented yet.

1. **MAJOR — D13: multi-file atomicity needs a defined commit boundary.**  
   [D13:803–823](/Users/admin/x2theme/exact2/llp/1121-design-system.rfc.md:803) permits transactions spanning `.facet` and `.contract` files, but digest checks alone neither serialize concurrent writers nor recover a transaction whose second file replacement fails. Standalone serving and dev-loop serving also need one project writer. **Fix:** specify exclusive writer ownership, serialized check/validate/commit, recovery from partial writes, and publication only after the complete commit. Alternatively, explicitly restrict v1 transactions to one file. Add a second-file-failure gate.

2. **MAJOR — D13/D15: the operation set cannot express every promised inverse.**  
   [D13:823](/Users/admin/x2theme/exact2/llp/1121-design-system.rfc.md:823) promises an inverse transaction, but the operation table lacks removal of an axis value, param, individual `when`, or state row. Undoing the first `set-param` must restore **absence of the app override**, so subsequent component-default changes remain effective; writing the previous value is insufficient. **Fix:** define explicit unset/remove operations with precise targets, and return inverses conditioned on the resulting file digests. Gate restoration of both values and inheritance.

3. **MAJOR — D13/D14: `resolve` lacks the context needed for concrete results.**  
   [D13:797](/Users/admin/x2theme/exact2/llp/1121-design-system.rfc.md:797) supplies only tuple and scheme, while the tool promises resolved swatches and contrast badges. Platform colours require [per-view traits](/Users/admin/x2theme/exact2/llp/1095-platform-colours.rfc.md:380); relative lengths require [font and display context](/Users/admin/x2theme/exact2/kernel/src/style/relative.rs:12). D12 already recognizes this distinction. **Fix:** define standalone `resolve` as returning the residual value plus explicitly labelled fallbacks. Concrete measurements should identify a connected build and node/view context; contrast badges must identify which resolution they assess.

4. **MAJOR — D15: component param names are not stable declaration identities.**  
   [D15:937](/Users/admin/x2theme/exact2/llp/1121-design-system.rfc.md:937) names params `PostButton.radius`, or `reading.Article.measure`. Contract permits separate files to declare identically named components and import them under aliases. Its [internal collision names depend on traversal order](/Users/admin/x2theme/exact2/contract/cli/src/sources.rs:495). Neither the source name nor that generated name safely identifies a public edit target. **Fix:** define a stable component identity incorporating its module/package, or an explicit Facet identity. Import aliases must resolve to that identity, and import reordering must not redirect an override.

5. **MAJOR — D15: the unused-param rule rejects the language-neutral example.**  
   [D15:954](/Users/admin/x2theme/exact2/llp/1121-design-system.rfc.md:954) refuses a param unless a component row reads it. The immediately following `MapCard` has no Contract rows; Swift reads its param through a binding. Params used only through another param also need transitive reachability. **Fix:** check Contract params through the dependency closure rooted in component rows. Treat declared native binding exports as roots, consistently with D7’s retention rule; do not require an unavailable proof from native source code.

6. **MAJOR — D13/D15: `set-param` bypasses the compiled-consumer exception.**  
   [D13:809](/Users/admin/x2theme/exact2/llp/1121-design-system.rfc.md:809) classifies every `set-param` as live, and stage 7 requires every instance to update live. However, params admit D3’s kinds, including duration/easing, which are [substituted into consumers at build time](/Users/admin/x2theme/exact2/llp/1121-design-system.rfc.md:361). Changing such a table entry cannot update the compiled transition. **Fix:** classify changes by the entire dependency closure, independently of the operation’s name. Any affected compiled consumer forces rebuild/reload; narrow stage 7’s gate accordingly.

7. **MAJOR — D15/§3: editor constraints lack a complete value contract.**  
   [D15:948](/Users/admin/x2theme/exact2/llp/1121-design-system.rfc.md:948) offers `toggle` without a boolean kind or two-value mapping, and `select` without syntax for its listed choices. Length ranges omit unit/comparison rules despite admitting contextual lengths such as `em`. The example also assigns `radius.full`—[999px](/Users/admin/x2theme/exact2/llp/1121-design-system.rfc.md:1036)—to a param bounded by 40, and writes bare `12px` where §3 requires a Contract string. **Fix:** specify controls by kind, choice/mapping syntax, and unit-bearing range semantics; reject ranged values that cannot be checked at build time. Validate every folded variant and correct the examples.

8. **MINOR — D14: the mandatory Figma bridge rests on an incorrect restriction.**  
   [D14:890](/Users/admin/x2theme/exact2/llp/1121-design-system.rfc.md:890) says plugins cannot open sockets to arbitrary hosts. Figma’s [manifest documentation](https://developers.figma.com/docs/plugins/manifest/#networkaccess-networkaccess) permits `ws`, `wss`, localhost, and wildcard network access. **Fix:** make a bridge conditional on credential handling or deployment needs, rather than a claimed networking prohibition. Direct clients must still obey Facet’s authentication rules.

9. **MINOR — §5/D6: restaging left a wrong prerequisite reference.**  
   [D6:473](/Users/admin/x2theme/exact2/llp/1121-design-system.rfc.md:473) makes platform typography “stage 4’s prerequisite.” Stage 4 is now the colour live table; typography belongs to [stage 6](/Users/admin/x2theme/exact2/llp/1121-design-system.rfc.md:1097). **Fix:** update the reference so the early protocol/live-table work does not acquire an unintended typography dependency.

The earlier local-token bootstrap finding stays closed. Remote pairing remains explicitly unresolved behind Q5/stage 10; it is not yet a qualified remote security design.

No files changed.
