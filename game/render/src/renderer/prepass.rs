use super::*;

impl<const ASSETS: bool> RendererWithAssets<ASSETS> {
    pub(super) fn prepare_batch_coverage(&mut self) {
        prepare_coverage(
            &mut self.batch_coverage,
            &self.batches,
            &self.slot_list,
            self.lights.fading,
            |slot| {
                let entity = if ASSETS && slot >= crate::RENDER_SLOT_BASE {
                    self.models.records[(slot - crate::RENDER_SLOT_BASE) as usize].transform
                } else {
                    slot
                };
                self.lights.slot_fading(entity)
            },
        );
    }

    pub(crate) fn batch_fading(&self, batch: usize) -> bool {
        self.batch_coverage[batch]
    }
    /// HDR frames retain a separate depth pass. Direct frames draw depth inside
    /// forward, keeping it on tile until the Equal-tested colour draws finish.
    pub(super) fn encode_depth_prepass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        frame: &FrameInput<'_>,
        size: (u32, u32),
        direct: bool,
    ) -> u32 {
        if !self.prepass_active || direct {
            return 0;
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("opaque depth prepass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.targets.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: timing::writes(frame.timestamps, 30),
            occlusion_query_set: None,
            multiview_mask: None,
        });
        if frame.timestamps.is_some() {
            self.mark(30);
        }
        self.draw_depth_prepass(&mut pass, size, false)
    }

    pub(super) fn draw_depth_prepass(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        size: (u32, u32),
        direct: bool,
    ) -> u32 {
        if !self.prepass_active {
            return 0;
        }
        self.probe_stage(super::probe::Stage::Prepass);
        pass.set_vertex_buffer(0, self.vertices.raw.slice(..));
        pass.set_index_buffer(self.indices.raw.slice(..), wgpu::IndexFormat::Uint32);
        let mut draws = 0;
        self.scene_viewport(pass, size, false);
        let mut layer = false;
        let mut bound = super::passes::Bound::default();
        bound.group(pass, 1, &self.shadow_sample);
        for (index, group) in self.cull.groups.iter().enumerate() {
            if group.flags & 1 == 0 {
                continue;
            }
            let viewmodel = self.batches[group.batch].viewmodel;
            if viewmodel != layer {
                self.scene_viewport(pass, size, viewmodel);
                layer = viewmodel;
            }
            let fading = self.batch_fading(group.batch);
            if let Some(material) = self.model_material(group.batch) {
                let material = &self.models.materials[material.0];
                let opaque = material.alpha == exact_game::asset::AlphaMode::Opaque && !fading;
                bound.group(pass, 2, self.depth_material(material, opaque));
                bound.group(pass, 3, self.models.bind.as_ref().unwrap());
                let key = 4 * usize::from(material.double_sided)
                    + 16 * usize::from(group.mirrored)
                    + 32 * usize::from(opaque)
                    + crate::pipeline::MODEL_DIRECT * usize::from(direct);
                bound.pipeline(pass, self.pipelines.model_camera_depth(&self.device, key));
            } else {
                bound.pipeline(
                    pass,
                    self.pipelines
                        .camera_depth(&self.device, group.mirrored, fading, direct),
                );
            }
            self.draw_group(pass, &mut bound, 0, index);
            draws += 1;
        }
        draws
    }
}

