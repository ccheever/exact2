//! A premultiplied composition surface over the system backdrop.
//! @ref LLP 1149 D7; LLP 1115.004 D4.3.
use super::*;
use std::any::Any;
use std::cell::RefCell;
use windows::Win32::{
    Foundation::{
        CloseHandle, DXGI_STATUS_OCCLUDED, HANDLE, HWND, WAIT_EVENT, WAIT_OBJECT_0, WAIT_TIMEOUT,
    },
    Graphics::{DirectComposition::*, Dwm::*},
    System::Threading::WaitForSingleObject,
    UI::Controls::MARGINS,
};

/// A live HWND and its owner, retained until DirectComposition releases it.
#[derive(Clone)]
pub struct Window {
    pub(super) hwnd: HWND,
    visual: Rc<RefCell<std::rc::Weak<Visual>>>,
    background: Rc<Cell<[u8; 4]>>,
    _owner: Rc<dyn Any>,
}
impl Window {
    /// Retain the window for every swap-chain use on its owning thread.
    pub fn from_owner<T: raw_window_handle::HasWindowHandle + 'static>(
        owner: Rc<T>,
    ) -> Result<Self> {
        let raw_window_handle::RawWindowHandle::Win32(handle) = owner
            .window_handle()
            .map_err(|e| Error::Invalid(e.to_string()))?
            .as_raw()
        else {
            return Err(Error::Unsupported("not a Windows window"));
        };
        Ok(Self {
            hwnd: HWND(handle.hwnd.get() as *mut _),
            visual: Default::default(),
            background: Rc::new(Cell::new([255; 4])),
            _owner: owner,
        })
    }

    /// An owned native fixture for the renderer's explicit measurements.
    pub fn fixture(size: [u32; 2]) -> Result<Self> {
        use windows::{
            core::w,
            Win32::{System::Com::*, UI::WindowsAndMessaging::*},
        };
        struct Owner(HWND);
        impl Drop for Owner {
            fn drop(&mut self) {
                unsafe {
                    let _ = DestroyWindow(self.0);
                    CoUninitialize();
                }
            }
        }
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
            match CreateWindowExW(
                WS_EX_NOREDIRECTIONBITMAP,
                w!("STATIC"),
                w!("Ink measurement"),
                WS_POPUP | WS_VISIBLE,
                0,
                0,
                size[0] as i32,
                size[1] as i32,
                None,
                None,
                None,
                None,
            ) {
                Ok(hwnd) => Ok(Self {
                    hwnd,
                    visual: Default::default(),
                    background: Rc::new(Cell::new([255; 4])),
                    _owner: Rc::new(Owner(hwnd)),
                }),
                Err(e) => {
                    CoUninitialize();
                    Err(e.into())
                }
            }
        }
    }

    pub fn dispatch_pending(&self) {
        use windows::Win32::UI::WindowsAndMessaging::*;
        unsafe {
            let mut message = MSG::default();
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }

    /// The host's resolved window colour, including its scheme and contrast palette.
    /// Used only when DWM refuses Mica. @ref LLP 1095 D1; LLP 1149 S2.7.
    pub fn set_background(&self, color: [u8; 4]) {
        self.background.set(opaque_background(color));
    }

    pub fn background(&self) -> [u8; 4] {
        self.background.get()
    }

    pub fn hwnd(&self) -> usize {
        self.hwnd.0 as usize
    }
    pub fn hinstance(&self) -> usize {
        unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(
                self.hwnd,
                windows::Win32::UI::WindowsAndMessaging::GWLP_HINSTANCE,
            ) as usize
        }
    }
}

struct Visual {
    device: IDCompositionDevice,
    target: IDCompositionTarget,
    visual: IDCompositionVisual,
    content: std::cell::Cell<usize>,
    backdrop: Cell<Backdrop>,
}

#[derive(Clone, Copy)]
struct Backdrop {
    available: bool,
    retries: u8,
}
impl Backdrop {
    fn new(available: bool) -> Self {
        Self {
            available,
            retries: 0,
        }
    }

