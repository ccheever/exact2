I'll review the r6 restructure of the design-tool RFC against the architecture it depends on, read-only, and start by reading the review skill and the documents in scope.The restructure is the right shape. A live app plus hot reload can replace captures. It is not buildable as written: the dev channel is one-way, and two of the new decisions contradict rulings the RFC itself keeps.

**RIGHT WITH CHANGES**

## BLOCKER

**1. D4/D5/D3 — builds cannot answer on the channel the relay uses.** D4 says the whole relay is four server-to-build messages on "the channel builds already hold," and that an older host "says so by name" (`llp/1122-design-tool.rfc.md:251-266`). D5's `selected` report and D3's effective scheme and mode are build-to-server (`:242-243`, `:279-285`). T2's gate requires the effective mode before inspect exists (`:652`). That stream is server-sent events only. `host/web/dev.js:3` is an `EventSource`; a frame that is not a generation is a no-op (`dev.js:155-158`). Native `PlanURL.handle` returns unless the frame is a generation revision (`host/apple/Sources/ExactKit/PlanURL.swift:812-823`). The JS page stream only acts on `error` and `reload` (`host/web-js/dev.mjs:160-168`). A host also cannot open `tamber/1`: a native or wasm session cannot set headers or send socket frames (`llp/1121-tamber.rfc.md:941-943`). The SSE `hello` is `{hello:true, epoch, seq, generation, …}` (`host/web/dev.mjs:262-266`), not the build list D4 addresses messages with.

Fix: keep the four messages as the drive set, with a `kind` dispatched before generation handling so a control frame cannot be read as a plan push. Add one build-to-server POST on the dev loop (status on connect and on change: effective scheme, contrast, mode, plan digest; `selected`; `unsupported` by name), bound to that SSE connection. Tamber publishes it. Do not send `selected` over `tamber/1`. Say this in §9 Q7 so a "yes" does not freeze the missing reply.

**2. D4/D5/§9 Q2 — `outline` makes the host read the source map.** The message is a source span, and the build outlines every mounted node whose site maps to that span (`llp/1122-design-tool.rfc.md:260`). Q2, as ruled, says the server joins maps and a host never does (`:748-752`). That is the same line as `rules/DEFERRED.md:865-872` and LLP 1035.002 D6: the host reports `site` and `instance` only, with no spans (`llp/1035.002-native-result-inspection.rfc.md:166-178`). A host cannot resolve a span.

Fix: the server inverts the map it already joined and sends site ids (every `each` row shares a site). The host matches sites it already has and never opens the map. Keep the waiver as written.

## MAJOR

**3. D4/§4 T2 — there is not one channel.** The default dev loop for a non-game is the JS target (`host/web/dev.mjs:83-88`). Its page listens on `/__dev/page` (`host/web-js/dev.mjs:180-186`). A simulator's `/__dev` and `/exact.json` are forwarded to a second process, the resident bake (`host/web-js/dev.mjs:217-222`, `:244-256`; `producersOnly` builds `--bake`, `host/web/dev.mjs:90-96`). A `pin` or scrub published on one stream does not reach the frame and the devices together, which is T2's gate (`llp/1122-design-tool.rfc.md:646-652`).

Fix: T2 and T2b run one `dev.mjs --wasm` loop, frame and devices on the same `/__dev`. The JS page joins only after finding 4. If both loops stay up, name both streams and require a publish on each.

**4. D5 — the JS fallback condition is too weak, so fail-closed will offer the wrong rung.** D5 unblocks a JS frame once it reports a token beside the computed value and its own plan and map (`llp/1122-design-tool.rfc.md:301-305`). The JS `layout` source is CSS: any declared property is `authored` or `dynamic` (`host/web-js/agent.js:139-140`), not the kernel's `own` / `class:` / `tag`. The DOM has `data-site` and no instance path (`host/web-js/src/emit.rs:171`; `nodeDetail` returns `site` only, `agent.js:166-167`). Fail-closed trusts the host's source (`llp/1122-design-tool.rfc.md:307-314`), so a style row would be offered as the template rung. Holds and outlines are keyed by site and instance (`:466-467`). `/exact.json` on that loop is the resident producer's plan, which D5 already notes.

