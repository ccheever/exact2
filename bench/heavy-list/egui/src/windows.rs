//! Windows image decoding and the S2 DWM readiness proxy; no per-frame timer.
use std::{cell::Cell, path::Path};
use windows::{
    Win32::{
        Foundation::{FILETIME, GENERIC_READ},
        Graphics::{Dwm::DwmFlush, Imaging::*},
        System::{Com::*, SystemInformation::GetSystemTimeAsFileTime, Threading::*},
    },
    core::{Interface, PCWSTR, Result},
};

thread_local! {
    static COMPLETE: Cell<bool> = const { Cell::new(false) };
    static REPORTED: Cell<bool> = const { Cell::new(false) };
    static COM: Com = {
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok().expect("WIC worker COM") };
        Com
    };
}
struct Com;
impl Drop for Com {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

pub fn thumbnail(path: &Path, width: u32, height: u32) -> Result<egui::ColorImage> {
    use std::os::windows::ffi::OsStrExt;
    COM.with(|_| {});
    let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        let factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let decoder = factory.CreateDecoderFromFilename(
            PCWSTR(path.as_ptr()),
            None,
            GENERIC_READ,
            WICDecodeMetadataCacheOnDemand,
        )?;
        let frame = decoder.GetFrame(0)?;
        let (mut source_width, mut source_height) = (0, 0);
        frame.GetSize(&mut source_width, &mut source_height)?;
        let scale = (width as f64 / source_width as f64).max(height as f64 / source_height as f64);
        let scaled_width = (source_width as f64 * scale).ceil() as u32;
        let scaled_height = (source_height as f64 * scale).ceil() as u32;
        let transform: IWICBitmapSourceTransform = frame.cast()?;
        let (mut decoded_width, mut decoded_height) = (scaled_width, scaled_height);
        transform.GetClosestSize(&mut decoded_width, &mut decoded_height)?;
        let mut format = GUID_WICPixelFormat24bppBGR;
        transform.GetClosestPixelFormat(&mut format)?;
        assert_eq!(format, GUID_WICPixelFormat24bppBGR, "JPEG transform format");
        let mut pixels = vec![0; decoded_width as usize * decoded_height as usize * 3];
        transform.CopyPixels(
            std::ptr::null(),
            decoded_width,
            decoded_height,
            &format,
            WICBitmapTransformRotate0,
            decoded_width * 3,
            &mut pixels,
        )?;
        let bitmap = factory.CreateBitmapFromMemory(
            decoded_width,
            decoded_height,
            &format,
            decoded_width * 3,
            &pixels,
        )?;
        let scaler = factory.CreateBitmapScaler()?;
        scaler.Initialize(
            &bitmap,
            scaled_width,
            scaled_height,
            WICBitmapInterpolationModeFant,
        )?;
        let converter = factory.CreateFormatConverter()?;
        converter.Initialize(
            &scaler,
            &GUID_WICPixelFormat32bppRGBA,
            WICBitmapDitherTypeNone,
            None,
            0.,
            WICBitmapPaletteTypeCustom,
        )?;
        let crop = WICRect {
            X: ((scaled_width - width) / 2) as i32,
            Y: ((scaled_height - height) / 2) as i32,
            Width: width as i32,
            Height: height as i32,
        };
        let mut rgba = vec![0; width as usize * height as usize * 4];
        converter.CopyPixels(&crop, width * 4, &mut rgba)?;
        Ok(egui::ColorImage::from_rgba_unmultiplied(
            [width as usize, height as usize],
            &rgba,
        ))
    }
}

pub fn scene_complete(complete: bool, item: usize, offset: f32) {
    COMPLETE.set(complete);
    if complete && !REPORTED.get() {
        eprintln!("egui: complete scene item={item} offset={offset}");
    }
}

pub fn needs_observation() -> bool {
    COMPLETE.get() && !REPORTED.get()
}

pub fn after_frame() {
    if !needs_observation() {
        return;
    }
    // App::logic is entered after the preceding UI frame's paint/present returns.
    // Eframe exposes no post-present callback or swap chain to the application.
    unsafe {
        DwmFlush().expect("DWM readiness synchronization");
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
        )
        .expect("process creation time");
        let ticks = |t: FILETIME| ((t.dwHighDateTime as u64) << 32) | t.dwLowDateTime as u64;
        let age = (ticks(GetSystemTimeAsFileTime()) - ticks(created)) as f64 / 10_000.;
        eprintln!("egui: process to first-screen compositor sync {age:.3} ms (DWM proxy)");
        eprintln!("egui: DXGI display timestamp unavailable: eframe owns the swap chain");
    }
    REPORTED.set(true);
}
