use super::*;

impl Module {
    /// Child composition; changing away from Each retires captured child textures.
    pub fn children_mode(&mut self, id: u32) -> ChildrenMode {
        let mode = self
            .instances
            .get(&id)
            .map_or(ChildrenMode::Overlay, |i| i.surface.children_mode());
        if mode != ChildrenMode::Each && self.instances.get(&id).is_some_and(|i| !i.each.is_empty())
        {
            self.children_count(id, 0);
        }
        mode
    }

    /// The `index`th direct child of a canvas, painted by the host (LLP 1014
    /// D5): `frame` in the canvas's points, `width`×`height` premultiplied
    /// RGBA pixels. Creates or replaces the texture at a new size, writes
    /// the pixels, tells the surface, and marks the canvas dirty.
    pub fn child(
        &mut self,
        id: u32,
        index: usize,
        name: &str,
        frame: [f32; 4],
        size: [u32; 2],
        bytes: &[u8],
    ) -> bool {
        let [width, height] = size;
        self.check_device();
        if !frame.iter().all(|n| n.is_finite()) || frame[2] < 0. || frame[3] < 0. {
            self.error = format!("child {index}: invalid frame");
            return false;
        }
        if width == 0 && height == 0 && bytes.is_empty() {
            let Some(inst) = self.instances.get_mut(&id) else {
                return false;
            };
            if index > inst.each.len() {
                return false;
            }
            if index == inst.each.len() {
                inst.each.push(None);
            }
            inst.each[index] = None;
            inst.surface.child(index, name, None, frame);
            inst.dirty = true;
            return true;
        }
        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4));
        if width == 0 || height == 0 || expected != Some(bytes.len()) {
            self.error = format!("child {index}: {} bytes for {width}x{height}", bytes.len());
            return false;
        }
        let Some(gpu) = self.gpu.as_ref() else {
            self.error = "no device".into();
            return false;
        };
        let Some(inst) = self.instances.get_mut(&id) else {
            self.error = "no such canvas".into();
            return false;
        };
        // Children arrive in order: the next index appends, an earlier one
        // replaces; a gap is a host's mistake, refused — never an allocation
        // the host's number sizes.
        if index > inst.each.len() {
            self.error = format!(
                "child {index}: out of order ({} children so far)",
                inst.each.len()
            );
            return false;
        }
        if inst.each.len() == index {
            inst.each.push(None);
        }
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let same = matches!(&inst.each[index], Some(c) if c.width == width && c.height == height);
        if !same {
            let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("child"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            inst.surface.child(index, name, Some(&view), frame);
            inst.each[index] = Some(ChildTexture {
                texture,
                width,
                height,
            });
        } else {
            // The same texture; the frame may have moved.
            let texture = &inst.each[index].as_ref().expect("checked").texture;
            let view = texture.create_view(&Default::default());
            inst.surface.child(index, name, Some(&view), frame);
        }
        let child = inst.each[index].as_ref().expect("just set");
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &child.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: None,
            },
            size,
        );
        inst.children_generation += 1;
        inst.dirty = true;
        true
    }

    /// How many direct children a canvas has now (LLP 1014 D5): the textures
    /// past it are dropped and the surface told.
    pub fn children_count(&mut self, id: u32, count: usize) -> bool {
        self.check_device();
        let Some(inst) = self.instances.get_mut(&id) else {
            self.error = "no such canvas".into();
            return false;
        };
        if inst.each.len() > count {
            for index in count..inst.each.len() {
                inst.surface.child(index, "", None, [0.0; 4]);
            }
            inst.each.truncate(count);
        }
        inst.dirty = true;
        true
    }

    /// Where a canvas's surface put its `index`th child (LLP 1014 D5).
    pub fn placement(&self, id: u32, index: usize) -> Option<Placement> {
        self.instances
            .get(&id)
            .and_then(|i| i.surface.placement(index))
    }

    /// The canvas's children, painted by the host (LLP 1014 D3): `width`×`height`
    /// premultiplied RGBA, rows top-down, tightly packed. Creates or
    /// replaces the texture at a new size, writes the pixels, and marks the
    /// canvas dirty.
    pub fn texture(&mut self, id: u32, width: u32, height: u32, bytes: &[u8]) -> bool {
        self.check_device();
        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4));
        if width == 0 || height == 0 || expected != Some(bytes.len()) {
            self.error = format!("children: {} bytes for {width}x{height}", bytes.len());
            return false;
        }
        let Some(gpu) = self.gpu.as_ref() else {
            self.error = "no device".into();
            return false;
        };
        let Some(inst) = self.instances.get_mut(&id) else {
            self.error = "no such canvas".into();
            return false;
        };
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let same = matches!(&inst.children, Some(c) if c.width == width && c.height == height && c.metal.is_none());
        if !same {
            let make = |label: &str| {
                gpu.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::COPY_DST
                        | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                })
            };
            let texture = make("children");
            let view = texture.create_view(&Default::default());
            let previous = matches!(
                inst.surface.children_mode(),
                ChildrenMode::Composite { previous: true }
            )
            .then(|| make("previous children"));
            let previous_view = previous
                .as_ref()
                .map(|p| p.create_view(&Default::default()));
            inst.surface.children(Some(&view), previous_view.as_ref());
            inst.children = Some(Children {
                texture,
                previous,
                width,
                height,
                metal: None,
            });
        } else if let Some(Children {
            texture,
            previous: Some(previous),
            ..
        }) = &inst.children
        {
            // What the children were, before the write below lands: a copy
            // submitted now runs before a write enqueued after it.
            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            encoder.copy_texture_to_texture(
                texture.as_image_copy(),
                previous.as_image_copy(),
                size,
            );
            gpu.queue.submit([encoder.finish()]);
        }
        inst.children_generation += 1;
        let children = inst.children.as_ref().expect("just set");
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &children.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: None,
            },
            size,
        );
        inst.dirty = true;
        true
    }
}
