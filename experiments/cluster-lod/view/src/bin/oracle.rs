//! Shared native CLI/integration oracle. Every case reports its counts; failures accumulate.
use super::{readback, selection_readback};
use clod_format::Reader;
use clod_view::{
    Mode, Renderer, View,
    scene::{Camera, Scene},
    select::{self, Selection},
    wgpu,
};
use glam::Vec3;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
type Result<T> = std::result::Result<T, String>;

fn camera(scene: &Scene, step: u32, count: u32, aspect: f32) -> Camera {
    if step < count / 2 {
        return scene.camera(
            step as f32 / (count / 2 - 1) as f32,
            aspect,
            45f32.to_radians(),
        );
    }
    let a = (step - count / 2) as f32 * 2.399963;
    let outward = Vec3::new(a.cos(), a.sin(), 0.12);
    let eye = scene.center + Vec3::new(a.cos() * 0.2, a.sin() * 0.2, 0.3);
    Camera::perspective(
        eye,
        eye + outward,
        aspect,
        65f32.to_radians(),
        0.002,
        scene.radius * 12.0 + 10.0,
    )
}
fn boundary(
    reader: &Reader<'_>,
    scene: &Scene,
    camera: &Camera,
    height: u32,
    threshold: f32,
    pair: [u32; 2],
) -> bool {
    let c = &reader.clusters[pair[0] as usize];
    let m = scene.instances[pair[1] as usize].transform();
    let scale = scene.instances[pair[1] as usize].scale();
    for b in [&c.simplified, &c.refined_bounds] {
        let error = select::projected(b, m, scale, camera, height);
        if (error - threshold).abs() <= threshold.abs().max(1e-30) * 1e-5 {
            return true;
        }
    }
    let center = m.transform_point3(Vec3::from_slice(&c.sphere));
    let radius = c.sphere[3] * scale;
    if camera
        .planes()
        .iter()
        .any(|p| (p.truncate().dot(center) + p.w + radius + 1e-5).abs() <= 1e-5)
    {
        return true;
    }
    if camera.orthographic_span.is_some() {
        return false;
    }
    let axis = m.transform_vector3(Vec3::from_array(c.cone_axis)) / scale;
    let direction = (m.transform_point3(Vec3::from_array(c.cone_apex)) - camera.eye).normalize();
    (direction.dot(axis) - c.cone_cutoff - 1e-5).abs() <= 1e-5
}
fn pair_set(cut: &Selection) -> BTreeSet<[u32; 2]> {
    cut.pages.iter().flatten().copied().collect()
}
fn compare_sets(
    reader: &Reader<'_>,
    scene: &Scene,
    camera: &Camera,
    height: u32,
    threshold: f32,
    a: &Selection,
    b: &Selection,
) -> (usize, usize) {
    let a = pair_set(a);
    let b = pair_set(b);
    let mut near = 0;
    let mut bad = 0;
    for pair in a.symmetric_difference(&b) {
        if boundary(reader, scene, camera, height, threshold, *pair) {
            near += 1;
        } else {
            bad += 1;
        }
    }
    (near, bad)
}
fn edge_counts(
    reader: &Reader<'_>,
    cut: &Selection,
    instance: u32,
) -> (usize, clod_format::oracle::Edges, usize) {
    let mut selected = vec![false; reader.clusters.len()];
    let mut positions = BTreeMap::new();
    let mut triangles = Vec::new();
    for &[cluster, i] in cut.pages.iter().flatten() {
        if i != instance {
            continue;
        }
        selected[cluster as usize] = true;
        let (vertices, indices) = reader.geometry(cluster as usize).expect("validated");
        let ids: Vec<_> = vertices
            .iter()
            .map(|v| {
                let next = positions.len() as u32;
                *positions
                    .entry(v.position.map(f32::to_bits))
                    .or_insert(next)
            })
            .collect();
        triangles.extend(
            indices
                .chunks_exact(3)
                .map(|t| [ids[t[0] as usize], ids[t[1] as usize], ids[t[2] as usize]]),
        );
    }
    let edges = clod_format::oracle::edges(&triangles);
    (
        triangles.len(),
        edges,
        clod_format::oracle::overlaps(reader, &selected),
    )
}
fn truncated(reader: &Reader<'_>, cut: &Selection, instances: usize, quota: usize) -> Selection {
    let mut counts = vec![0usize; instances];
    let mut out = Selection {
        pages: vec![Vec::new(); reader.pages.len()],
        ..Selection::default()
    };
    for (page, pairs) in cut.pages.iter().enumerate() {
        for &pair in pairs {
            let n = &mut counts[pair[1] as usize];
            if *n < quota {
                out.pages[page].push(pair);
                out.clusters += 1;
                let tris = reader.clusters[pair[0] as usize].triangle_count as u64;
                out.triangles += tris;
                out.padded_triangles += reader.header.config.max_triangles as u64 - tris;
            } else {
                out.overflow += 1;
            }
            *n += 1;
        }
    }
    out
}
fn gpu_frame(
    renderer: &mut Renderer,
    reader: &Reader<'_>,
    scene: &Scene,
    camera: &Camera,
    cull: bool,
) -> Result<(Vec<u8>, [Selection; 2])> {
    let frame = renderer.render_gpu(scene, camera, View::Lit, 1.0, cull, false)?;
    let (pixels, _) = readback::read(renderer, &frame)?;
    let cuts = selection_readback::selections(
        renderer,
        reader.pages.len(),
        reader.header.config.max_triangles,
        true,
    )?;
    Ok((pixels, cuts))
}
pub fn run(
    reader: &Reader<'_>,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    size: [u32; 2],
    cameras: u32,
    closed: bool,
) -> Result<()> {
    let mut failures = Vec::new();
    let mut checked = 0;
    let mut pixels_checked = 0u64;
    let mut edge_triangles = 0;
    let mut identical = 0;
    let mut near_total = 0;
    let mut cull_images = 0;
    let mut deterministic = 0;
    let mut overflow_total = 0;
    for layout in ["single", "ring:12", "grid:400"] {
        let scene = Scene::layout(reader, layout)?;
        let create = || {
            Renderer::new(
                device.clone(),
                queue.clone(),
                reader,
                None,
                &scene,
                size,
                Mode::Cluster,
            )
        };
        let mut gpu = create()?;
        // Equality checks require a complete cut; the default performance quota
        // can truncate the topology-preserving scan bakes. Exercise the full core
        // binding budget here, and test quota truncation separately below.
        let quota = (128 * 1024 * 1024 / 8 / scene.instances.len()) as u32;
        gpu.enable_gpu_selection(reader, Some(quota))?;
        println!(
            "{}",
            json!({"oracle":"oracle_capacity","layout":layout,"capacity_per_instance":gpu.gpu_capacity_per_instance()})
        );
        let mut cpu = create()?;
        let light = scene.light_camera();
        let shadow = select::select(
            reader,
            &scene.instances,
            &light,
            clod_view::SHADOW_SIZE,
            2.0,
        );
        for step in 0..cameras {
            let camera = camera(&scene, step, cameras, size[0] as f32 / size[1] as f32);
            let reference = select::select(reader, &scene.instances, &camera, size[1], 1.0);
            let (pixels, cuts) = match gpu_frame(&mut gpu, reader, &scene, &camera, true) {
                Ok(x) => x,
                Err(e) => {
                    failures.push(format!("{layout}/{step}: {e}"));
                    continue;
                }
            };
            checked += 1;
            let (near, bad) =
                compare_sets(reader, &scene, &camera, size[1], 1.0, &reference, &cuts[0]);
            let (shadow_near, shadow_bad) = compare_sets(
                reader,
                &scene,
                &light,
                clod_view::SHADOW_SIZE,
                2.0,
                &shadow,
                &cuts[1],
            );
            near_total += near + shadow_near;
            if bad + shadow_bad > 0 {
                failures.push(format!(
                    "{layout}/{step}: main/shadow set mismatch {bad}/{shadow_bad}"
                ));
            }
            if cuts.iter().any(|c| c.overflow > 0) {
                failures.push(format!(
                    "{layout}/{step}: unexpected overflow {}/{}",
                    cuts[0].overflow, cuts[1].overflow
                ));
            }
            let mut image_bytes = 0;
            // Even a permitted floating-point boundary difference must preserve
            // the image. Open scans cannot use closed-edge topology as a fallback.
            if bad + shadow_bad == 0 {
                let frame = cpu.render(&scene, &camera, View::Lit, &reference, &shadow)?;
                let (expected, _) = readback::read(&cpu, &frame)?;
                image_bytes = pixels.iter().zip(&expected).filter(|(a, b)| a != b).count();
                pixels_checked += size[0] as u64 * size[1] as u64;
                if image_bytes > 0 {
                    failures.push(format!(
                        "{layout}/{step}: CPU/GPU pixels: {image_bytes} changed bytes"
                    ));
                } else {
                    identical += 1;
                }
            }
            let (repeat, repeat_cuts) = gpu_frame(&mut gpu, reader, &scene, &camera, true)?;
            let deterministic_ok = pixels == repeat
                && readback::png_bytes(&pixels, size[0], size[1])?
                    == readback::png_bytes(&repeat, size[0], size[1])?
                && cuts
                    .iter()
                    .zip(&repeat_cuts)
                    .all(|(a, b)| pair_set(a) == pair_set(b));
            if !deterministic_ok {
                failures.push(format!("{layout}/{step}: nondeterminism"));
            } else {
                deterministic += 1;
            }
            let mut cull_bytes = 0;
            let mut shadow_bytes = 0;
            // Even steps include both the hero path and inside-the-grid outward cameras.
            if step % 2 == 0 || near + shadow_near > 0 || (closed && layout == "single") {
                let shadow_on = selection_readback::shadow(&gpu)?;
                let (unculled, unculled_cuts) =
                    gpu_frame(&mut gpu, reader, &scene, &camera, false)?;
                let shadow_off = selection_readback::shadow(&gpu)?;
                if step % 2 == 0 {
                    cull_images += 1;
                    cull_bytes = pixels.iter().zip(&unculled).filter(|(a, b)| a != b).count();
                    shadow_bytes = shadow_on
                        .iter()
                        .zip(&shadow_off)
                        .filter(|(a, b)| a != b)
                        .count();
                    if cull_bytes + shadow_bytes > 0 {
                        failures.push(format!("{layout}/{step}: culling changed {cull_bytes} color and {shadow_bytes} shadow bytes"));
                    }
                }
                if closed && (layout == "single" || near + shadow_near > 0) {
                    // L1's same decoded-edge oracle, with culling disabled.
                    let ref_unculled = select::select_culled(
                        reader,
                        &scene.instances,
                        &camera,
                        size[1],
                        1.0,
                        false,
                    );
                    let affected: BTreeSet<_> = pair_set(&ref_unculled)
                        .symmetric_difference(&pair_set(&unculled_cuts[0]))
                        .map(|p| p[1])
                        .chain((closed && layout == "single").then_some(0))
                        .collect();
                    for instance in affected {
                        let (triangles, edges, overlaps) =
                            edge_counts(reader, &unculled_cuts[0], instance);
                        let (_, cpu_edges, _) = edge_counts(reader, &ref_unculled, instance);
                        let diff = pair_set(&ref_unculled)
                            .symmetric_difference(&pair_set(&unculled_cuts[0]))
                            .count();
                        println!(
                            "edge_diagnostic layout={layout} step={step} gpu_bad={} cpu_bad={} pinches={} worst_incidence={} overlaps={overlaps} differences={diff}",
                            edges.bad,
                            cpu_edges.bad,
                            edges.pinches.len(),
                            edges.max_use
                        );
                        edge_triangles += triangles;
                        if edges.bad > 0 || overlaps > 0 {
                            failures.push(format!("{layout}/{step}/{instance}: decoded GPU cut has {} nonzero edges, {overlaps} ancestor overlaps",edges.bad));
                        }
                    }
                }
            }
            if closed && shadow_near > 0 {
                let (_, full) = gpu_frame(&mut gpu, reader, &scene, &camera, false)?;
                let reference_light = select::select_culled(
                    reader,
                    &scene.instances,
                    &light,
                    clod_view::SHADOW_SIZE,
                    2.0,
                    false,
                );
                let affected: BTreeSet<_> = pair_set(&reference_light)
                    .symmetric_difference(&pair_set(&full[1]))
                    .map(|p| p[1])
                    .collect();
                for instance in affected {
                    let (triangles, edges, overlaps) = edge_counts(reader, &full[1], instance);
                    println!(
                        "{}",
                        json!({"oracle":"shadow_cut_topology","layout":layout,"step":step,"instance":instance,"pinched_edges":edges.pinches.len(),"worst_incidence":edges.max_use,"overlaps":overlaps,"nonzero_winding":edges.bad})
                    );
                    edge_triangles += triangles;
                    if edges.bad > 0 || overlaps > 0 {
                        failures.push(format!(
                            "{layout}/{step}/{instance}: shadow GPU cut has {} nonzero edges, {overlaps} ancestor overlaps",edges.bad
                        ));
                    }
                }
            }
            println!(
                "{}",
                json!({"oracle":"gpu_camera","layout":layout,"camera":step,"visible":cuts[0].clusters,"shadow_visible":cuts[1].clusters,"candidates":cuts[0].candidates,"shadow_candidates":cuts[1].candidates,"near_boundary":near+shadow_near,"bad_set_differences":bad+shadow_bad,"image_changed_bytes":image_bytes,"cull_color_changed_bytes":cull_bytes,"cull_shadow_changed_bytes":shadow_bytes,"deterministic":deterministic_ok})
            );
        }
        if layout == "single" {
            let camera = scene.camera(0.0, size[0] as f32 / size[1] as f32, 45f32.to_radians());
            for threshold in [0.0, f32::MAX / 2.0] {
                let frame = gpu.render_gpu(&scene, &camera, View::Lit, threshold, false, false)?;
                let _ = readback::read(&gpu, &frame)?;
                let actual = selection_readback::selections(
                    &gpu,
                    reader.pages.len(),
                    reader.header.config.max_triangles,
                    true,
                )?;
                let expected = select::select_culled(
                    reader,
                    &scene.instances,
                    &camera,
                    size[1],
                    threshold,
                    false,
                );
                let shadow_expected = select::select_culled(
                    reader,
                    &scene.instances,
                    &light,
                    clod_view::SHADOW_SIZE,
                    (threshold * 2.0).min(f32::MAX / 2.0),
                    false,
                );
                let equal =
                    actual[0].pages == expected.pages && actual[1].pages == shadow_expected.pages;
                if !equal {
                    failures.push(format!("extreme threshold {threshold}: set mismatch"));
                }
                println!(
                    "{}",
                    json!({"oracle":"extreme_threshold", "threshold":threshold, "main_triangles":actual[0].triangles, "shadow_triangles":actual[1].triangles, "equal":equal})
                );
            }
        }
        let camera = scene.camera(0.0, size[0] as f32 / size[1] as f32, 45f32.to_radians());
        let reference = select::select(reader, &scene.instances, &camera, size[1], 1.0);
        let mut tiny = create()?;
        tiny.enable_gpu_selection(reader, Some(1))?;
        let (pixels, cuts) = gpu_frame(&mut tiny, reader, &scene, &camera, true)?;
        let expected = truncated(reader, &reference, scene.instances.len(), 1);
        let expected_shadow = truncated(reader, &shadow, scene.instances.len(), 1);
        let frame = cpu.render(&scene, &camera, View::Lit, &expected, &expected_shadow)?;
        let (expected_pixels, _) = readback::read(&cpu, &frame)?;
        let overflow = cuts[0].overflow + cuts[1].overflow;
        overflow_total += overflow;
        let correct = overflow > 0
            && cuts[0].pages == expected.pages
            && cuts[1].pages == expected_shadow.pages
            && pixels == expected_pixels
            && cuts[0].overflow == expected.overflow
            && cuts[1].overflow == expected_shadow.overflow;
        if !correct {
            failures.push(format!("{layout}: overflow oracle failed"));
        }
        println!(
            "{}",
            json!({"oracle":"overflow","layout":layout,"capacity_per_instance":1,"dropped":overflow,"drawn":cuts[0].clusters+cuts[1].clusters,"matches_selected_subset_and_pixels":correct})
        );
    }
    println!(
        "{}",
        json!({"oracle":"gpu_summary","cameras_per_layout":cameras,"checked":checked,"identical_images":identical,"pixels_checked":pixels_checked,"near_boundary_differences":near_total,"culling_image_pairs":cull_images,"deterministic_png_pairs":deterministic,"edge_triangles":edge_triangles,"overflow_dropped":overflow_total,"failures":failures})
    );
    if checked == 0 {
        failures.push("no cameras checked".into());
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}
