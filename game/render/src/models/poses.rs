use super::*;

impl<const ASSETS: bool> crate::renderer::RendererWithAssets<ASSETS> {
    /// Feed poses for transparency and winding on completed ticks. A full pass
    /// walks every model instance; otherwise only instances in blocks whose
    /// local or propagated poses changed and those still interpolating.
    pub(crate) fn model_poses(
        &mut self,
        world: &exact_game::World,
        entities: &[exact_game::Entity],
        initial: bool,
        moved: Moved<'_>,
        poses: &[crate::world::PreparedPose],
        // Feed supplies its complete, sorted reset list. Direct callers can
        // omit it to resolve fresh ancestors from the world.
        resets: Option<&[(u32, [f32; 10])]>,
    ) {
        if let Some(skinning) = &mut self.models.skinning {
            skinning.feed(&self.queue, world, entities, initial);
        }
        let models = &mut self.models;
        // A visibility-only rebatch can keep every model, or compact survivors.
        // Reconcile identities without rereading unchanged model assets.
        let reconcile = matches!(
            moved,
            Moved::All
                | Moved::Pages {
                    reconcile: true,
                    ..
                }
        );
        let rebuilt = reconcile && models.reconcile_pose_history(entities);
        if reconcile && !rebuilt {
            // An empty draw list can shrink active poses while retaining the
            // histories. Showing the same entities again must restore those
            // entries even when no pose page changed in the meantime.
            for (active, history) in models.poses.iter_mut().zip(&models.pose_history) {
                if let Some(saved) = &history.saved {
                    *active = saved.2;
                }
            }
        }
        let mut touched = std::mem::take(&mut models.touched);
        touched.clear();
        match moved {
            Moved::Pages { pages, .. } if !initial && !rebuilt => {
                touched.append(&mut models.moving);
                let history = &models.pose_history;
                let at =
                    |index: usize| history.partition_point(|h| (h.entity.index() as usize) < index);
                for &page in pages {
                    touched.extend(at(page * exact_game::PAGE)..at((page + 1) * exact_game::PAGE));
                }
                touched.sort_unstable();
                touched.dedup();
                for &i in &touched {
                    models.update_pose(world, i, None, poses, resets);
                }
            }
            _ => {
                models.moving.clear();
                for i in 0..models.pose_history.len() {
                    let cached = matches!(moved, Moved::Pages { .. })
                        .then(|| models.pose_history[i].saved.map(|saved| saved.0))
                        .flatten();
                    let digest = cached.unwrap_or_else(|| {
                        world
                            .get::<exact_game::Mesh>(models.pose_history[i].entity)
                            .and_then(|m| match &*m {
                                exact_game::Mesh::Asset(name) => {
                                    models.loaded.get(name).map(|m| m.digest)
                                }
                                _ => None,
                            })
                            .unwrap_or(0)
                    });
                    models.update_pose(world, i, Some((digest, initial)), poses, resets);
                }
            }
        }
        models.touched = touched;
    }
}
/// Which model instances a completed tick may have moved.
pub(crate) enum Moved<'a> {
    /// Every instance: structure, parents or assets changed.
    All,
    /// Instances on these pages (`PAGE` entity blocks), in index order, whose
    /// local or propagated poses may have changed. Model assets/names are unchanged;
    /// reconcile identities if visibility or other draw membership changed.
    Pages { pages: &'a [usize], reconcile: bool },
}
impl Models {
    /// Whether any instance's owner pose has a negative scale axis at either
    /// endpoint, so its winding can differ from its batch's.
    pub(crate) fn any_mirrored_owner(&self) -> bool {
        self.mirrored != 0
    }
    // One instance's history step. A full pass supplies its asset digest and
    // whether the feed is initial; an incremental one keeps the saved digest.
    fn update_pose(
        &mut self,
        world: &exact_game::World,
        i: usize,
        full: Option<(u64, bool)>,
        poses: &[crate::world::PreparedPose],
        resets: Option<&[(u32, [f32; 10])]>,
    ) {
        #[cfg(test)]
        POSE_STEPS.with(|n| n.set(n.get() + 1));
        let history = &mut self.pose_history[i];
        let Some(pose) = crate::world::prepared_pose(world, history.entity, poses) else {
            return;
        };
        let digest = full.map_or_else(|| history.saved.map_or(0, |s| s.0), |(d, _)| d);
        let saved = history
            .saved
            .get_or_insert((digest, world.tick(), [pose; 2]));
        // Conservative: any negative axis at either endpoint may flip parity
        // somewhere between them.
        let mirrored = |pair: &[exact_game::Transform; 2]| {
            pair.iter().any(|t| t.scale.cmplt(glam::Vec3::ZERO).any())
        };
        if full.is_some_and(|(_, initial)| initial)
            || resets.map_or_else(
                || crate::world::scene::snap(world, history.entity, false),
                |rows| {
                    rows.binary_search_by_key(&history.entity.index(), |r| r.0)
                        .is_ok()
                },
            )
            || saved.0 != digest
        {
            saved.2 = [pose; 2];
        } else {
            if saved.1 != world.tick() {
                saved.2[0] = saved.2[1];
            }
            saved.2[1] = pose;
        }
        saved.0 = digest;
        saved.1 = world.tick();
        let now = mirrored(&saved.2);
        if saved.2[0] != saved.2[1] {
            self.moving.push(i);
        }
        if history.mirrored != now {
            history.mirrored = now;
            if now {
                self.mirrored += 1;
            } else {
                self.mirrored -= 1;
            }
        }
        if let Some(active) = self.poses.get_mut(i) {
            *active = saved.2;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_game::{Game, Input, Mesh, Sim, Transform, Visible, World, PAGE};

    #[test]
    fn visibility_rebatches_keep_static_poses_and_compact_moving_histories() {
        let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
            return;
        };
        let mut renderer =
            crate::Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
        renderer
            .prepare_model("actor", &crate::test_model::skinned_model())
            .unwrap();
        struct Scene;
        impl Game for Scene {
            type Args = ();
            const ID: &'static str = "visibility-pose-history";
            fn setup(w: &mut World, _: &()) {
                for i in 0..PAGE * 2 {
                    w.spawn((Transform::at(i as f32, 0., 0.), Mesh::asset("actor")));
                }
                w.spawn_named("effect", (Transform::default(), Mesh::cube(1.)));
            }
            fn tick(_: &mut World, _: &Input, _: &()) {}
        }
        let mut sim = Sim::<Scene>::new(()).unwrap();
        let mut feed = crate::Feed::default();
        feed.feed(sim.world(), &mut renderer).unwrap();
        let tick = 1000. / 60.;
        sim.run(tick);
        feed.feed(sim.world(), &mut renderer).unwrap();
        let before = POSE_STEPS.with(|n| n.get());
        let effect = sim.world().named("effect").unwrap();
        sim.world_mut().insert(effect, Visible(false));
        feed.feed(sim.world(), &mut renderer).unwrap();
        assert_eq!(POSE_STEPS.with(|n| n.get()), before);
        let entities = renderer
            .models
            .pose_history
            .iter()
            .map(|h| h.entity)
            .collect::<Vec<_>>();
        let moving = entities[PAGE + 3];
        sim.world_mut()
            .get_mut::<Transform>(moving)
            .unwrap()
            .position
            .y = 4.;
        sim.run(tick);
        feed.feed(sim.world(), &mut renderer).unwrap();
        assert_ne!(
            renderer.models.poses[PAGE + 3][0],
            renderer.models.poses[PAGE + 3][1]
        );
        for visible in [false, true] {
            sim.world_mut().insert(entities[0], Visible(visible));
            feed.feed(sim.world(), &mut renderer).unwrap();
            let cached = renderer.models.poses.clone();
            let active = renderer
                .models
                .pose_history
                .iter()
                .map(|h| h.entity)
                .collect::<Vec<_>>();
            renderer.model_poses(sim.world(), &active, false, Moved::All, &[], None);
            assert_eq!(
                renderer.models.poses, cached,
                "visibility keeps the exact history"
            );
            let at = active.iter().position(|e| *e == moving).unwrap();
            assert_eq!(cached[at][0].position.y, 0.);
            assert_eq!(cached[at][1].position.y, 4.);
            assert!(renderer
                .models
                .pose_history
                .iter()
                .all(|h| h.saved.unwrap().0 != 0));
        }
        sim.run(tick);
        feed.feed(sim.world(), &mut renderer).unwrap();
        assert_eq!(
            renderer.models.poses[PAGE + 3][0],
            renderer.models.poses[PAGE + 3][1]
        );
        let saved = renderer.models.poses.clone();
        for &entity in &entities {
            sim.world_mut().insert(entity, Visible(false));
        }
        feed.feed(sim.world(), &mut renderer).unwrap();
        assert!(renderer.models.poses.is_empty());
        for &entity in &entities {
            sim.world_mut().remove::<Visible>(entity);
        }
        feed.feed(sim.world(), &mut renderer).unwrap();
        assert_eq!(
            renderer.models.poses, saved,
            "showing static models restores active poses"
        );
        let root = sim.world_mut().spawn(Transform::at(10., 0., 0.));
        let other = sim.world_mut().spawn(Transform::at(-10., 0., 0.));
        for entity in [entities[0], moving] {
            sim.world_mut().insert(entity, exact_game::Parent(root));
        }
        sim.run(tick);
        feed.feed(sim.world(), &mut renderer).unwrap();
        sim.run(tick);
        feed.feed(sim.world(), &mut renderer).unwrap();
        sim.world_mut()
            .get_mut::<Transform>(moving)
            .unwrap()
            .position
            .y += 2.;
        sim.world_mut()
            .insert(entities[0], exact_game::Parent(other));
        sim.run(tick);
        feed.feed(sim.world(), &mut renderer).unwrap();
        for (entity, snapped) in [(entities[0], true), (moving, false)] {
            let at = renderer
                .models
                .pose_history
                .iter()
                .position(|h| h.entity == entity)
                .unwrap();
            let pair = renderer.models.poses[at];
            assert_eq!(
                pair[0] == pair[1],
                snapped,
                "only the reparented model snaps"
            );
            if snapped {
                assert_eq!(pair[1].position.x, -10.);
            } else {
                assert_eq!(pair[0].position.y, 4.);
                assert_eq!(pair[1].position.y, 6.);
            }
        }
    }
}
