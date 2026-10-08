//! Triangle downsample bit-identical to `image` 0.25's `DynamicImage::resize(.., Triangle)` for an
//! RGB8 source, then `to_rgba8()`: the same weights and the same per-channel f32 sums in the same
//! order, walked row by row instead of column by column.

/// `(left, weights)` per output index, exactly as `image::imageops::sample` computes them.
fn taps(len: u32, nlen: u32) -> Vec<(usize, Vec<f32>)> {
    let ratio = len as f32 / nlen as f32;
    let sratio = if ratio < 1.0 { 1.0 } else { ratio };
    let support = 1.0f32 * sratio;
    (0..nlen)
        .map(|o| {
            let input = (o as f32 + 0.5) * ratio;
            let left = ((input - support).floor() as i64).clamp(0, len as i64 - 1);
            let right = ((input + support).ceil() as i64).clamp(left + 1, len as i64);
            let input = input - 0.5;
            let mut sum = 0.0f32;
            let mut ws: Vec<f32> = (left..right)
                .map(|i| {
                    let x = ((i as f32 - input) / sratio).abs();
                    let w = if x < 1.0 { 1.0 - x } else { 0.0 };
                    sum += w;
                    w
                })
                .collect();
            for w in ws.iter_mut() {
                *w /= sum;
            }
            (left as usize, ws)
        })
        .collect()
}

/// `(rgba, width, height)` of `img` fitted within `max`×`max`, as `image` would.
pub fn fit_rgba8(img: &image::DynamicImage, max: u32) -> (Vec<u8>, u32, u32) {
    let (w, h) = (img.width(), img.height());
    let image::DynamicImage::ImageRgb8(rgb) = img else {
        let r = if w.max(h) > max { img.resize(max, max, image::imageops::FilterType::Triangle) } else { img.clone() };
        let r = r.to_rgba8();
        let (w, h) = r.dimensions();
        return (r.into_raw(), w, h);
    };
    if w.max(h) <= max {
        let r = img.to_rgba8();
        return (r.into_raw(), w, h);
    }
    // image's resize_dimensions(w, h, max, max, fill = false)
    let ratio = f64::min(f64::from(max) / f64::from(w), f64::from(max) / f64::from(h));
    let nw = ((f64::from(w) * ratio).round() as u32).max(1);
    let nh = ((f64::from(h) * ratio).round() as u32).max(1);
    let src = rgb.as_raw();
    let (w, h) = (w as usize, h as usize);
    // Vertical first (image's order): nh rows of w RGB f32 sums.
    let row = w * 3;
    let mut tmp = vec![0.0f32; row * nh as usize];
    let vt = taps(h as u32, nh);
    for (oy, (left, ws)) in vt.iter().enumerate() {
        let acc = &mut tmp[oy * row..(oy + 1) * row];
        for (k, &wt) in ws.iter().enumerate() {
            let s = &src[(left + k) * row..(left + k + 1) * row];
            for (a, &v) in acc.iter_mut().zip(s) {
                *a += v as f32 * wt;
            }
        }
    }
    let ht = taps(w as u32, nw);
    let mut out = vec![255u8; nw as usize * nh as usize * 4];
    let n = ht.iter().map(|(_, ws)| ws.len()).max().unwrap_or(1);
    if n <= 4 && w >= 4 {
        // Every output's taps padded to four with trailing zero weights: each sum is non-negative,
        // and adding `v * 0.0` to it is exact, so the padded sums equal image's.
        let fixed: Vec<(usize, [f32; 4])> = ht
            .iter()
            .map(|(left, ws)| {
                let mut f = [0.0f32; 4];
                f[..ws.len()].copy_from_slice(ws);
                ((*left).min(w - 4), f)
            })
            .collect();
        // A left clamped back (the last outputs) moves its weights right to stay on their pixels.
        let fixed: Vec<(usize, [f32; 4])> = fixed
            .into_iter()
            .zip(&ht)
            .map(|((l, f), (left, ws))| {
                if l == *left { (l, f) } else {
                    let mut g = [0.0f32; 4];
                    let d = left - l;
                    g[d..d + ws.len()].copy_from_slice(ws);
                    (l, g)
                }
            })
            .collect();
        for oy in 0..nh as usize {
            let t = &tmp[oy * row..(oy + 1) * row];
            let o = &mut out[oy * nw as usize * 4..(oy + 1) * nw as usize * 4];
            for ((left, wt), px) in fixed.iter().zip(o.chunks_exact_mut(4)) {
                let p = &t[left * 3..left * 3 + 12];
                let (mut r, mut g, mut b) = (0.0f32, 0.0f32, 0.0f32);
                for k in 0..4 {
                    r += p[k * 3] * wt[k];
                    g += p[k * 3 + 1] * wt[k];
                    b += p[k * 3 + 2] * wt[k];
                }
                px[0] = r.clamp(0.0, 255.0).round() as u8;
                px[1] = g.clamp(0.0, 255.0).round() as u8;
                px[2] = b.clamp(0.0, 255.0).round() as u8;
            }
        }
    } else {
    for oy in 0..nh as usize {
        let t = &tmp[oy * row..(oy + 1) * row];
        let o = &mut out[oy * nw as usize * 4..(oy + 1) * nw as usize * 4];
        for (ox, (left, ws)) in ht.iter().enumerate() {
            let (mut r, mut g, mut b) = (0.0f32, 0.0f32, 0.0f32);
            for (k, &wt) in ws.iter().enumerate() {
                let p = &t[(left + k) * 3..(left + k) * 3 + 3];
                r += p[0] * wt;
                g += p[1] * wt;
                b += p[2] * wt;
            }
            let q = |v: f32| v.clamp(0.0, 255.0).round() as u8;
            o[ox * 4] = q(r);
            o[ox * 4 + 1] = q(g);
            o[ox * 4 + 2] = q(b);
        }
    }
    }
    (out, nw, nh)
}
