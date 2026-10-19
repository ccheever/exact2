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
        ..Decorations::default()
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
        ..Decorations::default()
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
        ..Decorations::default()
    };
    assert_eq!(record.resolve("@alice").spans, [span("mention", 0, 6)]);
    assert_eq!(record.resolve("x@alice").spans, [span("mention", 0, 7)]);
    assert!(record.resolve("").spans.is_empty());
}

#[test]
fn stale_metadata_never_hides_or_absorbs_inserted_source() {
    let record = Decorations {
        value: "a **bold** z".into(),
        hidden: vec![Hidden {
            range: range(2, 4),
            reveal: Some(range(2, 10)),
        }],
        atoms: vec![Atom {
            style: span("chip", 4, 8),
            label: "bold".into(),
        }],
        ..Decorations::default()
    };
    // Inserting before both runs shifts them while keeping their metadata.
    let before = record.resolve("xa **bold** z");
    assert_eq!(before.hidden[0].range, range(3, 5));
    assert_eq!(before.hidden[0].reveal, Some(range(3, 11)));
    assert_eq!(before.atoms[0].style.range, range(5, 9));
    // An edit inside one run reveals that run until a fresh record arrives.
    assert!(record.resolve("a *X*bold** z").hidden.is_empty());
    assert!(record.resolve("a **boXld** z").atoms.is_empty());
    // At either edge, conservative reveal prevents new text joining the chip.
    assert!(record.resolve("a **Xbold** z").atoms.is_empty());
    assert!(record.resolve("a **boldX** z").atoms.is_empty());
    // A fresh record can make those exact ranges hidden/atomic again.
    let fresh = Decorations {
        value: "a **boXld** z".into(),
        ..record
    };
    assert_eq!(fresh.resolve(&fresh.value).atoms.len(), 1);
}

#[test]
fn hidden_reveal_and_block_markers_survive_unrelated_edits() {
    let record = Decorations {
        value: "- [ ] task".into(),
        blocks: vec![Block {
            style: span("task", 0, 10),
            marker: Some(Marker::CheckboxOff),
        }],
        hidden: vec![Hidden {
            range: range(0, 6),
            reveal: Some(range(0, 6)),
        }],
        ..Decorations::default()
    };
    let applied = record.resolve("- [ ] task!");
    assert_eq!(applied.blocks[0].marker, Some(Marker::CheckboxOff));
    assert_eq!(applied.blocks[0].style.range, range(0, 10));
    let hidden = applied.hidden[0];
    assert!(hidden.revealed(range(6, 6)));
    assert!(!hidden.revealed(range(7, 7)));
    assert!(hidden.revealed(range(5, 8)));
    for word in ["bullet", "source", "checkbox:off", "checkbox:on", "rule"] {
        assert_eq!(Marker::parse(word).unwrap().as_str(), word);
    }
    assert_eq!(Marker::parse(""), None);
    assert_eq!(Marker::parse("ordered"), None);
}