    fn after_present(&mut self, set: impl FnOnce() -> bool) -> bool {
        if self.available || self.retries == 3 {
            return false;
        }
        self.retries += 1;
        self.available = set();
        eprintln!(
            "exact-ink: Mica retry {}/3 after present: {}",
            self.retries,
            if self.available {
                "restored"
            } else {
                "refused"
            }
        );
        // Restoring Mica needs one transparent frame to replace the opaque fallback.
        self.available || self.retries < 3
    }
}
impl Drop for Visual {
    fn drop(&mut self) {
        unsafe {
            let _ = self.visual.SetContent(None);
            let _ = self.target.SetRoot(None);
            let _ = self.device.Commit();
        }
    }
}
pub(super) struct Composition {
    swap: IDXGISwapChain2,
    scene: Rc<Visual>,
    latency: HANDLE,
    acquired: bool,
    size: [u32; 2],
    _window: Window,
    pub creation_ms: f64,
    swap_ms: f64,
    visual_ms: f64,
    active: bool,
    reported_display: bool,
    occluded_checks: u32,
}
impl Drop for Composition {
    fn drop(&mut self) {
        unsafe {
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
            let swap_ms = start.elapsed().as_secs_f64() * 1000.;
            let visual_started = Instant::now();
            let existing = window.visual.borrow().upgrade();
            let scene = match existing {
                Some(scene) => {
                    // A device reset can clear the HWND's material while the
                    // composition visual survives the device handoff.
                    let available = if fault("mica-recovery") {
                        eprintln!("exact-ink: injected Mica refusal during device recovery");
                        false
                    } else {
                        set_backdrop(window.hwnd)
                    };
                    scene.backdrop.set(Backdrop::new(available));
                    scene
                }
                None => {
                    // No rendering device: the visual must not retain WARP after
                    // its swap chain retires. DCompositionCreateDevice2 permits
                    // this for swap-chain content. @ref LLP 1149 D11.
                    let device: IDCompositionDevice =
                        DCompositionCreateDevice2(None::<&windows::core::IUnknown>)?;
                    let target = device.CreateTargetForHwnd(window.hwnd, true)?;
                    let visual = device.CreateVisual()?;
                    target.SetRoot(&visual)?;
                    let scene = Rc::new(Visual {
                        device,
                        target,
                        visual,
                        content: Default::default(),
                        backdrop: Cell::new(Backdrop::new(set_backdrop(window.hwnd))),
                    });
                    *window.visual.borrow_mut() = Rc::downgrade(&scene);
                    scene
                }
            };
            let visual_ms = visual_started.elapsed().as_secs_f64() * 1000.;
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
            Ok(Self {
                swap,
                scene,
                latency,
                acquired: false,
                size,
                _window: window,
                creation_ms: start.elapsed().as_secs_f64() * 1000.,
                swap_ms,
                visual_ms,
                active: false,
                reported_display: false,
                occluded_checks: 0,
            })
        }
    }

    fn refresh_period(&self) -> Duration {
        let rate = unsafe { self.scene.device.GetFrameStatistics() }
            .ok()
            .map(|s| s.currentCompositionRate);
        rate.and_then(|r| refresh_period(r.Numerator, r.Denominator))
            .unwrap_or(Duration::from_secs_f64(1. / 60.))
    }

    fn acquire(&mut self, timeout_ms: u32) -> Result<bool> {
        unsafe {
            if !window_drawable(self._window.hwnd) {
                self.occluded_checks = self.occluded_checks.saturating_add(1);
                return Ok(false);
            }
            // A visual without its first picture can TEST as occluded. The
            // window's visibility still guards its first wait and present.
            let activating =
                !self.active || self.scene.content.get() != self.swap.as_raw() as usize;
            if !activating && !presented(self.swap.Present(0, DXGI_PRESENT_TEST))? {
                self.occluded_checks = self.occluded_checks.saturating_add(1);
                return Ok(false);
            }
            self.occluded_checks = 0;
            if !self.acquired {
                self.acquired = latency_ready(WaitForSingleObject(self.latency, timeout_ms))?;
            }
        }
        Ok(self.acquired)
    }

