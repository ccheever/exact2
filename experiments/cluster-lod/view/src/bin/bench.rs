//! Same-camera timings and two independent image metrics, without a fail-fast row loop.
use super::{Options, Result, readback, sample};
use clod_format::Reader;
use clod_view::{Renderer, View, scene::Scene};
use serde_json::json;

pub fn run(
    cluster: &mut Renderer,
    naive: &mut Renderer,
    reader: &Reader<'_>,
    scene: &Scene,
    o: &Options,
) -> Result<()> {
    let mut failures = Vec::new();
    let mut rows = 0;
    for &t in &o.times {
        for &threshold in &o.thresholds {
            let result = (|| -> Result<()> {
                let mut samples = [Vec::new(), Vec::new()];
                let mut pixels = [Vec::new(), Vec::new()];
                for (index, renderer) in [&mut *cluster, &mut *naive].into_iter().enumerate() {
                    for frame in 0..=o.frames {
                        let s = sample(renderer, reader, scene, o, t, threshold)?;
                        if s.report["overflow"] != 0 || s.report["shadow_overflow"] != 0 {
                            return Err(format!("overflow in mode {index}"));
                        }
                        if frame > 0 {
                            samples[index].push(s.report);
                        }
                        pixels[index] = s.pixels;
                    }
                }
                let error = readback::difference(&pixels[0], &pixels[1]);
                let mut histogram = [0usize; 256];
                for (a, b) in pixels[0].chunks_exact(4).zip(pixels[1].chunks_exact(4)) {
                    histogram[(0..3).map(|i| a[i].abs_diff(b[i])).max().unwrap() as usize] += 1;
                }
                let n = (o.width as usize) * (o.height as usize);
                let rank = (n * 999).div_ceil(1000);
                let mut cumulative = 0;
                let p999 = histogram
                    .iter()
                    .position(|count| {
                        cumulative += count;
                        cumulative >= rank
                    })
                    .unwrap();
                let mut coverage = o.clone();
                coverage.view = View::Coverage;
                let c = sample(cluster, reader, scene, &coverage, t, threshold)?;
                let n = sample(naive, reader, scene, &coverage, t, threshold)?;
                let (interior, cracks) =
                    coverage_cracks(&c.pixels, &n.pixels, o.width as usize, o.height as usize);
                let median = |index: usize, key: &str| -> Result<f64> {
                    let mut values = samples[index]
                        .iter()
                        .map(|s| s[key].as_f64().ok_or_else(|| format!("missing {key}")))
                        .collect::<Result<Vec<_>>>()?;
                    values.sort_by(f64::total_cmp);
                    Ok(values[values.len() / 2])
                };
                let main = median(0, "gpu_main_ms")?;
                let baseline = median(1, "gpu_main_ms")?;
                println!(
                    "{}",
                    json!({"command":"bench","asset":o.file.file_stem().map(|s|s.to_string_lossy()),"layout":o.layout,"size":[o.width,o.height],"t":t,"threshold_px":threshold,"measured_frames":o.frames,"warmup_frames":1,"source_triangles":reader.header.source_triangles,"instances":scene.instances.len(),"source_triangles_times_instances":reader.header.source_triangles as u64*scene.instances.len() as u64,"triangles_drawn":samples[0][0]["triangles_drawn"],"naive_triangles_drawn":samples[1][0]["triangles_drawn"],"gpu_main_ms":main,"gpu_shadow_ms":median(0,"gpu_shadow_ms")?,"gpu_select_ms":median(0,"gpu_select_ms")?,"gpu_select_main_ms":median(0,"gpu_select_main_ms")?,"gpu_select_shadow_ms":median(0,"gpu_select_shadow_ms")?,"gpu_ms":median(0,"gpu_ms")?,"cpu_ms":median(0,"cpu_ms")?,"naive_gpu_main_ms":baseline,"ratio":baseline/main,"bytes_resident_cluster":samples[0][0]["bytes_resident_gpu"],"bytes_resident_naive":samples[1][0]["bytes_resident_gpu"],"image_mean":error.mean,"image_p999":p999 as f64/255.0,"coverage_interior_pixels":interior,"coverage_cracks":cracks,"overflow":0})
                );
                rows += 1;
                Ok(())
            })();
            if let Err(e) = result {
                failures.push(format!("t={t} px={threshold}: {e}"));
            }
        }
    }
    println!(
        "{}",
        json!({"command":"bench_summary","rows":rows,"failures":failures})
    );
    if rows != o.times.len() * o.thresholds.len() || !failures.is_empty() {
        return Err(format!("bench: {rows} rows, {} failures", failures.len()));
    }
    Ok(())
}
fn coverage_cracks(cluster: &[u8], naive: &[u8], width: usize, height: usize) -> (usize, usize) {
    let mut interior = 0;
    let mut cracks = 0;
    for y in 1..height - 1 {
        for x in 1..width - 1 {
            if (y - 1..=y + 1)
                .all(|r| (x - 1..=x + 1).all(|c| naive[(r * width + c) * 4 + 1] == 255))
            {
                interior += 1;
                cracks += usize::from(cluster[(y * width + x) * 4 + 1] == 0);
            }
        }
    }
    (interior, cracks)
}
#[cfg(test)]
mod tests {
    #[test]
    fn rectangular_coverage_counts_one_pixel_cracks() {
        let naive = vec![255; 8 * 5 * 4];
        let mut cracked = naive.clone();
        cracked[(2 * 8 + 4) * 4 + 1] = 0;
        let cases = [
            (super::coverage_cracks(&naive, &naive, 8, 5), (18, 0)),
            (super::coverage_cracks(&cracked, &naive, 8, 5), (18, 1)),
        ];
        let failures: Vec<_> = cases
            .iter()
            .filter(|(actual, expected)| actual != expected)
            .collect();
        println!(
            "coverage_metric_cases={} results={cases:?} failures={}",
            cases.len(),
            failures.len()
        );
        assert!(failures.is_empty());
    }
}
