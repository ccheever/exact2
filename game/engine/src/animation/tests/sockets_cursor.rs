use super::*;

// Review item 2: a socket follower's global follows the rig's animated Pose,
// which writes none of the follower's (or its children's) rows. The pose
// cursor and the observed globals must still see the child move.
#[test]
fn a_socket_followers_child_in_another_block_is_reported_as_the_rig_animates() {
    let mut w = world();
    let mut model = w.model("rig.model").unwrap().clone();
    model.nodes[0].name = "head".into();
    w.assets
        .models
        .insert("rig.model".into(), crate::asset::ModelAsset::from(model));
    let rig = w.spawn((
        Transform::default(),
        Mesh::asset("rig.model"),
        Animation::play("slow"),
    ));
    let follower = w.spawn((Transform::default(), SocketFollow::new(rig, "head")));
    for _ in 0..crate::PAGE {
        w.spawn(Transform::default());
    }
    let child = w.spawn((Transform::at(0., 1., 0.), crate::Parent(follower)));
    assert_ne!(
        child.index() as usize / crate::PAGE,
        follower.index() as usize / crate::PAGE
    );
    step(&mut w);
    w.step_clock();
    w.propagate();
    let cursor = w.pose_cursor();
    let hash = w.hash();
    let before = w.global(child).unwrap().translation;
    for _ in 0..6 {
        step(&mut w);
        w.step_clock();
        w.propagate();
    }
    let after = w.global(child).unwrap().translation;
    let head = socket(&w, rig, "head").unwrap().position;
    assert!(after.x > before.x, "{before:?} -> {after:?}");
    assert!((Vec3::from(after) - (head + Vec3::Y)).length() < 1e-5);
    let block = (child.index() as usize / crate::PAGE * crate::PAGE) as u32;
    assert!(w.poses_changed_since(cursor).any(|p| p == block));
    assert_ne!(w.hash(), hash);
    // Nothing moved since this cursor: nothing is reported.
    let still = w.pose_cursor();
    assert_eq!(w.poses_changed_since(still).count(), 0);
}