    fn activate(&mut self) -> Result<()> {
        use windows::Win32::UI::WindowsAndMessaging::*;
        if self.active && self.scene.content.get() == self.swap.as_raw() as usize {
            return Ok(());
        }
        // Change content only after the new swap chain has its first picture.
        // The old chain stays visible until this commit. @ref LLP 1149 D7.
        unsafe {
            let hwnd = self._window.hwnd;
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
            if style & WS_EX_LAYERED.0 != 0 || style & WS_EX_NOREDIRECTIONBITMAP.0 == 0 {
                return Err(Error::Invalid(
                    "ink requires WS_EX_NOREDIRECTIONBITMAP at window creation and no WS_EX_LAYERED".into(),
                ));
            }
            self.scene.visual.SetContent(&self.swap)?;
            self.scene.device.Commit()?;
            self.scene.content.set(self.swap.as_raw() as usize);
        }
        self.active = true;
        Ok(())
    }
    fn resize(&mut self, size: [u32; 2]) -> Result<()> {
        if self.size == size {
            return Ok(());
        }
        self.acquired = false;
        unsafe {
            self.swap.ResizeBuffers(
                2,
                size[0],
                size[1],
                DXGI_FORMAT_B8G8R8A8_UNORM,
                DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT,
            )?;
            let old = std::mem::replace(&mut self.latency, HANDLE::default());
            let _ = CloseHandle(old);
            self.swap.SetMaximumFrameLatency(1)?;
            self.latency = self.swap.GetFrameLatencyWaitableObject();
            if self.latency.is_invalid() {
                return Err(Error::Device(
                    "DXGI frame latency handle after resize".into(),
                ));
            }
        }
        self.size = size;
        Ok(())
    }
}

pub(super) struct Retirement {
    present: Option<u32>,
    failed_checks: u8,
}
impl Retirement {
    fn displayed(&mut self, count: Option<u32>) -> bool {
        match (self.present, count) {
            (Some(expected), Some(actual)) => actual >= expected,
            _ => {
                self.failed_checks = self.failed_checks.saturating_add(1);
                self.failed_checks >= 3
            }
        }
    }
}

impl D3d11 {
    /// Keep the old device until its commands and the replacement present finish.
    /// A swap-chain handoff never waits for either on the frame path.
    pub fn retire_after_present(&mut self, mut previous: D3d11) {
        // The completion query follows Present; no later WARP frame will flush it.
        unsafe { previous.context.Flush() };
        let present = self
            .composition
            .as_ref()
            .and_then(|c| unsafe { c.swap.GetLastPresentCount().ok() })
            .map(|count| count.max(1));
        // A later replacement covers every older chain. Keep each completion
        // query, but do not depend on statistics from an already-lost device.
        self.retiring_devices
            .extend(
                previous
                    .retiring_devices
                    .drain(..)
                    .map(|(device, mut retirement)| {
                        retirement.present = present;
                        (device, retirement)
                    }),
            );
        self.retiring_devices.push((
            Box::new(previous),
            Retirement {
                present,
                failed_checks: 0,
            },
        ));
        self.collect_presented_devices();
    }

    pub fn collect_presented_devices(&mut self) -> usize {
        self.retiring_devices.retain_mut(|(previous, present)| {
            if std::env::var_os("EXACT_AGENT_WINDOW_READY").is_some()
                && previous
                    .composition
                    .as_ref()
                    .is_some_and(|c| !c.reported_display)
            {
                if let Ok(ms) = previous.presented_process_ms() {
                    eprintln!("exact-ink: process to displayed WARP frame {ms:.3} ms (DXGI)");
                    previous.composition.as_mut().unwrap().reported_display = true;
                }
            }
            let complete = match previous.completed() {
                Ok(done) => done >= previous.submitted,
                Err(Error::DeviceLost(_)) => true,
                Err(_) => false,
            };
            let count = self.composition.as_ref().and_then(|c| unsafe {
                let mut stats = DXGI_FRAME_STATISTICS_MEDIA::default();
                c.swap
                    .cast::<IDXGISwapChainMedia>()
                    .and_then(|s| s.GetFrameStatisticsMedia(&mut stats))
                    .ok()
                    .map(|()| stats.PresentCount)
            });
            !(complete && present.displayed(count))
        });
        self.retiring_devices.len()
    }

