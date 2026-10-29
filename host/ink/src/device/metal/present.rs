//! A compose target is sampled into a drawable; no window frame is read back.
//! @ref LLP 1149 D2.7, D7, D11.
use super::*;
use crate::device::apple::{Layer, RenderLayer};
use crate::present::Thread;
use objc2::Message;
use objc2_quartz_core::CAMetalDrawable;
use std::sync::{atomic::Ordering, mpsc::Receiver};

pub(super) struct Command(pub Retained<ProtocolObject<dyn MTLCommandBuffer>>);
// SAFETY: encoded on its owner thread, then committed once on the render thread.
// The owner only queries completion after that handoff.
unsafe impl Send for Command {}
struct Drawable(Retained<ProtocolObject<dyn CAMetalDrawable>>);
// SAFETY: ownership moves from the render thread to the main-thread transaction.
unsafe impl Send for Drawable {}
impl Drawable {
    fn present(self) {
        self.0.present();
    }
}

impl Command {
    pub fn commit(self) {
        self.0.commit();
    }
}

struct Work {
    renderer: std::sync::Arc<std::sync::Mutex<Renderer>>,
    texture: Retained<ProtocolObject<dyn MTLTexture>>,
    command: Retained<ProtocolObject<dyn MTLCommandBuffer>>,
    benchmark: (bool, f32),
}
// SAFETY: the retained texture is read on the ordered GPU queue, never mutated
// by the worker; encoding and committing this command belong only to the worker.
unsafe impl Send for Work {}
impl Work {
    fn run(self, present: bool) -> Result<()> {
        if !present {
            self.command.commit();
            return Ok(());
        }
        crate::device::apple::benchmark_present(self.benchmark);
        let result = self
            .renderer
            .lock()
            .unwrap()
            .present(&self.texture, &self.command);
        self.command.waitUntilCompleted();
        if self.command.status() == MTLCommandBufferStatus::Error {
            return Err(Error::DeviceLost(format!(
                "window command failed: {:?}",
                self.command.error()
            )));
        }
        result
    }
}

pub(super) struct Window {
    pub thread: Thread,
    pending: std::collections::VecDeque<Receiver<Result<()>>>,
    uploads: std::sync::Arc<std::sync::Mutex<Uploads>>,
    renderer: std::sync::Arc<std::sync::Mutex<Renderer>>,
    _layer: Layer,
}

struct Renderer {
    layer: RenderLayer,
    pipeline: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    sampler: Retained<ProtocolObject<dyn MTLSamplerState>>,
    white: Retained<ProtocolObject<dyn MTLTexture>>,
    ring: Ring,
}
// SAFETY: the renderer is used by one worker, under its mutex. Metal objects
// support cross-thread ownership; the NSView stays in the main-thread Layer.
unsafe impl Send for Renderer {}

