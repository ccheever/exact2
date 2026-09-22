use super::*;
use crate::{BodyKind, Physics};
use exact_game::{Game, Input, Paranoid, Sim, Vec3, World};

// EXPHYS v2's final BroadPhaseBvh serde field is the pending bool. Decode a
// re-encoded value after changing just that field: a v1 omission negative control.
fn pending(p: &Physics) -> bool {
    let mut saved = p.executor.0.borrow_mut();
    let bytes = bincode::DefaultOptions::new()
        .serialize(&saved.live().rapier.broad_phase)
        .unwrap();
    *bytes.last().unwrap() == 1
}
fn clear_pending(p: &Physics) {
    let mut saved = p.executor.0.borrow_mut();
    let r = &mut saved.live().rapier;
    let mut bytes = bincode::DefaultOptions::new()
        .serialize(&r.broad_phase)
        .unwrap();
    assert_eq!(
        bytes.pop(),
        Some(1),
        "negative control needs a true boundary"
    );
    bytes.push(0);
    r.broad_phase = bincode::DefaultOptions::new().deserialize(&bytes).unwrap();
    saved.dirty = true;
}
// Exercise a collision-only refresh after a regular step. Unlike PhysicsPipeline,
// CollisionPipeline leaves its requested optimization for the next update.
fn collision_refresh(w: &World) {
    let p = w.resource::<Physics>();
    let mut saved = p.executor.0.borrow_mut();
    saved.dirty = true;
    let live = saved.live();
    crate::step::sync(w, live);
    let r = &mut live.rapier;
    CollisionPipeline::new().step(
        r.integration_parameters.prediction_distance(),
        &mut r.islands,
        &mut r.broad_phase,
        &mut r.narrow_phase,
        &mut r.bodies,
        &mut r.colliders,
        &(),
        &(),
    );
}
struct Boundary;
impl Game for Boundary {
    const ID: &'static str = "exphys-v2-pending-boundary";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        crate::register(w);
        for i in 0..128 {
            w.spawn_named(
                format!("body-{i}"),
                (
                    Transform::at((i % 16) as f32 * 2., 0., (i / 16) as f32 * 2.),
                    Collider::default(),
                    Body {
                        kind: BodyKind::Static,
                        ..Default::default()
                    },
                ),
            );
        }
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        crate::step(w);
        if w.tick() == 5 {
            // Two bulk collision-only updates establish in-place-update debt,
            // then leave its optimizer pending at a non-initial saved tick.
            for pass in 0..2 {
                for i in 0..128 {
                    let mut t = w.require_mut::<Transform>(format!("body-{i}").as_str());
                    t.position += Vec3::new(
                        ((i * 37 % 17) as f32 - 8.) * 0.3,
                        0.,
                        0.4 + pass as f32 * 0.1,
                    );
                }
                collision_refresh(w);
            }
        }
    }
}
#[test]
fn non_initial_pending_bvh_continuation_and_omission_negative_control() {
    let mut reference = None;
    for mode in [Paranoid::Off, Paranoid::Save, Paranoid::FreshGame] {
        let mut s = Sim::<Boundary>::new(()).unwrap().paranoid(mode);
        s.run(100.);
        assert_eq!(s.world().tick(), 6);
        assert!(
            pending(&s.world().resource::<Physics>()),
            "must reach a non-initial true pending boundary"
        );
        let bytes = s.save().unwrap();
        let mut restored = Sim::<Boundary>::new(()).unwrap();
        restored.restore(&bytes).unwrap();
        assert!(pending(&restored.world().resource::<Physics>()));
        let mut omitted = Sim::<Boundary>::new(()).unwrap();
        omitted.restore(&bytes).unwrap();
        clear_pending(&omitted.world().resource::<Physics>());
        s.run(100.);
        restored.run(100.);
        omitted.run(100.);
        assert!(!pending(&s.world().resource::<Physics>()));
        assert!(!pending(&restored.world().resource::<Physics>()));
        assert!(!pending(&omitted.world().resource::<Physics>()));
        assert_eq!(s.save().unwrap(), restored.save().unwrap());
        assert_ne!(
            s.world().hash(),
            omitted.world().hash(),
            "clearing the serialized flag must change continuation"
        );
        let state = s.save().unwrap();
        if let Some(expected) = &reference {
            assert_eq!(&state, expected);
        } else {
            reference = Some(state);
        }
        eprintln!("EXPHYS v2 {mode:?}: tick 6 pending=true; tick 12 restored continuation equal; omitted flag diverges");
    }
}
