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
}
pub struct Frame {
    pub pixels: wgpu::Buffer,
    pub timestamps: Option<wgpu::Buffer>,
    pub row_bytes: u32,
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
        if self.mode == Mode::Naive
            && matches!(view, View::Clusters | View::Depth | View::Triangles)
        {
            return Err("cluster, DAG depth and triangle debug views require cluster mode".into());
        }
        if self.mode == Mode::Cluster {
            if selection.pages.len() != self.pages.len()
                || shadow_selection.pages.len() != self.pages.len()
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
            params: [view as u32, self.max_depth, 0, 0],
        };
        self.queue
            .write_buffer(&self.globals, 0, bytemuck::bytes_of(&globals));
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("cluster reference frame"),
            });
        let writes = |start, end| {
            self.query
                .as_ref()
                .map(|query_set| wgpu::RenderPassTimestampWrites {
                    query_set,
                    beginning_of_pass_write_index: Some(start),
                    end_of_pass_write_index: Some(end),
                })
        };
        {
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
                timestamp_writes: writes(0, 1),
                ..Default::default()
            });
            pass.set_pipeline(&self.shadow_pipeline);
            pass.set_bind_group(0, &self.global_bind, &[]);
            self.draw(&mut pass, &self.shadow_lists, shadow_selection);
        }
        let color_view = self.color.create_view(&Default::default());
        {
            let attachments = [Some(wgpu::RenderPassColorAttachment {
                view: &self.msaa,
                depth_slice: None,
                resolve_target: Some(&color_view),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(if view == View::Overdraw {
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
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: writes(2, 3),
                ..Default::default()
            });
            pass.set_bind_group(0, &self.global_bind, &[]);
            pass.set_bind_group(1, &self.lists[0].bind, &[]);
            pass.set_bind_group(2, &self.shadow_bind, &[]);
            // Keep one ground draw in every view and report it separately from geometry.
            pass.set_pipeline(&self.ground);
            pass.draw(0..6, 0..1);
            pass.set_pipeline(if view == View::Overdraw {
                &self.overdraw
            } else {
                &self.main
            });
            self.draw(&mut pass, &self.lists, selection);
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
                size: 32,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            encoder.resolve_query_set(query, 0..4, resolve, 0);
            encoder.copy_buffer_to_buffer(resolve, 0, &read, 0, 32);
            Some(read)
        } else {
            None
        };
        self.queue.submit([encoder.finish()]);
        let main_triangles = self.chunks.iter().map(|c| c.count as u64 / 3).sum::<u64>()
            * self.instance_count as u64;
        let mut stats = if self.mode == Mode::Cluster {
            FrameStats {
                selected_clusters: selection.clusters,
                triangles: selection.triangles,
                padded_triangles: selection.padded_triangles,
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
                draws: self.chunks.len() as u32,
                shadow_triangles: main_triangles,
                shadow_draws: self.chunks.len() as u32,
                ..Default::default()
            }
        };
        stats.resident_bytes = self.static_bytes
            + self
                .lists
                .iter()
                .chain(&self.shadow_lists)
                .map(|l| l.capacity)
                .sum::<u64>();
        stats.readback_bytes = pixels.size() + timestamps.as_ref().map_or(0, |b| b.size());
        Ok(Frame {
            pixels,
            timestamps,
            row_bytes,
            stats,
        })
    }
    fn draw(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        lists: &[crate::gpu::ListGpu],
        selection: &Selection,
    ) {
        if self.mode == Mode::Cluster {
            for (list, pairs) in lists.iter().zip(&selection.pages) {
                pass.set_bind_group(1, &list.bind, &[]);
                pass.draw(0..self.max_triangles * 3, 0..pairs.len() as u32);
            }
        } else {
            pass.set_bind_group(1, &lists[0].bind, &[]);
            for chunk in &self.chunks {
                pass.set_vertex_buffer(0, chunk.vertices.slice(..));
                pass.set_index_buffer(chunk.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..chunk.count, 0, 0..self.instance_count);
            }
        }
    }
}