impl Window {
    pub fn collect(&mut self) -> Result<()> {
        while let Some(pending) = self.pending.front() {
            match pending.try_recv() {
                Ok(result) => {
                    result?;
                    self.pending.pop_front();
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(_) => return Err(Error::DeviceLost("Metal presentation stopped".into())),
            }
        }
        Ok(())
    }
    pub fn wait(&mut self) -> Result<()> {
        while let Some(pending) = self.pending.pop_front() {
            pending
                .recv()
                .map_err(|_| Error::DeviceLost("Metal presentation stopped".into()))??;
        }
        self.thread.drain()
    }

    pub fn upload(
        &mut self,
        texture: &Texture,
        command: Retained<ProtocolObject<dyn MTLCommandBuffer>>,
        origin: [u32; 2],
        size: [u32; 2],
        bytes: &[u8],
    ) -> Result<()> {
        let work = Upload {
            ring: self.uploads.clone(),
            command,
            texture: texture.raw.clone(),
            row: size[0] as usize * texture.desc.format.channels() as usize,
            origin,
            size,
            bytes: bytes.to_vec(),
        };
        self.pending
            .push_back(self.thread.send(move || work.run())?);
        Ok(())
    }
}

type UploadSlot = (
    Retained<ProtocolObject<dyn MTLBuffer>>,
    Retained<ProtocolObject<dyn MTLCommandBuffer>>,
);
struct Uploads {
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    slots: [Option<UploadSlot>; 2],
    next: usize,
}
// SAFETY: only the render worker uses the ring; each slot is written after
// its previous GPU copy completes. Metal retains the resources it encodes.
unsafe impl Send for Uploads {}

struct Upload {
    ring: std::sync::Arc<std::sync::Mutex<Uploads>>,
    command: Retained<ProtocolObject<dyn MTLCommandBuffer>>,
    texture: Retained<ProtocolObject<dyn MTLTexture>>,
    row: usize,
    origin: [u32; 2],
    size: [u32; 2],
    bytes: Vec<u8>,
}
// SAFETY: this job owns its pixels; encoding belongs to the render worker.
unsafe impl Send for Upload {}
impl Upload {
    fn run(self) -> Result<()> {
        let result = self.encode();
        // Even a failed upload must complete its submission's retirement fence.
        self.command.commit();
        result
    }
    fn encode(&self) -> Result<()> {
        let mut ring = self.ring.lock().unwrap();
        let slot = ring.next;
        let old = ring.slots[slot].take();
        if let Some((_, command)) = &old {
            command.waitUntilCompleted();
            if command.status() == MTLCommandBufferStatus::Error {
                return Err(Error::DeviceLost("image upload failed".into()));
            }
        }
        let stride = self.row.div_ceil(256) * 256;
        let length = stride * self.size[1] as usize;
        let buffer = match old {
            Some((buffer, _)) if buffer.length() >= length => buffer,
            _ => ring
                .device
                .newBufferWithLength_options(length, MTLResourceOptions::StorageModeShared)
                .ok_or_else(|| Error::Device("upload staging allocation".into()))?,
        };
        let encoder = self
            .command
            .blitCommandEncoder()
            .ok_or_else(|| Error::Device("upload blit encoder".into()))?;
        // SAFETY: the ring slot completed; every row is within its checked
        // allocation. The texture region and source length were checked by Device.
        // https://developer.apple.com/documentation/metal/mtlblitcommandencoder
        unsafe {
            for (y, row) in self.bytes.chunks_exact(self.row).enumerate() {
                std::ptr::copy_nonoverlapping(
                    row.as_ptr(),
                    buffer.contents().as_ptr().cast::<u8>().add(y * stride),
                    row.len(),
                );
            }
            encoder.copyFromBuffer_sourceOffset_sourceBytesPerRow_sourceBytesPerImage_sourceSize_toTexture_destinationSlice_destinationLevel_destinationOrigin_options(
                &buffer, 0, stride, 0,
                MTLSize { width: self.size[0] as usize, height: self.size[1] as usize, depth: 1 },
                &self.texture, 0, 0, point(self.origin), MTLBlitOption::None,
            );
        }
        encoder.endEncoding();
        ring.slots[slot] = Some((buffer, self.command.clone()));
        ring.next = (slot + 1) % 2;
        Ok(())
    }
}

impl Metal {
    pub fn attach_layer(&mut self, layer: Layer) -> Result<()> {
        self.wait(self.submitted)?;
        let started = Instant::now();
        let pipelines = setup::pipelines(&self.device, MTLPixelFormat::BGRA8Unorm)?;
        self.info.shaders_ms += started.elapsed().as_secs_f64() * 1000.;
        self.info.cached &= pipelines.cached;
        pipelines.save();
        let render_layer = layer.render_layer();
        render_layer.raw.setDevice(Some(&self.device));
        self.window = Some(Window {
            thread: Thread::new()?,
            pending: Default::default(),
            uploads: std::sync::Arc::new(std::sync::Mutex::new(Uploads {
                device: self.device.clone(),
                slots: Default::default(),
                next: 0,
            })),
            renderer: std::sync::Arc::new(std::sync::Mutex::new(Renderer {
                layer: render_layer,
                pipeline: pipelines.replace,
                sampler: self.sampler.clone(),
                white: self.white_alpha.raw.clone(),
                ring: Ring::default(),
            })),
            _layer: layer,
        });
        Ok(())
    }

    pub fn has_layer(&self) -> bool {
        self.window.is_some()
    }

