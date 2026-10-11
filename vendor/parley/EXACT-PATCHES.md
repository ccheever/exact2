# parley 0.11.1 — local patches

Complete crates.io archive, including upstream MIT/Apache licenses and
`.cargo_vcs_info.json`. No feature or dependency change: exact2 builds it
with default features off and `std` on (no `system`, no fontconfig, no
`complex-scripts`; LLP 1085.000 §3).

- Upstream: https://github.com/linebender/parley
- Exact release archive: https://static.crates.io/crates/parley/parley-0.11.1.crate
- Archive SHA256: 22d2ff88bd3f7d68d1d9b09c7e6209f9a8e8c05088295140a2bcf2e9b17038c5
- Upstream VCS revision: eea3503dd6cf17130cbb07348e0ff2c918300e94
- Root `[patch.crates-io]` (and `game/`, `snapback4/`) selects this copy.

Every patch below is upstream status **to send** (LLP 1085.000 §8 Q3). The
first seven were made and measured in the parity spike against Chrome 154
(Mac Studio, `~/bench/dioxus/q5-spike`, `results-spike.md`; one commit and one
diff per patch there); the rest were made on `text/parley`. Remove a patch
when a pinned upstream release supplies the same behaviour and the tests
named here pass.

1. **Base direction (G1).** `BaseDirection {Auto, Ltr, Rtl}` and
   `set_base_direction` on the ranged, style-run and tree builders; an RTL
   base is passed to the bidi resolver (`analyze_text` passed `None`). CSS
   `direction` sets a paragraph's base direction; the first-strong rule is
   `unicode-bidi: plaintext`. Tests: `sharing_tests.rs`
   (`rtl_direction_sets_the_base_direction_and_start_alignment`); spike: LLP
   1053's cases order as Chrome does.
2. **Ligature continuation index (G4).** `break_next`'s ligature loop called
   `Run::get` with a layout cluster index where it takes a run index, summing
   a later run's clusters (`क्षत्रिय श्रृंखला` measured 109.88 against 106.67 px).
3. **South-East Asian scripts without dictionaries (G5).** Without
   `complex-scripts`, SA characters reach the line segmenter as UAX #14 LB1
   resolves them (AL, or CM for marks) by a same-length substitution: ICU4X
   otherwise ended every SA run with a break (before U+200B) and lost LB8's
   after it. Spike: per script, no more missing or extra opportunities than
   cosmic-text on the width-0 set.
4. **RTL multi-character clusters (G10).** Each character keeps its own
   `CharInfo`, the cluster start first in logical order; a break opportunity
   had moved between a Hebrew letter and its mark.
5. **Content widths.** `calculate_content_widths` reads RTL clusters in
   logical order, hangs every trailing space, and counts `text-indent`.
6. **Hanging spaces.** Spaces that end the text or a hard line hang without
   starting an empty line; an overflowing NBSP is not hung and broken after.
7. **UAX #9 L1.** A line's trailing whitespace takes the paragraph's level
   (the item is split when needed), so it no longer shifts visible text.
8. **Soft hyphens (G9).** `push_run` notes each U+00AD cluster with its run
   face's `-` glyph and advance (`LayoutData::soft_hyphens`). A break after
   one is open only if the hyphen fits too; a line that ends there shows the
   hyphen, its advance part of the line (`start_new_line`, recorded in
   `LayoutData::hyphenated` and undone by the next `break_lines`); content
   widths count it at such a break and an unchosen one as zero. Chrome's
   behaviour, as cosmic-text's patch had it. Tests: `css_tests.rs`
   (`a_line_broken_at_a_soft_hyphen_shows_the_faces_hyphen`).
9. **Tab stops.** A tab advances to the next multiple of eight of its run's
   space advance (CSS `tab-size: 8`) from the line's start, set when its line
   is committed and undone by the next `break_lines`; it draws as the space
   glyph (the face has no tab glyph). Parley had no tab stops. Tests:
   `css_tests.rs` (`pre_keeps_spaces_and_tabs_and_breaks_only_at_line_feeds`).
