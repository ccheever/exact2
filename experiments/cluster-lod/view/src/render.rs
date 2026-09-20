use crate::{
    Mode, Renderer, View,
    gpu::{Globals, Result},
    scene::{Camera, Scene},
    select::Selection,
};

#[derive(Default, Clone, Debug)]
pub struct FrameStats {
    pub selected_clusters: u64,
    pub triangles: u64,
    pub padded_triangles: u64,
    pub draws: u32,
    pub shadow_clusters: u64,
    pub shadow_triangles: u64,
    pub shadow_padding: u64,
    pub shadow_draws: u32,
    pub resident_bytes: u64,
    pub readback_bytes: u64,
    pub candidates: u64,
    pub shadow_candidates: u64,
    pub overflow: u64,
    pub shadow_overflow: u64,
}
pub struct Frame {
    pub pixels: wgpu::Buffer,
    pub timestamps: Option<wgpu::Buffer>,
    pub row_bytes: u32,
    pub gpu_selected: bool,
    pub shadows: bool,
    pub stats: FrameStats,
}
impl Renderer {
    /// Encode, submit and schedule copies; the host drives polling/map completion.
    pub fn render(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        view: View,
        selection: &Selection,
        shadow_selection: &Selection,
    ) -> Result<Frame> {
        self.render_inner(scene, camera, view, selection, shadow_selection, None)
    }
    pub fn render_gpu(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        view: View,
        threshold: f32,
        cull: bool,
        brute: bool,
    ) -> Result<Frame> {
        if self.compute.is_none() {
            return Err("enable GPU selection at scene load first".into());
        }
        self.render_inner(
            scene,
            camera,
            view,
            &Selection::default(),
            &Selection::default(),
            Some((threshold, cull && view != View::Overdraw, brute)),
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn render_inner(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        view: View,
        selection: &Selection,
        shadow_selection: &Selection,
        gpu: Option<(f32, bool, bool)>,
    ) -> Result<Frame> {
        let shadows = self.shadows && view != View::Coverage;
        let cull = gpu.map_or(self.culling, |(_, c, _)| c) && view != View::Overdraw;
        let ranges = |camera: &Camera| {
            let mut ranges: Vec<std::ops::Range<u32>> = Vec::new();
            let planes = camera.planes();
            if self.mode == Mode::Naive {
                for (i, instance) in scene.instances.iter().enumerate() {
                    if cull
                        && !crate::select::sphere_visible(
                            self.instance_sphere,
                            instance.transform(),
                            instance.scale(),
                            &planes,
                        )
                    {
                        continue;
                    }
                    let i = i as u32;
                    if let Some(last) = ranges.last_mut().filter(|r| r.end == i) {
                        last.end += 1;
                    } else {
                        ranges.push(i..i + 1);
                    }
                }
            }
            ranges
        };
        let main_instances = ranges(camera);
        let shadow_instances = if shadows {
            ranges(&scene.light_camera())
        } else {
            Vec::new()
        };
        if gpu.is_none() && self.compute.is_some() {
            return Err(
                "CPU reference rendering needs its own renderer with CPU-owned lists".into(),
            );
        }
        if self.mode == Mode::Naive
            && matches!(view, View::Clusters | View::Depth | View::Triangles)
        {
            return Err("cluster, DAG depth and triangle debug views require cluster mode".into());
        }
        if self.mode == Mode::Cluster && gpu.is_none() {
            if selection.pages.len() != self.pages.len()
                || (shadows && shadow_selection.pages.len() != self.pages.len())
            {
                return Err("selection page count mismatch".into());
            }
            for (lists, selected) in [
                (&mut self.lists, selection),
                (&mut self.shadow_lists, shadow_selection),
            ] {
                for (id, pairs) in selected.pages.iter().enumerate() {
                    let bytes = bytemuck::cast_slice(pairs);
                    let size = (bytes.len() as u64).max(8);
                    if size > 128 * 1024 * 1024 {
                        return Err(format!(
                            "page {id} visible list exceeds 128 MiB core storage binding"
                        ));
                    }
                    if size > lists[id].capacity {
                        lists[id] = Self::make_list(
                            &self.device,
                            &self.page_layout,
                            &self.pages[id],
                            &self.clusters,
                            &self.instances,
                            &self.dummy_draws,
                            size.next_power_of_two(),
                        );
                    }
                    if !bytes.is_empty() {
                        self.queue.write_buffer(&lists[id].buffer, 0, bytes);
                    }
                }
            }
        }
        let globals = Globals {
            vp: camera.matrix.to_cols_array(),
            light_vp: scene.light_camera().matrix.to_cols_array(),
            eye: camera.eye.extend(1.0).to_array(),
            ground: [scene.center.x, scene.center.y, -0.015, scene.radius * 50.0],
            params: [
                view as u32,
                self.max_depth,
                gpu.is_some() as u32,
                shadows as u32,
            ],
        };
        self.queue
            .write_buffer(&self.globals, 0, bytemuck::bytes_of(&globals));
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("cluster reference frame"),
            });
        if let Some((threshold, cull, brute)) = gpu {
            let compute = self.compute.as_ref().expect("enabled");
            compute.encode(
                self,
                &mut encoder,
                camera,
                self.height,
                threshold,
                cull,
                brute,
                0,
            );
            if shadows {
                compute.encode(
                    self,
                    &mut encoder,
                    &scene.light_camera(),
                    2048,
                    (threshold * 2.0).min(f32::MAX / 2.0),
                    cull,
                    brute,
                    1,
                );
            } else {
                encoder.clear_buffer(&compute.passes[1].draws, 0, None);
            }
        }
        let writes = |start, end| {
            self.query
                .as_ref()
                .map(|query_set| wgpu::RenderPassTimestampWrites {
                    query_set,
                    beginning_of_pass_write_index: Some(start),
                    end_of_pass_write_index: Some(end),
                })
        };
        if shadows {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("sun shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: writes(
                    if gpu.is_some() { 4 } else { 0 },
                    if gpu.is_some() { 5 } else { 1 },
                ),
                ..Default::default()
            });
            pass.set_pipeline(&self.shadow_pipeline);
            pass.set_bind_group(0, &self.global_bind, &[]);
            self.draw(
                &mut pass,
                &self.shadow_lists,
                shadow_selection,
                gpu.map(|_| 1),
                &shadow_instances,
            );
        }
        let color_view = self.color.create_view(&Default::default());
        {
            let attachments = [Some(wgpu::RenderPassColorAttachment {
                view: &self.msaa,
                depth_slice: None,
                resolve_target: Some(&color_view),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(if view == View::Coverage {
                        wgpu::Color {
                            r: 1.0,
                            g: 0.0,
                            b: 1.0,
                            a: 1.0,
                        }
                    } else if view == View::Overdraw {
                        wgpu::Color::BLACK
                    } else {
                        wgpu::Color {
                            r: 0.035,
                            g: 0.050,
                            b: 0.072,
                            a: 1.0,
                        }
                    }),
                    store: wgpu::StoreOp::Discard,
                },
            })];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("hardware raster"),
                color_attachments: &attachments,
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: writes(
                    if gpu.is_some() { 6 } else { 2 },
                    if gpu.is_some() { 7 } else { 3 },
                ),
                ..Default::default()
            });
            pass.set_bind_group(0, &self.global_bind, &[]);
            pass.set_bind_group(1, &self.lists[0].bind, &[]);
            pass.set_bind_group(2, &self.shadow_bind, &[]);
            // Keep one ground draw in every view and report it separately from geometry.
            if view != View::Coverage {
                pass.set_pipeline(&self.ground);
                pass.draw(0..6, 0..1);
            }
            pass.set_pipeline(if view == View::Overdraw {
                &self.overdraw
            } else {
                &self.main
            });
            self.draw(
                &mut pass,
                &self.lists,
                selection,
                gpu.map(|_| 0),
                &main_instances,
            );
        }
        let row_bytes = (self.width * 4).div_ceil(256) * 256;
        let pixels = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame readback"),
            size: row_bytes as u64 * self.height as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.color,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &pixels,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row_bytes),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
        let timestamps = if let (Some(query), Some(resolve)) = (&self.query, &self.query_resolve) {
            let read = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("timestamps"),
                size: 64,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            encoder.resolve_query_set(query, 0..if gpu.is_some() { 8 } else { 4 }, resolve, 0);
            encoder.copy_buffer_to_buffer(
                resolve,
                0,
                &read,
                0,
                if gpu.is_some() { 64 } else { 32 },
            );
            Some(read)
        } else {
            None
        };
        self.queue.submit([encoder.finish()]);
        let source_triangles = self.chunks.iter().map(|c| c.count as u64 / 3).sum::<u64>();
        let main_triangles =
            source_triangles * main_instances.iter().map(|r| r.len() as u64).sum::<u64>();
        let mut stats = if self.mode == Mode::Cluster {
            FrameStats {
                selected_clusters: selection.clusters,
                triangles: selection.triangles,
                padded_triangles: selection.padded_triangles,
                candidates: selection.candidates,
                shadow_candidates: shadow_selection.candidates,
                overflow: selection.overflow,
                shadow_overflow: shadow_selection.overflow,
                draws: self.pages.len() as u32,
                shadow_clusters: shadow_selection.clusters,
                shadow_triangles: shadow_selection.triangles,
                shadow_padding: shadow_selection.padded_triangles,
                shadow_draws: self.pages.len() as u32,
                ..Default::default()
            }
        } else {
            FrameStats {
                triangles: main_triangles,
                draws: (self.chunks.len() * main_instances.len()) as u32,
                shadow_triangles: source_triangles
                    * shadow_instances.iter().map(|r| r.len() as u64).sum::<u64>(),
                shadow_draws: (self.chunks.len() * shadow_instances.len()) as u32,
                ..Default::default()
            }
        };
        if !shadows {
            stats.shadow_clusters = 0;
            stats.shadow_triangles = 0;
            stats.shadow_padding = 0;
            stats.shadow_draws = 0;
            stats.shadow_candidates = 0;
            stats.shadow_overflow = 0;
        }
        stats.resident_bytes = self.static_bytes
            + if let Some(c) = &self.compute {
                c.bytes
            } else {
                self.lists
                    .iter()
                    .chain(&self.shadow_lists)
                    .map(|l| l.capacity)
                    .sum::<u64>()
            };
        stats.readback_bytes = pixels.size() + timestamps.as_ref().map_or(0, |b| b.size());
        Ok(Frame {
            pixels,
            timestamps,
            row_bytes,
            gpu_selected: gpu.is_some(),
            shadows,
            stats,
        })
    }
    fn draw(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        lists: &[crate::gpu::ListGpu],
        selection: &Selection,
        gpu: Option<usize>,
        instances: &[std::ops::Range<u32>],
    ) {
        if self.mode == Mode::Cluster {
            for (id, list) in lists.iter().enumerate() {
                pass.set_bind_group(1, &list.bind, &[]);
                if let Some(p) = gpu {
                    pass.draw_indirect(
                        &self.compute.as_ref().expect("enabled").passes[p].draws,
                        id as u64 * 32,
                    );
                } else {
                    pass.draw(
                        0..self.max_triangles * 3,
                        0..selection.pages[id].len() as u32,
                    );
                }
            }
        } else {
            pass.set_bind_group(1, &lists[0].bind, &[]);
            for chunk in &self.chunks {
                pass.set_vertex_buffer(0, chunk.vertices.slice(..));
                pass.set_index_buffer(chunk.indices.slice(..), wgpu::IndexFormat::Uint32);
                for range in instances {
                    pass.draw_indexed(0..chunk.count, 0, range.clone());
                }
            }
        }
    }
}
