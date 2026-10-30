//! Archive-only boot; a miss compiles away from the presentation queue.
//! @ref LLP 1149 D6.3; LLP 1149.001 Stage 0b.
use super::*;
use crate::device::apple::{Layer, RenderLayer};
use crate::present::Thread;
use objc2_quartz_core::CAMetalDrawable;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};

struct Context {
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    info: Info,
}
// SAFETY: device and queue are thread-safe retained handles. Only the compiler
// worker uses its clone to create immutable pipeline states.
unsafe impl Send for Context {}

struct Prepared {
    rgba: setup::Pipelines,
    bgra: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    shaders_ms: f64,
}
// SAFETY: ownership transfers after compilation; no descriptors are shared.
unsafe impl Send for Prepared {}

/// The CPU remains installed until a whole ink frame can replace it.
/// The compiler owns no view and never runs on the presentation worker.
pub struct Startup {
    context: Context,
    ready: Option<Prepared>,
    pending: Option<mpsc::Receiver<Result<Prepared>>>,
    layer: Option<Layer>,
    thread: Rc<Thread>,
    shown: Arc<AtomicBool>,
    wake: Arc<dyn Fn() + Send + Sync>,
    frames: std::collections::VecDeque<mpsc::Receiver<Result<()>>>,
}

impl Startup {
    pub fn start(wake: Arc<dyn Fn() + Send + Sync>) -> Result<Self> {
        Self::open(None, wake)
    }

    /// An explicit archive location, including an absent archive, follows the
    /// same accelerator-only rule as the platform cache.
    pub fn with_archive(
        path: std::path::PathBuf,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<Self> {
        Self::open(Some(path), wake)
    }

    fn open(path: Option<std::path::PathBuf>, wake: Arc<dyn Fn() + Send + Sync>) -> Result<Self> {
        let _trace = Trace::new(c"boot Metal constructor");
        let started = Instant::now();
        let device = {
            let _trace = Trace::new(c"boot MTLCreateSystemDefaultDevice");
            MTLCreateSystemDefaultDevice()
                .ok_or_else(|| Error::NoAdapter("MTLCreateSystemDefaultDevice".into()))?
        };
        let queue = {
            let _trace = Trace::new(c"boot command queue");
            device
                .newCommandQueue()
                .ok_or_else(|| Error::Device("command queue allocation".into()))?
        };
        let context = Context {
            info: Info {
                adapter: device.name().to_string(),
                api: "direct Metal".into(),
                device_ms: started.elapsed().as_secs_f64() * 1000.,
                shaders_ms: 0.,
                cached: false,
            },
            device,
            queue,
        };
        let path = path.or_else(|| setup::cache_path(&context.device));
        if std::env::var_os("G1_OUTPUT").is_some() {
            if let Some(path) = &path {
                eprintln!("exact-ink: Metal archive {}", path.display());
            }
        }
        let build = |strict| prepare(&context.device, path.clone(), strict);
        let (ready, pending) = match build(true) {
            Ok(prepared) => (Some(prepared), None),
            Err(_) => {
                let worker = Context {
                    device: context.device.clone(),
                    queue: context.queue.clone(),
                    info: context.info.clone(),
                };
                let (sender, receiver) = mpsc::sync_channel(1);
                let compiler_wake = wake.clone();
                std::thread::Builder::new()
                    .name("ink-pipelines".into())
                    .spawn(move || {
                        let result = autoreleasepool(|_| prepare(&worker.device, path, false));
                        let _ = sender.send(result);
                        compiler_wake();
                    })
                    .map_err(|e| Error::Device(e.to_string()))?;
                (None, Some(receiver))
            }
        };
        Ok(Self {
            context,
            ready,
            pending,
            layer: None,
            thread: Rc::new(Thread::new()?),
            shown: Arc::new(AtomicBool::new(false)),
            wake,
            frames: Default::default(),
        })
    }

    pub fn attach(&mut self, layer: Layer) {
        let _trace = Trace::new(c"boot CPU attach layer");
        let raw = layer.render_layer();
        raw.raw.setDevice(Some(&self.context.device));
        // A fallback uses a blit, which framebuffer-only drawables forbid.
        raw.raw.setFramebufferOnly(false);
        self.layer = Some(layer);
    }

    /// A cache miss must submit a CPU frame even if compilation wins the race.
    pub fn poll(&mut self, cpu_submitted: bool) -> Result<Option<super::super::Selected>> {
        self.retire_frames()?;
        if self.layer.is_some() && !self.shown.load(Ordering::Acquire) {
            return Ok(None);
        }
        if self.ready.is_none() && cpu_submitted {
            if let Some(receiver) = &self.pending {
                match receiver.try_recv() {
                    Ok(result) => {
                        self.pending = None;
                        self.ready = Some(result?);
                    }
                    Err(mpsc::TryRecvError::Empty) => return Ok(None),
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.pending = None;
                        return Err(Error::Device("pipeline worker stopped".into()));
                    }
                }
            }
        }
        let Some(prepared) = self.ready.take() else {
            return Ok(None);
        };
        let mut info = self.context.info.clone();
        info.shaders_ms = prepared.shaders_ms;
        let mut metal = Metal::from_parts(
            self.context.device.clone(),
            self.context.queue.clone(),
            prepared.rgba,
            info,
        )?;
        if let Some(layer) = &self.layer {
            // Restore the ink layer only after every queued CPU blit has completed.
            let restore = layer.render_layer();
            self.thread.send(move || restore.framebuffer_only())?;
            metal.attach_prepared_layer(layer.clone(), prepared.bgra, self.thread.clone())?;
        } else {
            // Headless preparation keeps the presentation state for a later attach.
            metal.prepared_present = Some((prepared.bgra, self.thread.clone()));
        }
        Ok(Some(super::super::Selected::Metal(Box::new(metal))))
    }