10. **Exact unjustification.** Justification records each widened space's
    advance and `unjustify` restores it, instead of subtracting the
    adjustment again, which was not exact in floating point: a layout broken
    again after a justified width broke differently from a fresh one. Tests:
    `sharing_tests.rs` (`shared_layout_matches_a_fresh_shape_at_every_width`).
11. **Host accessors.** `Run::bidi_level` (the text-flow walker reorders
    clusters by level) and `Layout::capacity_bytes` (the host's residency
    accounting). The unused `LineItemData::is_rtl` is removed.
12. **Blink's fit rule.** `break_next` decides whether content fits as
    Blink's line breaker does: the available width and the indent rounded
    to LayoutUnits (1/64 px), content allowed to end one unit past it
    (`AvailableWidthToFit`, `LayoutUnit::AddEpsilon`). Parley compared
    float advances with the float width, so a line Chrome lays out at
    exactly 200.00 px broke before its last word when its advances summed
    a few thousandths over. Host parity: messages line starts @200
    830 -> 832 of 835, Markdown @200 and @500 +1 each, one message line
    lost to an emoji advance 0.05 px narrower than Chrome's. Tests:
    `css_tests.rs`
    (`content_ending_within_a_64th_of_a_pixel_past_the_width_fits`).
13. **Line box direction.** `set_line_direction` on the three builders
    (`LayoutData::line_level`): a host that resolves bidi levels by the
    first strong character (`BaseDirection::Auto`) inside a box whose CSS
    `direction` it knows gives the box's direction here. Patch 7's L1 level
    for a line's trailing whitespace, which edge that whitespace is
    measured and hangs at, and alignment's start, end and overflow edges
    follow it; the bidi levels of the text itself do not. Without it, an
    `ltr` paragraph whose first strong character is Hebrew hung each
    wrapped line's trailing space at the left, before the visible text in
    visual order (cosmic-text drew no glyph for that space; Chrome, whose
    base is `ltr` there, hangs it at the right). Host parity:
    `bidi-ltr-starts-hebrew`@120 5 -> 2 visual-order inversions (main 2;
    the 2 left are the first-strong base itself), `bidi-arabic-digits`@120
    1 -> 0. Tests: `sharing_tests.rs`
    (`ltr_text_that_starts_rtl_hangs_its_trailing_spaces_at_the_right`).
14. **Tabs in content widths.** `calculate_content_widths` takes a tab to
    its next stop from where it stands in the min- and the max-content
    layout (patch 9's rule), not the advance the last `break_lines` left
    it, and hangs a tab that ends a segment as it hangs a space, as Chrome
    does under `pre-wrap`. Min-content of `"\tend\t"` was the tab's stop
    (33.28 px) where Chrome's is `end` (28.77 px). Host parity, nine tab
    cases against Chrome: min-content 8 -> 9 of 9, max-content 9 of 9.
    Tests: `css_tests.rs`
    (`pre_wrap_min_content_hangs_a_tab_that_ends_a_segment`).
15. **Script_Extensions itemization.** `shape_text` gave every Common
    character the current run's script. A Common character whose
    Script_Extensions exclude that script (`「` after Latin: Bopomofo,
    Hangul, Han, Hiragana, Katakana, Yi) now starts a run of the first of
    them, which the Common characters after it join and a real script
    among them settles without a new run, as Chrome's ScriptRunIterator
    itemizes. In `was 「“quote` the `“` was shaped with `q` and kerned
    (0.97 px), where Chrome shapes it in the bracket's run. Host parity:
    `mixed` max-content 4 -> 5 of 5; two line widths moved closer to
    Chrome, none further. Tests: `css_tests.rs`
    (`a_common_character_after_a_cjk_bracket_is_shaped_in_the_brackets_run`).
16. **Inline glyph iteration.** `#[inline]` on `GlyphIter::next` and
    `Glyph::style_index`. Neither is generic, so a build without LTO (the
    host's `host-dev` profile) called each once per glyph from the host's
    run, cluster and glyph walk: 21 ms of a 1 MiB paragraph's 992,721
    glyphs, 7.5 ms inlined (expo-build-1000). No behaviour changes.

All other archive files are byte-for-byte upstream. The upstream test suite
(not in the archive) passed with patches 1–4 applied; with 4–7, five tests
change as each patch intends (the spike's results list them).