    pub fn attach_window(&mut self, window: Window, size: [u32; 2]) -> Result<()> {
        if !self.software && fault("hardware-attach") {
            return Err(Error::Device(
                "injected hardware composition refusal".into(),
            ));
        }
        if size.contains(&0) {
            return Err(Error::Invalid("composition size".into()));
        }
        let composition = Composition::new(&self.device, window.clone(), size)?;
        if std::env::var_os("EXACT_AGENT_WINDOW_READY").is_some() {
            eprintln!(
                "exact-ink: composition {:.3} ms (swap chain {:.3}, device and visual {:.3})",
                composition.creation_ms, composition.swap_ms, composition.visual_ms
            );
        }
        self.composition = Some(composition);
        Ok(())
    }
    pub fn has_window(&self) -> bool {
        self.composition.is_some()
    }
    pub fn presentation_pending(&self) -> bool {
        self.presentation_pending
    }
    pub fn reset_present_deadline(&mut self) {
        self.presentation_pending = false;
        if let Some(c) = &mut self.composition {
            c.occluded_checks = 0;
        }
    }
    pub fn presentation_retry_delay(&self) -> Option<Duration> {
        if !self.presentation_pending {
            return None;
        }
        let c = self.composition.as_ref()?;
        Some(retry_delay(c.refresh_period(), c.occluded_checks))
    }

    /// Reserve capacity before painting, waiting one display period rounded to ms.
    /// `present` consumes this reservation without polling the handle again.
    pub fn prepare_present(&mut self, size: [u32; 2]) -> Result<bool> {
        self.collect_presented_devices();
        self.presentation_pending = true;
        let c = self
            .composition
            .as_mut()
            .ok_or(Error::Unsupported("no composition window"))?;
        if let Err(error) = c.resize(size) {
            c.occluded_checks = 0;
            return Err(error);
        }
        let timeout = c.refresh_period().as_nanos().div_ceil(1_000_000).min(100) as u32;
        let ready = match c.acquire(timeout) {
            Ok(ready) => ready,
            Err(error) => {
                c.occluded_checks = 0;
                return Err(error);
            }
        };
        self.presentation_pending = !ready;
        Ok(ready)
    }