    pub fn pending(&self) -> bool {
        self.pending.is_some() || self.ready.is_some()
    }

    fn retire_frames(&mut self) -> Result<()> {
        while let Some(frame) = self.frames.front() {
            match frame.try_recv() {
                Ok(result) => {
                    self.frames.pop_front();
                    result?;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    return Err(Error::DeviceLost("CPU present worker stopped".into()))
                }
            }
        }
        Ok(())
    }

    pub fn present_cpu(&mut self, pixels: Arc<tiny_skia::Pixmap>) -> Result<()> {
        self.retire_frames()?;
        if self.frames.len() >= 2 {
            return Ok(());
        }
        let layer = self
            .layer
            .as_ref()
            .ok_or(Error::Unsupported("CPU startup has no Metal layer"))?;
        let work = CpuFrame {
            device: self.context.device.clone(),
            queue: self.context.queue.clone(),
            layer: layer.render_layer(),
            pixels,
            benchmark: crate::device::apple::benchmark_scene(),
            shown: self.shown.clone(),
            wake: self.wake.clone(),
        };
        // Noncoalesced: the first CPU frame cannot be cancelled by a fast compiler.
        self.frames.push_back(self.thread.send(move || work.run())?);
        Ok(())
    }
}

fn prepare(
    device: &ProtocolObject<dyn MTLDevice>,
    path: Option<std::path::PathBuf>,
    strict: bool,
) -> Result<Prepared> {
    let _trace = Trace::new(if strict {
        c"boot archive pipelines"
    } else {
        c"worker compile pipelines"
    });
    let started = Instant::now();
    let builder = setup::Builder::open(device, path, strict)?;
    let rgba = builder.rgba(device)?;
    let bgra = builder.make(device, MTLPixelFormat::BGRA8Unorm, false)?;
    let shaders_ms = started.elapsed().as_secs_f64() * 1000.;
    if !strict {
        rgba.save();
    }
    Ok(Prepared {
        rgba,
        bgra,
        shaders_ms,
    })
}

struct CpuFrame {
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    layer: RenderLayer,
    pixels: Arc<tiny_skia::Pixmap>,
    benchmark: (bool, f32),
    shown: Arc<AtomicBool>,
    wake: Arc<dyn Fn() + Send + Sync>,
}
// SAFETY: the worker owns encoding and its immutable pixels. No NSView crosses.
unsafe impl Send for CpuFrame {}
impl CpuFrame {
    fn run(self) -> Result<()> {
        let _trace = Trace::new(c"worker CPU present");
        crate::device::apple::benchmark_painter(false);
        crate::device::apple::benchmark_present(self.benchmark);
        let Some(drawable) = self.layer.raw.nextDrawable() else {
            self.layer.retry();
            return Ok(());
        };
        self.layer.track(ProtocolObject::from_ref(&*drawable));
        let shown = self.shown.clone();
        let wake = self.wake.clone();
        let handler =
            block2::RcBlock::new(move |drawable: NonNull<ProtocolObject<dyn MTLDrawable>>| {
                // SAFETY: Metal supplies the retained drawable to its completion block.
                if unsafe { drawable.as_ref().presentedTime() } > 0.
                    && !shown.swap(true, Ordering::AcqRel)
                {
                    wake();
                }
            });
        // SAFETY: Metal copies the block and keeps its captures through presentation.
        unsafe {
            drawable.addPresentedHandler(block2::RcBlock::as_ptr(&handler));
        }
        let target = drawable.texture();
        if target.width() != self.pixels.width() as usize
            || target.height() != self.pixels.height() as usize
        {
            self.layer.retry();
            return Ok(());
        }
        let source = cpu_texture(&self.device, &self.pixels)?;
        let command = self
            .queue
            .commandBuffer()
            .ok_or_else(|| Error::Device("CPU command allocation".into()))?;
        blit(&command, &source, &target)?;
        command.presentDrawable(ProtocolObject::from_ref(&*drawable));
        command.commit();
        command.waitUntilCompleted();
        if command.status() == MTLCommandBufferStatus::Error {
            return Err(Error::DeviceLost(format!(
                "CPU present failed: {:?}",
                command.error()
            )));
        }
        Ok(())
    }
}

