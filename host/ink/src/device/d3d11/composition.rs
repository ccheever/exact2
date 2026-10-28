//! A premultiplied composition surface over the system backdrop.
//! @ref LLP 1149 D7; LLP 1115.004 D4.3.
use super::*;
use std::any::Any;
use windows::Win32::{
    Foundation::{CloseHandle, HANDLE, HWND, WAIT_OBJECT_0},
    Graphics::{DirectComposition::*, Dwm::*},
    System::Threading::WaitForSingleObject,
    UI::Controls::MARGINS,
};

/// A live HWND and its owner, retained until DirectComposition releases it.
#[derive(Clone)]
pub struct Window {
    pub(super) hwnd: HWND,
    _owner: Rc<dyn Any>,
}
impl Window {
    /// # Safety
    /// `hwnd` must belong to `owner`, and remain live until its last Rc is dropped.
    /// Creation and all later presentation must run on the window's thread.
    pub unsafe fn new(hwnd: isize, owner: Rc<dyn Any>) -> Self {
        Self {
            hwnd: HWND(hwnd as *mut _),
            _owner: owner,
        }
    }
}

pub(super) struct Composition {
    swap: IDXGISwapChain2,
    device: IDCompositionDevice,
    target: IDCompositionTarget,
    visual: IDCompositionVisual,
    latency: HANDLE,
    size: [u32; 2],
    _window: Window,
    pub creation_ms: f64,
}
impl Drop for Composition {
    fn drop(&mut self) {
        unsafe {
            let _ = self.visual.SetContent(None);
            let _ = self.target.SetRoot(None);
            let _ = self.device.Commit();
            let _ = CloseHandle(self.latency);
        }
    }
}
impl Composition {
    pub(super) fn new(device: &ID3D11Device, window: Window, size: [u32; 2]) -> Result<Self> {
        let start = Instant::now();
        // SAFETY: Window owns the live HWND; every COM reference and the waitable
        // handle is retained through presentation and released before Window.
        unsafe {
            let dxgi: IDXGIDevice = device.cast()?;
            let factory: IDXGIFactory2 = dxgi.GetAdapter()?.GetParent()?;
            let desc = DXGI_SWAP_CHAIN_DESC1 {
                Width: size[0],
                Height: size[1],
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: 2,
                Scaling: DXGI_SCALING_STRETCH,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
                AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
                Flags: DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT.0 as u32,
                ..Default::default()
            };
            let swap: IDXGISwapChain2 = factory
                .CreateSwapChainForComposition(device, &desc, None)?
                .cast()?;
            swap.SetMaximumFrameLatency(1)?;
            let device: IDCompositionDevice = DCompositionCreateDevice(&dxgi)?;
            let target = device.CreateTargetForHwnd(window.hwnd, true)?;
            let visual = device.CreateVisual()?;
            visual.SetContent(&swap)?;
            target.SetRoot(&visual)?;
            device.Commit()?;
            // The first statistics query establishes the interval; it may report
            // DXGI_ERROR_FRAME_STATISTICS_DISJOINT before any frame is presented.
            let mut statistics = DXGI_FRAME_STATISTICS_MEDIA::default();
            let _ = swap
                .cast::<IDXGISwapChainMedia>()?
                .GetFrameStatisticsMedia(&mut statistics);
            let latency = swap.GetFrameLatencyWaitableObject();
            if latency.is_invalid() {
                return Err(Error::Device("DXGI frame latency handle".into()));
            }
            let backdrop = DWMSBT_MAINWINDOW;
            DwmSetWindowAttribute(
                window.hwnd,
                DWMWA_SYSTEMBACKDROP_TYPE,
                (&backdrop as *const DWM_SYSTEMBACKDROP_TYPE).cast(),
                size_of::<DWM_SYSTEMBACKDROP_TYPE>() as u32,
            )?;
            DwmExtendFrameIntoClientArea(
                window.hwnd,
                &MARGINS {
                    cxLeftWidth: -1,
                    cxRightWidth: -1,
                    cyTopHeight: -1,
                    cyBottomHeight: -1,
                },
            )?;
            Ok(Self {
                swap,
                device,
                target,
                visual,
                latency,
                size,
                _window: window,
                creation_ms: start.elapsed().as_secs_f64() * 1000.,
            })
        }
    }
    fn resize(&mut self, size: [u32; 2]) -> Result<()> {
        if self.size == size {
            return Ok(());
        }
        unsafe {
            self.swap.ResizeBuffers(
                2,
                size[0],
                size[1],
                DXGI_FORMAT_B8G8R8A8_UNORM,
                DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT,
            )?;
        }
        self.size = size;
        Ok(())
    }
}

