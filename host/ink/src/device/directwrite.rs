//! DirectWrite's grayscale candidate. Layout and advances stay with the shaper.
//! @ref LLP 1149 D3.2–D3.3; LLP 1149.001 M9.
use super::windows_library::Library;
use crate::glyph::Font;
use std::{collections::HashMap, mem::ManuallyDrop};
use swash::scale::image::{Content, Image};
use windows::{
    core::{s, w, Interface, GUID, HRESULT, PCWSTR},
    Win32::Graphics::DirectWrite::*,
};

pub struct DirectWrite {
    factory: IDWriteFactory5,
    loader: IDWriteInMemoryFontFileLoader,
    faces: HashMap<([u64; 2], u32), IDWriteFontFace>,
    _library: Library,
}

impl Drop for DirectWrite {
    fn drop(&mut self) {
        self.faces.clear();
        unsafe {
            let _ = self.factory.UnregisterFontFileLoader(&self.loader);
        }
    }
}

impl DirectWrite {
    pub fn new() -> windows::core::Result<Self> {
        unsafe {
            let library = Library::open(w!("dwrite.dll"))?;
            type Create = unsafe extern "system" fn(
                DWRITE_FACTORY_TYPE,
                *const GUID,
                *mut *mut std::ffi::c_void,
            ) -> HRESULT;
            let create: Create = library.get(s!("DWriteCreateFactory"))?;
            let mut raw = std::ptr::null_mut();
            create(
                DWRITE_FACTORY_TYPE_ISOLATED,
                &IDWriteFactory5::IID,
                &mut raw,
            )
            .ok()?;
            let factory = IDWriteFactory5::from_raw(raw);
            let loader = factory.CreateInMemoryFontFileLoader()?;
            factory.RegisterFontFileLoader(&loader)?;
            Ok(Self {
                factory,
                loader,
                faces: HashMap::new(),
                _library: library,
            })
        }
    }

    /// Admission requires separate evidence for variable and colour faces.
    /// A normalized coordinate cannot be passed as a DirectWrite axis value.
    pub fn candidate(font: &Font<'_>) -> bool {
        let Some(face) = swash::FontRef::from_index(font.data, font.index as usize) else {
            return false;
        };
        !font.skew
            && face.variations().next().is_none()
            && face.table(u32::from_be_bytes(*b"COLR")).is_none()
            && face.table(u32::from_be_bytes(*b"sbix")).is_none()
            && face.table(u32::from_be_bytes(*b"CBDT")).is_none()
    }

    fn face(&mut self, font: &Font<'_>) -> windows::core::Result<IDWriteFontFace> {
        let key = (font.id, font.index);
        if let Some(face) = self.faces.get(&key) {
            return Ok(face.clone());
        }
        unsafe {
            // A null owner makes DirectWrite copy the bytes, so the cached face
            // cannot outlive borrowed shaper storage. The loader stays registered.
            let file = self.loader.CreateInMemoryFontFileReference(
                &self.factory,
                font.data.as_ptr().cast(),
                font.data.len().try_into().map_err(|_| {
                    windows::core::Error::from_hresult(windows::Win32::Foundation::E_INVALIDARG)
                })?,
                None,
            )?;
            let mut supported = windows::core::BOOL(0);
            let mut kind = DWRITE_FONT_FILE_TYPE::default();
            let mut face_kind = DWRITE_FONT_FACE_TYPE::default();
            let mut count = 0;
            file.Analyze(&mut supported, &mut kind, Some(&mut face_kind), &mut count)?;
            if !supported.as_bool() || font.index >= count {
                return Err(windows::core::Error::from_hresult(
                    windows::Win32::Foundation::E_INVALIDARG,
                ));
            }
            let base: IDWriteFactory = self.factory.cast()?;
            let face = base.CreateFontFace(
                face_kind,
                &[Some(file)],
                font.index,
                DWRITE_FONT_SIMULATIONS_NONE,
            )?;
            self.faces.insert(key, face.clone());
            Ok(face)
        }
    }

