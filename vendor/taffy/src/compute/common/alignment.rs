//! Generic CSS alignment code that is shared between both the Flexbox and CSS Grid algorithms.
use crate::style::{AlignContent, AlignContentKeyword, AlignItems, AlignItemsKeyword, AlignmentSafety};

/// Resolve the `safe`/`unsafe` overflow-position fallback for a self-level alignment value
/// (used by `align-self` / `justify-self`-style sites and by absolutely-positioned items in
/// flex/grid). If the alignment subject overflows its alignment container and the requested
/// alignment is `safe`, fall back to logical `Start` per CSS Box Alignment
/// <https://www.w3.org/TR/css-align-3/#overflow-values>. Otherwise drop the safety modifier
/// and return the bare keyword.
#[inline]
pub(crate) fn resolve_self_alignment_safety(alignment: AlignItems, overflows: bool) -> AlignItemsKeyword {
    if matches!(alignment.safety, AlignmentSafety::Safe) && overflows {
        AlignItemsKeyword::Start
    } else {
        alignment.keyword
    }
}

/// Resolve distribution and overflow fallbacks for the given [`AlignContent`] value, returning the
/// bare position keyword the alignment math should use.
///
/// Follows <https://www.w3.org/TR/css-align-3/> and
/// <https://github.com/w3c/csswg-drafts/issues/10154>, except for the Chrome-measured
/// `space-between` fallback described below.
pub(crate) fn apply_alignment_fallback(
    free_space: f32,
    num_items: usize,
    alignment_mode: AlignContent,
) -> AlignContentKeyword {
    let mut keyword = alignment_mode.keyword;
    let mut is_safe = matches!(alignment_mode.safety, AlignmentSafety::Safe);

    // 1. If there is only a single item being aligned or the items overflow the container, the
    //    distributed alignment keywords (`stretch`, `space-*`) fall back to a positional keyword.
    //    EXACT PATCH 53: Chrome 151 uses unsafe flex-start for stretch and space-between,
    //    preserving the flex-start edge under wrap-reverse and reversed main axes. CSS Align
    //    §4.3 specifies flex-start for stretch, but safe flex-start for space-between.
    //    Space-around and space-evenly retain their safe-center fallback.
    //    https://www.w3.org/TR/css-align-3/#distribution-values
    if num_items <= 1 || free_space <= 0.0 {
        (keyword, is_safe) = match keyword {
            AlignContentKeyword::Stretch | AlignContentKeyword::SpaceBetween => (AlignContentKeyword::FlexStart, false),
            AlignContentKeyword::SpaceAround | AlignContentKeyword::SpaceEvenly => (AlignContentKeyword::Center, true),
            other => (other, is_safe),
        };
    }

    // 2. Safe alignment falls back to `Start` whenever the alignment subject would overflow the
    //    alignment container.
    if free_space <= 0.0 && is_safe {
        keyword = AlignContentKeyword::Start;
    }

    keyword
}

/// Generic alignment function that is used:
///   - For both align-content and justify-content alignment
///   - For both the Flexbox and CSS Grid algorithms
///
/// CSS Grid does not apply gaps as part of alignment, so the gap parameter should
/// always be set to zero for CSS Grid.
pub(crate) fn compute_alignment_offset(
    free_space: f32,
    num_items: usize,
    gap: f32,
    alignment_mode: AlignContentKeyword,
    layout_is_flex_reversed: bool,
    is_first: bool,
) -> f32 {
    if is_first {
        match alignment_mode {
            AlignContentKeyword::Start => 0.0,
            AlignContentKeyword::FlexStart => {
                if layout_is_flex_reversed {
                    free_space
                } else {
                    0.0
                }
            }
            AlignContentKeyword::End => free_space,
            AlignContentKeyword::FlexEnd => {
                if layout_is_flex_reversed {
                    0.0
                } else {
                    free_space
                }
            }
            AlignContentKeyword::Center => free_space / 2.0,
            AlignContentKeyword::Stretch => 0.0,
            AlignContentKeyword::SpaceBetween => 0.0,
            AlignContentKeyword::SpaceAround => {
                if free_space >= 0.0 {
                    (free_space / num_items as f32) / 2.0
                } else {
                    free_space / 2.0
                }
            }
            AlignContentKeyword::SpaceEvenly => {
                if free_space >= 0.0 {
                    free_space / (num_items + 1) as f32
                } else {
                    free_space / 2.0
                }
            }
        }
    } else {
        let free_space = free_space.max(0.0);
        gap + match alignment_mode {
            AlignContentKeyword::Start
            | AlignContentKeyword::FlexStart
            | AlignContentKeyword::End
            | AlignContentKeyword::FlexEnd
            | AlignContentKeyword::Center
            | AlignContentKeyword::Stretch => 0.0,
            AlignContentKeyword::SpaceBetween => free_space / (num_items - 1) as f32,
            AlignContentKeyword::SpaceAround => free_space / num_items as f32,
            AlignContentKeyword::SpaceEvenly => free_space / (num_items + 1) as f32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distribution_fallback_preserves_flex_start_edge() {
        let mut wrong = Vec::new();
        for alignment in [AlignContent::STRETCH, AlignContent::SPACE_BETWEEN] {
            for num_items in [1, 2] {
                for free_space in [-10.0, 0.0, 10.0] {
                    if num_items > 1 && free_space > 0.0 {
                        continue;
                    }
                    let keyword = apply_alignment_fallback(free_space, num_items, alignment);
                    for reversed in [false, true] {
                        let offset = compute_alignment_offset(free_space, num_items, 0.0, keyword, reversed, true);
                        let want = if reversed { free_space } else { 0.0 };
                        if offset != want {
                            wrong.push((alignment, num_items, free_space, reversed, offset, want));
                        }
                    }
                }
            }
        }
        assert!(wrong.is_empty(), "{wrong:?}");
    }

    #[test]
    fn distribution_fallback_keeps_safety_and_positive_spacing() {
        for alignment in [AlignContent::SPACE_AROUND, AlignContent::SPACE_EVENLY] {
            assert_eq!(apply_alignment_fallback(-10.0, 2, alignment), AlignContentKeyword::Start);
            assert_eq!(apply_alignment_fallback(10.0, 1, alignment), AlignContentKeyword::Center);
        }
        for alignment in [
            AlignContent::SAFE_FLEX_START,
            AlignContent::SAFE_FLEX_END,
            AlignContent::SAFE_CENTER,
        ] {
            assert_eq!(apply_alignment_fallback(-10.0, 2, alignment), AlignContentKeyword::Start);
            assert_eq!(apply_alignment_fallback(10.0, 2, alignment), alignment.keyword);
        }
        for alignment in [
            AlignContent::FLEX_START,
            AlignContent::FLEX_END,
            AlignContent::CENTER,
        ] {
            assert_eq!(apply_alignment_fallback(-10.0, 2, alignment), alignment.keyword);
        }
        for alignment in [
            AlignContent::STRETCH,
            AlignContent::SPACE_BETWEEN,
            AlignContent::SPACE_AROUND,
            AlignContent::SPACE_EVENLY,
        ] {
            assert_eq!(apply_alignment_fallback(10.0, 2, alignment), alignment.keyword);
        }
    }
}