    pub fn present(&mut self, id: TextureId) -> Result<f64> {
        use std::sync::atomic::{AtomicU32, Ordering};
        static PRESENTED: AtomicU32 = AtomicU32::new(0);
        let inject = !self.software && fault("slow-warp-recovery");
        let result = if inject
            && PRESENTED
                .compare_exchange(1, 2, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
        {
            Err(Error::DeviceLost(
                "injected hardware loss with WARP pending".into(),
            ))
        } else {
            self.present_frame(id)
        };
        if inject && matches!(result, Ok(ms) if ms > 0.) {
            let _ = PRESENTED.compare_exchange(0, 1, Ordering::Relaxed, Ordering::Relaxed);
        }
        self.presentation_pending = matches!(result, Ok(ms) if ms == 0.);
        if matches!(result, Ok(ms) if ms > 0.) {
            if let Some(c) = &self.composition {
                let mut backdrop = c.scene.backdrop.get();
                self.presentation_pending |=
                    backdrop.after_present(|| set_backdrop(c._window.hwnd));
                c.scene.backdrop.set(backdrop);
            }
        }
        result
    }
    fn present_frame(&mut self, id: TextureId) -> Result<f64> {
        let start = Instant::now();
        self.collect_presented_devices();
        let desc = self.texture(id)?.desc;
        let size = [desc.width, desc.height];
        let composition = self
            .composition
            .as_mut()
            .ok_or(Error::Unsupported("no composition window"))?;
        composition.resize(size)?;
        let activating = !composition.active
            || composition.scene.content.get() != composition.swap.as_raw() as usize;
        let swap = composition.swap.clone();
        let opaque = !composition.scene.backdrop.get().available;
        let background = composition
            ._window
            .background
            .get()
            .map(|v| f64::from(v) / 255.);
        let mut did_present;
        unsafe {
            if !composition.acquire(0)? {
                return Ok(0.);
            }
            composition.acquired = false;
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
            frame.batches[0].replace = !opaque;
            self.draw(
                &frame,
                &view.unwrap(),
                size,
                Some(if opaque { background } else { [0.; 4] }),
                None,
            )?;
            let status = swap.Present(1, DXGI_PRESENT(0));
            did_present = presented(status)?;
        }
        let s = self.submit()?;
        self.textures.get_mut(&id).unwrap().last = s;
        if activating || did_present {
            self.composition.as_mut().unwrap().activate()?;
        }
        if activating && !did_present {
            // A not-yet-attached composition swap chain can return OCCLUDED
            // from the real present too. Retry its populated buffer after Commit.
            did_present = presented(unsafe { swap.Present(1, DXGI_PRESENT(0)) })?;
        }
        if !did_present {
            let c = self.composition.as_mut().unwrap();
            c.occluded_checks = c.occluded_checks.saturating_add(1);
        }
        Ok(if did_present {
            start.elapsed().as_secs_f64() * 1000.
        } else {
            0.
        })
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
            let rate = c.scene.device.GetFrameStatistics()?.currentCompositionRate;
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

fn refresh_period(numerator: u32, denominator: u32) -> Option<Duration> {
    (numerator > 0 && denominator > 0)
        .then(|| Duration::from_secs_f64(f64::from(denominator) / f64::from(numerator)))
}

fn retry_delay(period: Duration, occluded_checks: u32) -> Duration {
    period
        .saturating_mul(1 << occluded_checks.min(8))
        .min(Duration::from_secs(1))
}

fn presented(status: windows::core::HRESULT) -> Result<bool> {
    if status == DXGI_STATUS_OCCLUDED {
        return Ok(false);
    }
    status.ok()?;
    Ok(true)
}

fn latency_ready(status: WAIT_EVENT) -> Result<bool> {
    match status {
        WAIT_OBJECT_0 => Ok(true),
        WAIT_TIMEOUT => Ok(false),
        _ => Err(windows::core::Error::from_thread().into()),
    }
}

fn window_drawable(hwnd: HWND) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{IsIconic, IsWindowVisible};
    unsafe { IsWindowVisible(hwnd).as_bool() && !IsIconic(hwnd).as_bool() }
}

fn opaque_background(color: [u8; 4]) -> [u8; 4] {
    [color[0], color[1], color[2], 255]
}

fn set_backdrop(hwnd: HWND) -> bool {
    unsafe {
        let backdrop = DWMSBT_MAINWINDOW;
        optional_backdrop(
            || {
                if fault("mica") {
                    return Err(windows::core::Error::from_hresult(
                        windows::Win32::Foundation::E_FAIL,
                    ));
                }
                DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_SYSTEMBACKDROP_TYPE,
                    (&backdrop as *const DWM_SYSTEMBACKDROP_TYPE).cast(),
                    size_of::<DWM_SYSTEMBACKDROP_TYPE>() as u32,
                )
            },
            || {
                if fault("mica") {
                    return Err(windows::core::Error::from_hresult(
                        windows::Win32::Foundation::E_FAIL,
                    ));
                }
                DwmExtendFrameIntoClientArea(
                    hwnd,
                    &MARGINS {
                        cxLeftWidth: -1,
                        cxRightWidth: -1,
                        cyTopHeight: -1,
                        cyBottomHeight: -1,
                    },
                )
            },
        )
    }
}

fn optional_backdrop(
    set: impl FnOnce() -> windows::core::Result<()>,
    extend: impl FnOnce() -> windows::core::Result<()>,
) -> bool {
    let results = [set(), extend()];
    if let Some(error) = results.into_iter().find_map(std::result::Result::err) {
        static SAID: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if !SAID.swap(true, std::sync::atomic::Ordering::Relaxed) {
            eprintln!("exact-ink: Mica unavailable: {error}");
        }
        false
    } else {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_recovery_retries_after_present_without_replacing_the_device() {
        let mut backdrop = Backdrop::new(false);
        assert!(backdrop.after_present(|| false));
        assert!(!backdrop.available);
        assert!(backdrop.after_present(|| true));
        assert!(backdrop.available);
        assert!(!backdrop.after_present(|| panic!("restored material must stop retrying")));
        let mut refused = Backdrop::new(false);
        for retry in 1..=3 {
            assert_eq!(refused.after_present(|| false), retry < 3);
        }
        assert!(!refused.after_present(|| panic!("permanent refusal must stop retrying")));
    }

    #[test]
    fn failed_statistics_release_after_three_completed_checks() {
        let mut retirement = Retirement {
            present: None,
            failed_checks: 0,
        };
        assert!(!retirement.displayed(Some(8)));
        assert!(!retirement.displayed(None));
        assert!(retirement.displayed(None));
        let mut retirement = Retirement {
            present: Some(8),
            failed_checks: 0,
        };
        assert!(!retirement.displayed(Some(7)));
        assert!(!retirement.displayed(None));
        assert!(!retirement.displayed(None));
        assert!(retirement.displayed(None));
    }

    #[test]
    fn skipped_present_follows_refresh_and_occlusion_backs_off() {
        for hz in [60, 120, 144] {
            let period = refresh_period(hz, 1).unwrap();
            assert_eq!(retry_delay(period, 0), period);
            assert!(retry_delay(period, 8) >= Duration::from_secs(1));
        }
        assert_eq!(refresh_period(0, 1), None);
    }

    #[test]
    fn a_device_less_compositor_cannot_create_a_cpu_surface() {
        use windows::Win32::Foundation::{E_ILLEGAL_METHOD_CALL, E_INVALIDARG};
        unsafe {
            let device: IDCompositionDevice =
                DCompositionCreateDevice2(None::<&windows::core::IUnknown>).unwrap();
            let result = device.CreateSurface(
                8,
                8,
                DXGI_FORMAT_B8G8R8A8_UNORM,
                DXGI_ALPHA_MODE_PREMULTIPLIED,
            );
            // Build 26200 returns E_ILLEGAL_METHOD_CALL for the documented refusal.
            let error = result.unwrap_err().code();
            assert!(
                matches!(error, E_INVALIDARG | E_ILLEGAL_METHOD_CALL),
                "{error:?}"
            );
        }
    }

    #[test]
    fn mica_refusal_keeps_the_reported_window_colour_opaque() {
        for color in [[243, 243, 243, 255], [32, 32, 32, 0], [18, 56, 92, 128]] {
            assert_eq!(
                opaque_background(color),
                [color[0], color[1], color[2], 255]
            );
        }
    }

    #[test]
    fn a_latency_timeout_skips_the_frame_without_losing_the_device() {
        assert!(!latency_ready(WAIT_TIMEOUT).unwrap());
        assert!(latency_ready(WAIT_OBJECT_0).unwrap());
    }

    #[test]
    fn a_refused_backdrop_still_attempts_frame_extension() {
        let extended = Cell::new(false);
        let available = optional_backdrop(
            || {
                Err(windows::core::Error::from_hresult(
                    windows::Win32::Foundation::E_FAIL,
                ))
            },
            || {
                extended.set(true);
                Err(windows::core::Error::from_hresult(
                    windows::Win32::Foundation::E_FAIL,
                ))
            },
        );
        assert!(extended.get());
        assert!(!available);
        assert!(optional_backdrop(|| Ok(()), || Ok(())));
    }

    #[test]
    #[ignore = "native Windows composition requires fxc and a desktop session"]
    fn a_blocked_latency_handle_skips_activation_and_keeps_the_device() {
        use windows::Win32::System::Threading::CreateEventW;
        let window = Window::fixture([80, 60]).unwrap();
        let mut device = D3d11::new().unwrap();
        device.attach_window(window.clone(), [80, 60]).unwrap();
        let target = device.create_target(80, 60).unwrap();
        device
            .render(&Frame::default(), target, Some([1.; 4]))
            .unwrap();
        let blocked = unsafe { CreateEventW(None, true, false, None).unwrap() };
        let original =
            std::mem::replace(&mut device.composition.as_mut().unwrap().latency, blocked);
        let start = Instant::now();
        assert!(!device.prepare_present([80, 60]).unwrap());
        let result = device.present(target);
        device.composition.as_mut().unwrap().latency = original;
        unsafe { CloseHandle(blocked).unwrap() };
        assert_eq!(result.unwrap(), 0.);
        assert!(start.elapsed() < Duration::from_millis(100));
        assert!(device.presentation_pending());
        assert!(!device.composition.as_ref().unwrap().active);
        assert!(device.prepare_present([80, 60]).unwrap());
        // A successful wait owns the next present even if another zero-timeout
        // poll would refuse. Replace the handle to make a second poll observable.
        let blocked = unsafe { CreateEventW(None, true, false, None).unwrap() };
        let original =
            std::mem::replace(&mut device.composition.as_mut().unwrap().latency, blocked);
        let result = device.present(target);
        device.composition.as_mut().unwrap().latency = original;
        unsafe { CloseHandle(blocked).unwrap() };
        assert!(result.unwrap() > 0.);
        assert!(!device.composition.as_ref().unwrap().acquired);
        assert!(!device.presentation_pending());
    }

    #[test]
    #[ignore = "native Windows composition requires fxc and a desktop session"]
    fn a_hidden_window_skips_activation_until_it_is_visible() {
        use windows::Win32::UI::WindowsAndMessaging::*;
        let window = Window::fixture([80, 60]).unwrap();
        let mut device = D3d11::new().unwrap();
        device.attach_window(window.clone(), [80, 60]).unwrap();
        let target = device.create_target(80, 60).unwrap();
        device
            .render(&Frame::default(), target, Some([1.; 4]))
            .unwrap();
        assert!(device.prepare_present([80, 60]).unwrap());
        unsafe {
            let _ = ShowWindow(window.hwnd, SW_HIDE);
        }
        assert_eq!(device.present(target).unwrap(), 0.);
        assert!(device.composition.as_ref().unwrap().acquired);
        assert!(!device.composition.as_ref().unwrap().active);
        unsafe {
            let _ = ShowWindow(window.hwnd, SW_SHOW);
        }
        assert!(device.present(target).unwrap() > 0.);
    }

    #[test]
    #[ignore = "native Windows desktop; completion and display retirement"]
    fn multiple_retiring_devices_survive_until_trim_observes_completion() {
        let window = Window::fixture([80, 60]).unwrap();
        let mut device = D3d11::new().unwrap();
        device.attach_window(window, [80, 60]).unwrap();
        let target = device.create_target(80, 60).unwrap();
        device
            .render(&Frame::default(), target, Some([0.; 4]))
            .unwrap();
        device.present(target).unwrap();
        let composition = device.composition.take();
        for _ in 0..2 {
            let mut previous = D3d11::open(true).unwrap();
            let target = previous.create_target(8, 8).unwrap();
            previous
                .render(&Frame::default(), target, Some([1.; 4]))
                .unwrap();
            device.retire_after_present(previous);
        }
        device.composition = composition;
        // Hold the display condition independently of how quickly WARP finishes.
        // Missing statistics otherwise release a device after three checks.
        for (_, retirement) in &mut device.retiring_devices {
            retirement.present = Some(u32::MAX);
        }
        assert_eq!(device.retiring_devices.len(), 2);
        device.trim().unwrap();
        assert_eq!(device.retiring_devices.len(), 2);
        for (previous, present) in &mut device.retiring_devices {
            previous.wait(previous.submitted).unwrap();
            present.present = Some(0);
        }
        super::super::super::compositor_sync_process_ms().unwrap();
        device.resize_target(target, 90, 60).unwrap();
        assert!(device.retiring_devices.is_empty());
    }

    #[test]
    #[ignore = "native Windows desktop; recovery from WARP and Mica"]
    fn recovery_replaces_warp_after_completion_and_alpha_reaches_mica() {
        use windows::Win32::{Graphics::Gdi::*, UI::WindowsAndMessaging::*};
        let window = Window::fixture([320, 240]).unwrap();
        window.dispatch_pending();
        let hwnd = window.hwnd;
        let color = 0x00336699;
        let mut warp = D3d11::open(true).unwrap();
        warp.attach_window(window.clone(), [320, 240]).unwrap();
        let warp_target = warp.create_target(320, 240).unwrap();
        let clear = [
            0x99 as f64 / 255.,
            0x66 as f64 / 255.,
            0x33 as f64 / 255.,
            1.,
        ];
        warp.render(&Frame::default(), warp_target, Some(clear))
            .unwrap();
        warp.present(warp_target).unwrap();
        let pixel = || unsafe {
            let mut at = windows::Win32::Foundation::POINT { x: 160, y: 120 };
            assert!(ClientToScreen(hwnd, &mut at).as_bool());
            let dc = GetDC(None);
            let pixel = GetPixel(dc, at.x, at.y).0;
            ReleaseDC(None, dc);
            pixel
        };
        let wait_for = |expected: Option<u32>| {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                window.dispatch_pending();
                super::super::super::compositor_sync_process_ms().unwrap();
                let actual = pixel();
                if expected.map_or(actual != color && actual != u32::MAX, |c| actual == c) {
                    return actual;
                }
                assert!(
                    Instant::now() < deadline,
                    "desktop pixel {actual:x}, expected {expected:?}"
                );
            }
        };
        wait_for(Some(color));
        warp.render(&Frame::default(), warp_target, Some([0.; 4]))
            .unwrap();
        warp.present(warp_target).unwrap();
        let mica_before = wait_for(None);
        warp.render(&Frame::default(), warp_target, Some(clear))
            .unwrap();
        warp.present(warp_target).unwrap();
        wait_for(Some(color));
        let mut d = D3d11::new().unwrap();
        let t = d.create_target(320, 240).unwrap();
        d.render(
            &Frame::default(),
            t,
            Some([
                0x99 as f64 / 255.,
                0x66 as f64 / 255.,
                0x33 as f64 / 255.,
                1.,
            ]),
        )
        .unwrap();
        let (ready, started) = std::sync::mpsc::channel();
        let address = hwnd.0 as usize;
        let sampling = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let active = sampling.clone();
        let samples = std::thread::spawn(move || {
            let mut samples = Vec::new();
            let start = Instant::now();
            let mut ready = Some(ready);
            while active.load(std::sync::atomic::Ordering::Acquire) {
                unsafe {
                    DwmFlush().unwrap();
                    let mut point = windows::Win32::Foundation::POINT { x: 160, y: 120 };
                    assert!(ClientToScreen(HWND(address as *mut _), &mut point).as_bool());
                    let dc = GetDC(None);
                    samples.push((
                        start.elapsed().as_secs_f64() * 1000.,
                        GetPixel(dc, point.x, point.y).0,
                    ));
                    ReleaseDC(None, dc);
                }
                if let Some(ready) = ready.take() {
                    ready.send(()).unwrap();
                }
            }
            samples
        });
        started.recv().unwrap();
        d.attach_window(window.clone(), [320, 240]).unwrap();
        d.present(t).unwrap();
        d.retire_after_present(warp);
        for _ in 0..3 {
            assert_eq!(wait_for(Some(color)), color);
        }
        sampling.store(false, std::sync::atomic::Ordering::Release);
        let samples = samples.join().unwrap();
        assert!(
            samples.len() >= 2 && samples.iter().all(|s| s.1 == color),
            "handoff samples: {samples:?}"
        );
        println!("S2_HANDOFF desktop_ms_and_rgb={samples:?}");
        unsafe {
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
            assert_eq!(style & WS_EX_LAYERED.0, 0);
            assert_ne!(style & WS_EX_NOREDIRECTIONBITMAP.0, 0);
            let mut backdrop = DWM_SYSTEMBACKDROP_TYPE::default();
            DwmGetWindowAttribute(
                hwnd,
                DWMWA_SYSTEMBACKDROP_TYPE,
                (&mut backdrop as *mut DWM_SYSTEMBACKDROP_TYPE).cast(),
                size_of::<DWM_SYSTEMBACKDROP_TYPE>() as u32,
            )
            .unwrap();
            assert_eq!(backdrop, DWMSBT_MAINWINDOW);
        }
        d.render(&Frame::default(), t, Some([0.; 4])).unwrap();
        d.present(t).unwrap();
        let mica_after = wait_for(None);
        assert!(
            (0..3).all(|c| ((mica_before >> (8 * c)) as u8)
                .abs_diff((mica_after >> (8 * c)) as u8)
                < 5),
            "Mica changed: {mica_before:06x} -> {mica_after:06x}"
        );
        println!("S2_MICA before={mica_before:06x}, after={mica_after:06x}, layered=false");
        d.trim().unwrap();
        assert!(
            d.retiring_devices.is_empty(),
            "completed WARP device must retire"
        );
    }
    #[test]
    fn occluded_present_is_skipped_and_device_removal_is_an_error() {
        use windows::Win32::Foundation::S_OK;
        assert!(presented(S_OK).unwrap());
        assert!(!presented(DXGI_STATUS_OCCLUDED).unwrap());
        assert!(matches!(
            presented(DXGI_ERROR_DEVICE_REMOVED),
            Err(Error::DeviceLost(_))
        ));
    }

    #[test]
    #[ignore = "native Windows composition requires fxc and a desktop session"]
    fn resize_reacquires_latency_control_and_presents_again() {
        let window = Window::fixture([120, 80]).unwrap();
        let mut d = D3d11::new().unwrap();
        d.attach_window(window, [120, 80]).unwrap();
        for size in [[120, 80], [160, 100], [90, 60]] {
            let t = d.create_target(size[0], size[1]).unwrap();
            d.render(&Frame::default(), t, Some([0.2, 0.4, 0.6, 1.]))
                .unwrap();
            d.present(t).unwrap();
            let c = d.composition.as_ref().unwrap();
            assert!(!c.latency.is_invalid());
            assert_eq!(unsafe { c.swap.GetMaximumFrameLatency().unwrap() }, 1);
            d.retire_texture(t).unwrap();
        }
    }
}
