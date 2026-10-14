# Code review: QUEUE's notes on the web bugs, 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `a8b592830e340ee68325b52d2dcaaa0a3add72cb1be2aecf2535dec382ef4556`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** READY (Astra), NOT READY (Grok).
- **Disposition:** Grok's MAJOR (the stale lead-in) and two MINORs (the contract invocation, what AppKit's notes say) and Astra's NIT taken in the landing commit.

---

**MAJOR** — The lead-in still says the first three are small bugs — `QUEUE.md:906`

That sentence counted the autofocus item, the dotted-link item, and the disabled-button item. This commit removes the first two and says the third is not a bug, and leaves the count in place. The paragraph now has one reclassified item and a grab-bag line, so the lead-in sends a reader looking for three small bugs.

**MINOR** — The contract invocation is not what the scripts or docs use — `QUEUE.md:898`

Nothing in `scripts/` runs `cargo run -p contract --bin contract`. Callers use `cargo run -q -p contract --` (`README.md`, `docs/contract-for-humans.md`, `docs/reference.md`). The package and its binary are both named `contract` (`contract/cli/Cargo.toml`), and `exact-web-js` does depend on that package (`host/web-js/Cargo.toml`), so the crate cycle is real. The parenthetical names a command the tree does not.

**MINOR** — The AppKit notes do not say live scrolling covers momentum — `QUEUE.md:923`

`ScrollEndMac.swift` does end a live scroll on `NSScrollView.didEndLiveScrollNotification`, and `ScrollEndMacTests.swift` posts that notification itself. The 10.9 AppKit release notes say `DidEnd` waits until an animation finishes, with rubber banding as the example. They do not mention momentum. Momentum is a separate `momentumPhase` on scroll-wheel events in the older notes. The "unverified" label and the trackpad check are right; the parenthetical attributes a sentence Apple did not write.

The rest of the edit matches the tree. Bound `autofocus=false` is `data-autofocus` and no HTML attribute (`host/web-js/src/rows.rs`, `host/web-js/rt.js`), landed in `a1ba9277a`, the parent of `ffe36f63a`. Dotted deep links landed in `169ea1efb` (`HEAD^`): a navigation to `/profile/jay.bsky.team` is the document, and `.zip` stays in `FILE_EXTENSIONS` (`scripts/origin.mjs`). LLP 1115.000 D6 does say a disabled custom pressable dims on every host; the web rule is `opacity: .5` (`host/web/index.html:83`) and the Mac multiplies by `0.5` in `syncDisabledLook`. An authored opacity wins on the web because it is inline or a `#exact-root#exact-root .cN` class. The JS-target refusals are in `emit.rs`, `rows.rs`, and `style.rs`, including the dynamic-declaration error.

NOT READY