Fix: the JS frame stays on wasm until the report matches the kernel row for row, including origin and `instance`. The server should treat the map's `origin` (`contract/cli/src/map.rs:219-227`) as the rung authority whenever the host report is not the runner's.

**5. D8 — a hold does not survive the reload that actually happens.** D8 says the build re-applies holds after a reload, keyed so a view id is not required (`llp/1122-design-tool.rfc.md:466-468`). The JS reload is a new document. Its checkpoint is slots, answers, clock, and focus (`host/web-js/dev.mjs:164`), not holds. A wasm program rebuild is `location.reload` (`host/web/dev.js` `programChanged`). Nothing remains to re-apply. T4's gate requires the hold to come back after an `:active` edit (`llp/1122-design-tool.rfc.md:678`).

Fix: the server replays each build's holds (and pins) on that build's next hello. The build drops a hold whose site and instance are gone.

**6. D3/D5/D12 — the iframe is not a free copy of a tab.** LLP 1020 D1: a bare `iframe` is 300×150 and the guest never affects layout (`llp/1020-webview.rfc.md:136-140`). D3 sizes "a phone width" and does not set a height (`llp/1122-design-tool.rfc.md:226-227`). LLP 1020 D4: on the web the parent reaches only as far as origin rules allow (`llp/1020-webview.rfc.md:227-229`). The tool origin and the dev origin differ, so the tool cannot scroll the frame. `outline` on an offscreen node is invisible, which is the capture framing r4 had and r6 deleted. LLP 1121 D14 still says embedding is later because a JS-target app cannot host another session (`llp/1121-tamber.rfc.md:984-987`) and r6 does not amend it. An iframe is a second document, not a second session in the JS runtime. That is why it works, and it should be written there. D3 also allows a second frame now (`:227`) while T7 still has open whether each copy needs its own `--storage` (`:693-695`). Two frames of one dev URL share localStorage. For a remote client, `src` of the loopback dev URL is loaded by the remote browser as its own loopback (`:562-564`).

Fix: the frame's style sets width and height. On `outline` and `selected`, the host scrolls a match into view. Amend 1121 D14. Defer the second frame to T7. For T6, set `src` to the tunneled origin that reaches the developer's dev server, or proxy the app same-origin through the Tamber server.

**7. D4/§4 T7 — r4's text-size pin was dropped and T7 still needs it.** Round 1 fixed text size as `prefer root-font-size` in px (the r6 ledger, `llp/1122-design-tool.rfc.md:105`). `pin` is now scheme, contrast, and an app mode (`:262`). T7 still requires Light, Dim, and Black at two text sizes (`:690`). Nothing sets the second size.

Fix: add root font size to `pin`, the same fact `prefer` and `setRootFontSize` already set.

## MINOR

**8. D7 — the workbench points at the wrong decision.** It says a preview screen is D4 (`llp/1122-design-tool.rfc.md:445-446`). Preview screens are D3 (`:244-248`). Point it at D3.

**9. D5/D13 — `selected` must be tied to the dev connection.** Dev POSTs such as `/__dev/checkpoint` are accepted with no token (`host/web-js/dev.mjs:188-190`). A page that can reach the dev server could publish a selection. Bind the POST to a nonce from that build's SSE hello. D13's "token holders receive it" only covers the fan-out (`llp/1122-design-tool.rfc.md:579-580`).

**10. D8/D5 — one hold, and the web hit.** D8 does not say whether a second `hold` replaces the first, while "several states at once" waits for T7 (`llp/1122-design-tool.rfc.md:479-480`). Say one hold per build. On the web, inspect should use the event target from the hit test the runtime already uses (`host/web/geometry-glue.js:93-102`), so inspect and a real tap cannot disagree.

**11. §9 Q6 — a hosted relay would carry source.** Q6 reads as a rendezvous (`llp/1122-design-tool.rfc.md:758-762`). A selection report includes file, line, and the call-site chain (`:294`). D10 keeps that on the machine unless a remote model is named (`:532-534`). The question should say the relay would see source and the live app.

r4's capture identity, chrome-inset mapping, and tile relay were right to drop: a real hit has no stale geometry, and platform chrome is not a hit (T2c's gate). `set-row`, the commit guard, scrub sessions, fail-closed rungs, and hold semantics are still specified and still coherent.
