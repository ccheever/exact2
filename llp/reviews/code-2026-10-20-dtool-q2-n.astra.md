# Code review: a folded :disabled or :checked row read as its base (LLP 1122 queue item (2)), the narrow verification round of the post-round fixes, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree at `311f838635`.
- **Method:** one brief (sha256 `8c8b71a86632ad3dc5fec8ee3db4fdfdc30cc146c75cbabb28a7a9362a53c7d3`) quoting round 2's two reviews verbatim, asking of each finding "closed? broke anything?"; commit 311f838635. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** 1 BLOCKER (new: param identity by suffix); the round 2 BLOCKER and SHOULD-FIXes closed; 1 NIT open.
- **Disposition:** taken in the commit named in the sign-off request (and a second narrow round on it):
  - (1) A param base's full name is the compiler's: the map lists each source's `home` (its components' identities' home, LLP 1121 D15, `Sources::homes`), and the join builds `<home>#<component>.<name>` from the base's file and the component it is written in, matching exactly; no suffix match, no runtime names. Another file's component of that name is never it; a param not read, or a file with no home, is refused; tested (an app param beside another file's, only another's cached, a homeless map, a vendored package).
  - (7) Two parts of one state are told apart by their full file and line.
  - (2)-(6) closed, as the review says.

HEAD closes the whitespace blocker, but introduces a **new param-targeting blocker**.

1. **BLOCKER — A3/B2 are only partially closed; param identity is now lost.** Package source recovery works at [tamber-inspect.mjs:469](/tmp/rv-dt-q2n/scripts/tamber-inspect.mjs:469), but [writtenToken:496](/tmp/rv-dt-q2n/scripts/tamber-inspect.mjs:496) matches only `#Card.fill`, ignoring the source file.

   An in-memory join for a package’s folded `Card.fill`, with only `b.contract#Card.fill` cached and the disabled literal showing, offers **`b.contract#Card.fill`**. That cache condition is reachable: a new plan’s selection retains old param names while `describe` runs ([tamber-server.mjs:238](/tmp/rv-dt-q2n/scripts/tamber-server.mjs:238), [398](/tmp/rv-dt-q2n/scripts/tamber-server.mjs:398)). The edit can therefore target another component.

   With both `app.contract#Card.fill` and `b.contract#Card.fill` loaded, HEAD instead refuses the app’s disabled base as ambiguous; **HEAD’s parent resolves it correctly**. Recover the compiler’s full identity from the source file; suffix matching is insufficient.

2. **BLOCKER — A1 closed.** [tamber-inspect.mjs:490](/tmp/rv-dt-q2n/scripts/tamber-inspect.mjs:490) trims before matching one complete param identifier. Leading/trailing whitespace resolves correctly; an empty table refuses it; `param.fill param.edge` retains the shorthand’s template edit. Confirmed against the parent with in-memory probes.

3. **SHOULD-FIX — A2/B1 closed in the map.** [states.rs:520](/tmp/rv-dt-q2n/contract/lower/src/states.rs:520) keeps both same-state parts. The earlier class provenance survives when the later expression’s `none` arm exposes it. The value expression and checked-under-disabled order are unchanged.

4. **SHOULD-FIX — A4 closed.** [tamber-inspect.mjs:502](/tmp/rv-dt-q2n/scripts/tamber-inspect.mjs:502) normalizes the prefixed role spelling and returns the canonical token text. `-EXACT-SECONDARY-LABEL` now retains the `secondary-label` alternative.

5. **NIT — A5 closed.** The replacement fixture uses overlapping `border-color`/`border-top-color` attributes, which survive class filtering, and asserts both top-side parts plus the class-only right side ([source_map.rs:1027](/tmp/rv-dt-q2n/contract/cli/tests/it/source_map.rs:1027), [1060](/tmp/rv-dt-q2n/contract/cli/tests/it/source_map.rs:1060)). It now exercises the relevant provenance path.

6. **NIT — B3 closed.** [tamber-inspect.mjs:455](/tmp/rv-dt-q2n/scripts/tamber-inspect.mjs:455) no longer passes the host row’s unread-param flag into a part’s edit. The literal part remains editable while the base is refused; a param-valued part still edits its own source.

7. **NIT — New inspector labels do not always distinguish parts.** [inspector.ts:111](/tmp/rv-dt-q2n/apps/design-tool/inspector.ts:111) uses basename plus line. Parts at `styles/card.contract:6` and `ui/card.contract:6` both become `:disabled border-top-color · card.contract:6`, including identical row keys/test IDs. Confirmed through `selectionView`. The runtime preserves repeated keys, so this is ambiguous labeling/addressing, not demonstrated row loss.

No other regression found in base/part addresses, cross-file style rungs, literal displays, color/tint mapping, placeless parts, or active/look handling. The compiler diff changes only provenance metadata; **plan-byte equality was not remeasured**.

No files changed. Only source inspection and in-memory probes ran; no builds or test suites.
