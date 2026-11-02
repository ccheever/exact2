//! Also runs without Cargo while the lead coordinates workspace membership:
//! rustc --edition=2021 --test apps/messages-stress/data/tests/it/generation.rs -o /tmp/messages-stress-tests
#[path = "../../src/model.rs"]
mod model;

use model::{history, Controls, MAX_DRAFT_CHARS, MAX_REVISION};
use std::collections::HashSet;

#[test]
fn every_preset_has_exactly_the_requested_unique_deterministic_rows() {
    for count in [100, 1_000, 10_000, 100_000] {
        let controls = Controls::new(count, 0, 8).unwrap();
        let rows = history(controls, "").unwrap();
        assert_eq!(rows.len(), count);
        assert_eq!(
            rows.iter().map(|r| &r.id).collect::<HashSet<_>>().len(),
            count
        );
        assert_eq!(
            rows[0],
            history(Controls::new(100, 0, 8).unwrap(), "").unwrap()[0]
        );
        assert!(rows.iter().all(|r| r.body.len() <= 2_048));
    }
}

#[test]
fn rows_include_both_sides_paragraphs_and_different_text_lengths() {
    let rows = history(Controls::new(100, 0, 1).unwrap(), "").unwrap();
    assert!(rows.iter().any(|r| r.outgoing));
    assert!(rows.iter().any(|r| !r.outgoing));
    assert!(rows.iter().any(|r| r.body.contains('\n')));
    assert!(
        rows.iter()
            .map(|r| r.body.len())
            .collect::<HashSet<_>>()
            .len()
            >= 6
    );
    assert_eq!(
        rows,
        history(Controls::new(100, 0, 1).unwrap(), "").unwrap()
    );
}

#[test]
fn each_revision_changes_exactly_the_selected_tail_without_changing_keys() {
    for batch in [1, 8, 32] {
        let before = history(Controls::new(100, 0, batch).unwrap(), "").unwrap();
        let after = history(Controls::new(100, 1, batch).unwrap(), "").unwrap();
        let next = history(Controls::new(100, 2, batch).unwrap(), "").unwrap();
        assert_eq!(
            before
                .iter()
                .zip(&after)
                .filter(|(a, b)| a.body != b.body)
                .count(),
            batch
        );
        assert_eq!(
            after
                .iter()
                .zip(&next)
                .filter(|(a, b)| a.body != b.body)
                .count(),
            batch
        );
        assert!(before.iter().zip(&after).all(|(a, b)| a.id == b.id));
        assert_eq!(&before[..100 - batch], &after[..100 - batch]);
    }
}

#[test]
fn invalid_controls_are_refused_before_any_generation() {
    for count in [0, 99, 101, 100_001, usize::MAX] {
        assert!(Controls::new(count, 0, 1).is_err());
    }
    for batch in [0, 2, 33, usize::MAX] {
        assert!(Controls::new(100, 0, batch).is_err());
    }
    assert!(Controls::new(100, MAX_REVISION + 1, 1).is_err());
    assert!(Controls::new(100, MAX_REVISION, 1).is_ok());
}

#[test]
fn local_echo_is_unicode_safe_bounded_and_independent_of_history() {
    let controls = Controls::new(100, 0, 8).unwrap();
    let plain = history(controls, "").unwrap();
    let draft = "🦀".repeat(MAX_DRAFT_CHARS);
    let echoed = history(controls, &draft).unwrap();
    assert_eq!(echoed.len(), 101);
    assert_eq!(&echoed[..100], &plain);
    assert_eq!(echoed[100].body, draft);
    assert_eq!(echoed[100].id, "local-echo");
    assert!(echoed[100].outgoing);
    assert!(history(controls, &"a".repeat(MAX_DRAFT_CHARS + 1)).is_err());
    assert_eq!(history(controls, "next").unwrap().len(), 101);
}

#[test]
fn manual_pages_cover_the_logical_history_without_claiming_windowing() {
    let controls = Controls::new(1_000, 1, 8).unwrap();
    let full = history(controls, "").unwrap();
    for offset in (0..1_000).step_by(100) {
        let page = model::page(controls, "", offset, false).unwrap();
        assert_eq!(page, full[offset..offset + 100]);
    }
    assert!(model::page(controls, "", 1_000, false).is_err());
    assert!(model::page(controls, "", 1, false).is_err());
    assert_eq!(model::page(controls, "", 0, true).unwrap(), full);
    let large = model::page(Controls::new(100_000, 0, 8).unwrap(), "", 99_900, false).unwrap();
    assert_eq!(large.len(), 100);
    assert_eq!(large[99].id, "m-099999");
}

#[test]
fn cursor_windows_match_the_full_generator_and_keep_echo_at_the_tail() {
    let controls = Controls::new(1_000, 2, 8).unwrap();
    let full = history(controls, "echo").unwrap();
    for (cursor, expected_start, expected_end) in [
        ("0", 0, 200),
        ("300", 200, 400),
        ("799", 699, 899),
        ("900", 800, 1_000),
        ("950", 850, 1_000),
        ("999", 899, 1_000),
        ("100000", 899, 1_000),
        ("", 800, 1_000),
    ] {
        let answer = model::window(controls, "echo", cursor).unwrap();
        let start: usize = answer.earlier.parse().unwrap();
        let end = answer.later.parse::<usize>().unwrap() + 1;
        assert_eq!((start, end), (expected_start, expected_end), "{cursor}");
        assert!(end - start <= model::WINDOW_SIZE);
        assert_eq!(answer.has_earlier, start > 0);
        assert_eq!(answer.has_later, end < controls.count);
        let with_echo = end + usize::from(!answer.has_later);
        assert_eq!(answer.rows, full[start..with_echo]);
    }
    assert!(model::window(controls, "", "-1").is_err());
    assert!(model::window(controls, &"x".repeat(MAX_DRAFT_CHARS + 1), "").is_err());
}

#[test]
fn existing_cursor_keeps_its_window_start_as_history_grows() {
    let original = model::window(Controls::new(1_000, 0, 8).unwrap(), "", "999").unwrap();
    assert_eq!(original.earlier, "899");
    assert_eq!(original.rows.len(), 101);
    assert!(!original.has_later);
    for count in [10_000, 100_000] {
        let grown = model::window(Controls::new(count, 0, 8).unwrap(), "", "999").unwrap();
        assert_eq!(grown.earlier, original.earlier);
        assert_eq!(grown.later, "1098");
        assert_eq!(grown.rows.len(), model::WINDOW_SIZE);
        assert_eq!(&grown.rows[..original.rows.len()], original.rows);
        assert!(grown.has_later);
    }
}
