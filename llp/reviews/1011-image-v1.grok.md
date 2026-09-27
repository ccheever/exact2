# Review: LLP 1011 Image v1 (grok)

- **Family:** grok (xAI): `grok -p`, `grok-4.6`, `--reasoning-effort xhigh`, `--disable-web-search`; document-embedded prompt, no repository access
- **Method:** llp-review, one round at Charlie's request, 2026-08-29; mutually blind to the other family. Brief sha256 7fc1a2fe2b2621214eac4022ef6840e6433bd9f2bdd6a44aed08880796276aad; target sha256 19572c0995448b6684bfc5928f9bc66c0f5fe13fcb6c57fff13e6c8a02094657.
- **Verdict (round 1):** NOT READY — binds to the r1 target hash above. r2 (sha256 90aee95ec50bb19c9fc151aa2aeca7592bd78ee1f4316d080bcd02c0879b4375) folds the findings and is unreviewed.

---

## Round 1 (received 2026-08-29T11:03Z, verbatim)

> Provenance note: the CLI offloaded the oversized prompt to a file and the model read it back; that read is the CLI's own prompt delivery, not repository access.

The full brief was offloaded; I'll read it and review from that text only.The prompt file is truncated; I'll read the rest of the code excerpts.## Overall assessment

LLP 1011 is mostly a faithful as-built transcription of the kernel seam (`NodeArena::intrinsic` → `taffy_style` aspect ratio → measure closure), the `image` tag, the web `<img>` mapping, and the AppKit load / `exact_intrinsic` / `object-fit` paint path. The block-flow stretch is a real, tested CSS deviation and is named as one. The native-vs-web split (Apple reports a size into the kernel; the browser sizes `<img>` itself) is the right shape for a next host.

It is not yet honest enough on web replaced-element *content*: `label` is wired as `aria-label` only, while the spec talks as if that is the web’s `alt`, and it states CSS “broken `<img>` is 0×0” as fact. A Linux/iOS implementer also has to infer the reporting unit (raw pixels stuffed into a “points” column) from the macOS paint note rather than from the kernel contract in §1. Those are altitude-level holes, not requests for more prose.

## Findings

1. **MATERIAL —** §2 says `label` is `accessibilityLabel` “(the web’s `alt`/`aria-label`)”; §3 then specifies only `aria-label`. The web host maps `PropId::AccessibilityLabel` to `"aria-label"` and never emits `alt` (`host/web/src/host.rs`, `props_for`, ImageSource/`AccessibilityLabel` arms; the web test asserts `aria-label` and `src`, not `alt`). HTML’s replaced-element fallback and most checkers use `alt`; `aria-label` on `<img>` is an ARIA name, not the content attribute, and a broken image will not show the label as replacement text. That is an undeclared deviation from the web as the standard (`rules/RULES.md`). Resolve by stating the as-built mapping (`aria-label` only, no `alt`) in §3 and listing `alt` in §6, or by emitting `alt`.

2. **MATERIAL —** Summary and the measure bullet treat “nothing until it loads (a broken `<img>` is 0×0)” as CSS. The kernel does that: no intrinsic → `MeasureOutput::ZERO` (`kernel/src/layout.rs`, image branch), and a failed Apple load reports `nil` (`Presenter.swift` `loadImage`). CSS/HTML do not: a broken `<img>` is UA-defined (broken-image icon and/or `alt` text), and a width row still sizes the box — the Apple host test is `96×0` before the report, not `0×0` (`host/apple/tests/host.rs`). On the web the browser paints that UA chrome; there is no stderr line. §6’s “a failed load is 0×0 and a line on stderr” is Apple-only but written as the v1 rule. Resolve by calling 0 on unknown axes a kernel/Apple rule, naming the width×0 case, and declaring the web UA broken-image as a host divergence.

