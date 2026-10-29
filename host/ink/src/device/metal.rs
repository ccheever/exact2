//! Direct Metal over the same instances and shader as wgpu. @ref LLP 1149 D5.2.
use super::check_region;
use super::texture_ids;
use super::{Device, Error, Format, Info, Result, Submission, TextureDesc};
use crate::instance::{flags, Bindings, Frame, Instance, Kind, TextureId};
use objc2::{
    rc::{autoreleasepool, Retained},
    runtime::ProtocolObject,
};
use objc2_metal::*;
use std::{
    cell::Cell,
    collections::{BTreeMap, HashMap},
    ptr::NonNull,
    rc::Rc,
    time::Instant,
};

mod mips;
#[cfg(target_os = "macos")]
mod present;
mod setup;
#[cfg(test)]
mod tests;

struct Texture {
    raw: Retained<ProtocolObject<dyn MTLTexture>>,
    desc: TextureDesc,
    last: Submission,
}

#[derive(Clone)]
pub struct Buffer {
    raw: Retained<ProtocolObject<dyn MTLBuffer>>,
    last: Rc<Cell<Submission>>,
    owner: Rc<()>,
}

pub struct Metal {
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    pipeline: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    replace_pipeline: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    sampler: Retained<ProtocolObject<dyn MTLSamplerState>>,
    repeat_sampler: Retained<ProtocolObject<dyn MTLSamplerState>>,
    buffers: [Buffer; 3],
    next_buffer: usize,
    white_alpha: Texture,
    white_color: Texture,
    textures: HashMap<TextureId, Texture>,
    next_texture: u64,
    submitted: Submission,
    done: Submission,
    pending: BTreeMap<u64, Option<Retained<ProtocolObject<dyn MTLCommandBuffer>>>>,
    retired_textures: Vec<Texture>,
    retired_buffers: Vec<Buffer>,
    owner: Rc<()>,
    error: Option<String>,
    info: Info,
    #[cfg(target_os = "macos")]
    window: Option<present::Window>,
}

impl Metal {
    pub fn new() -> Result<Self> {
        autoreleasepool(|_| {
            let start = Instant::now();
            let device = MTLCreateSystemDefaultDevice()
                .ok_or_else(|| Error::NoAdapter("MTLCreateSystemDefaultDevice".into()))?;
            let queue = device
                .newCommandQueue()
                .ok_or_else(|| Error::Device("command queue allocation".into()))?;
            let adapter = device.name().to_string();
            let device_ms = start.elapsed().as_secs_f64() * 1000.;
            let start = Instant::now();
            let pipelines = setup::pipelines(&device, MTLPixelFormat::RGBA8Unorm)?;
            let shaders_ms = start.elapsed().as_secs_f64() * 1000.;
            pipelines.save();
            let sampler = setup::sampler(&device, false)?;
            let repeat_sampler = setup::sampler(&device, true)?;
            let owner = Rc::new(());
            let buffers = [
                buffer(&device, &owner, 192)?,
                buffer(&device, &owner, 192)?,
                buffer(&device, &owner, 192)?,
            ];
            let white_alpha = texture(
                &device,
                TextureDesc {
                    width: 1,
                    height: 1,
                    format: Format::Alpha,
                    mipmapped: false,
                },
            )?;
            let white_color = texture(&device, TextureDesc::rgba(1, 1))?;
            upload(&white_alpha, [0, 0], [1, 1], &[255]);
            upload(&white_color, [0, 0], [1, 1], &[255; 4]);
            Ok(Self {
                device,
                queue,
                pipeline: pipelines.blend,
                replace_pipeline: pipelines.replace,
                sampler,
                repeat_sampler,
                buffers,
                next_buffer: 0,
                white_alpha,
                white_color,
                textures: HashMap::new(),
                next_texture: 1,
                submitted: Submission(0),
                done: Submission(0),
                pending: BTreeMap::new(),
                retired_textures: Vec::new(),
                retired_buffers: Vec::new(),
                owner,
                error: None,
                info: Info {
                    adapter,
                    api: "direct Metal".into(),
                    device_ms,
                    shaders_ms,
                    cached: pipelines.cached,
                },
                #[cfg(target_os = "macos")]
                window: None,
            })
        })
    }

