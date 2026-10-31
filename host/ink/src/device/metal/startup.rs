//! Archive-only boot; a miss compiles away from the presentation queue.
//! @ref LLP 1149 D6.3; LLP 1149.001 Stage 0b.
use super::*;
use crate::device::apple::{Layer, RenderLayer};
use crate::present::Thread;
use objc2_quartz_core::CAMetalDrawable;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Mutex,
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
    completed: Arc<AtomicBool>,
    cpu_submitted: bool,
    compiler: Option<Compiler>,
    layer: Option<Layer>,
    thread: Rc<Thread>,
    shown: Arc<AtomicBool>,
    wake: Arc<dyn Fn() + Send + Sync>,
    frames: std::collections::VecDeque<mpsc::Receiver<Result<()>>>,
    textures: Arc<Mutex<CpuTextures>>,
}

impl Startup {
    pub fn waker(&self) -> Arc<dyn Fn() + Send + Sync> {
        self.wake.clone()
    }

    pub fn start(wake: Arc<dyn Fn() + Send + Sync>) -> Result<Self> {
        Self::open(None, wake, None)
    }

    /// An explicit archive location, including an absent archive, follows the
    /// same accelerator-only rule as the platform cache.
    pub fn with_archive(
        path: std::path::PathBuf,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<Self> {
        Self::open(Some(path), wake, None)
    }

    /// Hold an archive miss before compilation so tests can choose either race.
    #[doc(hidden)]
    #[cfg(debug_assertions)]
    pub fn with_archive_gated(
        path: std::path::PathBuf,
        wake: Arc<dyn Fn() + Send + Sync>,
        gate: mpsc::Receiver<()>,
    ) -> Result<Self> {
        Self::open(Some(path), wake, Some(gate))
    }

    fn open(
        path: Option<std::path::PathBuf>,
        wake: Arc<dyn Fn() + Send + Sync>,
        gate: Option<mpsc::Receiver<()>>,
    ) -> Result<Self> {
        crate::counters::initialize();
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
        let completed = Arc::new(AtomicBool::new(false));
        let (ready, pending, compiler) = match build(true) {
            Ok(prepared) => (Some(prepared), None, None),
            Err(_) => {
                let worker = Context {
                    device: context.device.clone(),
                    queue: context.queue.clone(),
                    info: context.info.clone(),
                };
                let (sender, receiver) = mpsc::sync_channel(1);
                let done = completed.clone();
                let compiler = Compiler::spawn(wake.clone(), move |cancelled| {
                    let result = wait_gate(gate, cancelled)
                        .and_then(|()| autoreleasepool(|_| prepare(&worker.device, path, false)));
                    let _ = sender.send(result);
                    done.store(true, Ordering::Release);
                })?;
                (None, Some(receiver), Some(compiler))
            }
        };
        Ok(Self {
            context,
            ready,
            pending,
            completed,
            cpu_submitted: false,
            compiler,
            layer: None,
            thread: Rc::new(Thread::new()?),
            shown: Arc::new(AtomicBool::new(false)),
            wake,
            frames: Default::default(),
            textures: Default::default(),
        })
    }

    pub fn attach(&mut self, layer: Layer) {
        let _trace = Trace::new(c"boot CPU attach layer");
        crate::counters::presentation_support(true, true);
        let raw = layer.render_layer();
        raw.raw.setDevice(Some(&self.context.device));
        // A fallback uses a blit, which framebuffer-only drawables forbid.
        raw.raw.setFramebufferOnly(false);
        self.layer = Some(layer);
    }

    /// Use a finished compile immediately, or wait for an already queued CPU frame.
    pub fn poll(&mut self) -> Result<Option<super::super::Selected>> {
        self.retire_frames()?;
        if self.ready.is_none() {
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
        if self.waiting_for_cpu() {
            return Ok(None);
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

    /// A non-consuming readiness check; compiling alone never creates damage.
    pub fn ready(&self) -> bool {
        !self.waiting_for_cpu()
            && (self.ready.is_some()
                || self.pending.is_some() && self.completed.load(Ordering::Acquire))
    }

    fn waiting_for_cpu(&self) -> bool {
        self.cpu_submitted && self.layer.is_some() && !self.shown.load(Ordering::Acquire)
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

    pub fn present_cpu(
        &mut self,
        pixels: Arc<tiny_skia::Pixmap>,
        transaction: bool,
    ) -> Result<bool> {
        self.retire_frames()?;
        if self.frames.len() >= 2 {
            return Ok(false);
        }
        let layer = self
            .layer
            .as_ref()
            .ok_or(Error::Unsupported("CPU startup has no Metal layer"))?;
        // Reserve before enqueueing: ink uploads may fill the command pool
        // during the handoff, and the worker must be able to commit their predecessor.
        let command = self
            .context
            .queue
            .commandBuffer()
            .ok_or_else(|| Error::Device("CPU command allocation".into()))?;
        let work = CpuFrame {
            device: self.context.device.clone(),
            command,
            layer: layer.render_layer(),
            pixels,
            benchmark: crate::device::apple::benchmark_scene(),
            shown: self.shown.clone(),
            wake: self.wake.clone(),
            transaction: transaction || layer.live_resize(),
            textures: self.textures.clone(),
        };
        // Noncoalesced: the first CPU frame cannot be cancelled by a fast compiler.
        let wake = self.wake.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        if work.transaction {
            let completion = super::present::Command(work.command.clone());
            if let Some(drawable) = self.thread.call(move || work.run())?? {
                drawable.present();
            }
            self.thread.send(move || {
                let _ = sender.send(completion.completed());
                wake();
            })?;
        } else {
            self.thread.send(move || {
                let _ = sender.send(work.run().map(|_| ()));
                wake();
            })?;
        }
        self.frames.push_back(receiver);
        self.cpu_submitted = true;
        Ok(true)
    }
}

impl Drop for Startup {
    fn drop(&mut self) {
        // Joining includes serialization; a hung driver cannot hold window teardown.
        self.compiler.take();
    }
}

type Wake = Arc<dyn Fn() + Send + Sync>;
struct Compiler {
    join: Option<std::thread::JoinHandle<()>>,
    wake: Arc<Mutex<Option<Wake>>>,
    cancelled: Arc<AtomicBool>,
}
impl Compiler {
    fn spawn(
        wake: Arc<dyn Fn() + Send + Sync>,
        work: impl FnOnce(&AtomicBool) + Send + 'static,
    ) -> Result<Self> {
        let wake = Arc::new(Mutex::new(Some(wake)));
        let notify = wake.clone();
        let cancelled = Arc::new(AtomicBool::new(false));
        let stop = cancelled.clone();
        let join = std::thread::Builder::new()
            .name("ink-pipelines".into())
            .spawn(move || {
                work(&stop);
                if let Some(wake) = notify.lock().unwrap().as_ref() {
                    wake();
                }
            })
            .map_err(|e| Error::Device(e.to_string()))?;
        Ok(Self {
            join: Some(join),
            wake,
            cancelled,
        })
    }
}
impl Drop for Compiler {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        self.wake.lock().unwrap().take();
        let Some(join) = self.join.take() else {
            return;
        };
        let deadline = Instant::now() + std::time::Duration::from_secs(2);
        // std's JoinHandle has no timed join or completion notification.
        while !join.is_finished() && Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        if join.is_finished() {
            let _ = join.join();
        } else {
            eprintln!("exact-ink: pipeline worker did not stop within 2 s; wake disconnected");
        }
    }
}

fn wait_gate(gate: Option<mpsc::Receiver<()>>, cancelled: &AtomicBool) -> Result<()> {
    let Some(gate) = gate else {
        return Ok(());
    };
    while !cancelled.load(Ordering::Acquire) {
        match gate.recv_timeout(std::time::Duration::from_millis(50)) {
            Ok(()) => return Ok(()),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    Err(Error::Device("pipeline worker cancelled".into()))
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
    command: Retained<ProtocolObject<dyn MTLCommandBuffer>>,
    layer: RenderLayer,
    pixels: Arc<tiny_skia::Pixmap>,
    benchmark: (bool, f32),
    shown: Arc<AtomicBool>,
    wake: Arc<dyn Fn() + Send + Sync>,
    transaction: bool,
    textures: Arc<Mutex<CpuTextures>>,
}
// SAFETY: the worker owns encoding and its immutable pixels. No NSView crosses.
unsafe impl Send for CpuFrame {}
impl CpuFrame {
    fn run(self) -> Result<Option<super::present::Drawable>> {
        let _trace = Trace::new(c"worker CPU present");
        let result = self.encode();
        // An unavailable drawable still releases the reserved command-pool slot.
        if !self.transaction {
            if let Ok(Some(drawable)) = &result {
                self.command
                    .presentDrawable(ProtocolObject::from_ref(&*drawable.0));
            }
        }
        if crate::counters::enabled() && matches!(&result, Ok(Some(_))) {
            crate::counters::submitted();
        }
        self.command.commit();
        if self.transaction {
            self.command.waitUntilScheduled();
        } else {
            self.command.waitUntilCompleted();
        }
        if self.command.status() == MTLCommandBufferStatus::Error {
            return Err(Error::DeviceLost(format!(
                "CPU present failed: {:?}",
                self.command.error()
            )));
        }
        result.map(|drawable| if self.transaction { drawable } else { None })
    }

    fn encode(&self) -> Result<Option<super::present::Drawable>> {
        self.layer.raw.setPresentsWithTransaction(self.transaction);
        crate::device::apple::benchmark_painter(false);
        crate::device::apple::benchmark_present(self.benchmark);
        let Some(drawable) = self.layer.raw.nextDrawable() else {
            self.layer.retry();
            return Ok(None);
        };
        self.layer
            .track(ProtocolObject::from_ref(&*drawable), false);
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
            return Ok(None);
        }
        let source = self.textures.lock().unwrap().upload(
            &self.device,
            &self.pixels,
            self.command.clone(),
        )?;
        let command = &self.command;
        blit(command, &source, &target)?;
        Ok(Some(super::present::Drawable(drawable)))
    }
}

#[derive(Default)]
struct CpuTextures {
    slots: [Option<CpuTexture>; 2],
    next: usize,
}
struct CpuTexture {
    texture: Retained<ProtocolObject<dyn MTLTexture>>,
    bytes: Vec<u8>,
    command: Option<Retained<ProtocolObject<dyn MTLCommandBuffer>>>,
}
// SAFETY: only the presentation worker uploads; the mutex serializes access.
unsafe impl Send for CpuTextures {}
impl CpuTextures {
    fn upload(
        &mut self,
        device: &ProtocolObject<dyn MTLDevice>,
        pixels: &tiny_skia::Pixmap,
        command: Retained<ProtocolObject<dyn MTLCommandBuffer>>,
    ) -> Result<Retained<ProtocolObject<dyn MTLTexture>>> {
        let slot = &mut self.slots[self.next];
        if let Some(previous) = slot.as_mut().and_then(|s| s.command.take()) {
            super::present::Command(previous).completed()?;
        }
        if slot.as_ref().is_none_or(|s| {
            s.texture.width() != pixels.width() as usize
                || s.texture.height() != pixels.height() as usize
        }) {
            *slot = Some(CpuTexture {
                texture: cpu_texture(device, pixels.width(), pixels.height())?,
                bytes: vec![0; pixels.data().len()],
                command: None,
            });
        }
        let slot = slot.as_mut().unwrap();
        for (src, dst) in pixels
            .data()
            .chunks_exact(4)
            .zip(slot.bytes.chunks_exact_mut(4))
        {
            dst.copy_from_slice(&[src[2], src[1], src[0], src[3]]);
        }
        // SAFETY: the previous GPU reader completed; the buffer covers the texture.
        unsafe {
            slot.texture
                .replaceRegion_mipmapLevel_withBytes_bytesPerRow(
                    MTLRegion {
                        origin: point([0, 0]),
                        size: MTLSize {
                            width: pixels.width() as usize,
                            height: pixels.height() as usize,
                            depth: 1,
                        },
                    },
                    0,
                    NonNull::new(slot.bytes.as_mut_ptr().cast()).unwrap(),
                    pixels.width() as usize * 4,
                );
        }
        slot.command = Some(command);
        self.next = (self.next + 1) % 2;
        Ok(slot.texture.clone())
    }
}

fn cpu_texture(
    device: &ProtocolObject<dyn MTLDevice>,
    width: u32,
    height: u32,
) -> Result<Retained<ProtocolObject<dyn MTLTexture>>> {
    let desc = MTLTextureDescriptor::new();
    desc.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
    // SAFETY: Pixmap has nonzero dimensions and owns their checked allocation.
    unsafe {
        desc.setWidth(width as usize);
        desc.setHeight(height as usize);
    }
    desc.setStorageMode(MTLStorageMode::Shared);
    device
        .newTextureWithDescriptor(&desc)
        .ok_or_else(|| Error::Device("CPU texture allocation".into()))
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
    fn dropping_startup_joins_its_compiler_and_disconnects_the_wake() {
        let finished = Arc::new(AtomicBool::new(false));
        let done = finished.clone();
        let (_release, gate) = mpsc::channel();
        let compiler = Compiler::spawn(Arc::new(|| panic!("wake after shutdown")), move |stop| {
            assert!(wait_gate(Some(gate), stop).is_err());
            done.store(true, Ordering::Release);
        })
        .unwrap();
        drop(compiler);
        assert!(
            finished.load(Ordering::Acquire),
            "the worker must exit before Drop returns"
        );
    }

    #[test]
    fn a_full_cpu_queue_refuses_the_frame_until_a_slot_completes() {
        let mut startup = Startup::start(Arc::new(|| {})).unwrap();
        let (first, a) = mpsc::channel();
        let (second, b) = mpsc::channel();
        startup.frames.extend([a, b]);
        assert!(!startup
            .present_cpu(Arc::new(tiny_skia::Pixmap::new(1, 1).unwrap()), false)
            .unwrap());
        assert_eq!(startup.frames.len(), 2);
        first.send(Ok(())).unwrap();
        startup.retire_frames().unwrap();
        assert_eq!(startup.frames.len(), 1);
        second
            .send(Err(Error::DeviceLost(
                "injected CPU command failure".into(),
            )))
            .unwrap();
        assert!(matches!(startup.retire_frames(), Err(Error::DeviceLost(_))));
        assert!(startup.frames.is_empty());
    }

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
        let mut ring = CpuTextures::default();
        let target = cpu_texture(&device, 7, 5).unwrap();
        let queue = device.newCommandQueue().unwrap();
        let mut handles = Vec::new();
        for value in 0..8 {
            let mut frame = pixels.clone();
            frame.data_mut().fill(value);
            let command = queue.commandBuffer().unwrap();
            let source = ring.upload(&device, &frame, command.clone()).unwrap();
            handles.push(Retained::as_ptr(&source));
            blit(&command, &source, &target).unwrap();
            command.commit();
        }
        assert_ne!(handles[0], handles[1]);
        assert!(handles
            .iter()
            .enumerate()
            .all(|(i, p)| *p == handles[i % 2]));
        let command = queue.commandBuffer().unwrap();
        let source = ring.upload(&device, &pixels, command.clone()).unwrap();
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
