# Review round 1, LLP 1121 stage 8 E1 (held states), Astra (gpt-6-astra, max)

**WRONG**

1. **BLOCKING — Wasm rebuilds discard holds before replay.** [input-glue.js:334](/tmp/rv-s8e1-r1/host/web/input-glue.js:334): rebuilding removes the old DOM, triggering `gone` before the new-plan status. That deletes the server’s hold, so even successful correspondence cannot replay it. Suppress terminal drop reports during generation replacement, then replay after the new plan is installed.

2. **BLOCKING — Missing holds are never cleared on registration.** [tamber-builds.mjs:178](/tmp/rv-s8e1-r1/scripts/tamber-builds.mjs:178): `replayHold` returns without sending anything when no hold exists. A surviving page therefore retains its overlay after server restart or an owner disconnect while the build was offline. Failed correspondence also sends no clear, leaving obsolete polled commands queued. Send an authoritative `hold: null` in all these cases.

3. **BLOCKING — Held-report ordering loses its guard after clearing.** [tamber-builds.mjs:356](/tmp/rv-s8e1-r1/scripts/tamber-builds.mjs:356): revision checking depends on an existing hold. A late applied report after explicit clear restores stale `held` status; replay also reuses the old revision, allowing an old `gone` report to delete the replayed hold. Keep per-build revision tracking through clears, require matching reports, and assign a new revision to replay.

4. **BLOCKING — A pending JS hold can overtake a newer clear.** [inspect-dev.js:452](/tmp/rv-s8e1-r1/host/web-js/inspect-dev.js:452): frames run concurrently. While one awaits `inspectTable()`, a later clear completes; the earlier hold then applies. This reproduced with reports ordered “clear revision 2, apply revision 1.” Serialize hold commands and check their currency after asynchronous work.

5. **BLOCKING — Unresolvable replacements leave the previous overlay active.** [dev.js:699](/tmp/rv-s8e1-r1/host/web/dev.js:699), [inspect-dev.js:459](/tmp/rv-s8e1-r1/host/web-js/inspect-dev.js:459): missing or ambiguous addresses report `gone` without calling the glue, so the previous hold remains painted while the server says none exists. Clear the previous hold when replacement resolution fails.

6. **BLOCKING — The JS development module imports the wrong runtime URL.** [inspect-dev.js:447](/tmp/rv-s8e1-r1/host/web-js/inspect-dev.js:447): this module is served at `/__dev/inspect.js`, so `./rt.js` resolves to `/__dev/rt.js`, which is not served. Pages without already-loaded input glue cannot hold hover/focus states. Import the runtime from its actual served URL and await its initialization.

7. **BLOCKING — JS-target inspection never includes held rows.** [inspect-dev.js:265](/tmp/rv-s8e1-r1/host/web-js/inspect-dev.js:265), [agent.js:167](/tmp/rv-s8e1-r1/host/web-js/agent.js:167): neither selection reports nor layout node replies call `heldOf`. Add the held payload to both paths and test those public replies, rather than only calling the glue directly.

8. **SHOULD-FIX — Agent replacement leaves the server’s old hold replayable.** [tamber-builds.mjs:359](/tmp/rv-s8e1-r1/scripts/tamber-builds.mjs:359): an agent clear/replacement invokes the old callback with `replaced`, which never removes the server record. After correcting revision tracking, treat a matching terminal replacement report as ending that command.

9. **SHOULD-FIX — Existing real presses can erase held paint without dropping the hold.** [input-glue.js:352](/tmp/rv-s8e1-r1/host/web/input-glue.js:352): applying a hold during an existing pointer press leaves `press` tracking intact. Its subsequent release removes `data-pressed`, while `heldOf` still reports active. Keyboard tracking has the same conflict. Reconcile held and real press ownership so paint and reported state remain consistent.

10. **SHOULD-FIX — Active resolution accepts buttons with no press handler.** [input-glue.js:347](/tmp/rv-s8e1-r1/host/web/input-glue.js:347): `button[data-button-style]` can win over an enclosing actual press handler, or succeed without any handler. This contradicts the explicit ruling. Resolve active holds through press-handler nodes only; otherwise report `gone`/`unsupported`.

11. **SHOULD-FIX — Instance matching depends on JSON member order.** [dev.js:697](/tmp/rv-s8e1-r1/host/web/dev.js:697), [inspect-dev.js:455](/tmp/rv-s8e1-r1/host/web-js/inspect-dev.js:455): `{region:1, arm:0}` matches, but the equivalent `{arm:0, region:1}` reports `gone`. Validate and compare instance steps structurally.

12. **SHOULD-FIX — Hold refusals are broadcast instead of owner-scoped.** [tamber-builds.mjs:362](/tmp/rv-s8e1-r1/scripts/tamber-builds.mjs:362): hold `unsupported` reports enter the generic broadcast path and are compared against pin revisions. Track their hold command’s owner/revision and deliver refusals only to that owner, as ruled.

13. **SHOULD-FIX — Held rows expose CSS expressions without token provenance.** [input-glue.js:362](/tmp/rv-s8e1-r1/host/web/input-glue.js:362): token-backed rows return literal `var(...)`/`light-dark(...)` declarations without resolved values or token metadata. Reuse runner/inspect metadata for active rows and preserve equivalent look metadata; mark unavailable provenance incomplete.

14. **SHOULD-FIX — Hover/focus holds do not guarantee snapping.** [looks.rs:39](/tmp/rv-s8e1-r1/host/web/src/host/looks.rs:39): adding an important paint declaration still permits an authored transition on that property. Suppress transition interpolation for held overlay changes. The browser test’s `finish()` helper masks this; test immediately with an authored transition.

15. **SHOULD-FIX — Leaving hover/focus does not release the hold.** [input-glue.js:375](/tmp/rv-s8e1-r1/host/web/input-glue.js:375): only `pointerover` and `focusin` are observed. Holding an already-hovered/focused node and then leaving it keeps the overlay. Handle the corresponding exit events, accounting for movement within descendants.

16. **NIT — New assertions fail formatting.** [looks.rs:88](/tmp/rv-s8e1-r1/host/web/src/host/looks.rs:88): format the added assertions; `cargo fmt --all -- --check` reports them.

Focused tests and caps passed. In-memory probes reproduced the ordering and cleanup failures. Browser/build verification was not run; the broader driver test run stalled and was stopped.
