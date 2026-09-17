use super::*;

// Luminance is measured after ACES and the output transfer, in normalized bytes.
fn luminance(p: &fixture::Pixels, x: u32, y: u32) -> f32 {
    let c = p.at(x, y);
    (0.2126 * c[0] as f32 + 0.7152 * c[1] as f32 + 0.0722 * c[2] as f32) / 255.0
}

fn ground(f: &FrameInput<'_>, w: u32, h: u32) -> Vec<Option<Vec3>> {
    let inverse = (f.proj * f.view).inverse();
    (0..w * h)
        .map(|i| {
            let ndc = Vec3::new(
                (i % w) as f32 / w as f32 * 2.0 + 1.0 / w as f32 - 1.0,
                1.0 - (i / w) as f32 / h as f32 * 2.0 - 1.0 / h as f32,
                0.0,
            );
            let a = inverse.project_point3(ndc);
            let d = inverse.project_point3(ndc + Vec3::Z * 0.5) - a;
            let t = -a.y / d.y;
            (t > 0.0).then_some(a + d * t)
        })
        .collect()
}

fn clean_patch(p: Vec3) -> bool {
    p.x.abs() < 19.0 && p.z.abs() < 19.0 && (p.x.abs() > 7.0 || p.z.abs() > 7.0)
}

fn statistics(p: &fixture::Pixels, mask: &[bool]) -> (f32, usize) {
    let mut sum = 0.0;
    let mut count = 0;
    for y in 2..p.height - 2 {
        for x in 2..p.width - 2 {
            let mut mean = 0.0;
            let mut valid = true;
            for yy in y - 2..=y + 2 {
                for xx in x - 2..=x + 2 {
                    valid &= mask[(yy * p.width + xx) as usize];
                    mean += luminance(p, xx, yy);
                }
            }
            if valid {
                sum += (luminance(p, x, y) - mean / 25.0).abs();
                count += 1;
            }
        }
    }
    (sum / count as f32, count)
}

pub(crate) fn scene(gpu: &Gpu) -> (Renderer, FrameInput<'static>) {
    let (mut r, mut f, _) = shadow_scene(gpu, 0.0);
    r.write_transforms_both(
        0,
        &transform(Vec3::ZERO, Quat::IDENTITY, Vec3::new(40.0, 1.0, 40.0)),
    )
    .unwrap();
    let mut matte = material([0.5; 3], 0.0);
    matte[5] = 1.0;
    r.write_materials(0, &matte.repeat(2)).unwrap();
    f.proj = directx::perspective(60f32.to_radians(), 16.0 / 9.0, 0.1, 100.0);
    f.sun.as_mut().unwrap().illuminance = 3.0;
    (r, f)
}

