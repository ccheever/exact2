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
    viewport: Option<[u32; 2]>,
) -> Result<Option<Coverage>> {
    if !transform.is_finite() || transform.invert().is_none() {
        return Ok(None);
    }
    let Some(transformed) = region.clone().transform(transform) else {
        return Ok(None);
    };
    let bounds = transformed.bounds();
    let bound = |v: f32, axis: usize| viewport.map_or(v, |s| v.clamp(0., s[axis] as f32));
    let x = bound(bounds.left().floor(), 0);
    let y = bound(bounds.top().floor(), 1);
    let right = bound(bounds.right().ceil(), 0);
    let bottom = bound(bounds.bottom().ceil(), 1);
    if right <= x || bottom <= y {
        return Ok(None);
    }
    let size = [(right - x) as u32, (bottom - y) as u32];
    if size.iter().any(|s| *s > atlas.max_entry_dimension(device)) {
        return Err(Error::Invalid(
            "coverage exceeds device texture limit".into(),
        ));
    }
    let rect = [x, y, right - x, bottom - y];
    let mut key = viewport.unwrap_or([u32::MAX; 2]).to_vec();
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

/// Each group is a union; successive groups intersect (an SVG clip's `then`).
pub fn clip_union(
    atlas: &mut Atlas,
    device: &mut impl Device,
    groups: &[Vec<(Path, FillRule)>],
    transform: Transform,
    viewport: Option<[u32; 2]>,
) -> Result<Coverage> {
    let mut key = vec![u32::MAX];
    key.extend(viewport.unwrap_or([u32::MAX; 2]));
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
    let mut bounds = [
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    ];
    for (n, group) in groups.iter().enumerate() {
        key.push(group.len() as u32);
        for (path, rule) in group {
            path_key(&mut key, path, *rule);
            if n == 0 {
                if let Some(p) = path.clone().transform(transform) {
                    let b = p.bounds();
                    bounds[0] = bounds[0].min(b.left());
                    bounds[1] = bounds[1].min(b.top());
                    bounds[2] = bounds[2].max(b.right());
                    bounds[3] = bounds[3].max(b.bottom());
                }
            }
        }
    }
    if !bounds.iter().all(|v| v.is_finite()) {
        bounds = [0.; 4];
    }
    let bound = |v: f32, axis: usize| viewport.map_or(v, |s| v.clamp(0., s[axis] as f32));
    let x = bound(bounds[0].floor(), 0);
    let y = bound(bounds[1].floor(), 1);
    let w = (bound(bounds[2].ceil(), 0) - x).max(1.) as u32;
    let h = (bound(bounds[3].ceil(), 1) - y).max(1.) as u32;
    if w > atlas.max_entry_dimension(device) || h > atlas.max_entry_dimension(device) {
        return Err(Error::Invalid(
            "coverage clip exceeds device texture limit".into(),
        ));
    }
    let rect = [x, y, w as f32, h as f32];
    if let Some(entry) = atlas.get(&key) {
        return Ok(Coverage { rect, entry });
    }
    let ts = Transform::from_translate(-x, -y).pre_concat(transform);
    let mut pixels = vec![255u8; w as usize * h as usize];
    for group in groups {
        let mut mask = Mask::new(w, h).ok_or_else(|| Error::Invalid("clip allocation".into()))?;
        for (path, rule) in group {
            mask.fill_path(path, *rule, true, ts);
        }
        for (a, b) in pixels.iter_mut().zip(mask.data()) {
            *a = ((u16::from(*a) * u16::from(*b) + 127) / 255) as u8;
        }
    }
    let entry = atlas.insert(device, key, [w, h], &pixels)?;
    Ok(Coverage { rect, entry })
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[test]
fn large_coverage_survives_repeated_frames_and_retires_when_idle() {
    let mut device = crate::Wgpu::new().unwrap();
    let mut atlas = Atlas::paths();
    let path =
        tiny_skia::PathBuilder::from_rect(tiny_skia::Rect::from_xywh(0., 0., 600., 520.).unwrap());
    let mut texture = None;
    for frame in 0..3 {
        atlas.begin(&mut device).unwrap();
        let mask = coverage(
            &mut atlas,
            &mut device,
            &path,
            FillRule::Winding,
            &[],
            Transform::identity(),
            Some([600, 520]),
        )
        .unwrap()
        .unwrap();
        if frame > 0 {
            assert_eq!(Some(mask.entry.texture), texture);
            assert_eq!(atlas.stats().uploaded_bytes, 0);
        }
        texture = Some(mask.entry.texture);
    }
    for _ in 0..121 {
        atlas.begin(&mut device).unwrap();
    }
    assert_eq!(atlas.stats().overflow_bytes, 0);
}
