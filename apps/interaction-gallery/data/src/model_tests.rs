use super::*;

#[test]
fn fixture_counts_have_stable_unique_ids_and_bounded_projection() {
    let mut g = Gallery::default();
    let first = g.rows();
    for count in COUNTS {
        g.load(count).unwrap();
        assert_eq!(g.ids().len(), count);
        assert!(g.ids().windows(2).all(|w| w[0] != w[1]));
        assert_eq!(first, g.rows());
        g.page_to(usize::MAX);
        assert!(g.rows().len() <= PAGE_SIZE);
        assert_eq!(g.rows().last().unwrap().id, Id(count as u32 - 1).key());
        assert!(g.rows().iter().all(|p| p.asset.starts_with("assets/")));
    }
    let before = g.clone();
    assert!(g.load(25_001).is_err());
    assert_eq!(g, before);
    assert!(Id::parse("photo-00000").is_ok());
    for key in [
        "photo-1",
        "photo-000001",
        "photo--0001",
        "photo-abcde",
        "../photo-00001",
    ] {
        assert!(Id::parse(key).is_err());
    }
}

#[test]
fn assets_are_distinct_but_the_pool_does_not_grow_with_record_count() {
    let mut assets = std::collections::BTreeSet::new();
    for n in 0..25_000 {
        assets.insert(photo(Id(n), 0).asset);
    }
    assert_eq!(assets.len(), ASSET_COUNT);
    let g = Gallery::default();
    assert_eq!(g.rows()[0].asset, g.rows()[6].asset);
    assert_ne!(g.rows()[0].id, g.rows()[6].id);
}

#[test]
fn preview_and_cancel_never_mutate_order_and_duplicate_commit_is_inert() {
    let mut g = Gallery::default();
    let order = g.ids().to_vec();
    let token = g.lift(Id(2)).unwrap();
    g.before(token, Some(Id(0))).unwrap();
    assert_eq!(g.ids(), order);
    g.cancel(token);
    g.commit(token).unwrap();
    assert_eq!(g.ids(), order);
    let token = g.lift(Id(2)).unwrap();
    g.before(token, Some(Id(0))).unwrap();
    g.commit(token).unwrap();
    assert_eq!(&g.ids()[..4], &[Id(2), Id(0), Id(1), Id(3)]);
    let once = g.clone();
    g.commit(token).unwrap();
    assert_eq!(g, once);
}

#[test]
fn stale_interaction_cannot_commit_a_new_lift_or_close_a_new_photo() {
    let mut g = Gallery::default();
    let old = g.lift(Id(0)).unwrap();
    g.cancel(old);
    let new = g.lift(Id(1)).unwrap();
    g.nudge(new, true).unwrap();
    let before = g.clone();
    g.commit(old).unwrap();
    g.cancel(old);
    assert_eq!(g, before);
    g.open(Id(0)).unwrap();
    let first = g.viewer_token;
    g.adjacent(first, true).unwrap();
    assert_eq!(g.selected, Some(Id(1)));
    g.close(first);
    assert!(g.viewer);
    assert_ne!(g.viewer_token, first);
}

#[test]
fn concurrent_insert_rebases_preview_by_identity_and_removed_target_cancels() {
    let mut g = Gallery::default();
    let t = g.lift(Id(1)).unwrap();
    g.before(t, Some(Id(3))).unwrap();
    g.insert_first().unwrap();
    g.commit(t).unwrap();
    let position = g.position(Id(1)).unwrap();
    assert_eq!(g.ids()[position + 1], Id(3));
    let t = g.lift(Id(1)).unwrap();
    g.before(t, Some(Id(4))).unwrap();
    g.remove(Id(4)).unwrap();
    let order = g.ids().to_vec();
    g.commit(t).unwrap();
    assert_eq!(g.ids(), order);
    assert!(g.moving.is_none());
}

#[test]
fn removed_dragged_record_does_not_reappear_on_late_drop() {
    let mut g = Gallery::default();
    let token = g.lift(Id(3)).unwrap();
    g.remove(Id(3)).unwrap();
    g.commit(token).unwrap();
    assert!(!g.ids().contains(&Id(3)));
    g.insert_first().unwrap();
    assert_eq!(g.ids()[0], Id(100));
    assert!(!g.ids().contains(&Id(3)));
}

