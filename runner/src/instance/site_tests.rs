use super::*;
use exact_plan::builder::PlanBuilder;

#[test]
fn reused_frames_preserve_nested_scopes_and_release_rows_on_error() {
    let plan = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1)
        .finish()
        .unwrap();
    let sites = SiteIndex::new(&plan);
    let mut ids = Ids::default();
    let env = Env {
        plan: &plan,
        strings: &[],
        router: None,
        lists: None,
        format: None,
        geometry: None,
        slots: &[],
        derives: &[],
        resources: &[],
        params: &[],
        frames: &[],
        now_ms: 0.0,
        pending_resources: &[],
        failed_resources: &[],
        failed_why: &[],
        pending_mutations: &[],
        store_dependent_derives: &[],
        store_dependent_resources: &[],
    };
    let mut update = Update::new(env, &sites, &mut ids);
    let outer = Rc::new(RefCell::new(BTreeMap::from([(0, Value::Number(7.0))])));
    let inner = Rc::new(RefCell::new(BTreeMap::from([(0, Value::Number(9.0))])));
    let released = [Rc::downgrade(&outer), Rc::downgrade(&inner)];
    let frame = |row, item| Frame {
        row: Some(row),
        region: Some(5),
        item: Some(Value::Number(item)),
        ..Frame::default()
    };
    let result: Result<(), _> = update.with_frame(&[], frame(outer, 1.0), |update, frames| {
        assert_eq!(frames.len(), 1);
        assert_eq!(
            Frame::row_of(frames, 5).unwrap().borrow()[&0],
            Value::Number(7.0)
        );
        update.with_frame(frames, frame(inner, 2.0), |_, frames| {
            assert_eq!(frames.len(), 2);
            assert_eq!(frames[0].item, Some(Value::Number(1.0)));
            assert_eq!(frames[1].item, Some(Value::Number(2.0)));
            assert_eq!(
                Frame::row_of(frames, 5).unwrap().borrow()[&0],
                Value::Number(9.0)
            );
            Err(InstanceError::List("nested failure"))
        })
    });
    assert_eq!(result, Err(InstanceError::List("nested failure")));
    assert!(released.iter().all(|row| row.upgrade().is_none()));

    // The next sibling must see its own scope, without either failed row.
    update
        .with_frame(&[], Frame::default(), |_, frames| {
            assert_eq!(frames, &[Frame::default()]);
            assert!(Frame::row_of(frames, 5).is_none());
            Ok(())
        })
        .unwrap();
}

#[test]
fn site_index_keeps_parent_arm_and_stable_mixed_order() {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let root = b.node(NodeType::View as u8, None, None, 0, &[], &[], None);
    let late = b.node(NodeType::Text as u8, Some(root), None, 5, &[], &[], None);
    let early = b.node(NodeType::Text as u8, Some(root), None, 1, &[], &[], None);
    let yes = b.constant(&Value::Bool(true));
    let unit = b.constant(&Value::Unit);
    let (first, arms) = b.region(RegionKind::When, Some(root), None, 5, yes, unit, 2);
    let (second, _) = b.region(RegionKind::When, Some(root), None, 5, yes, unit, 2);
    let left = b.node(NodeType::Text as u8, None, Some(arms[0]), 0, &[], &[], None);
    let right = b.node(NodeType::Text as u8, None, Some(arms[1]), 0, &[], &[], None);
    let nested = b.node(
        NodeType::Text as u8,
        Some(left),
        Some(arms[0]),
        0,
        &[],
        &[],
        None,
    );
    let plan = b.finish().unwrap();
    let sites = SiteIndex::new(&plan);
    assert_eq!(sites.children(None, None), &[(0, Site::Node(root))]);
    assert_eq!(
        sites.children(Some(root), None),
        &[
            (1, Site::Node(early)),
            (5, Site::Region(first)),
            (5, Site::Node(late)),
            (5, Site::Region(second)),
        ]
    );
    assert_eq!(
        sites.children(None, Some(arms[0])),
        &[(0, Site::Node(left))]
    );
    assert_eq!(
        sites.children(None, Some(arms[1])),
        &[(0, Site::Node(right))]
    );
    assert_eq!(
        sites.children(Some(left), Some(arms[0])),
        &[(0, Site::Node(nested))]
    );
    assert!(sites.children(Some(left), Some(arms[1])).is_empty());
    assert!(sites.children(Some(right), Some(arms[1])).is_empty());
}
