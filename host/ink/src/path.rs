//! Device-space coverage preserves tiny-skia's fractional edges and clip order.
//! @ref LLP 1149 D1.2, D2.3.
use crate::atlas::{Atlas, Entry};
use crate::device::{Device, Error, Result};
use tiny_skia::{FillRule, Mask, Path, PathSegment, Transform};

#[derive(Clone, Copy)]
pub struct Coverage {
    pub rect: [f32; 4],
    pub entry: Entry,
}

pub fn coverage(
    atlas: &mut Atlas,
    device: &mut impl Device,
    region: &Path,
    rule: FillRule,
    clips: &[(&Path, FillRule)],
    transform: Transform,
    viewport: [u32; 2],
) -> Result<Option<Coverage>> {
    if !transform.is_finite() || transform.invert().is_none() {
        return Ok(None);
    }
    let Some(transformed) = region.clone().transform(transform) else {
        return Ok(None);
    };
    let bounds = transformed.bounds();
    let x = bounds.left().floor().max(0.).min(viewport[0] as f32);
    let y = bounds.top().floor().max(0.).min(viewport[1] as f32);
    let right = bounds.right().ceil().max(0.).min(viewport[0] as f32);
    let bottom = bounds.bottom().ceil().max(0.).min(viewport[1] as f32);
    if right <= x || bottom <= y {
        return Ok(None);
    }
    let size = [(right - x) as u32, (bottom - y) as u32];
    let rect = [x, y, right - x, bottom - y];
    let mut key = vec![viewport[0], viewport[1]];
    key.extend(
        [
            transform.sx,
            transform.ky,
            transform.kx,
            transform.sy,
            transform.tx,
            transform.ty,
        ]
        .map(f32::to_bits),
    );
    path_key(&mut key, region, rule);
    for &(path, rule) in clips {
        path_key(&mut key, path, rule);
    }
    if let Some(entry) = atlas.get(&key) {
        return Ok(Some(Coverage { rect, entry }));
    }
    let ts = Transform::from_translate(-x, -y).pre_concat(transform);
    let new_mask =
        || Mask::new(size[0], size[1]).ok_or_else(|| Error::Invalid("path mask allocation".into()));
    let mut clip: Option<Mask> = None;
    for &(path, rule) in clips {
        let mut next = new_mask()?;
        next.fill_path(path, rule, true, ts);
        if let Some(previous) = clip.as_ref() {
            for (a, b) in next.data_mut().iter_mut().zip(previous.data()) {
                *a = (u16::from(*a) * u16::from(*b) / 255) as u8;
            }
        }
        clip = Some(next);
    }
    let mut mask = new_mask()?;
    mask.fill_path(region, rule, true, ts);
    if let Some(clip) = clip {
        for (a, b) in mask.data_mut().iter_mut().zip(clip.data()) {
            *a = ((u16::from(*a) * u16::from(*b) + 127) / 255) as u8;
        }
    }
    let entry = atlas.insert(device, key, size, mask.data())?;
    Ok(Some(Coverage { rect, entry }))
}

fn path_key(key: &mut Vec<u32>, path: &Path, rule: FillRule) {
    key.push(u32::from(rule == FillRule::EvenOdd));
    key.push(path.len() as u32);
    for segment in path.segments() {
        match segment {
            PathSegment::MoveTo(p) => key.extend([0, p.x.to_bits(), p.y.to_bits()]),
            PathSegment::LineTo(p) => key.extend([1, p.x.to_bits(), p.y.to_bits()]),
            PathSegment::QuadTo(a, b) => key.extend([
                2,
                a.x.to_bits(),
                a.y.to_bits(),
                b.x.to_bits(),
                b.y.to_bits(),
            ]),
            PathSegment::CubicTo(a, b, c) => key.extend([
                3,
                a.x.to_bits(),
                a.y.to_bits(),
                b.x.to_bits(),
                b.y.to_bits(),
                c.x.to_bits(),
                c.y.to_bits(),
            ]),
            PathSegment::Close => key.push(4),
        }
    }
}