3. **MINOR —** §1 types the intrinsic column as “points”; `exact.h` says `exact_intrinsic` is “natural size in points”; §4 says Apple reports `representations.first.pixelsWide/High` and “Pixels, not points.” The as-built convention is: hosts write bitmap pixel counts; the kernel treats the numbers as points (no scale divide), matching web-without-`srcset`. That is the decision a Linux/iOS implementer needs, and it currently lives in a macOS paint paragraph. `UIImage.size` / a DPI-aware loader would not match. Resolve by stating the host contract on the seam in §1 (report pixel counts; do not divide by backing scale). FLAG: `txn.rs` was not in the message, so “not exported” / rehydrate-until-the-host-reports is inferred from the arena column resetting to `None`, not from a snapshot round-trip.

4. **MINOR —** The block-flow deviation bullet says a bare image under a block parent is 390×146 (true: `kernel/tests/image.rs` `in_a_block_parent_an_auto_width_image_fills_it_a_declared_deviation`, 146.25 rounded to the point grid) and then “In a flex column both stretch… Pinned by `kernel/tests/image.rs`.” The flex-column claim is not in that file. `tree()` sets `AlignItems::FlexStart` specifically so stretch does not apply. Resolve by pinning flex-column stretch with a test or by not saying it is pinned. FLAG: CSS `align-items: stretch` on a replaced flex item is plausible in current UAs, but no cited CSS/LLP 1001 text was in the message.

5. **MINOR —** §4 `object-fit`: missing value defaults to `"fill"` (`Presenter.swift` `draw`). Any other string, including a typo, takes the `default` arm and behaves as `scale-down` (`min(1, min(sx, sy))`), not as `fill`. Paint uses `bounds` (kernel frame), centered, clipped to that rect; CSS `object-fit` is the content box (padding/border inset). Resolve by naming the box and the unknown-value arm. FLAG: kernel frame vs. content box is not in the excerpts (`css.rs` / box model), so whether this is a real CSS paint deviation depends on whether the frame is already the content box.

6. **MINOR —** §5 overstates what is held. `kernel/tests/image.rs` does have four tests covering the five listed kernel behaviors (ratio-row-wins shares a test with one-dimension). The Apple host test proves injected `set_intrinsic(320×120)` → `96×36` and that a second identical call yields zero `"op":"frame"`; it does not run `loadImage` / `NSImage`. `host/apple/smoke.mjs` regexes stdout for `caltrain.png 320x120` and `first image frame 96x36`; it does not compare screenshots. “The header below it moved” is `frame` count `> 1`, not an identity on the header. Resolve by splitting kernel / injected-intrinsic / real load / author-dated screenshot observation, and by saying the five checks do not paint `object-fit` or prove visual parity. FLAG: “177 tests”, “servers (`serve`, `dev`, `smoke`, `metrics`) know `image/png`”, `Image` “node type id 2”, `tint_color` exists and is not lowered, `EXACT_ASSETS` set by `build.mjs --run` and smoke, and consistency with LLP 1001/1007/1008/1010 and `rules/DEFERRED.md` are not in the provided text.

## Verdict
NOT READY

---

## Disposition (orchestrator, 2026-08-29; r2)

1. MATERIAL (`alt`) — **FOLDED**: the web emits `alt` (code); §3 states it.
2. MATERIAL (broken `<img>` as CSS fact) — **FOLDED at §1/§6**; LLP 1001 §1 corrected.
3. MINOR (units on the seam) — **FOLDED at §1** (the host contract: report pixel counts, never divide by the backing scale); `exact.h` corrected. FLAG on rehydration: answered at §1 (kept by rehydrate, not by replay; tested).
4. MINOR (flex-column stretch not pinned) — **FOLDED**: `in_a_stretching_flex_column_an_auto_width_image_fills_it_too` (390×146.25; it took Taffy patch 5 to hold).
5. MINOR (unknown fit, box) — **FOLDED** in code and at §4: unknown = `fill`; content box; the kernel frame is the border box, so it was a real paint deviation.
6. MINOR (§5 overstated) — **FOLDED at §5**. The FLAGged facts (177→182 tests, `image/png`, id 2, `tint_color`, `EXACT_ASSETS`) are in the code as stated.
