//! A premultiplied composition surface over the system backdrop.
//! @ref LLP 1149 D7; LLP 1115.004 D4.3.
use super::*;
use std::any::Any;
use windows::Win32::{
    Foundation::{CloseHandle, DXGI_STATUS_OCCLUDED, HANDLE, HWND, WAIT_OBJECT_0},
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
                WINDOW_EX_STYLE::default(),
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

pub(super) struct Composition {
    swap: IDXGISwapChain2,
    device: IDCompositionDevice,
    target: IDCompositionTarget,
    visual: IDCompositionVisual,
    latency: HANDLE,
    size: [u32; 2],
    _window: Window,
    pub creation_ms: f64,
    swap_ms: f64,
    visual_ms: f64,
    active: bool,
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
            let swap_ms = start.elapsed().as_secs_f64() * 1000.;
            let visual_started = Instant::now();
            let device: IDCompositionDevice = DCompositionCreateDevice(&dxgi)?;
            let target = device.CreateTargetForHwnd(window.hwnd, true)?;
            let visual = device.CreateVisual()?;
            visual.SetContent(&swap)?;
            target.SetRoot(&visual)?;
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
                device,
                target,
                visual,
                latency,
                size,
                _window: window,
                creation_ms: start.elapsed().as_secs_f64() * 1000.,
                swap_ms,
                visual_ms,
                active: false,
            })
        }
    }

    fn activate(&mut self) -> Result<()> {
        use windows::Win32::UI::WindowsAndMessaging::*;
        if self.active {
            return Ok(());
        }
        // Commit only after the first image has been submitted. The carrier
        // has released GDI; alpha now belongs to DWM. @ref LLP 1115.004 D4.3.
        unsafe {
            let hwnd = self._window.hwnd;
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
            if style & WS_EX_LAYERED.0 != 0 {
                return Err(Error::Invalid(
                    "ink composition cannot use WS_EX_LAYERED".into(),
                ));
            }
            SetWindowLongPtrW(
                hwnd,
                GWL_EXSTYLE,
                (style | WS_EX_NOREDIRECTIONBITMAP.0) as isize,
            );
            let backdrop = DWMSBT_MAINWINDOW;
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_SYSTEMBACKDROP_TYPE,
                (&backdrop as *const DWM_SYSTEMBACKDROP_TYPE).cast(),
                size_of::<DWM_SYSTEMBACKDROP_TYPE>() as u32,
            )?;
            DwmExtendFrameIntoClientArea(
                hwnd,
                &MARGINS {
                    cxLeftWidth: -1,
                    cxRightWidth: -1,
                    cyTopHeight: -1,
                    cyBottomHeight: -1,
                },
            )?;
            self.device.Commit()?;
        }
        self.active = true;
        Ok(())
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

impl D3d11 {
    pub fn attach_window(&mut self, window: Window, size: [u32; 2]) -> Result<()> {
        if size.contains(&0) {
            return Err(Error::Invalid("composition size".into()));
        }
        let composition = Composition::new(&self.device, window.clone(), size)?;
        eprintln!(
            "exact-ink: composition {:.3} ms (swap chain {:.3}, device and visual {:.3})",
            composition.creation_ms, composition.swap_ms, composition.visual_ms
        );
        self.composition = Some(composition);
        Ok(())
    }
    pub fn has_window(&self) -> bool {
        self.composition.is_some()
    }
    pub fn present(&mut self, id: TextureId) -> Result<f64> {
        let start = Instant::now();
        let desc = self.texture(id)?.desc;
        let size = [desc.width, desc.height];
        let composition = self
            .composition
            .as_mut()
            .ok_or(Error::Unsupported("no composition window"))?;
        composition.resize(size)?;
        let activating = !composition.active;
        let swap = composition.swap.clone();
        let mut did_present;
        unsafe {
            // Before Commit, DXGI considers the unattached visual occluded.
            // Its first image must be queued before that visual becomes visible.
            if composition.active && !presented(swap.Present(0, DXGI_PRESENT_TEST))? {
                return Ok(0.);
            }
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

fn presented(status: windows::core::HRESULT) -> Result<bool> {
    if status == DXGI_STATUS_OCCLUDED {
        return Ok(false);
    }
    status.ok()?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "native Windows desktop; GDI handoff and Mica"]
    fn gdi_picture_migrates_without_layering_and_alpha_reaches_mica() {
        use windows::Win32::{Graphics::Gdi::*, UI::WindowsAndMessaging::*};
        let window = Window::fixture([320, 240]).unwrap();
        window.dispatch_pending();
        let hwnd = window.hwnd;
        let color = 0x00336699;
        unsafe {
            let dc = GetDC(Some(hwnd));
            let brush = CreateSolidBrush(windows::Win32::Foundation::COLORREF(color));
            FillRect(
                dc,
                &RECT {
                    left: 0,
                    top: 0,
                    right: 320,
                    bottom: 240,
                },
                brush,
            );
            assert!(DeleteObject(brush.into()).as_bool());
            ReleaseDC(Some(hwnd), dc);
        }
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
        println!(
            "S2_MICA opaque={color:06x}, transparent={:06x}, layered=false",
            wait_for(None)
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
