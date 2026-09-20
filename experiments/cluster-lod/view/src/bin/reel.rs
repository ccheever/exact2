//! Native PNG sequence, timestamped bitmap caption and reproducible ffmpeg invocation.
use super::{Options, Result, sample, save};
use clod_format::Reader;
use clod_view::{Mode, Renderer, scene::Scene};
use serde_json::json;
use std::{
    collections::VecDeque, fs::File, io::Write, path::Path, process::Command, time::Instant,
};

pub fn run(renderer: &mut Renderer, reader: &Reader<'_>, scene: &Scene, o: &Options) -> Result<()> {
    if renderer.mode != Mode::Cluster {
        return Err("reel requires cluster mode".into());
    }
    let start = Instant::now();
    let (clearance, worst_t) = scene.path_clearance(reader, 2401, o.fov);
    let edge_pixels = scene.minimum_distance(o.fov) * 2.0 / clearance;
    println!(
        "{}",
        json!({"oracle":"camera_clearance","samples":2401,"source_triangle_clearance":clearance,"worst_t":worst_t,"median_edge_pixels_at_clearance":edge_pixels})
    );
    if edge_pixels > 2.1 {
        return Err(format!(
            "camera exceeds source detail budget: median edge upper bound {edge_pixels} px"
        ));
    }

    let out = o.output_dir();
    let frames = out.join("frames");
    let stills = out.join("stills");
    std::fs::create_dir_all(&frames).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&stills).map_err(|e| e.to_string())?;
    let mut log = File::create(out.join("frames.jsonl")).map_err(|e| e.to_string())?;
    let count = (o.seconds * o.fps as f32).round() as u32;
    let total = reader.header.source_triangles as u64 * scene.instances.len() as u64;
    let positions = [0.0, 0.15, 0.3, 0.45, 0.6, 0.75, 0.9, 1.0];
    let selected = positions.map(|t| (t * (count - 1) as f32).round() as u32);
    let mut failures = Vec::new();
    let mut times = Vec::new();
    let mut history = VecDeque::new();
    let mut triangles = Vec::new();
    let _ = sample(renderer, reader, scene, o, 0.0, o.thresholds[0])?;
    for i in 0..count {
        let t = i as f32 / (count - 1) as f32;
        renderer.wipe = wipe(t);
        let result = (|| -> Result<()> {
            let mut frame = sample(renderer, reader, scene, o, t, o.thresholds[0])?;
            let gpu = frame.report["gpu_ms"]
                .as_f64()
                .ok_or("reel requires valid GPU timestamps")?;
            let drawn = frame.report["triangles_drawn"]
                .as_u64()
                .ok_or("missing triangle count")?;
            if frame.report["overflow"] != 0 || frame.report["shadow_overflow"] != 0 {
                return Err(format!("dropped geometry at frame {i}"));
            }
            times.push(gpu);
            triangles.push(drawn);
            history.push_back(gpu);
            if history.len() > 30 {
                history.pop_front();
            }
            let mut recent: Vec<_> = history.iter().copied().collect();
            recent.sort_by(f64::total_cmp);
            let caption = format!(
                "TRIANGLES {} / {} X {}    GPU {:.2} MS (30F MED)",
                grouped(drawn),
                grouped(reader.header.source_triangles as u64),
                scene.instances.len(),
                recent[recent.len() / 2]
            );
            caption_pixels(&mut frame.pixels, o.width, o.height, &caption);
            let path = frames.join(format!("frame-{i:05}.png"));
            save(&path, &frame.pixels, o.width, o.height)?;
            for (j, &index) in selected.iter().enumerate() {
                if i == index {
                    std::fs::copy(
                        &path,
                        stills.join(format!("t-{:03}.png", (positions[j] * 100.0) as u32)),
                    )
                    .map_err(|e| e.to_string())?;
                }
            }
            frame.report["frame"] = json!(i);
            frame.report["wipe"] = json!(renderer.wipe);
            frame.report["caption_gpu_ms"] = json!(recent[recent.len() / 2]);
            writeln!(log, "{}", frame.report).map_err(|e| e.to_string())?;
            Ok(())
        })();
        if let Err(e) = result {
            failures.push(format!("frame {i}: {e}"));
        }
        if i % 60 == 0 || i + 1 == count {
            println!(
                "{}",
                json!({"reel_frame":i,"total":count,"elapsed_seconds":start.elapsed().as_secs_f64(),"failures":failures.len()})
            );
        }
    }
    if failures.is_empty() {
        let pattern = frames.join("frame-%05d.png");
        let primary = out.join("cluster-lod-reel.mp4");
        let small = out.join("cluster-lod-reel-1080p.mp4");
        let fps = o.fps.to_string();
        let n = count.to_string();
        let args = [
            "-y",
            "-v",
            "warning",
            "-framerate",
            &fps,
            "-i",
            path(&pattern)?,
            "-frames:v",
            &n,
            "-c:v",
            "libx264",
            "-preset",
            "slow",
            "-crf",
            "16",
            "-pix_fmt",
            "yuv420p",
            "-movflags",
            "+faststart",
            path(&primary)?,
        ];
        if let Err(e) = encode(&args) {
            failures.push(e);
        }
        if primary.exists()
            && let Err(e) = encode(&[
                "-y",
                "-v",
                "warning",
                "-i",
                path(&primary)?,
                "-vf",
                "scale=1920:1080:flags=lanczos",
                "-c:v",
                "libx264",
                "-preset",
                "slow",
                "-crf",
                "16",
                "-pix_fmt",
                "yuv420p",
                "-movflags",
                "+faststart",
                path(&small)?,
            ])
        {
            failures.push(e);
        }
    }
    times.sort_by(f64::total_cmp);
    triangles.sort_unstable();
    let summary = json!({"command":"reel","frames_requested":count,"frames_measured":times.len(),"seconds":o.seconds,"fps":o.fps,"source_triangles_times_instances":total,"triangles_min":triangles.first(),"triangles_max":triangles.last(),"gpu_ms_min":times.first(),"gpu_ms_median":times.get(times.len()/2),"gpu_ms_max":times.last(),"elapsed_seconds":start.elapsed().as_secs_f64(),"stills":selected,"failures":failures});
    println!("{summary}");
    std::fs::write(out.join("summary.json"), format!("{summary}\n")).map_err(|e| e.to_string())?;
    if !failures.is_empty() {
        return Err(format!("reel: {} failures", failures.len()));
    }
    Ok(())
}
fn path(p: &Path) -> Result<&str> {
    p.to_str().ok_or_else(|| "ffmpeg path is not UTF-8".into())
}
fn encode(args: &[&str]) -> Result<()> {
    let mut child = Command::new("/opt/homebrew/bin/ffmpeg")
        .args(args)
        .spawn()
        .map_err(|e| e.to_string())?;
    println!("{}", json!({"encoder_pid":child.id(),"args":args}));
    let status = child.wait().map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("ffmpeg: {status}"));
    }
    Ok(())
}
fn grouped(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}
pub fn wipe(t: f32) -> f32 {
    let ease = |x: f32| {
        let x = x.clamp(0.0, 1.0);
        x * x * x * (x * (x * 6.0 - 15.0) + 10.0)
    };
    if !(0.55..=0.75).contains(&t) {
        -1.0
    } else if t < 0.65 {
        -0.16 + 1.32 * ease((t - 0.55) / 0.10)
    } else {
        -0.16 + 1.32 * (1.0 - ease((t - 0.65) / 0.10))
    }
}
fn glyph(c: char) -> [u8; 7] {
    match c {
        '0' => [14, 17, 19, 21, 25, 17, 14],
        '1' => [4, 12, 4, 4, 4, 4, 14],
        '2' => [14, 17, 1, 2, 4, 8, 31],
        '3' => [30, 1, 1, 14, 1, 1, 30],
        '4' => [2, 6, 10, 18, 31, 2, 2],
        '5' => [31, 16, 16, 30, 1, 1, 30],
        '6' => [14, 16, 16, 30, 17, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8, 8],
        '8' => [14, 17, 17, 14, 17, 17, 14],
        '9' => [14, 17, 17, 15, 1, 1, 14],
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'G' => [14, 17, 16, 23, 17, 17, 14],
        'I' => [14, 4, 4, 4, 4, 4, 14],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 25, 21, 19, 19, 17],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'X' => [17, 17, 10, 4, 10, 17, 17],
        '/' => [1, 2, 2, 4, 8, 8, 16],
        '.' => [0, 0, 0, 0, 0, 6, 6],
        ',' => [0, 0, 0, 0, 6, 6, 4],
        '(' => [2, 4, 8, 8, 8, 4, 2],
        ')' => [8, 4, 2, 2, 2, 4, 8],
        _ => [0; 7],
    }
}
fn caption_pixels(pixels: &mut [u8], width: u32, height: u32, text: &str) {
    let scale = (width / 1100).max(1);
    let x0 = 24 * scale;
    let y0 = height.saturating_sub(25 * scale);
    let end = (x0 + text.len() as u32 * 6 * scale + 10 * scale).min(width);
    for y in y0.saturating_sub(8 * scale)..(y0 + 15 * scale).min(height) {
        for x in x0.saturating_sub(10 * scale)..end {
            let p = (y * width + x) as usize * 4;
            for c in &mut pixels[p..p + 3] {
                *c = (*c as f32 * 0.48 + 8.0) as u8;
            }
        }
    }
    for (i, c) in text.chars().enumerate() {
        for (y, row) in glyph(c).iter().enumerate() {
            for x in 0..5 {
                if row & (1 << (4 - x)) == 0 {
                    continue;
                }
                for yy in 0..scale {
                    for xx in 0..scale {
                        let px = x0 + (i as u32 * 6 + x) * scale + xx;
                        let py = y0 + y as u32 * scale + yy;
                        if px < width && py < height {
                            let p = (py * width + px) as usize * 4;
                            pixels[p..p + 3].copy_from_slice(&[224, 225, 218]);
                        }
                    }
                }
            }
        }
    }
}