    fn texture(&self, id: TextureId) -> Result<&Texture> {
        self.textures
            .get(&id)
            .ok_or_else(|| Error::Invalid(format!("unknown texture {}", id.0)))
    }

    fn slot(&self, id: TextureId, format: Format) -> Result<&ProtocolObject<dyn MTLTexture>> {
        let t = if id.0 == 0 {
            if format == Format::Alpha {
                &self.white_alpha
            } else {
                &self.white_color
            }
        } else {
            self.texture(id)?
        };
        if t.desc.format != format {
            return Err(Error::Invalid("texture slot format".into()));
        }
        Ok(&t.raw)
    }

    fn bindings(&self, b: Bindings) -> Result<[&ProtocolObject<dyn MTLTexture>; 7]> {
        Ok([
            self.slot(b.glyph, Format::Alpha)?,
            self.slot(b.color, Format::Rgba)?,
            self.slot(b.path, Format::Alpha)?,
            self.slot(b.ramp, Format::Rgba)?,
            self.slot(b.image, Format::Rgba)?,
            if b.mask.0 == 0 {
                &*self.white_alpha.raw
            } else {
                &*self.texture(b.mask)?.raw
            },
            self.slot(b.destination, Format::Rgba)?,
        ])
    }

    fn commands(&self) -> Result<Retained<ProtocolObject<dyn MTLCommandBuffer>>> {
        if let Some(e) = &self.error {
            return Err(Error::Device(e.clone()));
        }
        self.queue
            .commandBuffer()
            .ok_or_else(|| Error::Device("command buffer allocation".into()))
    }

    fn submit(
        &mut self,
        commands: Option<Retained<ProtocolObject<dyn MTLCommandBuffer>>>,
    ) -> Submission {
        if let Some(commands) = &commands {
            #[cfg(target_os = "macos")]
            if let Some(window) = &self.window {
                let command = present::Command(commands.clone());
                if let Err(e) = window.thread.send(move || command.commit()) {
                    self.error = Some(e.to_string());
                }
            } else {
                commands.commit();
            }
            #[cfg(not(target_os = "macos"))]
            commands.commit();
        }
        self.submitted.0 += 1;
        self.pending.insert(self.submitted.0, commands);
        self.submitted
    }

    /// CPU access follows GPU completion, including managed-memory synchronization.
    /// https://developer.apple.com/documentation/metal/mtltexture/getbytes(_:bytesperrow:from:mipmaplevel:)
    pub fn readback_level(&mut self, id: TextureId, level: u32) -> Result<Vec<u8>> {
        autoreleasepool(|_| {
            let t = self.texture(id)?;
            if level >= t.desc.levels() {
                return Err(Error::Invalid("mip level".into()));
            }
            #[cfg(target_os = "macos")]
            if t.raw.storageMode() == MTLStorageMode::Managed {
                let command = self.commands()?;
                let blit = command
                    .blitCommandEncoder()
                    .ok_or_else(|| Error::Device("blit encoder".into()))?;
                // SAFETY: a 2D texture has slice zero; the mip was checked above.
                unsafe {
                    blit.synchronizeTexture_slice_level(&t.raw, 0, level as usize);
                }
                blit.endEncoding();
                let s = self.submit(Some(command));
                self.textures.get_mut(&id).unwrap().last = s;
            }
            self.wait(self.last_use(id)?)?;
            let t = self.texture(id)?;
            let width = (t.desc.width >> level).max(1);
            let height = (t.desc.height >> level).max(1);
            let row = width as usize * t.desc.format.channels() as usize;
            let mut pixels = vec![0; row * height as usize];
            // SAFETY: CPU-visible storage, complete GPU writes, a checked mip
            // and a destination large enough for every tightly packed row.
            unsafe {
                t.raw.getBytes_bytesPerRow_fromRegion_mipmapLevel(
                    NonNull::new(pixels.as_mut_ptr()).unwrap().cast(),
                    row,
                    region([0, 0], [width, height]),
                    level as usize,
                );
            }
            Ok(pixels)
        })
    }
}

