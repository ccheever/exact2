//! Model-only buffers. Local poses cross the feed seam; matrices exist only on the GPU.
use crate::{
    buffers::{bytes, Buffer},
    DrawInstance, RenderError,
};
use exact_game::{animation, asset::Model, Entity, Pose, World};
use exact_gpu::wgpu;
struct Template {
    meta: u32,
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
        for (i, record) in records.iter().enumerate() {
            if let Some(skin) = record.skin {
                let t = self
                    .templates
                    .get(skin as usize)
                    .ok_or_else(|| RenderError::scene("unknown skin template".into()))?;
                self.offsets.push(palette as u32);
                self.records.push((i, skin as usize));
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
            let e = entities[record];
            let p = w.get::<Pose>(e);
            let (prev, curr) = p
                .as_ref()
                .filter(|p| p.local.len() == len && p.previous.len() == len)
                .map_or((&t.rest[..], &t.rest[..]), |p| {
                    (&p.previous[..], &p.local[..])
                });
            self.pose_words[offset..offset + len].copy_from_slice(
                if initial || w.fresh().contains(&e) {
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
mod tests {
    use super::*;
    use exact_game::{
        asset::{Node, Skin},
        Mesh, Transform,
    };
    use glam::{Mat4, Quat, Vec3};
    pub(super) fn read(gpu: &exact_gpu::Gpu, source: &wgpu::Buffer, size: u64) -> Vec<u8> {
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
    fn interpolate_locals_before_composing_and_inverse_bind() {
        let Ok(gpu) = exact_gpu::fixture::device() else {
            return;
        };
        let model = Model {
            nodes: vec![
                Node::default(),
                Node {
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
        let mut w = World::new(60, 0);
        let e = w.spawn((Transform::default(), Mesh::asset("test.model"), pose));
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
                transform: 0,
                geometry: crate::MeshId(0),
                material: crate::MaterialId(0),
                local: Mat4::IDENTITY,
                skin: Some(template),
            }],
        )
        .unwrap();
        skin.feed(&gpu.queue, &w, &[e], false);
        assert_eq!(
            crate::world::tests::allocations::count(|| {
                for _ in 0..300 {
                    skin.pack(&w, &[e], false);
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
        // Restore/carry and new batches prime current/current without changing saves.
        let saved = w.save();
        skin.pack(&w, &[e], true);
        let len = model.nodes.len() * 10;
        assert_eq!(&skin.pose_words[..len], &skin.pose_words[len..]);
        assert_eq!(w.save(), saved);
        skin.pack(&w, &[e], false);
        assert_ne!(&skin.pose_words[..len], &skin.pose_words[len..]);
        w.teleport(e, Transform::at(3., 0., 0.));
        skin.pack(&w, &[e], false);
        assert_eq!(&skin.pose_words[..len], &skin.pose_words[len..]);
    }
    #[test]
    #[ignore = "100 Fox palette GPU timestamp diagnostic"]
    fn hundred_fox_palettes() {
        let Ok(mut gpu) = exact_gpu::fixture::device() else {
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
        let gpu = exact_gpu::fixture::device().unwrap();
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
