mod common;
use common::*;
use exact_game::{Quat, Vec3};
use exact_game_physics as physics;
use rapier3d::{pipeline::PhysicsWorld, prelude::*};
#[test]
fn rest_poses_and_sleep_against_rapier() {
    for (name, side, layers, drop) in [
        ("drop", 1, 1, 2.0),
        ("stack", 1, 10, 0.0),
        ("pile", 5, 5, 2.0),
    ] {
        let mut sim = scene(name);
        let mut oracle = PhysicsWorld::new();
        let mut handles = Vec::new();
        // Match the executor's solver/material model; both use four substeps,
        // per-contact PGS sweeps, per-contact Coulomb friction and no opaque recycling.
        oracle.integration_parameters.num_internal_pgs_iterations = 8;
        oracle.integration_parameters.friction_model = FrictionModel::Coulomb;
        oracle.integration_parameters.friction_in_bias_pass = true;
        oracle.integration_parameters.contact_recycling = false;
        oracle.integration_parameters.static_contact_softness =
            oracle.integration_parameters.contact_softness;
        oracle.insert_collider(
            ColliderBuilder::cuboid(50.0, 0.5, 50.0)
                .translation(Vector::new(0.0, -0.5, 0.0))
                .friction(0.6),
            None,
        );
        for y in 0..layers {
            for z in 0..side {
                for x in 0..side {
                    let (body, _) = oracle.insert(
                        RigidBodyBuilder::dynamic().translation(Vector::new(
                            x as f32 * 1.01,
                            0.5 + drop + y as f32 * 1.01,
                            z as f32 * 1.01,
                        )),
                        ColliderBuilder::cuboid(0.5, 0.5, 0.5)
                            .density(1000.0)
                            .friction(0.6),
                    );
                    let a = oracle.bodies[body].activation_mut();
                    a.normalized_linear_threshold = 0.05;
                    a.angular_threshold = 0.05;
                    a.time_until_sleep = 0.5;
                    handles.push(body);
                }
            }
        }
        let mut exact_sleep = 0;
        let mut rapier_sleep = 0;
        for t in 1..=1200 {
            tick(&mut sim, t);
            oracle.step();
            if exact_sleep == 0 && physics::quiescent(sim.world()) {
                exact_sleep = t;
            }
            if rapier_sleep == 0 && handles.iter().all(|h| oracle.bodies[*h].is_sleeping()) {
                rapier_sleep = t;
            }
        }
        let mut max_position = 0.0f32;
        let mut max_angle = 0.0f32;
        for ((position, rotation), h) in positions(sim.world()).into_iter().zip(handles) {
            let body = &oracle.bodies[h];
            let p = Vec3::from_array(body.translation().to_array());
            let q = Quat::from_array(body.rotation().to_array());
            if p.distance(position) > max_position {
                max_position = p.distance(position);
                if max_position > 0.02 {
                    eprintln!("{name} differing poses exact={position:?} rapier={p:?}");
                }
            }
            let angle = 2.0 * exact_game::math::acos(q.dot(rotation).abs().min(1.0)) * 180.0
                / std::f32::consts::PI;
            max_angle = max_angle.max(angle);
        }
        eprintln!("oracle {name}: position={max_position:.6}m angle={max_angle:.4}deg sleep exact={exact_sleep} rapier={rapier_sleep}");
        assert!(
            max_position < 0.02,
            "{name}: oracle position difference {max_position}"
        );
        assert!(
            max_angle < 3.0,
            "{name}: oracle rotation difference {max_angle}"
        );
        assert!(
            exact_sleep > 0 && rapier_sleep > 0,
            "{name}: stability disagreement"
        );
        assert!(
            exact_sleep <= rapier_sleep * 2 && rapier_sleep <= exact_sleep * 2,
            "{name}: sleep-time disagreement"
        );
    }
}