    /// Raw coverage, before any gamma/contrast correction. No ClearType pixels
    /// enter a premultiplied composition surface.
    pub fn raster(
        &mut self,
        font: &Font<'_>,
        glyph: u16,
        size: f32,
        phase: u8,
    ) -> windows::core::Result<Image> {
        let face = self.face(font)?;
        unsafe {
            let mut run = DWRITE_GLYPH_RUN {
                fontFace: ManuallyDrop::new(Some(face)),
                fontEmSize: size,
                glyphCount: 1,
                glyphIndices: &glyph,
                ..Default::default()
            };
            let factory: IDWriteFactory2 = self.factory.cast()?;
            let analysis = factory.CreateGlyphRunAnalysis(
                &run,
                None,
                DWRITE_RENDERING_MODE_NATURAL_SYMMETRIC,
                DWRITE_MEASURING_MODE_NATURAL,
                DWRITE_GRID_FIT_MODE_DISABLED,
                DWRITE_TEXT_ANTIALIAS_MODE_GRAYSCALE,
                f32::from(phase) * 0.25,
                0.,
            );
            ManuallyDrop::drop(&mut run.fontFace);
            let analysis = analysis?;
            let bounds = analysis.GetAlphaTextureBounds(DWRITE_TEXTURE_ALIASED_1x1)?;
            let width = (bounds.right - bounds.left).max(0) as u32;
            let height = (bounds.bottom - bounds.top).max(0) as u32;
            let mut image = Image {
                content: Content::Mask,
                placement: swash::zeno::Placement {
                    left: bounds.left,
                    top: -bounds.top,
                    width,
                    height,
                },
                ..Default::default()
            };
            image.data.resize(width as usize * height as usize, 0);
            if !image.data.is_empty() {
                analysis.CreateAlphaTexture(
                    DWRITE_TEXTURE_ALIASED_1x1,
                    &bounds,
                    &mut image.data,
                )?;
            }
            Ok(image)
        }
    }

    /// Latin audit runs use DirectWrite's script-zero shaping. The caller must
    /// compare glyph IDs too; a different run is not evidence about advances.
    pub fn placements(
        &mut self,
        font: &Font<'_>,
        text: &str,
        size: f32,
    ) -> windows::core::Result<(Vec<u16>, Vec<f32>)> {
        let face = self.face(font)?;
        let text: Vec<u16> = text.encode_utf16().collect();
        let mut clusters = vec![0; text.len()];
        let mut props = vec![DWRITE_SHAPING_TEXT_PROPERTIES::default(); text.len()];
        let max = text.len() * 3 + 16;
        let mut glyphs = vec![0; max];
        let mut glyph_props = vec![DWRITE_SHAPING_GLYPH_PROPERTIES::default(); max];
        let mut count = 0;
        let script = DWRITE_SCRIPT_ANALYSIS::default();
        unsafe {
            let analyzer = self.factory.CreateTextAnalyzer()?;
            analyzer.GetGlyphs(
                PCWSTR(text.as_ptr()),
                text.len() as u32,
                &face,
                false,
                false,
                &script,
                PCWSTR::null(),
                None,
                None,
                None,
                0,
                max as u32,
                clusters.as_mut_ptr(),
                props.as_mut_ptr(),
                glyphs.as_mut_ptr(),
                glyph_props.as_mut_ptr(),
                &mut count,
            )?;
            glyphs.truncate(count as usize);
            let mut advances = vec![0.; count as usize];
            let mut offsets = vec![DWRITE_GLYPH_OFFSET::default(); count as usize];
            analyzer.GetGlyphPlacements(
                PCWSTR(text.as_ptr()),
                clusters.as_ptr(),
                props.as_mut_ptr(),
                text.len() as u32,
                glyphs.as_ptr(),
                glyph_props.as_ptr(),
                count,
                &face,
                size,
                false,
                false,
                &script,
                PCWSTR::null(),
                None,
                None,
                0,
                advances.as_mut_ptr(),
                offsets.as_mut_ptr(),
            )?;
            Ok((glyphs, advances))
        }
    }

    pub fn rendering_params(&self) -> windows::core::Result<[f32; 3]> {
        unsafe {
            let params = self.factory.CreateRenderingParams()?;
            Ok([
                params.GetGamma(),
                params.GetEnhancedContrast(),
                params.GetClearTypeLevel(),
            ])
        }
    }
}
