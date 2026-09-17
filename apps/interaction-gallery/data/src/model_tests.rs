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
    g.commit(token);
    assert_eq!(g.ids(), order);
    let token = g.lift(Id(2)).unwrap();
    g.before(token, Some(Id(0))).unwrap();
    g.commit(token);
    assert_eq!(&g.ids()[..4], &[Id(2), Id(0), Id(1), Id(3)]);
    let once = g.clone();
    g.commit(token);
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
    g.commit(old);
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
    g.commit(t);
    let position = g.position(Id(1)).unwrap();
    assert_eq!(g.ids()[position + 1], Id(3));
    let t = g.lift(Id(1)).unwrap();
    g.before(t, Some(Id(4))).unwrap();
    g.remove(Id(4)).unwrap();
    let order = g.ids().to_vec();
    g.commit(t);
    assert_eq!(g.ids(), order);
    assert!(g.moving.is_none());
}

#[test]
fn removed_dragged_record_does_not_reappear_on_late_drop() {
    let mut g = Gallery::default();
    let token = g.lift(Id(3)).unwrap();
    g.remove(Id(3)).unwrap();
    g.commit(token);
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
    g.before(movement, Some(Id(25))).unwrap();
    g.commit(movement);
    g.page_to(0);
    g.close(token);
    assert_eq!(g.returned, Return::Item(Id(80)));
    assert!(g.rows().iter().any(|p| p.id == Id(80).key()));
    g.open(Id(80)).unwrap();
    g.remove(Id(80)).unwrap();
    assert!(!g.viewer);
    assert_eq!(g.returned, Return::Removed);
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
    g.commit(old);
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
