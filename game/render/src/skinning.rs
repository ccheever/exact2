//! Model-only buffers. Local poses cross the feed seam; matrices exist only on the GPU.
use crate::{
    buffers::{bytes, Buffer},
    DrawInstance, RenderError,
};
use exact_game::{animation, asset::Model, Entity, Pose, World};
use exact_gpu::wgpu;
// CPU attachment chains use the same local TRS interpolation as skin.wgsl.
pub(crate) fn interpolated_local([a, b]: [exact_game::Transform; 2], alpha: f32) -> glam::Mat4 {
    let alpha = alpha.clamp(0., 1.);
    glam::Mat4::from_scale_rotation_translation(
        a.scale.lerp(b.scale, alpha),
        a.rotation
            .normalize()
            .slerp(b.rotation.normalize(), alpha)
            .normalize(),
        a.position.lerp(b.position, alpha),
    )
}
struct Template {
    meta: u32,
    words: usize,
    fresh: bool,
    rest: Vec<f32>,
    joints: usize,
}
pub(crate) struct Skinning {
    pub weights: Buffer,
    pub reallocations: u64,
    pub pipeline_creations: u64,
    pub palette: Buffer,
    meta: Buffer,
    poses: Buffer,
    jobs: Buffer,
    templates: Vec<Template>,
    metadata: Vec<u32>,
    pub offsets: Vec<u32>,
    records: Vec<(usize, usize)>,
    pose_words: Vec<f32>,
    jobs_words: Vec<u32>,
    pipeline: Option<wgpu::ComputePipeline>,
    joint_capacity: usize,
    layout: Option<wgpu::BindGroupLayout>,
    bind: Option<wgpu::BindGroup>,
    bind_buffers: Option<(wgpu::BindGroupLayout, [wgpu::Buffer; 5])>,
}
impl Skinning {
    pub fn new(device: &wgpu::Device) -> Self {
        let buffer = |label| Buffer::new(device, 64, wgpu::BufferUsages::STORAGE, label);
        Self {
            weights: buffer("game vertex joints weights"),
            reallocations: 0,
            pipeline_creations: 0,
            palette: buffer("game skin palette"),
            meta: buffer("game skin hierarchy"),
            poses: buffer("game skin locals"),
            jobs: buffer("game skin jobs"),
            templates: vec![],
            metadata: vec![],
            offsets: vec![],
            records: vec![],
            pose_words: vec![],
            jobs_words: vec![],
            pipeline: None,
            joint_capacity: 0,
            layout: None,
            bind: None,
            bind_buffers: None,
        }
    }
    pub fn add(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, model: &Model) -> Vec<u32> {
        if model.skins.is_empty() {
            return vec![];
        }
        if model.nodes.len() > self.joint_capacity {
            self.joint_capacity = model.nodes.len().next_power_of_two();
            let entries: Vec<_> = (0..5)
                .map(|binding| wgpu::BindGroupLayoutEntry {
                    binding,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: if binding == 4 {
                            wgpu::BufferBindingType::Uniform
                        } else {
                            wgpu::BufferBindingType::Storage {
                                read_only: binding != 3,
                            }
                        },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                })
                .collect();
            let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("game skin compute"),
                entries: &entries,
            });
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("game local skin compute"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/skin.wgsl").into()),
            });
            let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("game local skin compute"),
                layout: Some(
                    &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: None,
                        bind_group_layouts: &[Some(&layout)],
                        immediate_size: 0,
                    }),
                ),
                module: &shader,
                entry_point: Some("skin"),
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants: &[("JOINT_CAPACITY", self.joint_capacity as f64)],
                    ..Default::default()
                },
                cache: None,
            });
            self.layout = Some(layout);
            self.pipeline = Some(pipeline);
            self.pipeline_creations += 1;
        }
        let order = animation::node_order(model);
        let rest = animation::bind_pose(model);
        let mut ids = Vec::new();
        for skin in &model.skins {
            ids.push(self.templates.len() as u32);
            self.templates.push(Template {
                meta: self.metadata.len() as u32,
                fresh: false,
                words: 4 + model.nodes.len() * 2 + skin.joints.len() + skin.inverse_binds.len(),
                rest: rest.clone(),
                joints: skin.joints.len(),
            });
            self.metadata
                .extend([model.nodes.len() as u32, skin.joints.len() as u32, 0, 0]);
            for &node in &order {
                self.metadata
                    .extend([node, model.nodes[node as usize].parent.unwrap_or(u32::MAX)]);
            }
            self.metadata.extend(&skin.joints);
            self.metadata
                .extend(skin.inverse_binds.iter().map(|v| v.to_bits()));
        }
        self.reallocations += u64::from(self.meta.grow(
            device,
            queue,
            (self.metadata.len() * 4) as u64,
        ));
        self.meta.write(queue, 0, bytes(&self.metadata));
        ids
    }
    pub(crate) fn mark_fresh(&mut self, skins: &[u32]) {
        for &id in skins {
            self.templates[id as usize].fresh = true;
        }
    }
    pub(crate) fn retired_bytes(
        &self,
        loaded: &std::collections::BTreeMap<String, crate::models::Uploaded>,
        live: &std::collections::BTreeSet<String>,
    ) -> u64 {
        let retained: std::collections::BTreeSet<_> = loaded
            .iter()
            .filter(|(n, m)| m.active && live.contains(*n))
            .flat_map(|(_, m)| m.skins.iter().copied())
            .collect();
        let used: u64 = self
            .templates
            .iter()
            .enumerate()
            .filter(|(i, _)| retained.contains(&(*i as u32)))
            .map(|(_, t)| t.words as u64 * 4)
            .sum();
        self.meta.raw.size().saturating_sub(used)
    }
    pub(crate) fn compact_metadata(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        trim: bool,
    ) {
        let mut words = Vec::new();
        for t in &mut self.templates {
            let start = t.meta as usize;
            t.meta = words.len() as u32;
            words.extend_from_slice(&self.metadata[start..start + t.words]);
        }
        if words == self.metadata && !trim {
            return;
        }
        self.metadata = words;
        if trim || self.meta.raw.size() > 64 * 1024 * 1024 {
            self.meta = Buffer::new(
                device,
                (self.metadata.len() as u64 * 4).max(64),
                wgpu::BufferUsages::STORAGE,
                "game packed skin hierarchy",
            );
            self.reallocations += 1;
        }
        self.meta.write(queue, 0, bytes(&self.metadata));
    }
    pub(crate) fn reclaim(
        &mut self,
        loaded: &std::collections::BTreeMap<String, crate::models::Uploaded>,
    ) {
        let retained: std::collections::BTreeSet<_> = loaded
            .values()
            .flat_map(|m| m.skins.iter().copied())
            .collect();
        for (i, t) in self.templates.iter_mut().enumerate() {
            if !retained.contains(&(i as u32)) {
                t.rest.clear();
                t.words = 0;
                t.joints = 0;
            }
        }
        while self.templates.last().is_some_and(|t| t.words == 0) {
            self.templates.pop();
        }
    }
    pub fn set(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        uniform: &wgpu::Buffer,
        records: &[DrawInstance],
    ) -> Result<(), RenderError> {
        self.records.clear();
        self.offsets.clear();
        self.jobs_words.clear();
        let mut palette = 0usize;
        let mut pose = 0usize;
        for record in records {
            if let Some(skin) = record.skin {
                let t = self
                    .templates
                    .get(skin as usize)
                    .ok_or_else(|| RenderError::scene("unknown skin template".into()))?;
                self.offsets.push(palette as u32);
                self.records
                    .push((record.transform as usize, skin as usize));
                self.jobs_words
                    .extend([t.meta, pose as u32, palette as u32, 0]);
                pose += t.rest.len() * 2;
                palette += t.joints;
            } else {
                self.offsets.push(u32::MAX);
            }
        }
        let limit = device.limits().max_storage_buffer_binding_size;
        if (palette * 64) as u64 > limit
            || (pose * 4) as u64 > limit
            || (self.jobs_words.len() * 4) as u64 > limit
        {
            return Err(RenderError::scene(
                "skin palette/pose buffer exceeds device limit".into(),
            ));
        }
        self.reallocations += u64::from(self.palette.grow(device, queue, (palette * 64) as u64));
        self.reallocations += u64::from(self.poses.grow(device, queue, (pose * 4) as u64));
        self.pose_words.resize(pose, 0.);
        self.reallocations += u64::from(self.jobs.grow(
            device,
            queue,
            (self.jobs_words.len() * 4) as u64,
        ));
        self.jobs.write(queue, 0, bytes(&self.jobs_words));
        if let Some(layout) = &self.layout {
            let buffers = (
                layout.clone(),
                [
                    self.meta.raw.clone(),
                    self.poses.raw.clone(),
                    self.jobs.raw.clone(),
                    self.palette.raw.clone(),
                    uniform.clone(),
                ],
            );
            if self.bind_buffers.as_ref() == Some(&buffers) {
                return Ok(());
            }
            self.bind_buffers = Some(buffers);
            self.bind = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("game skin compute"),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.meta.raw.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.poses.raw.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.jobs.raw.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: self.palette.raw.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: uniform.as_entire_binding(),
                    },
                ],
            }));
        }
        Ok(())
    }
    fn pack(&mut self, w: &World, entities: &[Entity], initial: bool) {
        let mut offset = 0;
        for &(record, template) in &self.records {
            let t = &self.templates[template];
            let len = t.rest.len();
            let e = entities[entities
                .binary_search_by_key(&(record as u32), |e| e.index())
                .expect("skin entity belongs to model batches")];
            let p = w.get::<Pose>(e);
            let (prev, curr) = p
                .as_ref()
                .filter(|p| p.local.len() == len && p.previous.len() == len)
                .map_or((&t.rest[..], &t.rest[..]), |p| {
                    (&p.previous[..], &p.local[..])
                });
            self.pose_words[offset..offset + len].copy_from_slice(
                if initial || t.fresh || w.is_fresh(e) {
                    curr
                } else {
                    prev
                },
            );
            self.pose_words[offset + len..offset + 2 * len].copy_from_slice(curr);
            offset += 2 * len;
        }
    }
    pub fn feed(&mut self, queue: &wgpu::Queue, w: &World, entities: &[Entity], initial: bool) {
        self.pack(w, entities, initial);
        for template in &mut self.templates {
            template.fresh = false;
        }
        self.poses.write(queue, 0, bytes(&self.pose_words));
    }
    pub fn encode(&self, encoder: &mut wgpu::CommandEncoder, timestamps: Option<&wgpu::QuerySet>) {
        if self.records.is_empty() {
            return;
        }
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("game skin local interpolation"),
            timestamp_writes: timestamps.map(|query_set| wgpu::ComputePassTimestampWrites {
                query_set,
                beginning_of_pass_write_index: Some(32),
                end_of_pass_write_index: Some(33),
            }),
        });
        pass.set_pipeline(self.pipeline.as_ref().unwrap());
        pass.set_bind_group(0, self.bind.as_ref().unwrap(), &[]);
        pass.dispatch_workgroups(self.records.len() as u32, 1, 1);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use exact_game::{
        asset::{Node, Skin},
        Mesh, Transform,
    };
    use glam::{Mat4, Quat, Vec3};
    pub(crate) fn read(gpu: &exact_gpu::Gpu, source: &wgpu::Buffer, size: u64) -> Vec<u8> {
        let read = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(source, 0, &read, 0, size);
        gpu.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        read.slice(..)
            .map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
        gpu.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
        rx.recv().unwrap().unwrap();
        let bytes = read.slice(..).get_mapped_range().unwrap().to_vec();
        read.unmap();
        bytes
    }
    #[test]
    fn unchanged_skin_buffers_keep_the_compute_binding() {
        let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
            return;
        };
        let mut skin = Skinning::new(&gpu.device);
        let model = crate::test_model::skinned_model();
        let template = skin.add(&gpu.device, &gpu.queue, &model)[0];
        let uniform = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 80,
            usage: wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });
        let records = [DrawInstance {
            data: 0,
            transform: 1,
            geometry: crate::MeshId(0),
            material: crate::MaterialId(0),
            local: Mat4::IDENTITY,
            skin: Some(template),
        }];
        skin.set(&gpu.device, &gpu.queue, &uniform, &records)
            .unwrap();
        let binding = skin.bind.clone();
        skin.set(&gpu.device, &gpu.queue, &uniform, &records)
            .unwrap();
        assert_eq!(skin.bind, binding);
    }

    #[test]
    fn same_name_redelivery_rebuilds_gpu_rig_and_all_instance_batches() {
        let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
            return;
        };
        let mut model = crate::test_model::skinned_model();
        let mut renderer =
            crate::Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
        renderer.prepare_model("fox.model", &model).unwrap();
        let revision = renderer.models.revision;
        let before = renderer.models.loaded["fox.model"].nodes.clone();
        renderer.prepare_model("fox.model", &model.clone()).unwrap();
        assert_eq!(
            renderer.models.revision, revision,
            "equal content is reused"
        );
        let mut w = World::new(60, 0);
        let mut pose = Pose::default();
        pose.local = animation::bind_pose(&model);
        pose.previous = pose.local.clone();
        pose.previous[0] -= 10.; // A visible old history must not survive batch replacement.
        for _ in 0..2 {
            w.spawn((Transform::default(), Mesh::asset("fox.model"), pose.clone()));
        }
        let mut feed = crate::Feed::default();
        feed.feed(&w, &mut renderer).unwrap();
        let old_skin = before.iter().find_map(|n| n.3).unwrap();
        model.skins[0].inverse_binds[12] += 3.;
        model.nodes[0].transform[13] += 2.;
        model.materials[0].base_color[0] *= 0.5;
        renderer.prepare_model("fox.model", &model).unwrap();
        assert_eq!(renderer.models.revision, revision + 1);
        let after = &renderer.models.loaded["fox.model"].nodes;
        let new_skin = after.iter().find_map(|n| n.3).unwrap();
        assert_eq!(old_skin, new_skin, "retired skin slot reused");
        assert_eq!(before[0].0, after[0].0, "retired geometry slot reused");
        assert_eq!(before[0].1, after[0].1, "retired material slot reused");
        feed.feed(&w, &mut renderer).unwrap();
        assert_eq!(renderer.models.records.len(), before.len() * 2);
        assert!(renderer
            .models
            .records
            .iter()
            .filter_map(|r| r.skin)
            .all(|s| s == new_skin));
        let skin = renderer.models.skinning.as_ref().unwrap();
        let template = &skin.templates[new_skin as usize];
        assert_eq!(template.rest, animation::bind_pose(&model));
        let start =
            template.meta as usize + 4 + model.nodes.len() * 2 + model.skins[0].joints.len();
        assert_eq!(
            &skin.metadata[start..start + model.skins[0].inverse_binds.len()],
            model.skins[0]
                .inverse_binds
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>()
        );
        for pair in skin.pose_words.chunks_exact(pose.local.len() * 2) {
            assert_eq!(&pair[..pose.local.len()], &pair[pose.local.len()..]);
        }
    }

    #[test]
    fn displayed_affine_matches_gpu_with_animated_translation_scale_and_rotated_child() {
        let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
            return;
        };
        for mirrored in ["none", "owner", "joint", "offset"] {
            let model = Model {
                nodes: vec![
                    Node::default(),
                    Node {
                        name: "tip".into(),
                        parent: Some(0),
                        ..Default::default()
                    },
                ],
                skins: vec![Skin {
                    joints: vec![1],
                    inverse_binds: Mat4::IDENTITY.to_cols_array().to_vec(),
                    ..Default::default()
                }],
                ..Default::default()
            };
            let mut pairs = [
                [
                    Transform::at(-1., 2., 0.).with_scale(Vec3::new(1., 2., 1.)),
                    Transform {
                        position: Vec3::new(3., 1., 2.),
                        rotation: Quat::from_rotation_z(1.2),
                        scale: Vec3::new(3., 1., 2.),
                    },
                ],
                [
                    Transform {
                        position: Vec3::X,
                        rotation: Quat::from_rotation_z(0.4),
                        ..Default::default()
                    },
                    Transform {
                        position: Vec3::new(2., 1., 0.),
                        rotation: Quat::from_rotation_z(1.),
                        scale: Vec3::splat(1.5),
                    },
                ],
            ];
            if mirrored == "joint" {
                for t in &mut pairs[0] {
                    t.scale.x *= -1.;
                }
            }
            let pack = |i: usize| {
                pairs
                    .iter()
                    .flat_map(|p| crate::world::floats(p[i]))
                    .collect::<Vec<_>>()
            };
            let mut pose = Pose::default();
            pose.previous = pack(0);
            pose.local = pack(1);
            struct Rig;
            impl exact_game::Game for Rig {
                const ID: &'static str = "affine-oracle";
                const ASSETS: &'static [&'static str] = &["rig.model"];
                type Args = ();
                fn setup(_: &mut World, _: &()) {}
                fn tick(_: &mut World, _: &exact_game::Input, _: &()) {}
            }
            let mut sim = exact_game::Sim::<Rig>::new(()).unwrap();
            sim.deliver_asset(
                "rig.model",
                Ok(exact_game::asset::Content::Model(model.clone())),
            )
            .unwrap();
            let w = sim.world_mut();
            let parent = Transform::at(1., 2., 3.).with_scale(Vec3::new(2., 1., 3.));
            let parent_entity = w.spawn(parent);
            let mut owner = Transform {
                rotation: Quat::from_rotation_z(0.7),
                ..Transform::at(4., 2., 1.)
            };
            if mirrored == "owner" {
                owner.scale.x = -1.;
            }
            let e = w.spawn((
                owner,
                exact_game::Parent(parent_entity),
                Mesh::asset("rig.model"),
                pose,
            ));
            let mut offset = Transform::at(0.3, 0.5, 0.1).with_scale(0.7);
            if mirrored == "offset" {
                offset.scale.x *= -1.;
            }
            w.spawn((
                Transform::default(),
                exact_game::SocketFollow::new(e, "tip").offset(offset),
            ));
            w.load(&w.save()).unwrap();
            let mut skin = Skinning::new(&gpu.device);
            let template = skin.add(&gpu.device, &gpu.queue, &model)[0];
            let uniform = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: 80,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            skin.set(
                &gpu.device,
                &gpu.queue,
                &uniform,
                &[DrawInstance {
                    data: 0,
                    transform: e.index(),
                    geometry: crate::MeshId(0),
                    material: crate::MaterialId(0),
                    local: Mat4::IDENTITY,
                    skin: Some(template),
                }],
            )
            .unwrap();
            skin.feed(&gpu.queue, w, &[e], false);
            let mut attachments = crate::world::scene::Attachments::default();
            attachments.feed(w, true, true, false, false);
            attachments.feed(w, false, false, false, false);
            for alpha in [0., 0.5, 1.] {
                let mut values = [0f32; 20];
                values[19] = alpha;
                gpu.queue.write_buffer(&uniform, 0, bytes(&values));
                let mut encoder = gpu.device.create_command_encoder(&Default::default());
                skin.encode(&mut encoder, None);
                gpu.queue.submit([encoder.finish()]);
                let actual: Vec<_> = read(&gpu, &skin.palette.raw, 64)
                    .chunks_exact(4)
                    .map(|v| f32::from_ne_bytes(v.try_into().unwrap()))
                    .collect();
                let expected = Mat4::from_scale_rotation_translation(
                    parent.scale,
                    parent.rotation,
                    parent.position,
                ) * Mat4::from_scale_rotation_translation(
                    owner.scale,
                    owner.rotation,
                    owner.position,
                ) * Mat4::from_cols_slice(&actual)
                    * Mat4::from_scale_rotation_translation(
                        offset.scale,
                        offset.rotation,
                        offset.position,
                    );
                attachments.frame(alpha);
                let rendered_owner = attachments
                    .output
                    .iter()
                    .find(|a| a.entity == e)
                    .expect("the owner mesh must retain the same affine as its socket")
                    .matrix;
                let cpu_owner = Mat4::from(w.current_global(e).unwrap());
                assert!(rendered_owner.abs_diff_eq(cpu_owner, 1e-4));
                let displayed = attachments.output[0].matrix;
                for (a, b) in displayed
                    .to_cols_array()
                    .into_iter()
                    .zip(expected.to_cols_array())
                {
                    assert!(
                        (a - b).abs() < 1e-4,
                        "alpha {alpha}: attachment {displayed:?}, GPU {expected:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn interpolate_locals_before_composing_and_inverse_bind() {
        let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
            return;
        };
        let model = Model {
            nodes: vec![
                Node::default(),
                Node {
                    name: "tip".into(),
                    parent: Some(0),
                    transform: Mat4::from_translation(Vec3::X).to_cols_array(),
                    ..Default::default()
                },
            ],
            skins: vec![Skin {
                joints: vec![1],
                inverse_binds: Mat4::from_translation(Vec3::new(0., 0., 2.))
                    .to_cols_array()
                    .to_vec(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let mut skin = Skinning::new(&gpu.device);
        let template = skin.add(&gpu.device, &gpu.queue, &model)[0];
        let previous = animation::bind_pose(&model);
        let mut local = previous.clone();
        local[3..7].copy_from_slice(&Quat::from_rotation_z(std::f32::consts::FRAC_PI_2).to_array());
        let mut pose = Pose::default();
        pose.previous = previous;
        pose.local = local;
        struct RigGame;
        impl exact_game::Game for RigGame {
            const ID: &'static str = "displayed-attachment";
            const ASSETS: &'static [&'static str] = &["test.model"];
            type Args = ();
            fn setup(_: &mut World, _: &()) {}
            fn tick(_: &mut World, _: &exact_game::Input, _: &()) {}
        }
        let mut sim = exact_game::Sim::<RigGame>::new(()).unwrap();
        sim.deliver_asset(
            "test.model",
            Ok(exact_game::asset::Content::Model(model.clone())),
        )
        .unwrap();
        let w = sim.world_mut();
        let e = w.spawn((Transform::default(), Mesh::asset("test.model"), pose));
        let charm = w.spawn((
            Transform::at(9., 8., 7.),
            exact_game::SocketFollow::new(e, "tip"),
        ));
        w.load(&w.save()).unwrap();
        let uniform = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 80,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut values = [0f32; 20];
        values[19] = 0.5;
        gpu.queue.write_buffer(&uniform, 0, bytes(&values));
        skin.set(
            &gpu.device,
            &gpu.queue,
            &uniform,
            &[DrawInstance {
                data: 0,
                transform: e.index(),
                geometry: crate::MeshId(0),
                material: crate::MaterialId(0),
                local: Mat4::IDENTITY,
                skin: Some(template),
            }],
        )
        .unwrap();
        skin.feed(&gpu.queue, w, &[e], false);
        assert_eq!(
            crate::world::tests::allocations::count(|| {
                for _ in 0..300 {
                    skin.pack(w, &[e], false);
                }
            }),
            0
        );
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        skin.encode(&mut encoder, None);
        gpu.queue.submit([encoder.finish()]);
        let actual: Vec<_> = read(&gpu, &skin.palette.raw, 64)
            .chunks_exact(4)
            .map(|v| f32::from_ne_bytes(v.try_into().unwrap()))
            .collect();
        let expected = Mat4::from_rotation_z(std::f32::consts::FRAC_PI_4)
            * Mat4::from_translation(Vec3::new(1., 0., 2.));
        for (a, b) in actual.iter().zip(expected.to_cols_array()) {
            assert!((a - b).abs() < 1e-5, "{actual:?}");
        }
        assert!(
            actual[12] > 0.7,
            "a lerp of composed matrices would give 0.5"
        );
        let mut attachments = crate::world::scene::Attachments::default();
        attachments.feed(w, true, true, false, false);
        attachments.feed(w, false, false, false, false);
        attachments.frame(0.5);
        let displayed = attachments
            .output
            .iter()
            .find(|a| a.entity == charm)
            .unwrap()
            .pose
            .position;
        let palette = Mat4::from_cols_slice(&actual);
        let joint = (palette * Mat4::from_cols_slice(&model.skins[0].inverse_binds).inverse())
            .transform_point3(Vec3::ZERO);
        assert!(
            displayed.distance(joint) < 1e-4,
            "attachment {displayed:?}, GPU joint {joint:?}"
        );
        assert!(
            displayed.distance(Vec3::new(0.5, 0.5, 0.)) > 0.2,
            "endpoint interpolation is the negative control"
        );
        attachments.feed(w, false, false, false, true);
        attachments.frame(0.5);
        assert!(
            attachments.output[0].pose.position.distance(joint) < 1e-4,
            "unrelated model residency must not snap this rig"
        );
        let newborn = w.spawn((
            Transform::default(),
            exact_game::SocketFollow::new(e, "tip"),
        ));
        attachments.feed(w, false, false, false, false);
        attachments.frame(0.5);
        assert!(
            attachments
                .output
                .iter()
                .find(|a| a.entity == newborn)
                .unwrap()
                .pose
                .position
                .distance(joint)
                < 1e-4,
            "new attachments inherit the owner's displayed local history"
        );
        // Different attachments resolve different joints; invalid targets use their authored pose.
        let second = w.spawn((Transform::default(), exact_game::SocketFollow::new(e, "")));
        attachments.feed(w, false, false, false, false);
        attachments.frame(0.5);
        assert_eq!(
            attachments
                .output
                .iter()
                .find(|a| a.entity == second)
                .unwrap()
                .pose
                .position,
            Vec3::ZERO
        );
        w.get_mut::<exact_game::SocketFollow>(charm).unwrap().joint = "missing".into();
        attachments.feed(w, false, false, false, false);
        attachments.frame(0.5);
        assert!(!attachments.output.iter().any(|a| a.entity == charm));
        assert_eq!(
            w.get::<Transform>(charm).unwrap().position,
            Vec3::new(9., 8., 7.)
        );
        // Restore/carry and new batches prime current/current without changing saves.
        let saved = w.save();
        skin.pack(w, &[e], true);
        let len = model.nodes.len() * 10;
        assert_eq!(&skin.pose_words[..len], &skin.pose_words[len..]);
        assert_eq!(w.save(), saved);
        skin.pack(w, &[e], false);
        assert_ne!(&skin.pose_words[..len], &skin.pose_words[len..]);
        w.teleport(e, Transform::at(3., 0., 0.));
        skin.pack(w, &[e], false);
        assert_eq!(&skin.pose_words[..len], &skin.pose_words[len..]);
    }
    #[test]
    #[ignore = "100 Fox palette GPU timestamp diagnostic"]
    fn hundred_fox_palettes() {
        let Some(mut gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
            return;
        };
        let (device, queue) =
            exact_gpu::block_on(gpu.adapter.request_device(&wgpu::DeviceDescriptor {
                required_features: wgpu::Features::TIMESTAMP_QUERY,
                ..Default::default()
            }))
            .unwrap();
        gpu.device = device;
        gpu.queue = queue;
        let path = std::path::PathBuf::from(std::env::var_os("HOME").unwrap())
            .join("Library/Caches/exact2-game/gltf-samples/Fox.glb");
        let model = exact_game_bake::model(path).unwrap();
        let mut skin = Skinning::new(&gpu.device);
        let template = skin.add(&gpu.device, &gpu.queue, &model)[0];
        let mut w = World::new(60, 0);
        let entities: Vec<_> = (0..100).map(|_| w.spawn(Transform::default())).collect();
        let records: Vec<_> = entities
            .iter()
            .map(|e| DrawInstance {
                data: 0,
                transform: e.index(),
                geometry: crate::MeshId(0),
                material: crate::MaterialId(0),
                local: Mat4::IDENTITY,
                skin: Some(template),
            })
            .collect();
        let uniform = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 80,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut values = [0f32; 20];
        values[19] = 0.5;
        gpu.queue.write_buffer(&uniform, 0, bytes(&values));
        skin.set(&gpu.device, &gpu.queue, &uniform, &records)
            .unwrap();
        skin.feed(&gpu.queue, &w, &entities, false);
        let queries = gpu.device.create_query_set(&wgpu::QuerySetDescriptor {
            label: None,
            ty: wgpu::QueryType::Timestamp,
            count: 34,
        });
        let resolve = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 16,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let mut times = Vec::new();
        for i in 0..100 {
            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            skin.encode(&mut encoder, Some(&queries));
            gpu.queue.submit([encoder.finish()]);
            gpu.device
                .poll(wgpu::PollType::wait_indefinitely())
                .unwrap();
            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            encoder.resolve_query_set(&queries, 32..34, &resolve, 0);
            gpu.queue.submit([encoder.finish()]);
            let result = read(&gpu, &resolve, 16);
            let a = u64::from_ne_bytes(result[..8].try_into().unwrap());
            let b = u64::from_ne_bytes(result[8..].try_into().unwrap());
            if i >= 10 {
                times.push((b - a) as f64 * gpu.queue.get_timestamp_period() as f64 / 1e6);
            }
        }
        times.sort_by(f64::total_cmp);
        println!(
            "100 Fox palettes GPU p50 {:.6} ms p95 {:.6} ms",
            times[times.len() / 2],
            times[times.len() * 95 / 100]
        );
    }
}

#[cfg(test)]
mod normal_tests {
    use super::*;
    use glam::{Mat4, Quat, Vec3};
    #[test]
    fn skinned_normal_is_inverse_transpose_under_scaled_rotated_joints() {
        let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
            return;
        };
        // Execute the actual vertex skinning function through a compute entry point.
        let skin = include_str!("shaders/model.wgsl")
            .split("struct BakedMaterial")
            .next()
            .unwrap();
        let source = format!("{skin}\n@group(0) @binding(0) var<storage,read_write> output:array<vec4<f32>>;\n@compute @workgroup_size(1) fn test_normal() {{ let z=mat4x4<f32>(); let draw=ModelInstance(0u,0u,0u,0u,z,z); output[0]=vec4(normalize(skinned(draw,0u,vec3(0.0),normalize(vec3(1.0,1.0,1.0)))[1]),0.0); }}");
        let shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(source.into()),
            });
        let pipeline = gpu
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: None,
                layout: None,
                module: &shader,
                entry_point: Some("test_normal"),
                compilation_options: Default::default(),
                cache: None,
            });
        let a =
            Mat4::from_scale(Vec3::new(3., 1., 0.5)) * Mat4::from_quat(Quat::from_rotation_z(0.7));
        let b = Mat4::from_scale_rotation_translation(
            Vec3::new(1., 2., 4.),
            Quat::from_rotation_y(0.3),
            Vec3::ZERO,
        );
        let buffer = |data: &[u8]| {
            let b = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: data.len() as u64,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            gpu.queue.write_buffer(&b, 0, data);
            b
        };
        let matrices = buffer(bytes(&[a.to_cols_array(), b.to_cols_array()].concat()));
        let vertices = buffer(bytes(&[
            0u32,
            1,
            0,
            0,
            0.75f32.to_bits(),
            0.25f32.to_bits(),
            0,
            0,
        ]));
        let output = buffer(bytes(&[0f32; 4]));
        let bind = |group, entries: &[wgpu::BindGroupEntry<'_>]| {
            gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &pipeline.get_bind_group_layout(group),
                entries,
            })
        };
        let out = bind(
            0,
            &[wgpu::BindGroupEntry {
                binding: 0,
                resource: output.as_entire_binding(),
            }],
        );
        let empty1 = bind(1, &[]);
        let empty2 = bind(2, &[]);
        let input = bind(
            3,
            &[
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: matrices.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: vertices.as_entire_binding(),
                },
            ],
        );
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &out, &[]);
            pass.set_bind_group(1, &empty1, &[]);
            pass.set_bind_group(2, &empty2, &[]);
            pass.set_bind_group(3, &input, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        gpu.queue.submit([encoder.finish()]);
        let actual: Vec<f32> = super::tests::read(&gpu, &output, 16)
            .chunks_exact(4)
            .map(|v| f32::from_ne_bytes(v.try_into().unwrap()))
            .collect();
        let expected = (a * 0.75 + b * 0.25)
            .inverse()
            .transpose()
            .transform_vector3(Vec3::ONE.normalize())
            .normalize();
        assert!(
            Vec3::from_slice(&actual).distance(expected) < 1e-5,
            "{actual:?} expected {expected:?}"
        );
    }
}
