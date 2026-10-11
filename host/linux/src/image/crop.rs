//! The part of a picture a box that clips it shows.
//!
//! `object-fit: cover` scales a picture until it covers its box and clips
//! what hangs over: a 16:9 photo in a 4:3 box shows three quarters of its
//! width. Decoded whole, the picture held is the cover's size; decoded as
//! the part that shows, it is the box's. The part is found here, in the
//! pixels of the whole decode, so it is cut from that decode and its pixels
//! are the whole's (`DecodePlan::part`); a painter draws them where the
//! whole's would be (`Bitmap::placed`).
use exact_kernel::ObjectFit;
use exact_raster::PixelSize;

/// Pixels kept beyond the box's edge on each side: a painter's filter reads
/// a neighbour of the last pixel that shows.
const MARGIN: u32 = 2;

/// The part of a picture of `natural` size, decoded whole at `full`, that a
/// box of `shown` (device pixels) shows under `fit`: where it starts in the
/// whole decode and its size. `None` for the whole picture: a fit that does
/// not clip to the box (`contain`, `fill`, `scale-down`) or that this does
/// not cut (`none`), a box with no size, and a part that saves less than an
/// eighth of the pixels.
pub(super) fn part(
    natural: (u32, u32),
    full: PixelSize,
    shown: (f32, f32),
    fit: ObjectFit,
) -> Option<(u32, u32, (u32, u32))> {
    let (nw, nh) = (natural.0 as f32, natural.1 as f32);
    let sized = |v: f32| v.is_finite() && v > 0.0;
    if fit != ObjectFit::Cover
        || !sized(shown.0)
        || !sized(shown.1)
        || natural.0 == 0
        || natural.1 == 0
    {
        return None;
    }
    // The picture as drawn (`paint::object_fit`): scaled to cover, centred.
    let scale = (shown.0 / nw).max(shown.1 / nh);
    let (dw, dh) = (nw * scale, nh * scale);
    // One axis: the share of the picture the box shows, as pixels of the
    // whole decode, widened to whole pixels and by the margin.
    let axis = |box_len: f32, drawn: f32, pixels: u32| {
        let share = (box_len / drawn).min(1.0);
        let from = pixels as f32 * (1.0 - share) / 2.0;
        let to = pixels as f32 * (1.0 + share) / 2.0;
        let from = (from.floor() as u32).saturating_sub(MARGIN);
        let to = (to.ceil() as u32).saturating_add(MARGIN).min(pixels);
        (from.min(to), to - from.min(to))
    };
    let (x, w) = axis(shown.0, dw, full.width);
    let (y, h) = axis(shown.1, dh, full.height);
    let (part, whole) = (
        u64::from(w) * u64::from(h),
        u64::from(full.width) * u64::from(full.height),
    );
    (w > 0 && h > 0 && part * 8 <= whole * 7).then_some((x, y, (w, h)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn size(width: u32, height: u32) -> PixelSize {
        PixelSize { width, height }
    }

    /// A 16:9 photo in a 4:3 box: its height whole, the middle three
    /// quarters of its width, and the margin.
    #[test]
    fn cover_keeps_the_part_the_box_shows() {
        // 1600×900 decoded at 1344×756 for a 1002×752 box (756 high covers).
        let (x, y, (w, h)) = part(
            (1600, 900),
            size(1344, 756),
            (1002.0, 752.0),
            ObjectFit::Cover,
        )
        .unwrap();
        // Drawn 1336.9 wide: the box shows 1002/1336.9 of 1344 pixels = 1007.3,
        // from 168.3 to 1175.7: 168..1176, and two more each side.
        assert_eq!((x, w), (166, 1012));
        // Height: the box shows 752/752 of it: all 756 rows.
        assert_eq!((y, h), (0, 756));
        // A portrait photo in a landscape box is cut along the other axis.
        let (x, y, (w, h)) = part(
            (1200, 1600),
            size(900, 1200),
            (900.0, 500.0),
            ObjectFit::Cover,
        )
        .unwrap();
        assert_eq!((x, w), (0, 900));
        assert_eq!((y, h), (348, 504));
    }

    /// The other fits draw the whole picture (or are not cut here).
    #[test]
    fn only_cover_is_cut() {
        for fit in [
            ObjectFit::Contain,
            ObjectFit::Fill,
            ObjectFit::ScaleDown,
            ObjectFit::None,
        ] {
            assert_eq!(
                part((1600, 900), size(1344, 756), (1002.0, 752.0), fit),
                None,
                "{fit:?}"
            );
        }
    }

    /// A box of the picture's own shape shows all of it, and one that shows
    /// nearly all of it is not worth a second kind of decode.
    #[test]
    fn a_part_that_saves_little_is_the_whole() {
        assert_eq!(
            part(
                (1600, 900),
                size(800, 450),
                (800.0, 450.0),
                ObjectFit::Cover
            ),
            None
        );
        assert_eq!(
            part(
                (1600, 900),
                size(800, 450),
                (760.0, 450.0),
                ObjectFit::Cover
            ),
            None,
            "95% of it"
        );
        assert!(
            part(
                (1600, 900),
                size(800, 450),
                (600.0, 450.0),
                ObjectFit::Cover
            )
            .is_some(),
            "three quarters"
        );
    }

    /// The part never leaves the decode: at the edges the margin is cut
    /// short, and a part is whole pixels that contain what shows.
    #[test]
    fn the_part_is_whole_pixels_inside_the_decode() {
        for (natural, full, shown) in [
            ((1600u32, 900u32), size(1600, 900), (333.3f32, 899.9f32)),
            ((1600, 900), size(801, 451), (10.0, 451.0)),
            ((900, 1600), size(450, 800), (450.0, 3.0)),
            ((7, 5), size(7, 5), (2.0, 5.0)),
            ((4000, 3000), size(1000, 750), (999.5, 100.25)),
        ] {
            let Some((x, y, (w, h))) = part(natural, full, shown, ObjectFit::Cover) else {
                continue;
            };
            assert!(
                x + w <= full.width && y + h <= full.height,
                "{natural:?} {full:?} {shown:?}"
            );
            // What shows, in the whole decode's pixels, is inside the part.
            let scale = (shown.0 / natural.0 as f32).max(shown.1 / natural.1 as f32);
            let (dw, dh) = (natural.0 as f32 * scale, natural.1 as f32 * scale);
            let (left, right) = (
                full.width as f32 * (1.0 - shown.0 / dw) / 2.0,
                full.width as f32 * (1.0 + shown.0 / dw) / 2.0,
            );
            let (top, bottom) = (
                full.height as f32 * (1.0 - shown.1 / dh) / 2.0,
                full.height as f32 * (1.0 + shown.1 / dh) / 2.0,
            );
            assert!(x as f32 <= left.max(0.0) && (x + w) as f32 >= right.min(full.width as f32));
            assert!(y as f32 <= top.max(0.0) && (y + h) as f32 >= bottom.min(full.height as f32));
        }
    }

    /// A box larger than the picture is a picture drawn larger: the same
    /// shares of it show. A box with no size, or a picture with none, has no
    /// part.
    #[test]
    fn a_larger_box_and_an_empty_one() {
        let small = part(
            (400, 300),
            size(400, 300),
            (1000.0, 400.0),
            ObjectFit::Cover,
        );
        let same = part((400, 300), size(400, 300), (500.0, 200.0), ObjectFit::Cover);
        assert_eq!(small, same);
        assert!(small.is_some_and(|(x, y, (w, h))| x == 0 && w == 400 && y > 0 && y + h < 300));
        for shown in [
            (0.0, 10.0),
            (10.0, 0.0),
            (f32::NAN, 10.0),
            (f32::INFINITY, 10.0),
            (-4.0, 10.0),
        ] {
            assert_eq!(
                part((400, 300), size(400, 300), shown, ObjectFit::Cover),
                None,
                "{shown:?}"
            );
        }
        assert_eq!(
            part((0, 300), size(400, 300), (10.0, 10.0), ObjectFit::Cover),
            None
        );
    }
}