fn prepare_coverage(
    out: &mut Vec<bool>,
    batches: &[Batch],
    slots: &[u32],
    fading: bool,
    mut slot_fading: impl FnMut(u32) -> bool,
) {
    out.clear();
    out.extend(batches.iter().map(|batch| {
        fading
            && slots[batch.slots.start as usize..batch.slots.end as usize]
                .iter()
                .copied()
                .any(&mut slot_fading)
    }));
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::Renderer;
    use exact_game::{asset::*, *};
    use exact_gpu::fixture;

    #[test]
    fn coverage_scans_once_per_batch_and_refreshes_when_fading_changes() {
        let slots: Vec<u32> = (0..4096).collect();
        let batch = Batch::new(crate::MeshId(0), 0..4096);
        let visits = std::cell::Cell::new(0);
        let mut coverage = Vec::new();
        prepare_coverage(
            &mut coverage,
            std::slice::from_ref(&batch),
            &slots,
            true,
            |_| {
                visits.set(visits.get() + 1);
                false
            },
        );
        // Alternating winding can create one group per instance; every pass
        // reads the same prepared flag, without visiting any instance again.
        for _ in 0..4096 * 5 {
            assert!(!coverage[0]);
        }
        assert_eq!(visits.get(), 4096);
        prepare_coverage(
            &mut coverage,
            std::slice::from_ref(&batch),
            &slots,
            true,
            |slot| slot == 4095,
        );
        assert_eq!(coverage, [true]);
        prepare_coverage(
            &mut coverage,
            std::slice::from_ref(&batch),
            &slots,
            false,
            |_| panic!("no fading slots"),
        );
        assert_eq!(coverage, [false]);
    }

    #[test]
    fn fading_and_live_settings_only_compile_the_batches_that_need_them() {
        let Some(gpu) = crate::test_device::device_or_skip(fixture::device()) else {
            return;
        };
        let mut w = World::new(60, 1);
        let (vertices, indices) = crate::shapes::cube();
        let model = Model {
            meshes: vec![MeshData {
                positions: vertices.iter().flat_map(|v| v.position).collect(),
                normals: vertices.iter().flat_map(|v| v.normal).collect(),
                uvs: vertices.iter().flat_map(|v| v.uv).collect(),
                indices,
                bounds: [-0.5, -0.5, -0.5, 0.5, 0.5, 0.5],
                ..Default::default()
            }],
            nodes: vec![Node {
                mesh: Some(0),
                ..Default::default()
            }],
            materials: vec![MaterialData::default()],
            bounds: [-0.5, -0.5, -0.5, 0.5, 0.5, 0.5],
            ..Default::default()
        };
        let mut r = Renderer::configured(
            &gpu.device,
            &gpu.queue,
            wgpu::TextureFormat::Rgba8Unorm,
            SceneSamples::One,
            true,
        );
        for (name, x) in [("a.model", -1.), ("b.model", 1.)] {
            let mesh = w.generated_model(name, model.clone()).unwrap();
            w.spawn_named(name, (Transform::at(x, 0., 0.), mesh));
            r.prepare_model(name, &model).unwrap();
        }
        assert_eq!(r.asset_work().0, 0, "model residency compiles no pipeline");
        w.spawn((
            Transform::at(0., 0., -2.),
            exact_game::Mesh::cuboid(Vec3::new(5., 5., 0.1)),
            Material::rgb(0.2, 0.2, 0.2),
        ));
        let mut feed = crate::Feed::default();
        w.propagate();
        feed.feed(&w, &mut r).unwrap();
        let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 64,
                height: 64,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&Default::default());
        let mut frame = FrameInput {
            view: glam::camera::rh::view::look_at_mat4(Vec3::new(0., 0., 5.), Vec3::ZERO, Vec3::Y),
            proj: glam::camera::rh::proj::directx::orthographic(-2., 2., -2., 2., 0.1, 20.),
            camera_position: Vec3::new(0., 0., 5.),
            sun: Some(crate::Sun {
                direction: Vec3::new(1., -2., -3.).normalize(),
                color: Vec3::ONE,
                illuminance: 2.,
                shadows: Some(Shadows {
                    cascades: 1,
                    ..Default::default()
                }),
            }),
            environment: Environment {
                background: Some([0.; 3].into()),
                fog: None,
                bloom: None,
                ..Default::default()
            },
            ..Default::default()
        };
        // One sample and nothing to blend: every frame here draws direct.
        let direct = crate::pipeline::MODEL_DIRECT;
        r.draw(&view, (64, 64), &frame);
        r.pipelines.models.as_ref().unwrap().forward
            [1 + 32 + 64 + 128 + crate::pipeline::PREPASS + direct]
            .wait();
        assert_eq!(
            r.asset_work().0,
            7,
            "one forward specialization, opaque/coverage fallbacks, and both depth modes"
        );
        let solid = fixture::read(&gpu, &target).unwrap();
        w.insert(w.named("a.model").unwrap(), Opacity(0.5));
        feed.feed(&w, &mut r).unwrap();
        r.draw(&view, (64, 64), &frame);
        let faded = fixture::read(&gpu, &target).unwrap();
        let family = r.pipelines.models.as_ref().unwrap();
        let key = 1 + 64 + 128 + crate::pipeline::PREPASS + direct;
        family.forward[key].wait();
        assert_eq!(
            r.asset_work().0,
            8,
            "fading only specializes its forward batch"
        );
        assert!(family.forward[key].get().is_some() && family.forward[key + 32].get().is_some());
        assert!(family.shadow[0].get().is_some() && family.shadow[4].get().is_some());
        assert_eq!(
            r.batches
                .iter()
                .enumerate()
                .filter(|(i, _)| r.batch_fading(*i))
                .count(),
            1
        );
        let mut holes = 0;
        for y in 24..40 {
            for x in 8..24 {
                holes += usize::from(solid.at(x, y) != faded.at(x, y));
            }
        }
        assert!(holes > 20, "fading batch must still dither");
        for y in 24..40 {
            for x in 40..56 {
                assert_eq!(solid.at(x, y), faded.at(x, y), "solid batch ({x},{y})");
            }
        }
        // The same renderer changes fog, cascade count, filter and fade live.
        for step in 0..6 {
            match step {
                0 => frame.environment.fog = Some(Fog::default()),
                1 => {
                    frame
                        .sun
                        .as_mut()
                        .unwrap()
                        .shadows
                        .as_mut()
                        .unwrap()
                        .cascades = 3
                }
                2 => {
                    frame.sun.as_mut().unwrap().shadows.as_mut().unwrap().filter =
                        ShadowFilter::PcfSoft {
                            depth_bias: 0.0004,
                            normal_bias: 0.035,
                        }
                }
                3 => {
                    frame
                        .sun
                        .as_mut()
                        .unwrap()
                        .shadows
                        .as_mut()
                        .unwrap()
                        .resolution = 1024
                }
                4 => {
                    frame.sun.as_mut().unwrap().shadows.as_mut().unwrap().filter =
                        ShadowFilter::Bilinear {
                            depth_bias: 0.0004,
                            normal_bias: 0.035,
                        }
                }
                _ => {
                    w.insert(w.named("a.model").unwrap(), Opacity(1.));
                    feed.feed(&w, &mut r).unwrap();
                }
            }
            let before = r.asset_work().0;
            let start = std::time::Instant::now();
            r.draw(&view, (64, 64), &frame);
            assert_eq!(
                r.shadows.as_ref().unwrap().resolution,
                if step >= 3 { 1024 } else { 2048 }
            );
            let elapsed = start.elapsed();
            let immediate = fixture::read(&gpu, &target).unwrap();
            let key = 1
                + 2
                + 64
                + crate::pipeline::PREPASS
                + direct
                + if step == 0 { 128 } else { 0 }
                + if step >= 2 { 256 } else { 0 }
                + if step >= 4 {
                    crate::pipeline::MODEL_BILINEAR
                } else {
                    0
                };
            let family = r.pipelines.models.as_ref().unwrap();
            if step < 5 {
                family.forward[key].wait();
            }
            family.forward[key + 32].wait();
            let primitive_key = key
                - 64
                - direct
                - if step >= 4 {
                    crate::pipeline::MODEL_BILINEAR
                } else {
                    0
                }
                + if step >= 4 {
                    crate::pipeline::BILINEAR
                } else {
                    0
                }
                + crate::pipeline::DIRECT
                + crate::pipeline::LEAN
                + if r.pipelines.early {
                    crate::pipeline::EARLY
                } else {
                    0
                };
            r.pipelines.wait_forward(primitive_key);
            let after = r.asset_work().0;
            eprintln!(
                "live settings {step}: {} new pipelines, encode {elapsed:?}",
                after - before
            );
            assert_eq!(after - before, if step < 3 || step == 4 { 2 } else { 0 });
            r.draw(&view, (64, 64), &frame);
            assert_eq!(r.asset_work().0, after, "warm frame compiles nothing");
            let specialized = fixture::read(&gpu, &target).unwrap();
            for y in 0..64 {
                for x in 0..64 {
                    assert!(
                        immediate
                            .at(x, y)
                            .into_iter()
                            .zip(specialized.at(x, y))
                            .all(|(a, b)| a.abs_diff(b) <= 1),
                        "fallback differs at step {step} ({x},{y})"
                    );
                }
            }
        }
    }

    #[test]
    fn model_lighting_changes_survive_fallbacks_and_specialization() {
        let Some(gpu) = crate::test_device::device_or_skip(fixture::device()) else {
            return;
        };
        for (format, matte) in [
            (wgpu::TextureFormat::Rgba8Unorm, false),
            (wgpu::TextureFormat::Rgba8UnormSrgb, false),
            (wgpu::TextureFormat::Rgba8Unorm, true),
            (wgpu::TextureFormat::Rgba8UnormSrgb, true),
        ] {
            let mut w = World::new(60, 1);
            let mut r =
                Renderer::configured(&gpu.device, &gpu.queue, format, SceneSamples::One, true)
                    .with_matte_models(matte);
            r.allow_direct = true;
            let (vertices, indices) = crate::shapes::cube();
            for (name, x, alpha) in [
                ("opaque.model", -0.8, AlphaMode::Opaque),
                ("blended.model", 0.8, AlphaMode::Blend),
            ] {
                let model = Model {
                    meshes: vec![MeshData {
                        positions: vertices.iter().flat_map(|v| v.position).collect(),
                        normals: vertices.iter().flat_map(|v| v.normal).collect(),
                        uvs: vertices.iter().flat_map(|v| v.uv).collect(),
                        indices: indices.clone(),
                        bounds: [-0.5, -0.5, -0.5, 0.5, 0.5, 0.5],
                        ..Default::default()
                    }],
                    nodes: vec![Node {
                        mesh: Some(0),
                        ..Default::default()
                    }],
                    materials: vec![MaterialData {
                        base_color: [0.5, 0.5, 0.5, 0.5],
                        alpha_mode: alpha,
                        ..Default::default()
                    }],
                    bounds: [-0.5, -0.5, -0.5, 0.5, 0.5, 0.5],
                    ..Default::default()
                };
                let mesh = w.generated_model(name, model.clone()).unwrap();
                w.spawn((Transform::at(x, 0., 0.), mesh));
                r.prepare_model(name, &model).unwrap();
            }
            w.propagate();
            crate::Feed::default().feed(&w, &mut r).unwrap();
            let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: 64,
                    height: 64,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let target = texture.create_view(&Default::default());
            let local = [crate::LightInput {
                position: Vec3::new(0., 0., 3.),
                color: Vec3::ONE,
                intensity: 10.,
                range: 10.,
                direction: Vec3::NEG_Z,
                cone: None,
                shadows: false,
            }];
            let mut frame = FrameInput {
                view: glam::camera::rh::view::look_at_mat4(
                    Vec3::new(0., 0., 5.),
                    Vec3::ZERO,
                    Vec3::Y,
                ),
                proj: glam::camera::rh::proj::directx::orthographic(-2., 2., -2., 2., 0.1, 20.),
                camera_position: Vec3::new(0., 0., 5.),
                sun: None,
                environment: Environment {
                    ambient: 0.,
                    background: Some([0.; 3].into()),
                    fog: None,
                    bloom: None,
                    tone_mapping: ToneMapping::None,
                    ..Default::default()
                },
                ..Default::default()
            };
            for (fill, point) in [
                (false, false),
                (true, false),
                (false, true),
                (false, false),
                (true, false),
            ] {
                frame.fill = fill.then_some(crate::Sun {
                    direction: Vec3::NEG_Z,
                    color: Vec3::ONE,
                    illuminance: 3.,
                    shadows: None,
                });
                frame.lights = if point { &local } else { &[] };
                r.draw(&target, (64, 64), &frame);
                let immediate = fixture::read(&gpu, &texture).unwrap();
                r.pipelines.wait_specializations();
                r.draw(&target, (64, 64), &frame);
                let specialized = fixture::read(&gpu, &texture).unwrap();
                for x in [19, 45] {
                    let a = immediate.at(x, 32);
                    let b = specialized.at(x, 32);
                    if matte && fill && x == 19 {
                        let diffuse = 0.5 * 3. / std::f32::consts::PI;
                        let expected =
                            ((1.055 * diffuse.powf(1. / 2.4) - 0.055) * 255.).round() as u8;
                        assert!(
                            b[0].abs_diff(expected) <= 2,
                            "matte Lambert: {b:?}, expected {expected}"
                        );
                    }
                    assert!(
                        a.into_iter().zip(b).all(|(a, b)| a.abs_diff(b) <= 1),
                        "{format:?}, fill={fill}, point={point}: {a:?} != {b:?}"
                    );
                    assert_eq!(
                        b[0] > 20,
                        fill || point,
                        "{format:?}, x={x}, fill={fill}, point={point}: {b:?}"
                    );
                }
            }
        }
    }
}