impl Device for Metal {
    type Buffer = Buffer;
    fn info(&self) -> &Info {
        &self.info
    }
    fn create_buffer(&self, bytes: u64) -> Result<Buffer> {
        let bytes = usize::try_from(bytes).map_err(|_| Error::Invalid("buffer size".into()))?;
        if bytes == 0 || bytes > self.device.maxBufferLength() {
            return Err(Error::Invalid("buffer size".into()));
        }
        buffer(&self.device, &self.owner, bytes)
    }
    fn upload_buffer(&mut self, buffer: &Buffer, offset: u64, data: &[u8]) -> Result<Submission> {
        if !Rc::ptr_eq(&self.owner, &buffer.owner)
            || !offset.is_multiple_of(4)
            || !data.len().is_multiple_of(4)
            || offset
                .checked_add(data.len() as u64)
                .is_none_or(|end| end > buffer.raw.length() as u64)
        {
            return Err(Error::Invalid(
                "instance buffer upload bounds, owner or alignment".into(),
            ));
        }
        self.wait(buffer.last.get())?;
        // SAFETY: the checked range is inside this device's shared buffer; its
        // last GPU read completed, and &mut self serializes writes on this device.
        unsafe {
            std::ptr::copy_nonoverlapping(
                data.as_ptr(),
                buffer
                    .raw
                    .contents()
                    .as_ptr()
                    .cast::<u8>()
                    .add(offset as usize),
                data.len(),
            );
        }
        let s = self.submit(None);
        buffer.last.set(s);
        Ok(s)
    }
    fn create_texture(&mut self, desc: TextureDesc) -> Result<TextureId> {
        #[cfg(target_os = "macos")]
        let _trace = super::apple::Trace::new(c"texture allocate");
        if desc.width == 0 || desc.height == 0 || desc.width > 16384 || desc.height > 16384 {
            return Err(Error::Invalid("texture dimensions".into()));
        }
        if desc.mipmapped && desc.format == Format::Alpha {
            return Err(Error::Unsupported("alpha mip chain"));
        }
        let texture = autoreleasepool(|_| texture(&self.device, desc))?;
        let id = TextureId(self.next_texture);
        self.next_texture += 1;
        #[cfg(target_os = "macos")]
        super::apple::trace_mark(
            c"texture bytes",
            MTLResource::allocatedSize(&*texture.raw) as f64,
        );
        self.textures.insert(id, texture);
        Ok(id)
    }
    fn upload_texture(
        &mut self,
        id: TextureId,
        origin: [u32; 2],
        size: [u32; 2],
        data: &[u8],
    ) -> Result<Submission> {
        let desc = self.texture(id)?.desc;
        check_region(desc, origin, size)?;
        if data.len() as u64
            != u64::from(size[0]) * u64::from(size[1]) * u64::from(desc.format.channels())
        {
            return Err(Error::Invalid("texture upload length".into()));
        }
        #[cfg(target_os = "macos")]
        if let Some(window) = &mut self.window {
            if let Some(error) = &self.error {
                return Err(Error::Device(error.clone()));
            }
            let command = self
                .queue
                .commandBuffer()
                .ok_or_else(|| Error::Device("upload command buffer allocation".into()))?;
            window.upload(
                self.textures.get(&id).unwrap(),
                command.clone(),
                origin,
                size,
                data,
            )?;
            self.submitted.0 += 1;
            self.pending.insert(self.submitted.0, Some(command));
            self.textures.get_mut(&id).unwrap().last = self.submitted;
            return Ok(self.submitted);
        }
        self.wait(self.last_use(id)?)?;
        upload(self.texture(id)?, origin, size, data);
        let s = self.submit(None);
        self.textures.get_mut(&id).unwrap().last = s;
        Ok(s)
    }
    fn last_use(&self, id: TextureId) -> Result<Submission> {
        Ok(self.texture(id)?.last)
    }
    fn generate_mips(&mut self, id: TextureId) -> Result<Submission> {
        #[cfg(target_os = "macos")]
        let _trace = super::apple::Trace::new(c"mips encode");
        autoreleasepool(|_| {
            self.completed()?;
            let t = self.texture(id)?;
            if t.desc.format != Format::Rgba {
                return Err(Error::Unsupported("alpha mip chain"));
            }
            let command = self.commands()?;
            if t.desc.width.is_power_of_two() && t.desc.height.is_power_of_two() {
                let blit = command
                    .blitCommandEncoder()
                    .ok_or_else(|| Error::Device("blit encoder".into()))?;
                blit.generateMipmapsForTexture(&t.raw);
                blit.endEncoding();
            } else {
                self.area_mips(t, &command)?;
            }
            let s = self.submit(Some(command));
            self.textures.get_mut(&id).unwrap().last = s;
            Ok(s)
        })
    }
    fn resize_target(&mut self, id: TextureId, width: u32, height: u32) -> Result<TextureId> {
        let desc = self.texture(id)?.desc;
        if desc.format != Format::Rgba || desc.mipmapped {
            return Err(Error::Invalid("resize requires a target".into()));
        }
        if desc.width == width && desc.height == height {
            return Ok(id);
        }
        let next = self.create_target(width, height)?;
        self.retire_texture(id)?;
        Ok(next)
    }
    fn copy_region(
        &mut self,
        source: TextureId,
        origin: [u32; 2],
        target: TextureId,
        dst: [u32; 2],
        size: [u32; 2],
    ) -> Result<Submission> {
        autoreleasepool(|_| {
            let from = self.texture(source)?;
            let to = self.texture(target)?;
            check_region(from.desc, origin, size)?;
            check_region(to.desc, dst, size)?;
            if source == target || from.desc.format != to.desc.format {
                return Err(Error::Invalid(
                    "copy requires distinct textures of one format".into(),
                ));
            }
            let command = self.commands()?;
            let blit = command
                .blitCommandEncoder()
                .ok_or_else(|| Error::Device("blit encoder".into()))?;
            // SAFETY: checked bounds, distinct textures, same uncompressed format.
            unsafe {
                blit.copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toTexture_destinationSlice_destinationLevel_destinationOrigin(
                &from.raw, 0, 0, point(origin), region(origin, size).size, &to.raw, 0, 0, point(dst),
            );
            }
            blit.endEncoding();
            let s = self.submit(Some(command));
            for id in [source, target] {
                self.textures.get_mut(&id).unwrap().last = s;
            }
            Ok(s)
        })
    }
    fn render(
        &mut self,
        frame: &Frame,
        target: TextureId,
        clear: Option<[f64; 4]>,
    ) -> Result<Submission> {
        autoreleasepool(|_| {
            self.completed()?;
            let desc = self.texture(target)?.desc;
            #[cfg(target_os = "macos")]
            let _trace = super::apple::Trace::new(c"scene encode");
            if desc.format != Format::Rgba || desc.mipmapped {
                return Err(Error::Invalid(
                    "render target must be single-level RGBA".into(),
                ));
            }
            if frame
                .instances
                .iter()
                .any(|i| i.kind_brush & 65535 == Kind::Shadow as u32 && i.flags & flags::CLIP1 == 0)
            {
                return Err(Error::Invalid(
                    "a shadow requires its border box in clip1".into(),
                ));
            }
            for batch in &frame.batches {
                if batch.instances.start > batch.instances.end
                    || batch.instances.end as usize > frame.instances.len()
                {
                    return Err(Error::Invalid("batch instance range".into()));
                }
                let s = batch.scissor;
                if s.width != 0 && s.height != 0 {
                    check_region(desc, [s.x, s.y], [s.width, s.height])?;
                }
                if texture_ids(batch.bindings).contains(&target) {
                    return Err(Error::Invalid(
                        "a render target cannot sample itself".into(),
                    ));
                }
                self.bindings(batch.bindings)?;
            }
            let slot = self.next_buffer;
            self.next_buffer = (slot + 1) % 3;
            self.wait(self.buffers[slot].last.get())?;
            let bytes: &[u8] = bytemuck::cast_slice(&frame.instances);
            if bytes.len() > self.buffers[slot].raw.length() {
                self.buffers[slot] = self.create_buffer(bytes.len().next_power_of_two() as u64)?;
            }
            // SAFETY: this slot has completed; shared storage has capacity for
            // every instance. The encoder retains it until command completion.
            unsafe {
                std::ptr::copy_nonoverlapping(
                    bytes.as_ptr(),
                    self.buffers[slot].raw.contents().as_ptr().cast::<u8>(),
                    bytes.len(),
                );
            }
            let command = self.commands()?;
            let pass = MTLRenderPassDescriptor::new();
            let attachment = unsafe { pass.colorAttachments().objectAtIndexedSubscript(0) };
            attachment.setTexture(Some(&self.texture(target)?.raw));
            attachment.setStoreAction(MTLStoreAction::Store);
            attachment.setLoadAction(if clear.is_some() {
                MTLLoadAction::Clear
            } else {
                MTLLoadAction::Load
            });
            if let Some([red, green, blue, alpha]) = clear {
                attachment.setClearColor(MTLClearColor {
                    red,
                    green,
                    blue,
                    alpha,
                });
            }
            let encoder = command
                .renderCommandEncoderWithDescriptor(&pass)
                .ok_or_else(|| Error::Device("render encoder".into()))?;
            let viewport = [desc.width as f32, desc.height as f32, 0., 0.];
            encoder.setViewport(MTLViewport {
                originX: 0.,
                originY: 0.,
                width: desc.width as f64,
                height: desc.height as f64,
                znear: 0.,
                zfar: 1.,
            });
            // SAFETY: Metal copies the 16 viewport bytes; attribute buffer 1
            // holds the checked instance ranges, with a 192-byte descriptor stride.
            unsafe {
                encoder.setVertexBytes_length_atIndex(NonNull::from(&viewport).cast(), 16, 0);
                encoder.setVertexBuffer_offset_atIndex(Some(&self.buffers[slot].raw), 0, 1);
            }
            for batch in &frame.batches {
                let s = batch.scissor;
                if s.width == 0 || s.height == 0 || batch.instances.is_empty() {
                    continue;
                }
                encoder.setRenderPipelineState(if batch.replace {
                    &self.replace_pipeline
                } else {
                    &self.pipeline
                });
                encoder.setScissorRect(MTLScissorRect {
                    x: s.x as usize,
                    y: s.y as usize,
                    width: s.width as usize,
                    height: s.height as usize,
                });
                // SAFETY: the slots are naga's build.rs bindings; sampled
                // textures are live, format-checked and separate from the target.
                unsafe {
                    for (index, texture) in [0, 1, 2, 3, 4, 6, 7]
                        .into_iter()
                        .zip(self.bindings(batch.bindings)?)
                    {
                        encoder.setFragmentTexture_atIndex(Some(texture), index);
                    }
                    encoder.setFragmentSamplerState_atIndex(
                        Some(if batch.bindings.repeat {
                            &self.repeat_sampler
                        } else {
                            &self.sampler
                        }),
                        0,
                    );
                    encoder.drawPrimitives_vertexStart_vertexCount_instanceCount_baseInstance(
                        MTLPrimitiveType::TriangleStrip,
                        0,
                        4,
                        batch.instances.len(),
                        batch.instances.start as usize,
                    );
                }
            }
            encoder.endEncoding();
            let s = self.submit(Some(command));
            self.buffers[slot].last.set(s);
            self.textures.get_mut(&target).unwrap().last = s;
            for batch in &frame.batches {
                for id in texture_ids(batch.bindings) {
                    if id.0 != 0 {
                        self.textures.get_mut(&id).unwrap().last = s;
                    }
                }
            }
            Ok(s)
        })
    }
    fn readback(&mut self, id: TextureId) -> Result<Vec<u8>> {
        self.readback_level(id, 0)
    }
    fn wait(&mut self, s: Submission) -> Result<()> {
        #[cfg(target_os = "macos")]
        let _trace = super::apple::Trace::new(c"main resource wait");
        if s > self.submitted {
            return Err(Error::Invalid("unknown submission".into()));
        }
        if s <= self.completed()? {
            return Ok(());
        }
        if s > self.done {
            // A CPU upload has no command buffer. Waiting on the latest earlier
            // command still covers it because uploads waited on their resource.
            if let Some(command) = self
                .pending
                .range(..=s.0)
                .rev()
                .find_map(|(_, c)| c.as_ref())
            {
                command.waitUntilCompleted();
            }
        }
        self.completed().map(|_| ())
    }
    fn completed(&mut self) -> Result<Submission> {
        #[cfg(target_os = "macos")]
        if let Some(window) = &mut self.window {
            window.collect()?;
        }
        if let Some(e) = &self.error {
            return Err(Error::Device(e.clone()));
        }
        while let Some((&id, command)) = self.pending.first_key_value() {
            if let Some(command) = command {
                match command.status() {
                    MTLCommandBufferStatus::Completed => {
                        #[cfg(target_os = "macos")]
                        super::apple::trace_interval(
                            c"gpu command",
                            command.GPUStartTime(),
                            command.GPUEndTime(),
                            id as f64,
                        );
                    }
                    MTLCommandBufferStatus::Error => {
                        let message = command
                            .error()
                            .map(|e| e.to_string())
                            .unwrap_or_else(|| "Metal command failed".into());
                        self.error = Some(message.clone());
                        return Err(Error::Device(message));
                    }
                    _ => break,
                }
            }
            self.done = Submission(id);
            self.pending.pop_first();
        }
        self.retired_textures.retain(|t| t.last > self.done);
        self.retired_buffers.retain(|b| b.last.get() > self.done);
        Ok(self.done)
    }
    fn retire_texture(&mut self, id: TextureId) -> Result<()> {
        let t = self
            .textures
            .remove(&id)
            .ok_or_else(|| Error::Invalid("retired texture".into()))?;
        #[cfg(target_os = "macos")]
        super::apple::trace_mark(
            c"texture retired bytes",
            MTLResource::allocatedSize(&*t.raw) as f64,
        );
        self.retired_textures.push(t);
        self.completed().map(|_| ())
    }
    fn trim(&mut self) -> Result<()> {
        self.completed()?;
        for i in 0..3 {
            if self.buffers[i].raw.length() > 192 {
                let new = self.create_buffer(192)?;
                self.retired_buffers
                    .push(std::mem::replace(&mut self.buffers[i], new));
            }
        }
        Ok(())
    }
}