#[test]
fn photo_returns_to_current_identity_after_page_change_and_reorder() {
    let mut g = Gallery::default();
    g.open(Id(80)).unwrap();
    let token = g.viewer_token;
    g.page_to(0);
    let movement = g.lift(Id(80)).unwrap();
    assert!(!g.viewer, "an accepted lift ends the photo lifetime");
    let lifted = g.clone();
    g.close(token);
    assert_eq!(g, lifted, "old viewer close cannot finish the new move");
    g.before(movement, Some(Id(25))).unwrap();
    g.commit(movement).unwrap();
    g.open(Id(80)).unwrap();
    let current = g.viewer_token;
    assert_ne!(current, token);
    g.page_to(0);
    g.close(current);
    assert_eq!(g.returned, Return::Item(Id(80)));
    assert!(g.rows().iter().any(|p| p.id == Id(80).key()));
    g.open(Id(80)).unwrap();
    g.remove(Id(80)).unwrap();
    assert!(!g.viewer);
    assert_eq!(g.returned, Return::Removed);
}

#[test]
fn selecting_a_different_open_photo_replaces_its_token_but_same_item_does_not() {
    let mut g = Gallery::default();
    g.open(Id(0)).unwrap();
    let original = g.clone();
    g.select(Id(0)).unwrap();
    assert_eq!(g, original);
    g.select(Id(1)).unwrap();
    assert!(g.viewer);
    assert_eq!(g.selected, Some(Id(1)));
    assert!(g.viewer_token > original.viewer_token);
    let replacement = g.clone();
    g.close(original.viewer_token);
    assert_eq!(g, replacement);
}

#[test]
fn rejected_viewer_selection_preserves_every_field_even_at_token_exhaustion() {
    let mut g = Gallery::default();
    g.open(Id(0)).unwrap();
    let original = g.clone();
    assert!(g.select(Id(99_999)).is_err());
    assert_eq!(g, original);
    g.epoch = u32::MAX;
    let exhausted = g.clone();
    assert!(g.select(Id(1)).is_err());
    assert_eq!(g, exhausted);
    // Keeping the already accepted source needs no replacement token.
    g.select(Id(0)).unwrap();
    assert_eq!(g, exhausted);
}

#[test]
fn lift_closes_viewer_only_after_identity_and_token_validation() {
    let mut g = Gallery::default();
    g.open(Id(0)).unwrap();
    let original = g.clone();
    assert!(g.lift(Id(99_999)).is_err());
    assert_eq!(g, original);
    g.epoch = u32::MAX;
    let exhausted = g.clone();
    assert!(g.lift(Id(1)).is_err());
    assert_eq!(g, exhausted);
    g = original;
    let old = g.viewer_token;
    let moving = g.lift(Id(1)).unwrap();
    assert!(!g.viewer);
    assert_eq!(g.selected, Some(Id(1)));
    assert_eq!(g.moving.unwrap().token, moving);
    let accepted = g.clone();
    g.close(old);
    assert_eq!(g, accepted);
}

#[test]
fn reset_and_navigation_cancel_interactions_without_reusing_tokens() {
    let mut g = Gallery::default();
    let old = g.lift(Id(2)).unwrap();
    g.mode(Mode::Sheet);
    assert!(g.moving.is_none());
    let order = g.ids().to_vec();
    g.page_to(3);
    assert_eq!(g.ids(), order);
    g.load(1000).unwrap();
    let current = g.lift(Id(3)).unwrap();
    assert!(current > old);
    g.commit(old).unwrap();
    assert!(g.moving.is_some());
}

#[test]
fn empty_collections_and_bounds_are_defined() {
    let mut g = Gallery::default();
    for id in g.ids().to_vec() {
        g.remove(id).unwrap();
    }
    assert!(g.selected_photo().is_none());
    assert!(g.rows().is_empty());
    assert_eq!(g.pages(), 1);
    g.page_to(usize::MAX);
    assert_eq!(g.page, 0);
    g.insert_first().unwrap();
    assert_eq!(g.ids(), &[Id(100)]);
    g.load(MAX_ITEMS).unwrap();
    let before = g.clone();
    assert!(g.insert_first().is_err());
    assert_eq!(g, before);
}

#[test]
fn structural_revision_exhaustion_refuses_every_order_mutation_atomically() {
    for op in ["load", "insert", "remove", "place"] {
        let mut g = Gallery::default();
        g.open(Id(2)).unwrap();
        let token = if op == "place" {
            let token = g.lift(Id(2)).unwrap();
            g.before(token, Some(Id(0))).unwrap();
            token
        } else {
            0
        };
        g.revision = u32::MAX;
        let before = g.clone();
        match op {
            "load" => {
                let _ = g.load(1000);
            }
            "insert" => {
                let _ = g.insert_first();
            }
            "remove" => {
                let _ = g.remove(Id(2));
            }
            "place" => {
                let _ = g.commit(token);
            }
            _ => unreachable!(),
        }
        assert_eq!(
            g, before,
            "{op} must reserve revision before changing any field"
        );
    }
}