#[test]
fn shadow_image_statistics_sun_camera_matrix() {
    let Some(gpu) = gpu() else {
        return;
    };
    let (mut r, mut f) = scene(&gpu);
    let texture = target(&gpu, (1280, 720), wgpu::TextureFormat::Rgba8Unorm);
    let contact_target = target(&gpu, (256, 8192), wgpu::TextureFormat::Rgba8Unorm);
    let mut failures = Vec::new();
    for height in [9.0, 1.5] {
        f.camera_position = Vec3::new(0.0, height, 13.0);
        let aim = if height == 9.0 {
            Vec3::ZERO
        } else {
            Vec3::new(0.0, height, -20.0)
        };
        f.view = view::look_at_mat4(f.camera_position, aim, Vec3::Y);
        let positions = ground(&f, 1280, 720);
        let mask: Vec<_> = positions
            .iter()
            .map(|p| p.is_some_and(clean_patch))
            .collect();
        // A tall readback magnifies vertical ground resolution without changing
        // projection or cascade fits: the grazing view otherwise cannot resolve 3 cm.
        let contact_positions = ground(&f, 256, 8192);
        for elevation in [12f32, 30.0, 55.0, 80.0] {
            for azimuth in [35f32, 145.0] {
                let (se, ce) = elevation.to_radians().sin_cos();
                let (sa, ca) = azimuth.to_radians().sin_cos();
                f.sun.as_mut().unwrap().direction = Vec3::new(ce * ca, -se, ce * sa);
                f.sun.as_mut().unwrap().shadows = Some(Shadows::default());
                let shadow = render(&gpu, &mut r, &texture, &f);
                let name = format!("quality-h{height}-e{elevation}-a{azimuth}");
                shadow.save(&name);
                f.sun.as_mut().unwrap().shadows = None;
                let clean = render(&gpu, &mut r, &texture, &f);
                clean.save(&format!("{name}-clean"));
                let (mad, count) = statistics(&shadow, &mask);
                let (reference, _) = statistics(&clean, &mask);
                assert!(count > 10000, "empty lit-ground ROI");
                let lit_error = mask
                    .iter()
                    .enumerate()
                    .filter(|(_, valid)| **valid)
                    .map(|(k, _)| {
                        let (x, y) = (k as u32 % 1280, k as u32 / 1280);
                        (luminance(&shadow, x, y) - luminance(&clean, x, y)).abs()
                    })
                    .fold(0.0f32, f32::max);
                if lit_error > 2.0 / 255.0 {
                    failures.push(format!(
                        "{name}: lit ground differs from clean by {lit_error:.6}"
                    ));
                }

                // Clean images measured on Metal are below 0.0003; 0.001 leaves
                // quantization headroom but rejects the original concentric acne.
                if mad > 0.001 {
                    failures.push(format!("{name}: banding {mad:.6}"));
                }
                // Compare adjacent pixels straddling either end of each overlap.
                // Subtract the clean image's smooth BRDF gradient first.
                let splits: Vec<_> = (1..=3)
                    .map(|i| {
                        let t = i as f32 / 3.0;
                        0.7 * 0.1 * 600f32.powf(t) + 0.3 * (0.1 + 59.9 * t)
                    })
                    .collect();
                let mut step = 0f32;
                let mut crossings = 0;
                for (i, &end) in splits.iter().enumerate() {
                    let start = if i == 0 { 0.1 } else { splits[i - 1] };
                    for boundary in [end - (end - start) * 0.1, end] {
                        for y in 2..718 {
                            for x in 2..1278 {
                                let k = (y * 1280 + x) as usize;
                                if !mask[k] || !mask[k + 1280] {
                                    continue;
                                }
                                let depth = |p| -(f.view.transform_point3(p)).z;
                                let a = depth(positions[k].unwrap());
                                let b = depth(positions[k + 1280].unwrap());
                                if (a - boundary) * (b - boundary) > 0.0 {
                                    continue;
                                }
                                let residual = |y| {
                                    (luminance(&shadow, x, y) - luminance(&clean, x, y))
                                        / luminance(&clean, x, y).max(0.01)
                                };
                                step = step.max((residual(y) - residual(y + 1)).abs());
                                crossings += 1;
                            }
                        }
                    }
                }
                assert!(crossings > 100, "no cascade boundary coverage");
                if step > 0.02 {
                    failures.push(format!("{name}: cascade step {step:.6}"));
                }
                f.sun.as_mut().unwrap().shadows = Some(Shadows::default());
                let contact = render(&gpu, &mut r, &contact_target, &f);
                f.sun.as_mut().unwrap().shadows = None;
                let contact_lit = render(&gpu, &mut r, &contact_target, &f);
                let expected = tone(0.05) as f32 / 255.0;
                let mut gap = f32::INFINITY;
                let mut interior_error = f32::INFINITY;
                let mut interior_pixels = 0;
                let mut ratio = 1.0f32;
                let mut expected_ratio = 1.0f32;
                for y in 1..8191 {
                    for x in 1..255 {
                        let k = (y * 256 + x) as usize;
                        let Some(p) = contact_positions[k] else {
                            continue;
                        };
                        // Front of the cube is visible to both cameras, and lies
                        // inside the analytic projected shadow for both azimuths.
                        if p.x.abs() > 0.25 || p.z < 0.5 || p.z > 0.58 {
                            continue;
                        }
                        let a = contact_positions[k - 256].unwrap();
                        let b = contact_positions[k + 256].unwrap();
                        let half_pixel = (a.z - b.z).abs() * 0.25;
                        // Exclude MSAA coverage of the cube itself.
                        if p.z - half_pixel <= 0.5 {
                            continue;
                        }
                        let dark = luminance(&contact, x, y);
                        let lit = luminance(&contact_lit, x, y);
                        if dark < expected + (lit - expected) * 0.25 {
                            // Conservative upper bound, including pixel footprint.
                            gap = gap.min(p.z + half_pixel - 0.5);
                        }
                        let error = (dark - expected).abs();
                        if error <= 2.0 / 255.0 {
                            interior_pixels += 1;
                        }
                        if error < interior_error {
                            interior_error = error;
                            ratio = dark / lit;
                            expected_ratio = expected / lit;
                        }
                    }
                }
                if gap > 0.03 {
                    failures.push(format!("{name}: contact gap bound {gap:.5} m"));
                }
                if interior_error > 2.0 / 255.0 || ratio > 0.65 || interior_pixels < 4 {
                    failures.push(format!(
                        "{name}: interior ratio {ratio:.4}, expected {expected_ratio:.4}"
                    ));
                }
                eprintln!("{name}: contact gap <= {gap:.5} m, interior ratio={ratio:.4} expected={expected_ratio:.4}");
                eprintln!("{name}: MAD={mad:.6} clean={reference:.6} step={step:.6} pixels={count} crossings={crossings}");
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn sky_and_far_object_converge_in_height_fog() {
    let Some(gpu) = gpu() else {
        return;
    };
    let (mut r, mut f) = scene(&gpu);
    let (v, i) = shapes::cube();
    let cube = r.add_mesh(&v, &i);
    r.set_batches(&[Batch::new(cube, 1..2)], &[0, 1]).unwrap();
    r.write_transforms_both(
        1,
        &transform(
            Vec3::new(0.0, 1.5, -800.0),
            Quat::IDENTITY,
            Vec3::splat(100.0),
        ),
    )
    .unwrap();
    r.write_materials(1, &material([0.0; 3], 0.01)).unwrap();
    f.camera_position = Vec3::new(0.0, 1.5, 0.0);
    f.view = view::look_at_mat4(f.camera_position, f.camera_position - Vec3::Z, Vec3::Y);
    f.proj = directx::perspective(60f32.to_radians(), 16.0 / 9.0, 0.1, 2000.0);
    f.sun = None;
    let texture = target(&gpu, (1280, 720), wgpu::TextureFormat::Rgba8Unorm);
    for custom in [false, true] {
        f.environment = if custom {
            Environment {
                zenith: [0.1; 3],
                horizon: [0.1; 3],
                ground: [0.1; 3],
                sun_disc: 0.0,
                ..Default::default()
            }
        } else {
            Environment::default()
        };
        let color = if custom {
            [0.5, 0.25, 0.12]
        } else {
            f.environment.horizon
        };
        f.environment.fog = Some(Fog {
            density: 0.02,
            height_falloff: 0.1,
            color: custom.then_some(color),
        });
        let p = render(&gpu, &mut r, &texture, &f);
        p.save(&format!("fog-horizon-custom-{custom}"));
        let object = p.at(640, 360);
        let sky = p.at(760, 360);
        eprintln!(
            "horizon custom={custom}: object={object:?}, sky={sky:?}, expected={:?}",
            color.map(tone)
        );
        for channel in 0..3 {
            assert!(object[channel].abs_diff(sky[channel]) <= 2);
            assert!(sky[channel].abs_diff(tone(color[channel])) <= 2);
        }
        // An upward ray has finite integrated density: don't clamp its height
        // difference to 40 and over-fog the zenith on the 10 km sky segment.
        let ndc = Vec3::new(0.0, 1.0 - 1.0 / 720.0, 0.0);
        let inverse = (f.proj * f.view).inverse();
        let d =
            (inverse.project_point3(ndc + Vec3::Z * 0.5) - inverse.project_point3(ndc)).normalize();
        let transmission = (-0.02 * (-0.15f32).exp() / (0.1 * d.y)).exp();
        let clear = Vec3::from_array(f.environment.horizon)
            .lerp(Vec3::from_array(f.environment.zenith), d.y);
        let expected = Vec3::from_array(color)
            .lerp(clear, transmission)
            .to_array()
            .map(tone);
        for (actual, expected) in p.at(640, 0)[..3].iter().zip(expected) {
            assert!(actual.abs_diff(expected) <= 2);
        }
    }
}

#[test]
fn single_sided_sheet_still_casts() {
    let Some(gpu) = gpu() else {
        return;
    };
    let (mut r, mut f, mut batches) = shadow_scene(&gpu, 5.0);
    // Replace the closed cube with a zero-thickness horizontal sheet.
    batches[1].mesh = batches[0].mesh;
    r.set_batches(&batches, &[0, 1]).unwrap();
    let texture = target(&gpu, (1024, 1024), wgpu::TextureFormat::Rgba8Unorm);
    let shadow = render(&gpu, &mut r, &texture, &f);
    shadow.save("shadow-open-sheet");
    f.sun.as_mut().unwrap().shadows = None;
    let lit = render(&gpu, &mut r, &texture, &f);
    let p = Vec3::new(0.6, 0.0, -4.9);
    assert!(at_world(&lit, &f, p) > at_world(&shadow, &f, p) + 60);
}

#[test]
fn far_cascade_fade_is_clean() {
    let Some(gpu) = gpu() else {
        return;
    };
    let (mut r, mut f) = scene(&gpu);
    // The original 40 m plane ends before the default 60 m fade. Move the same
    // fixture back to exercise the far cascade's largest texels and final fade.
    r.write_transforms_both(
        0,
        &transform(
            Vec3::new(0.0, 0.0, -40.0),
            Quat::IDENTITY,
            Vec3::new(40.0, 1.0, 40.0),
        ),
    )
    .unwrap();
    r.write_transforms_both(
        1,
        &transform(Vec3::new(0.0, 0.5, -40.0), Quat::IDENTITY, Vec3::ONE),
    )
    .unwrap();
    f.camera_position = Vec3::new(0.0, 1.5, 13.0);
    f.view = view::look_at_mat4(f.camera_position, f.camera_position - Vec3::Z, Vec3::Y);
    // More vertical samples to resolve the final fade's several metres.
    let texture = target(&gpu, (1280, 1440), wgpu::TextureFormat::Rgba8Unorm);
    let positions = ground(&f, 1280, 1440);
    let mask: Vec<_> = positions
        .iter()
        .map(|p| p.is_some_and(|p| clean_patch(p + Vec3::Z * 40.0)))
        .collect();
    for elevation in [12f32, 30.0, 55.0, 80.0] {
        for azimuth in [35f32, 110.0] {
            let (se, ce) = elevation.to_radians().sin_cos();
            let (sa, ca) = azimuth.to_radians().sin_cos();
            f.sun.as_mut().unwrap().direction = Vec3::new(ce * ca, -se, ce * sa);
            f.sun.as_mut().unwrap().shadows = Some(Shadows::default());
            let p = render(&gpu, &mut r, &texture, &f);
            p.save(&format!("quality-far-e{elevation}-a{azimuth}"));
            f.sun.as_mut().unwrap().shadows = None;
            let clean = render(&gpu, &mut r, &texture, &f);
            let (mad, count) = statistics(&p, &mask);
            let (reference, _) = statistics(&clean, &mask);
            let mut max_error = 0f32;
            let mut fade_pixels = 0;
            for y in 0..1440 {
                for x in 0..1280 {
                    let k = (y * 1280 + x) as usize;
                    if !mask[k] {
                        continue;
                    }
                    let depth = -f.view.transform_point3(positions[k].unwrap()).z;
                    if !(54.0..62.0).contains(&depth) {
                        continue;
                    }
                    max_error = max_error.max(
                        (luminance(&p, x, y) - luminance(&clean, x, y)).abs()
                            / luminance(&clean, x, y),
                    );
                    fade_pixels += 1;
                }
            }
            eprintln!("far e{elevation} a{azimuth}: MAD={mad:.6} clean={reference:.6} maximum fade error={max_error:.6}, {count} lit pixels, {fade_pixels} fade pixels");
            assert!(count > 1000 && fade_pixels > 1000);
            assert!(mad < 0.001 && max_error < 0.02);
        }
    }
}