fn buffer(device: &ProtocolObject<dyn MTLDevice>, owner: &Rc<()>, bytes: usize) -> Result<Buffer> {
    let raw = device
        .newBufferWithLength_options(bytes, MTLResourceOptions::StorageModeShared)
        .ok_or_else(|| Error::Device("instance buffer allocation".into()))?;
    Ok(Buffer {
        raw,
        last: Rc::new(Cell::new(Submission(0))),
        owner: owner.clone(),
    })
}
fn texture(device: &ProtocolObject<dyn MTLDevice>, desc: TextureDesc) -> Result<Texture> {
    let descriptor = MTLTextureDescriptor::new();
    descriptor.setTextureType(MTLTextureType::Type2D);
    descriptor.setPixelFormat(if desc.format == Format::Alpha {
        MTLPixelFormat::R8Unorm
    } else {
        MTLPixelFormat::RGBA8Unorm
    });
    // SAFETY: the caller checked nonzero dimensions within Metal's limit;
    // levels() gives exactly the mip chain for these dimensions.
    unsafe {
        descriptor.setWidth(desc.width as usize);
        descriptor.setHeight(desc.height as usize);
        descriptor.setMipmapLevelCount(desc.levels() as usize);
    }
    descriptor.setStorageMode(MTLStorageMode::Shared);
    descriptor.setUsage(MTLTextureUsage::ShaderRead | MTLTextureUsage::RenderTarget);
    let raw = device
        .newTextureWithDescriptor(&descriptor)
        .ok_or_else(|| Error::Device("texture allocation".into()))?;
    Ok(Texture {
        raw,
        desc,
        last: Submission(0),
    })
}
fn upload(texture: &Texture, origin: [u32; 2], size: [u32; 2], data: &[u8]) {
    // SAFETY: callers check the region and byte count, and wait before mutation.
    unsafe {
        texture.raw.replaceRegion_mipmapLevel_withBytes_bytesPerRow(
            region(origin, size),
            0,
            NonNull::new(data.as_ptr().cast_mut()).unwrap().cast(),
            size[0] as usize * texture.desc.format.channels() as usize,
        );
    }
}
fn point([x, y]: [u32; 2]) -> MTLOrigin {
    MTLOrigin {
        x: x as usize,
        y: y as usize,
        z: 0,
    }
}
fn region(origin: [u32; 2], [width, height]: [u32; 2]) -> MTLRegion {
    MTLRegion {
        origin: point(origin),
        size: MTLSize {
            width: width as usize,
            height: height as usize,
            depth: 1,
        },
    }
}
