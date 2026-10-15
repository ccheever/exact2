//! Image placement shared by the paint walk and its callers.
use super::Rect4;
use exact_kernel::ObjectFit;

/// Where a picture goes under CSS `object-fit`, centred in the content box:
/// `fill` stretches, `contain`/`cover` keep the ratio, `none` is the natural
/// size, `scale-down` the smaller of none and contain (LLP 1011 §4).
pub fn object_fit(natural: (u32, u32), fit: ObjectFit, content: Rect4) -> Option<Rect4> {
    let (nw, nh) = (natural.0 as f32, natural.1 as f32);
    if nw <= 0.0 || nh <= 0.0 || content.2 <= 0.0 || content.3 <= 0.0 {
        return None;
    }
    let (sx, sy) = (content.2 / nw, content.3 / nh);
    let s = match fit {
        ObjectFit::Contain => Some(sx.min(sy)),
        ObjectFit::Cover => Some(sx.max(sy)),
        ObjectFit::None => Some(1.0),
        ObjectFit::ScaleDown => Some(sx.min(sy).min(1.0)),
        ObjectFit::Fill => None,
    };
    let (dw, dh) = match s {
        Some(s) => (nw * s, nh * s),
        None => (content.2, content.3),
    };
    Some((
        content.0 + (content.2 - dw) / 2.0,
        content.1 + (content.3 - dh) / 2.0,
        dw,
        dh,
    ))
}