#[test]
fn last_structural_revision_is_used_once_and_never_saturates_or_wraps() {
    let mut g = Gallery {
        revision: u32::MAX - 1,
        ..Gallery::default()
    };
    g.insert_first().unwrap();
    assert_eq!(g.revision, u32::MAX);
    let before = g.clone();
    assert!(g.remove(Id(0)).is_err());
    assert_eq!(g, before);
    assert!(g.load(100).is_err());
    assert_eq!(
        g, before,
        "failed reset must not consume an interaction token"
    );
}

#[test]
fn load_token_exhaustion_preserves_structural_revision_and_complete_state() {
    let mut g = Gallery::default();
    g.open(Id(3)).unwrap();
    g.page_to(2);
    g.epoch = u32::MAX;
    let before = g.clone();
    assert!(g.load(1000).is_err());
    assert_eq!(g, before);
}

#[test]
fn atomic_reorder_checks_revision_and_both_keys_before_changing_any_field() {
    let mut g = Gallery::default();
    g.open(Id(7)).unwrap();
    g.page_to(2);
    let before = g.clone();
    for (item, target, revision, reason) in [
        (Id(2), Some(Id(0)), 1, ReorderRefusal::StaleRevision),
        (Id(99999), Some(Id(0)), 0, ReorderRefusal::MissingItem),
        (Id(2), Some(Id(99999)), 0, ReorderRefusal::MissingBefore),
    ] {
        assert_eq!(
            g.reorder(item, target, revision),
            ReorderResult::Refused(reason)
        );
        assert_eq!(g, before);
    }
    assert_eq!(g.reorder(Id(2), Some(Id(0)), 0), ReorderResult::Moved);
    assert_eq!(&g.ids()[..4], [Id(2), Id(0), Id(1), Id(3)]);
    assert_eq!(g.revision, 1);
    assert_eq!(g.page, before.page);
    assert_eq!(g.selected, before.selected);
    assert_eq!(g.viewer_token, before.viewer_token);
    assert_eq!(g.viewer, before.viewer);
    let moved = g.clone();
    assert_eq!(
        g.reorder(Id(2), Some(Id(0)), 0),
        ReorderResult::Refused(ReorderRefusal::StaleRevision)
    );
    assert_eq!(g, moved);
}

#[test]
fn atomic_reorder_normalizes_noops_at_max_but_refuses_real_change() {
    let mut g = Gallery {
        revision: u32::MAX,
        ..Gallery::default()
    };
    let before = g.clone();
    for (item, target) in [(Id(2), Some(Id(2))), (Id(2), Some(Id(3))), (Id(99), None)] {
        assert_eq!(g.reorder(item, target, u32::MAX), ReorderResult::Unchanged);
        assert_eq!(g, before);
    }
    assert_eq!(
        g.reorder(Id(2), Some(Id(0)), u32::MAX),
        ReorderResult::Refused(ReorderRefusal::RevisionExhausted)
    );
    assert_eq!(g, before);
}

#[test]
fn unchanged_manual_place_can_finish_at_max_without_changing_order() {
    let mut g = Gallery::default();
    let token = g.lift(Id(2)).unwrap();
    g.revision = u32::MAX;
    let ids = g.ids().to_vec();
    g.commit(token).unwrap();
    assert_eq!(g.ids(), ids);
    assert_eq!(g.revision, u32::MAX);
    assert!(g.moving.is_none());
}

#[test]
fn atomic_reorder_refuses_manual_move_and_uses_current_order_after_changes() {
    let mut g = Gallery::default();
    let token = g.lift(Id(3)).unwrap();
    g.before(token, Some(Id(7))).unwrap();
    let manual = g.clone();
    assert_eq!(
        g.reorder(Id(4), None, 0),
        ReorderResult::Refused(ReorderRefusal::ManualMove)
    );
    assert_eq!(g, manual);
    g.cancel(token);
    g.insert_first().unwrap();
    g.remove(Id(1)).unwrap();
    let revision = g.revision;
    assert_eq!(
        g.reorder(Id(7), Some(Id(3)), revision),
        ReorderResult::Moved
    );
    let position = g.position(Id(7)).unwrap();
    assert_eq!(g.ids()[position + 1], Id(3));
    assert_eq!(g.ids()[0], Id(100));
    assert!(!g.ids().contains(&Id(1)));
}
