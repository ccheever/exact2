//! Transparent rows contribute nothing to an opacity group's source-over blend.
use tiny_skia::{Pixmap, PixmapPaint, PixmapRef, Transform};

pub(super) fn composite(below: &mut Pixmap, layer: &Pixmap, opacity: f32) {
    let stride = layer.width() as usize * 4;
    let mut rows = layer.data().chunks_exact(stride);
    let Some(top) = rows.position(|row| !empty(row)) else {
        return;
    };
    let bottom = rows
        .rposition(|row| !empty(row))
        .map_or(top + 1, |n| top + 2 + n);
    // Integer translation preserves the original nearest-neighbor samples.
    // Inspect actual pixels to cover every paint operation and nested group.
    let source = PixmapRef::from_bytes(
        &layer.data()[top * stride..bottom * stride],
        layer.width(),
        (bottom - top) as u32,
    )
    .expect("complete rows of a valid pixmap");
    below.draw_pixmap(
        0,
        top as i32,
        source,
        &PixmapPaint {
            opacity,
            ..PixmapPaint::default()
        },
        Transform::identity(),
        None,
    );
}

fn empty(row: &[u8]) -> bool {
    let mut blocks = row.chunks_exact(32);
    blocks.all(|block| block == [0; 32]) && blocks.remainder().iter().all(|&byte| byte == 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_skia::Color;

    #[test]
    fn cropped_opacity_is_byte_exact_for_sparse_and_full_layers() {
        for (width, height) in [(1, 1), (7, 19), (8, 20), (31, 27), (65, 33), (17, 8210)] {
            for case in 0..5 {
                let mut layer = Pixmap::new(width, height).unwrap();
                for (i, p) in layer.pixels_mut().iter_mut().enumerate() {
                    let y = i as u32 / width;
                    let draw = match case {
                        0 => false,
                        1 => y == 0,
                        2 => y == height - 1,
                        3 => y == height / 3 || y == height * 2 / 3,
                        _ => true,
                    };
                    if draw {
                        *p = Color::from_rgba8(219, 71, 253, (i * 37) as u8)
                            .premultiply()
                            .to_color_u8();
                    }
                }
                let mut below = Pixmap::new(width, height).unwrap();
                for (i, p) in below.pixels_mut().iter_mut().enumerate() {
                    *p = Color::from_rgba8(11, 252, 139, (i * 31) as u8)
                        .premultiply()
                        .to_color_u8();
                }
                for alpha in 0..=255 {
                    let opacity = alpha as f32 / 255.;
                    let mut expected = below.clone();
                    expected.draw_pixmap(
                        0,
                        0,
                        layer.as_ref(),
                        &PixmapPaint {
                            opacity,
                            ..Default::default()
                        },
                        Transform::identity(),
                        None,
                    );
                    let mut actual = below.clone();
                    composite(&mut actual, &layer, opacity);
                    assert!(
                        actual.data() == expected.data(),
                        "{width}x{height}, case={case}, alpha={alpha}"
                    );
                }
            }
        }
    }
}
