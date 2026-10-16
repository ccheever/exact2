# Code review: exact-tamber crate, round 2, 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `8646745a97ff601115082c4db233a1f75295d798106fd62966edcc5ce5ad11ec`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** fix from both (Astra 5 MAJOR, 4 MINOR; Grok 1 MINOR and three edges).
- **Disposition:** all fixed in round 3: typed percentages, more probes, the kernel platform-name rule, unclamped relative components, powerless hues refused, typed legacy commas, hidden under-dark whens, no inert report beside other refusals, Contract escapes, a zero-sum mix, longer hue at -180

---

I'll review the round-2 commit against the round-1 findings and the RFC, starting with the review brief, the diff, and the commit message.This is a defect review of the round-2 delta, not a branch-wide review. I'll read the commit message, the RFC, and the fix diff before judging whether the round-1 issues are actually closed.Tests passed. Next I’ll read the colour folder, the influence check, and the RFC folding rules to see whether the round-1 fixes match the spec.Auto-compacting conversation (80% full)...
Conversation compacted.
The review is ready to close. I’ll pin the zero-sum guard’s line in `color_fn.rs` and then give the verdict.The zero-sum guard is on line 540. I’ll check the platform-name check and the influence probes so the secondary notes stay accurate.Refusing `none` is right. Refusing a zero-sum `color-mix` is wrong.

A missing component has to stay missing. CSS Color 4 keeps modern `none` in the computed value, and interpolation uses the other color’s component there. `Wide` has no missing-component bit, and `exact_color::parse` already turns `none` into `0`, so folding it would change later mixes (a missing hue averaged with `0` instead of taking the other hue). The diagnostic at `tamber/src/color_fn.rs:113` says it is not folded yet. That matches the representation.

A zero-sum mix is a defined color this IR can store. CSS Color 5 (editor’s draft, 8 Oct 2026) removed the old “invalid if the percentages sum to zero” rule. The worked example `color-mix(in oklch, teal 0%, olive 0%)` is a 50/50 mix in oklch with alpha `0`: normalize does not scale a zero total, the leftover is 100%, the alpha multiplier is `1 - leftover` (`0`), and the mix progress is `0.5`. That is the mixed color at alpha `0`, which `Wide` can hold. `tamber/src/color_fn.rs:540` rejects it:

```540:546:tamber/src/color_fn.rs
    if !(0.0..=100.0).contains(&p) || !(0.0..=100.0).contains(&q) || p + q <= 0.0 {
        return fail(
            "tamber-syntax",
            format!("`{text}`: percentages are 0–100% and not both 0"),
        );
    }
    let (t, multiplier) = (q / (p + q), ((p + q) / 100.0).min(1.0));
```

Given the `0..=100` test, `p + q <= 0` is exactly the both-zero case, and it exists so the next line does not divide by zero. The in-range half of the guard is right (a percentage outside 0–100% stays invalid, including negatives). Both zero should take progress `0.5` and multiplier `0`. The guard was already in the old `fold.rs` and the rewritten commit message lists it as deliberate; the draft it is following has moved.

The round-1 fixes are in place and backed by the corpus goldens. Relative `color(srgb)` percentages use reference `1`, while legacy `rgb` channels scale by 255 and divide back. Absolute `calc()` percentages resolve against the channel (`rgb(calc(50%))` is `#800000ff`). Calc is typed: a hue percentage, an angle in `rgb`, and a number-plus-angle sum are refused. A legacy input keeps its unquantised channel for a further fold. Mixing is premultiplied in the mix space, so an even sRGB red/blue mix is `color(srgb 0.5 0 0.5)`. `color()` dial defaults, including `display-p3`, are accepted. `currentcolor` is `tamber-kind` and is not stored. `scheme` and `contrast` are refused as app axes. Dial-inert follows value influence: a dial discarded on the unused `light-dark` side is inert, and one that changes a token is not. CSS-wide keywords are reserved names. Values hidden by exhaustive `when`s are still folded. Bare role spellings are case-insensitive. Escaped quotes in metadata, spaces around `=` in `when` and `under`, a malformed dial, and a refused token or dial (one diagnostic, no second unknown-name) all behave as claimed. `under-light` compares linear sRGB, so `#ffffff` and `color(srgb 1 1 1)` agree. `from` and the channel keywords are ASCII case-insensitive. `cargo test -p exact-tamber --offline` passed (library, corpus, parity).

Three edges are real and smaller than the zero-sum refusal. `platform()` in `tamber/src/fold.rs:335` accepts any non-empty name, while `native_name_ok` (`kernel/src/style/roles.rs:279`) requires an iOS or macOS name such as `labelColor`; `ios foo` folds here and `parse_platform` returns nothing. The influence probes (`tamber/src/check.rs:631`) are opaque black and white, plus numeric range endpoints, so a color dial used only through its alpha, or a number formula that matches the default at both endpoints and differs in between, can be marked inert. Longer-hue at exactly `d == -180` (`color_fn.rs:589`) excludes that bound; `color/src/mix.rs:299` includes it, so `h1 = 180`, `h2 = 0`, `t = 0.5` lands on hue 90 here and 270 in the kernel. Both arcs are 180°. Omitting `in` on `color-mix` is still refused; the spec’s missing method is oklab, and writing `in oklab` is the documented workaround.

VERDICT: fix (MINOR tamber/src/color_fn.rs:540)
