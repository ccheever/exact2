//! WIC's frame decoder, scaler and premultiplied RGBA conversion. No COM
//! object or borrowed stream buffer leaves the raster worker.
#![allow(unsafe_code)]
use super::{read_all, DecodePlan, Pixmap, Read, Refusal, Seek};
use ::windows::core::Interface;
use ::windows::Win32::{Graphics::Imaging::*, System::Com::*};

struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        // SAFETY: paired with this thread's successful CoInitializeEx.
        unsafe { CoUninitialize() };
    }
}

pub(in crate::image) fn decode<R: Read + Seek>(
    input: R,
    plan: &DecodePlan,
    cancelled: impl Fn() -> bool,
) -> Result<Pixmap, Refusal> {
    let bytes = read_all(input, plan, &cancelled)?;
    // SAFETY: every interface is released before the apartment guard; the
    // stream's borrowed bytes outlive the entire decoder/transform chain.
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED)
            .ok()
            .map_err(|_| Refusal::DecodeFailed)?;
        let _apartment = Apartment;
        let factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)
                .map_err(|_| Refusal::DecodeFailed)?;
        let stream = factory.CreateStream().map_err(|_| Refusal::DecodeFailed)?;
        stream
            .InitializeFromMemory(&bytes)
            .map_err(|_| Refusal::DecodeFailed)?;
        let decoder = factory
            .CreateDecoderFromStream(&stream, std::ptr::null(), WICDecodeMetadataCacheOnDemand)
            .map_err(|_| Refusal::DecodeFailed)?;
        let frame = decoder.GetFrame(0).map_err(|_| Refusal::DecodeFailed)?;
        let (mut width, mut height) = (0, 0);
        frame
            .GetSize(&mut width, &mut height)
            .map_err(|_| Refusal::DecodeFailed)?;
        let swaps_axes = plan.header.orientation >= 5;
        let natural = if swaps_axes {
            (height, width)
        } else {
            (width, height)
        };
        if natural != plan.natural() {
            return Err(Refusal::ConflictingMetadata);
        }
        let converted = factory
            .CreateFormatConverter()
            .map_err(|_| Refusal::DecodeFailed)?;
        converted
            .Initialize(
                &frame,
                &GUID_WICPixelFormat32bppPRGBA,
                WICBitmapDitherTypeNone,
                None,
                0.0,
                WICBitmapPaletteTypeCustom,
            )
            .map_err(|_| Refusal::DecodeFailed)?;
        let full = plan.full();
        let scaled = factory
            .CreateBitmapScaler()
            .map_err(|_| Refusal::DecodeFailed)?;
        // HighQualityCubic widens the filter for reductions beyond 2x:
        // https://learn.microsoft.com/windows/win32/api/wincodec/ne-wincodec-wicbitmapinterpolationmode
        scaled
            .Initialize(
                &converted,
                if swaps_axes { full.height } else { full.width },
                if swaps_axes { full.width } else { full.height },
                WICBitmapInterpolationModeHighQualityCubic,
            )
            .map_err(|_| Refusal::DecodeFailed)?;
        let transform = match plan.header.orientation {
            2 => WICBitmapTransformFlipHorizontal,
            3 => WICBitmapTransformRotate180,
            4 => WICBitmapTransformFlipVertical,
            5 => WICBitmapTransformOptions(
                WICBitmapTransformRotate270.0 | WICBitmapTransformFlipHorizontal.0,
            ),
            6 => WICBitmapTransformRotate90,
            7 => WICBitmapTransformOptions(
                WICBitmapTransformRotate90.0 | WICBitmapTransformFlipHorizontal.0,
            ),
            8 => WICBitmapTransformRotate270,
            _ => WICBitmapTransformRotate0,
        };
        let source: IWICBitmapSource = if transform == WICBitmapTransformRotate0 {
            scaled.cast().map_err(|_| Refusal::DecodeFailed)?
        } else {
            let oriented = factory
                .CreateBitmapFlipRotator()
                .map_err(|_| Refusal::DecodeFailed)?;
            oriented
                .Initialize(&scaled, transform)
                .map_err(|_| Refusal::DecodeFailed)?;
            oriented.cast().map_err(|_| Refusal::DecodeFailed)?
        };
        if cancelled() {
            return Err(Refusal::Stale);
        }
        let mut pixels =
            Pixmap::new(plan.pixels.width, plan.pixels.height).ok_or(Refusal::InvalidDimensions)?;
        let rect = WICRect {
            X: plan.crop.x as i32,
            Y: plan.crop.y as i32,
            Width: plan.pixels.width as i32,
            Height: plan.pixels.height as i32,
        };
        source
            .CopyPixels(&rect, plan.pixels.width * 4, pixels.data_mut())
            .map_err(|_| Refusal::DecodeFailed)?;
        if cancelled() {
            return Err(Refusal::Stale);
        }
        for pixel in pixels.data_mut().chunks_exact_mut(4) {
            let alpha = pixel[3];
            for channel in &mut pixel[..3] {
                *channel = (*channel).min(alpha);
            }
        }
        Ok(pixels)
    }
}
