//! UIKit control fonts, chrome and the environment seam. @ref LLP 1104 D4–D5.
use exact_kernel::{
    ControlFont, ControlTextStyles, FieldChrome, FieldChromeRequest, FieldKind, FontStyle,
};
use std::ffi::c_void;

/// The registered platform body font, returned after the plan catalog is installed.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CControlFont {
    /// Borrowed UTF-8 family name, valid until the next callback.
    pub family: *const u8,
    /// Byte length of `family`.
    pub family_len: usize,
    /// Font registry id carried by computed text runs.
    pub family_id: u16,
    /// Logical points.
    pub size: f32,
    /// CSS font weight.
    pub weight: u16,
    /// Nonzero for italic.
    pub italic: u8,
}
/// Ask the host for its registered control font before layout.
pub type ControlTextFn = extern "C" fn(*mut c_void, u8) -> CControlFont;

/// The kernel's resolved field chrome request.
#[repr(C)]
pub struct CFieldChromeRequest {
    /// 0 field, 1 secure, 2 search, 3 textarea.
    pub kind: u8,
    /// Host font registry id.
    pub family_id: u16,
    /// Logical font size.
    pub size: f32,
    /// CSS weight.
    pub weight: u16,
    /// Nonzero for italic.
    pub italic: u8,
}
/// Chrome outside the content box and authored padding, in points.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct CFieldChrome {
    /// Top inset.
    pub top: f32,
    /// Right inset.
    pub right: f32,
    /// Bottom inset.
    pub bottom: f32,
    /// Left inset.
    pub left: f32,
    /// Single-line minimum outer height.
    pub minimum_height: f32,
    /// Nonzero if the answer requires another layout before presentation.
    pub provisional: u8,
}
/// Answer from the cache; a miss measures on main before returning a provisional answer.
pub type FieldChromeFn = extern "C" fn(*mut c_void, *const CFieldChromeRequest) -> CFieldChrome;

pub(crate) fn text_styles(f: ControlTextFn, ctx: *mut c_void) -> ControlTextStyles {
    let f = f(ctx, 0);
    let font = ControlFont {
        family: crate::app_module::text(f.family, f.family_len),
        family_id: f.family_id,
        size: f.size,
        weight: f.weight,
        style: if f.italic == 0 {
            FontStyle::Normal
        } else {
            FontStyle::Italic
        },
    };
    ControlTextStyles {
        field: font.clone(),
        textarea: font.clone(),
        button: font,
    }
}
pub(crate) fn button_fonts(f: ControlTextFn, ctx: *mut c_void) -> exact_kernel::ButtonFonts {
    let font = |size| {
        let f = f(ctx, size);
        Some(ControlFont {
            family: crate::app_module::text(f.family, f.family_len),
            family_id: f.family_id,
            size: f.size,
            weight: f.weight,
            style: if f.italic == 0 {
                FontStyle::Normal
            } else {
                FontStyle::Italic
            },
        })
    };
    exact_kernel::ButtonFonts {
        mini: font(1),
        small: font(2),
        medium: font(3),
        large: font(4),
    }
}
pub(crate) fn chrome(f: FieldChromeFn, ctx: *mut c_void, r: &FieldChromeRequest) -> FieldChrome {
    let r = CFieldChromeRequest {
        kind: match r.kind {
            FieldKind::Field => 0,
            FieldKind::SecureField => 1,
            FieldKind::SearchField => 2,
            FieldKind::Textarea => 3,
        },
        family_id: r.font.family_id,
        size: r.font.size,
        weight: r.font.weight,
        italic: u8::from(r.font.style != FontStyle::Normal),
    };
    let c = f(ctx, &r);
    FieldChrome {
        top: c.top,
        right: c.right,
        bottom: c.bottom,
        left: c.left,
        minimum_height: c.minimum_height,
        provisional: c.provisional != 0,
    }
}

/// Borrowed face/row JSON, identical to `exact_press_face`, and a border-box width offer.
#[repr(C)]
pub struct CButtonMeasureRequest {
    /// UTF-8 face and authored rows, alive for this call.
    pub face: *const u8,
    /// Byte length of face.
    pub face_len: usize,
    /// 0 definite, 1 min-content, 2 max-content.
    pub width_kind: u8,
    /// Logical border-box width when definite.
    pub width: f32,
}
/// UIKit's real fitting border box.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct CButtonMeasure {
    /// Fitting width.
    pub width: f32,
    /// Height at the offered width.
    pub height: f32,
    /// Nonzero on a cache miss, requiring silent relayout.
    pub provisional: u8,
}
/// Optional native button measure callback, with the text engine context.
pub type ButtonMeasureFn =
    extern "C" fn(*mut c_void, *const CButtonMeasureRequest) -> CButtonMeasure;

pub(crate) fn button_measure(
    f: ButtonMeasureFn,
    ctx: *mut c_void,
    r: &exact_kernel::ButtonMeasureRequest,
) -> exact_kernel::ButtonMeasure {
    let face = crate::button::face_json(Some(&r.face), Some(&r.style), &r.button_style);
    let (width_kind, width) = match r.width {
        exact_kernel::AxisOffer::Definite(w) => (0, w),
        exact_kernel::AxisOffer::MinContent => (1, 0.0),
        exact_kernel::AxisOffer::MaxContent => (2, 0.0),
    };
    let request = CButtonMeasureRequest {
        face: face.as_ptr(),
        face_len: face.len(),
        width_kind,
        width,
    };
    let answer = f(ctx, &request);
    exact_kernel::ButtonMeasure {
        width: answer.width,
        height: answer.height,
        provisional: answer.provisional != 0,
    }
}