impl D3d11 {
    pub fn attach_window(&mut self, window: Window, size: [u32; 2]) -> Result<()> {
        if size.contains(&0) {
            return Err(Error::Invalid("composition size".into()));
        }
        let composition = Composition::new(&self.device, window.clone(), size)?;
        if let Ok(initial) = std::env::var("EXACT_INK_RICHEDIT") {
            let editor = super::rich_edit::RichEdit::new(self, window, [320, 48])?;
            editor.type_text(&initial)?;
            self.editor = Some(editor);
        }
        eprintln!(
            "exact-ink: composition swap chain {:.3} ms",
            composition.creation_ms
        );
        self.composition = Some(composition);
        Ok(())
    }
    pub fn type_text(&self, text: &str) -> Result<bool> {
        if let Some(editor) = &self.editor {
            editor.type_text(text)?;
            return Ok(true);
        }
        Ok(false)
    }
    pub fn has_window(&self) -> bool {
        self.composition.is_some()
    }
    pub fn present(&mut self, id: TextureId) -> Result<f64> {
        let start = Instant::now();
        let desc = self.texture(id)?.desc;
        let size = [desc.width, desc.height];
        if let Some(editor) = self.editor.take() {
            let result = (|| {
                let texture = editor.draw(self)?;
                let mut encoder = crate::Encoder::new(size[0], size[1]);
                let y = size[1].saturating_sub(80) as f32;
                encoder
                    .push(
                        Instance::rounded([12., y, 344., 64.], [8.; 4], [0.9, 0.9, 0.9, 0.92]),
                        Bindings::default(),
                    )
                    .map_err(|_| Error::Invalid("editor background".into()))?;
                let mut image = Instance::new(
                    Kind::RoundedRect,
                    crate::Brush::Image,
                    [24., y + 8., 320., 48.],
                );
                image.uv = [0., 0., 1., 1.];
                encoder
                    .push(
                        image,
                        Bindings {
                            image: texture,
                            ..Default::default()
                        },
                    )
                    .map_err(|_| Error::Invalid("editor image".into()))?;
                self.render(&encoder.finish(), id, None)
            })();
            self.editor = Some(editor);
            result?;
        }
        let composition = self
            .composition
            .as_mut()
            .ok_or(Error::Unsupported("no composition window"))?;
        composition.resize(size)?;
        let swap = composition.swap.clone();
        unsafe {
            if WaitForSingleObject(composition.latency, 1000) != WAIT_OBJECT_0 {
                return Err(Error::Device("DXGI frame latency wait failed".into()));
            }
            let back: ID3D11Texture2D = swap.GetBuffer(0)?;
            let mut view = None;
            self.device
                .CreateRenderTargetView(&back, None, Some(&mut view))?;
            let mut encoder = crate::Encoder::new(size[0], size[1]);
            let mut quad = Instance::new(
                Kind::RoundedRect,
                crate::Brush::Image,
                [0., 0., size[0] as f32, size[1] as f32],
            );
            quad.uv = [0., 0., 1., 1.];
            encoder
                .push(
                    quad,
                    Bindings {
                        image: id,
                        ..Default::default()
                    },
                )
                .map_err(|_| Error::Invalid("composition quad".into()))?;
            let mut frame = encoder.finish();
            frame.batches[0].replace = true;
            self.draw(&frame, &view.unwrap(), size, Some([0.; 4]), None)?;
            swap.Present(1, DXGI_PRESENT(0)).ok()?;
        }
        let s = self.submit()?;
        self.textures.get_mut(&id).unwrap().last = s;
        Ok(start.elapsed().as_secs_f64() * 1000.)
    }
}

impl D3d11 {
    /// The displayed frame's DXGI timestamp relative to process creation. The
    /// measurement caller first waits with `compositor_sync_process_ms`.
    pub fn presented_process_ms(&self) -> Result<f64> {
        use windows::Win32::{
            Foundation::FILETIME,
            System::{
                Performance::{QueryPerformanceCounter, QueryPerformanceFrequency},
                SystemInformation::GetSystemTimePreciseAsFileTime,
                Threading::{GetCurrentProcess, GetProcessTimes},
            },
        };
        let c = self
            .composition
            .as_ref()
            .ok_or(Error::Unsupported("no composition window"))?;
        unsafe {
            let media: IDXGISwapChainMedia = c.swap.cast()?;
            let mut stats = DXGI_FRAME_STATISTICS_MEDIA::default();
            media.GetFrameStatisticsMedia(&mut stats)?;
            let expected = c.swap.GetLastPresentCount()?;
            if stats.PresentCount < expected || stats.SyncQPCTime == 0 {
                return Err(Error::Device(format!(
                    "DXGI has no timestamp for present {expected}: displayed {}, QPC {}",
                    stats.PresentCount, stats.SyncQPCTime,
                )));
            }
            let rate = c.device.GetFrameStatistics()?.currentCompositionRate;
            if rate.Numerator == 0 {
                return Err(Error::Unsupported("composition refresh rate"));
            }
            let (mut now_qpc, mut frequency) = (0, 0);
            QueryPerformanceCounter(&mut now_qpc)?;
            QueryPerformanceFrequency(&mut frequency)?;
            let now = GetSystemTimePreciseAsFileTime();
            let (mut created, mut exited, mut kernel, mut user) = (
                FILETIME::default(),
                FILETIME::default(),
                FILETIME::default(),
                FILETIME::default(),
            );
            GetProcessTimes(
                GetCurrentProcess(),
                &mut created,
                &mut exited,
                &mut kernel,
                &mut user,
            )?;
            let ticks =
                |t: FILETIME| (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime);
            let displayed_qpc = stats.SyncQPCTime as f64
                + (f64::from(stats.PresentRefreshCount) - f64::from(stats.SyncRefreshCount))
                    * frequency as f64
                    * f64::from(rate.Denominator)
                    / f64::from(rate.Numerator);
            Ok((ticks(now) - ticks(created)) as f64 / 10_000.
                - (now_qpc as f64 - displayed_qpc) * 1000. / frequency as f64)
        }
    }
}