    pub fn present(&mut self, source: TextureId) -> Result<f64> {
        autoreleasepool(|_| {
            let start = Instant::now();
            let texture = self.texture(source)?.raw.clone();
            let command = self.commands()?;
            let window = self
                .window
                .as_mut()
                .ok_or(Error::Unsupported("no Metal layer"))?;
            window.collect()?;
            let work = Work {
                renderer: window.renderer.clone(),
                texture,
                command: command.clone(),
                benchmark: crate::device::apple::benchmark_scene(),
            };
            window
                .pending
                .push_back(window.thread.frame(move |show| work.run(show))?);
            self.submitted.0 += 1;
            self.pending.insert(self.submitted.0, Some(command));
            self.textures.get_mut(&source).unwrap().last = self.submitted;
            Ok(start.elapsed().as_secs_f64() * 1000.)
        })
    }
}

impl Renderer {
    fn present(
        &mut self,
        source: &ProtocolObject<dyn MTLTexture>,
        command: &ProtocolObject<dyn MTLCommandBuffer>,
    ) -> Result<()> {
        if let Err(error) = self.ring.acquire() {
            command.commit();
            return Err(error);
        }
        let transaction = self.layer.live_resize.load(Ordering::Acquire);
        self.layer.raw.setPresentsWithTransaction(transaction);
        let Some(drawable) = self.layer.raw.nextDrawable() else {
            // A hidden or closing window may have no drawable. Commit the empty
            // buffer so resource retirement and a later screenshot can complete.
            command.commit();
            self.layer.retry();
            return Ok(());
        };
        self.layer.track(ProtocolObject::from_ref(&*drawable));
        if let Err(error) = encode(
            command,
            source,
            &drawable.texture(),
            &self.pipeline,
            &self.sampler,
            &self.white,
        ) {
            command.commit();
            return Err(error);
        }
        if transaction {
            // Apple's transaction sequence requires present() after scheduling,
            // on the view's thread; Layer::update drains this at the next wake.
            // https://developer.apple.com/documentation/quartzcore/cametallayer/presentswithtransaction
            command.commit();
            command.waitUntilScheduled();
            let drawable = Drawable(drawable);
            self.layer.transaction(move || drawable.present());
        } else {
            command.presentDrawable(ProtocolObject::from_ref(&*drawable));
            command.commit();
        }
        self.ring.retire(command);
        Ok(())
    }
}

fn encode(
    command: &ProtocolObject<dyn MTLCommandBuffer>,
    source: &ProtocolObject<dyn MTLTexture>,
    target: &ProtocolObject<dyn MTLTexture>,
    pipeline: &ProtocolObject<dyn MTLRenderPipelineState>,
    sampler: &ProtocolObject<dyn MTLSamplerState>,
    white: &ProtocolObject<dyn MTLTexture>,
) -> Result<()> {
    let pass = MTLRenderPassDescriptor::new();
    let attachment = unsafe { pass.colorAttachments().objectAtIndexedSubscript(0) };
    attachment.setTexture(Some(target));
    attachment.setLoadAction(MTLLoadAction::DontCare);
    attachment.setStoreAction(MTLStoreAction::Store);
    let encoder = command
        .renderCommandEncoderWithDescriptor(&pass)
        .ok_or_else(|| Error::Device("drawable encoder".into()))?;
    let width = target.width() as f32;
    let height = target.height() as f32;
    let viewport = [width, height, 0., 0.];
    let instance = Instance::new(Kind::Layer, crate::Brush::Image, [0., 0., width, height]);
    encoder.setRenderPipelineState(pipeline);
    encoder.setViewport(MTLViewport {
        originX: 0.,
        originY: 0.,
        width: width as f64,
        height: height as f64,
        znear: 0.,
        zfar: 1.,
    });
    // SAFETY: Metal copies these constants and retains both textures. The
    // full-size image samples encoded sRGB into an unorm BGRA drawable.
    unsafe {
        encoder.setVertexBytes_length_atIndex(NonNull::from(&viewport).cast(), 16, 0);
        encoder.setVertexBytes_length_atIndex(
            NonNull::from(&instance).cast(),
            size_of::<Instance>(),
            1,
        );
        for index in [0, 1, 2, 3, 4, 6, 7] {
            encoder.setFragmentTexture_atIndex(
                Some(if index == 0 || index == 2 || index == 6 {
                    white
                } else {
                    source
                }),
                index,
            );
        }
        encoder.setFragmentSamplerState_atIndex(Some(sampler), 0);
        encoder.drawPrimitives_vertexStart_vertexCount_instanceCount(
            MTLPrimitiveType::TriangleStrip,
            0,
            4,
            1,
        );
    }
    encoder.endEncoding();
    Ok(())
}

#[derive(Default)]
struct Ring {
    commands: [Option<Retained<ProtocolObject<dyn MTLCommandBuffer>>>; 3],
    next: usize,
}
impl Ring {
    fn acquire(&mut self) -> Result<()> {
        // Reuse follows completion, not the number of acquired drawables.
        if let Some(previous) = self.commands[self.next].take() {
            previous.waitUntilCompleted();
            if previous.status() == MTLCommandBufferStatus::Error {
                return Err(Error::DeviceLost(format!(
                    "drawable command failed: {:?}",
                    previous.error()
                )));
            }
        }
        Ok(())
    }
    fn retire(&mut self, command: &ProtocolObject<dyn MTLCommandBuffer>) {
        self.commands[self.next] = Some(command.retain());
        self.next = (self.next + 1) % self.commands.len();
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        let _ = self.wait();
        self._layer.update();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queued_uploads_reuse_two_staging_buffers_after_completion() {
        let mut device = Metal::new().unwrap();
        let ring = std::sync::Arc::new(std::sync::Mutex::new(Uploads {
            device: device.device.clone(),
            slots: Default::default(),
            next: 0,
        }));
        let worker = Thread::new().unwrap();
        let texture = device.create_texture(TextureDesc::rgba(7, 5)).unwrap();
        let mut receipts = Vec::new();
        let mut commands = Vec::new();
        for row in 0..5 {
            let command = device.commands().unwrap();
            commands.push(command.clone());
            let job = Upload {
                ring: ring.clone(),
                command,
                texture: device.texture(texture).unwrap().raw.clone(),
                row: 28,
                origin: [0, row],
                size: [7, 1],
                bytes: [row as u8 * 30, 40, 80, 255].repeat(7),
            };
            receipts.push(worker.send(move || job.run()).unwrap());
        }
        for receipt in receipts {
            receipt.recv().unwrap().unwrap();
        }
        for command in commands {
            command.waitUntilCompleted();
            assert_eq!(command.status(), MTLCommandBufferStatus::Completed);
        }
        let ring = ring.lock().unwrap();
        assert_eq!(ring.next, 1);
        assert!(ring
            .slots
            .iter()
            .all(|s| s.as_ref().unwrap().0.length() == 256));
        let pixels = device.readback(texture).unwrap();
        for (row, pixels) in pixels.chunks_exact(28).enumerate() {
            assert_eq!(pixels, [row as u8 * 30, 40, 80, 255].repeat(7));
        }
    }

    #[test]
    fn drawable_ring_presents_encoded_srgb_to_bgra_and_reuses_completed_slots() {
        let mut device = Metal::new().unwrap();
        let pipelines = setup::pipelines(&device.device, MTLPixelFormat::BGRA8Unorm).unwrap();
        let descriptor = MTLTextureDescriptor::new();
        descriptor.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
        descriptor.setStorageMode(MTLStorageMode::Shared);
        descriptor.setUsage(MTLTextureUsage::RenderTarget);
        unsafe {
            descriptor.setWidth(4);
            descriptor.setHeight(4);
        }
        let targets: [_; 3] =
            std::array::from_fn(|_| device.device.newTextureWithDescriptor(&descriptor).unwrap());
        let mut ring = Ring::default();
        for frame in 0..9u8 {
            ring.acquire().unwrap();
            let slot = ring.next;
            assert_eq!(slot, frame as usize % 3);
            let source = device.create_texture(TextureDesc::rgba(4, 4)).unwrap();
            let pixels: Vec<u8> = (0..16u8)
                .flat_map(|i| [20 + frame, i * 8, 120, 160])
                .collect();
            device
                .upload_texture(source, [0, 0], [4, 4], &pixels)
                .unwrap();
            let command = device.commands().unwrap();
            encode(
                &command,
                &device.texture(source).unwrap().raw,
                &targets[slot],
                &pipelines.replace,
                &device.sampler,
                &device.white_alpha.raw,
            )
            .unwrap();
            command.commit();
            ring.retire(&command);
            command.waitUntilCompleted();
            assert_eq!(command.status(), MTLCommandBufferStatus::Completed);
            let mut output = [0u8; 64];
            unsafe {
                targets[slot].getBytes_bytesPerRow_fromRegion_mipmapLevel(
                    NonNull::from(&mut output).cast(),
                    16,
                    region([0, 0], [4, 4]),
                    0,
                );
            }
            for (actual, rgba) in output.chunks_exact(4).zip(pixels.chunks_exact(4)) {
                assert_eq!(
                    actual,
                    [rgba[2], rgba[1], rgba[0], rgba[3]],
                    "frame {frame}"
                );
            }
            assert_eq!(device.readback(source).unwrap(), pixels);
        }
        assert!(ring.commands.iter().all(Option::is_some));
        assert_eq!(ring.next, 0);
        ring.acquire().unwrap();
        assert!(ring.commands[0].is_none());
    }
}
