#[path = "bin/options.rs"]
mod options;
#[path = "bin/prepare.rs"]
mod prepare;
#[path = "bin/readback.rs"]
mod readback;
use clod_format::Reader;
use clod_view::{
    Mode, Renderer, View,
    scene::Scene,
    select::{self, Selection},
};
use options::Options;
use serde_json::json;
use std::{path::Path, time::Instant};
type Result<T> = std::result::Result<T, String>;
struct Sample {
    pixels: Vec<u8>,
    report: serde_json::Value,
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{}", json!({"error":error}));
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let options = Options::parse()?;
    let start = Instant::now();
    let bytes = std::fs::read(&options.file).map_err(|e| e.to_string())?;
    let reader = Reader::new(&bytes).map_err(|e| e.to_string())?;
    let scene = Scene::layout(&reader, &options.layout)?;
    let needs_naive =
        options.mode == Mode::Naive || ["compare", "pop"].contains(&options.command.as_str());
    let baseline = needs_naive.then(|| prepare::baseline(&reader));
    eprintln!(
        "{}",
        json!({"stage":"load","seconds":start.elapsed().as_secs_f64(),"source_triangles":reader.header.source_triangles,"pages":reader.pages.len(),"instances":scene.instances.len(),"baseline_chunks":baseline.as_ref().map_or(0,Vec::len),"baseline_vertices":baseline.as_ref().map(|b|b.iter().map(|c|c.vertices.len()).sum::<usize>()),"camera_hero":scene.hero.to_array()})
    );
    let (device, queue, adapter) =
        pollster::block_on(clod_view::request_device(options.command == "time"))?;
    eprintln!(
        "{}",
        json!({"adapter":adapter.name,"backend":format!("{:?}",adapter.backend),"features":format!("{:?}",device.features()),"limits":"wgpu::Limits::default()"})
    );
    let create = |mode| {
        Renderer::new(
            device.clone(),
            queue.clone(),
            &reader,
            baseline.as_deref(),
            &scene,
            [options.width, options.height],
            mode,
        )
    };
    match options.command.as_str() {
        "render" | "time" => {
            let mut renderer = create(options.mode)?;
            let count = if options.command == "time" {
                options.frames
            } else {
                1
            };
            if options.command == "time" {
                let _ = sample(
                    &mut renderer,
                    &reader,
                    &scene,
                    &options,
                    options.times[0],
                    options.thresholds[0],
                )?;
            }
            let mut samples = Vec::new();
            for _ in 0..count {
                samples.push(sample(
                    &mut renderer,
                    &reader,
                    &scene,
                    &options,
                    options.times[0],
                    options.thresholds[0],
                )?);
            }
            let mut report = samples.last().expect("samples").report.clone();
            for key in [
                "selection_ms",
                "shadow_selection_ms",
                "encode_ms",
                "gpu_ms",
                "gpu_main_ms",
                "gpu_shadow_ms",
            ] {
                let mut values: Vec<f64> = samples
                    .iter()
                    .filter_map(|s| s.report[key].as_f64())
                    .collect();
                values.sort_by(f64::total_cmp);
                if !values.is_empty() {
                    report[key] = json!(values[values.len() / 2]);
                }
            }
            report["measured_frames"] = json!(count);
            report["warmup_frames"] = json!(u32::from(options.command == "time"));
            let out = if options.out.extension().is_some() {
                options.out.clone()
            } else {
                options.out.join("frame.png")
            };
            save(
                &out,
                &samples.last().expect("sample").pixels,
                options.width,
                options.height,
            )?;
            report["out"] = json!(out);
            println!("{report}");
        }
        "compare" => {
            let mut cluster = create(Mode::Cluster)?;
            let mut naive = create(Mode::Naive)?;
            compare(&mut cluster, &mut naive, &reader, &scene, &options)?;
        }
        "pop" => {
            let mut cluster = create(Mode::Cluster)?;
            let mut naive = create(Mode::Naive)?;
            pop(&mut cluster, &mut naive, &reader, &scene, &options)?;
        }
        _ => unreachable!(),
    }
    Ok(())
}
fn sample(
    renderer: &mut Renderer,
    reader: &Reader<'_>,
    scene: &Scene,
    options: &Options,
    t: f32,
    threshold: f32,
) -> Result<Sample> {
    let camera = options.camera(scene, t);
    let start = Instant::now();
    let selection = if renderer.mode == Mode::Cluster {
        select::select(reader, &scene.instances, &camera, options.height, threshold)
    } else {
        Selection::default()
    };
    let selection_ms = start.elapsed().as_secs_f64() * 1000.0;
    let start = Instant::now();
    let shadow = if renderer.mode == Mode::Cluster {
        select::select(
            reader,
            &scene.instances,
            &scene.light_camera(),
            2048,
            threshold * 2.0,
        )
    } else {
        Selection::default()
    };
    let shadow_selection_ms = start.elapsed().as_secs_f64() * 1000.0;
    let start = Instant::now();
    let frame = renderer.render(
        scene,
        &camera,
        if renderer.mode == Mode::Naive
            && matches!(options.view, View::Clusters | View::Depth | View::Triangles)
        {
            View::Lit
        } else {
            options.view
        },
        &selection,
        &shadow,
    )?;
    let encode_ms = start.elapsed().as_secs_f64() * 1000.0;
    let (pixels, times) = readback::read(renderer, &frame)?;
    let s = frame.stats;
    let report = json!({"command":options.command,"mode":format!("{:?}",renderer.mode).to_lowercase(),"view":format!("{:?}",options.view).to_lowercase(),"layout":options.layout,"size":[options.width,options.height],"t":t,"threshold_px":threshold,"selected_clusters":s.selected_clusters,"triangles_drawn":s.triangles,"padding_triangles":s.padded_triangles,"submitted_vertices_or_indices":(s.triangles+s.padded_triangles)*3,"draws":s.draws,"ground_draws":1,"shadow_clusters":s.shadow_clusters,"shadow_triangles":s.shadow_triangles,"shadow_padding":s.shadow_padding,"shadow_draws":s.shadow_draws,"selection_ms":selection_ms,"shadow_selection_ms":shadow_selection_ms,"encode_ms":encode_ms,"gpu_ms":times.map(|v|v[0]+v[1]),"gpu_main_ms":times.map(|v|v[1]),"gpu_shadow_ms":times.map(|v|v[0]),"bytes_resident_gpu":s.resident_bytes,"readback_bytes":s.readback_bytes,"eye":camera.eye.to_array()});
    Ok(Sample { pixels, report })
}
fn save(path: &Path, pixels: &[u8], width: u32, height: u32) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, readback::png_bytes(pixels, width, height)?).map_err(|e| e.to_string())
}
fn compare(
    cluster: &mut Renderer,
    naive: &mut Renderer,
    reader: &Reader<'_>,
    scene: &Scene,
    o: &Options,
) -> Result<()> {
    let mut failures = Vec::new();
    let mut checked = 0;
    let mut worst = -1.0;
    let mut worst_info = json!(null);
    for &t in &o.times {
        let reference = match sample(naive, reader, scene, o, t, 0.0) {
            Ok(s) => s,
            Err(e) => {
                failures.push(format!("naive t={t}: {e}"));
                continue;
            }
        };
        for &threshold in &o.thresholds {
            let s = match sample(cluster, reader, scene, o, t, threshold) {
                Ok(s) => s,
                Err(e) => {
                    failures.push(format!("cluster t={t}, threshold={threshold}: {e}"));
                    continue;
                }
            };
            let d = readback::difference(&s.pixels, &reference.pixels);
            checked += 1;
            let line = json!({"oracle":"compare","threshold_px":threshold,"t":t,"mean_abs":d.mean,"max":d.max as f64/255.0,"max_byte":d.max,"fraction_gt_2":d.fraction,"cluster_triangles":s.report["triangles_drawn"],"naive_triangles":reference.report["triangles_drawn"],"cluster_shadow_triangles":s.report["shadow_triangles"],"size":[o.width,o.height]});
            println!("{line}");
            if d.mean > worst {
                worst = d.mean;
                worst_info = line;
                let dir = o.output_dir();
                let mut difference = Vec::with_capacity(s.pixels.len());
                for (a, b) in s
                    .pixels
                    .chunks_exact(4)
                    .zip(reference.pixels.chunks_exact(4))
                {
                    for i in 0..3 {
                        difference.push(a[i].abs_diff(b[i]).saturating_mul(10));
                    }
                    difference.push(255);
                }
                for (name, pixels) in [
                    ("worst-cluster.png", &s.pixels),
                    ("worst-naive.png", &reference.pixels),
                    ("worst-diff-10x.png", &difference),
                ] {
                    if let Err(e) = save(&dir.join(name), pixels, o.width, o.height) {
                        failures.push(e);
                    }
                }
            }
        }
    }
    println!(
        "{}",
        json!({"oracle":"compare_summary","pairs":checked,"worst":worst_info,"failures":failures})
    );
    if checked == 0 || !failures.is_empty() {
        return Err(format!(
            "compare: {} failures, {checked} pairs",
            failures.len()
        ));
    }
    Ok(())
}
fn pop(
    cluster: &mut Renderer,
    naive: &mut Renderer,
    reader: &Reader<'_>,
    scene: &Scene,
    o: &Options,
) -> Result<()> {
    let mut previous: Option<(Sample, Sample)> = None;
    let mut worst = -1.0;
    let mut worst_step = 0;
    let mut max_signed = f64::NEG_INFINITY;
    let mut max_signed_step = 0;
    let mut checked = 0;
    let mut failures = Vec::new();
    for step in 0..o.steps {
        let t = step as f32 / (o.steps - 1) as f32;
        let c = sample(cluster, reader, scene, o, t, o.thresholds[0]);
        let n = sample(naive, reader, scene, o, t, 0.0);
        let (c, n) = match (c, n) {
            (Ok(c), Ok(n)) => (c, n),
            (c, n) => {
                failures.push(format!(
                    "step {step}: cluster {:?}; naive {:?}",
                    c.err(),
                    n.err()
                ));
                previous = None;
                continue;
            }
        };
        if let Some((pc, pn)) = &previous {
            let cd = readback::difference(&c.pixels, &pc.pixels).mean;
            let nd = readback::difference(&n.pixels, &pn.pixels).mean;
            let signed = cd - nd;
            // Stronger spatial metric: subtract signed temporal RGB deltas before taking abs.
            let mut residual = 0u64;
            for (((c, pc), n), pn) in c
                .pixels
                .chunks_exact(4)
                .zip(pc.pixels.chunks_exact(4))
                .zip(n.pixels.chunks_exact(4))
                .zip(pn.pixels.chunks_exact(4))
            {
                for i in 0..3 {
                    residual += ((c[i] as i32 - pc[i] as i32) - (n[i] as i32 - pn[i] as i32))
                        .unsigned_abs() as u64;
                }
            }
            let metric = residual as f64 / (o.width as u64 * o.height as u64 * 3) as f64 / 255.0;
            checked += 1;
            println!(
                "{}",
                json!({"oracle":"pop_step","step":step,"t":t,"cluster_frame_diff":cd,"naive_frame_diff":nd,"excess_frame_diff":signed,"temporal_residual":metric,"triangles":c.report["triangles_drawn"]})
            );
            if signed > max_signed {
                max_signed = signed;
                max_signed_step = step;
            }
            if metric > worst {
                worst = metric;
                worst_step = step;
                for (name, pixels) in [
                    ("pop-before.png", &pc.pixels),
                    ("pop-after.png", &c.pixels),
                    ("pop-naive-before.png", &pn.pixels),
                    ("pop-naive-after.png", &n.pixels),
                ] {
                    if let Err(e) = save(&o.output_dir().join(name), pixels, o.width, o.height) {
                        failures.push(e);
                    }
                }
            }
        }
        previous = Some((c, n));
    }
    println!(
        "{}",
        json!({"oracle":"pop_summary","steps":o.steps,"pairs":checked,"max_temporal_residual":worst,"worst_step":worst_step,"max_excess_frame_diff":max_signed,"max_excess_step":max_signed_step,"size":[o.width,o.height],"threshold_px":o.thresholds[0],"failures":failures})
    );
    if checked == 0 || !failures.is_empty() {
        return Err(format!("pop: {} failures, {checked} pairs", failures.len()));
    }
    Ok(())
}