fn cpu_texture(
    device: &ProtocolObject<dyn MTLDevice>,
    pixels: &tiny_skia::Pixmap,
) -> Result<Retained<ProtocolObject<dyn MTLTexture>>> {
    let desc = MTLTextureDescriptor::new();
    desc.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
    // SAFETY: Pixmap has nonzero dimensions and owns their checked allocation.
    unsafe {
        desc.setWidth(pixels.width() as usize);
        desc.setHeight(pixels.height() as usize);
    }
    desc.setStorageMode(MTLStorageMode::Shared);
    let texture = device
        .newTextureWithDescriptor(&desc)
        .ok_or_else(|| Error::Device("CPU texture allocation".into()))?;
    let mut bytes = pixels.data().to_vec();
    for pixel in bytes.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    // SAFETY: tightly packed BGRA bytes cover this new, unsubmitted texture.
    unsafe {
        texture.replaceRegion_mipmapLevel_withBytes_bytesPerRow(
            MTLRegion {
                origin: point([0, 0]),
                size: MTLSize {
                    width: pixels.width() as usize,
                    height: pixels.height() as usize,
                    depth: 1,
                },
            },
            0,
            NonNull::new(bytes.as_mut_ptr().cast()).unwrap(),
            pixels.width() as usize * 4,
        );
    }
    Ok(texture)
}

fn blit(
    command: &ProtocolObject<dyn MTLCommandBuffer>,
    source: &ProtocolObject<dyn MTLTexture>,
    target: &ProtocolObject<dyn MTLTexture>,
) -> Result<()> {
    let encoder = command
        .blitCommandEncoder()
        .ok_or_else(|| Error::Device("CPU blit encoder".into()))?;
    // SAFETY: both BGRA textures have the same checked extent.
    unsafe {
        encoder.copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toTexture_destinationSlice_destinationLevel_destinationOrigin(
        source, 0, 0, point([0, 0]), MTLSize { width: source.width(), height: source.height(), depth: 1 }, target, 0, 0, point([0, 0]),
    );
    }
    encoder.endEncoding();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_pixmap_blits_through_bgra_without_changing_a_channel() {
        let Some(device) = MTLCreateSystemDefaultDevice() else {
            assert!(
                std::env::var_os("EXACT_REQUIRED").is_none(),
                "Metal required"
            );
            return;
        };
        let mut pixels = tiny_skia::Pixmap::new(7, 5).unwrap();
        for (i, pixel) in pixels.data_mut().chunks_exact_mut(4).enumerate() {
            pixel.copy_from_slice(&[i as u8, 130, 250 - i as u8, 255]);
        }
        let source = cpu_texture(&device, &pixels).unwrap();
        let target = cpu_texture(&device, &tiny_skia::Pixmap::new(7, 5).unwrap()).unwrap();
        let queue = device.newCommandQueue().unwrap();
        let command = queue.commandBuffer().unwrap();
        blit(&command, &source, &target).unwrap();
        command.commit();
        command.waitUntilCompleted();
        assert_eq!(command.status(), MTLCommandBufferStatus::Completed);
        let mut bytes = vec![0u8; pixels.data().len()];
        // SAFETY: the GPU copy completed and the full shared target fits bytes.
        unsafe {
            target.getBytes_bytesPerRow_fromRegion_mipmapLevel(
                NonNull::new(bytes.as_mut_ptr().cast()).unwrap(),
                7 * 4,
                MTLRegion {
                    origin: point([0, 0]),
                    size: MTLSize {
                        width: 7,
                        height: 5,
                        depth: 1,
                    },
                },
                0,
            );
        }
        for pixel in bytes.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
        assert_eq!(bytes, pixels.data());
    }
}
