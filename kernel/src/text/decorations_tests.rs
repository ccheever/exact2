use super::*;

fn range(start: u32, end: u32) -> Range {
    Range { start, end }
}
fn span(name: &str, start: u32, end: u32) -> Span {
    Span {
        name: name.into(),
        range: range(start, end),
    }
}

#[test]
fn common_edit_uses_scalar_boundaries_and_utf16_offsets() {
    for (before, after, expected) in [
        ("abc", "abc", (3, 3, 3)),
        ("", "abc", (0, 0, 3)),
        ("abc", "", (0, 3, 0)),
        ("abc", "xabc", (0, 0, 1)),
        ("abc", "abxc", (2, 2, 3)),
        ("abc", "abcx", (3, 3, 4)),
        ("abcdef", "abef", (2, 4, 2)),
        ("abcdef", "abXYZef", (2, 4, 5)),
        ("a😀z", "a😎z", (1, 3, 3)),
        ("a😀z", "😀z", (0, 1, 0)),
        ("a😀z", "a😀😀z", (3, 3, 5)),
        ("aaa", "a", (1, 3, 1)),
    ] {
        assert_eq!(
            Edit::between(before, after),
            Edit {
                start: expected.0,
                old_end: expected.1,
                new_end: expected.2,
            },
            "{before:?} -> {after:?}"
        );
    }
}

#[test]
fn boundaries_before_inside_and_after_a_replacement_follow_one_rule() {
    let edit = Edit::between("abcdef", "abXYZef");
    assert_eq!(
        (0..=6).map(|at| edit.boundary(at)).collect::<Vec<_>>(),
        [0, 1, 2, 2, 5, 6, 7]
    );
    let insert = Edit::between("abcdef", "abXcdef");
    assert_eq!(
        (0..=6).map(|at| insert.boundary(at)).collect::<Vec<_>>(),
        [0, 1, 2, 4, 5, 6, 7]
    );
}

#[test]
fn record_identity_includes_its_source_even_when_ranges_are_equal() {
    let first = Decorations {
        value: "@alice".into(),
        spans: vec![span("mention", 0, 6)],
    };
    let second = Decorations {
        value: "@carol".into(),
        ..first.clone()
    };
    assert_ne!(first, second);
    assert!(!first.resolve("@alice").provisional);
    assert!(first.resolve("@carol").provisional);
    assert!(!second.resolve("@carol").provisional);
}

#[test]
fn ranges_reconcile_in_utf16_and_keep_overlap_precedence() {
    let record = Decorations {
        value: "😀@alice end".into(),
        spans: vec![span("mention", 2, 8), span("selected", 3, 6)],
    };
    for (current, expected) in [
        ("x😀@alice end", [range(3, 9), range(4, 7)]),
        ("😀@alice end!", [range(2, 8), range(3, 6)]),
        ("😀@alXice end", [range(2, 9), range(3, 7)]),
        ("😀@a end", [range(2, 4), range(3, 4)]),
    ] {
        let applied = record.resolve(current);
        assert!(applied.provisional);
        assert_eq!(
            applied.spans,
            [
                Span {
                    name: "mention".into(),
                    range: expected[0]
                },
                Span {
                    name: "selected".into(),
                    range: expected[1]
                }
            ],
            "{current:?}"
        );
    }
}

#[test]
fn ranges_are_clamped_to_the_source_before_reconciliation() {
    let record = Decorations {
        value: "@alice".into(),
        spans: vec![
            span("mention", 0, 999),
            span("empty", 3, 3),
            span("reversed", 5, 1),
            span("outside", 99, 100),
        ],
    };
    assert_eq!(record.resolve("@alice").spans, [span("mention", 0, 6)]);
    assert_eq!(record.resolve("x@alice").spans, [span("mention", 0, 7)]);
    assert!(record.resolve("").spans.is_empty());
}
