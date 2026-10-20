use std::{env, fs, path::Path};

fn main() {
    // The update trust is a rustc input of this crate: `development_build()`
    // reads it with `env!`. Unset is development, as bake/src/compat.rs reads it.
    println!("cargo:rerun-if-env-changed=EXACT_UPDATE_TRUST");
    let trust = env::var("EXACT_UPDATE_TRUST").unwrap_or_else(|_| "development".into());
    println!("cargo:rustc-env=EXACT_UPDATE_TRUST={trust}");
    let out = env::var_os("OUT_DIR").unwrap();
    // @ref llp/1046.006.000-render-hooks.rfc.md#d2-the-frame-as-data-frameview
    let frame = fs::read_to_string("src/shaders/frame.wgsl").unwrap();
    fs::write(
        Path::new(&out).join("frame_interface.rs"),
        exact_gpu_reflect::reflect("frame", &frame).unwrap(),
    )
    .unwrap();
    println!("cargo:rerun-if-changed=src/shaders/shadow_count.wgsl");
    fs::write(
        Path::new(&out).join("shadow_count_interface.rs"),
        exact_gpu_reflect::reflect(
            "shadow_count",
            &fs::read_to_string("src/shaders/shadow_count.wgsl").unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    println!("cargo:rerun-if-changed=src/shaders/depth.wgsl");
    fs::write(
        Path::new(&out).join("depth_interface.rs"),
        exact_gpu_reflect::reflect(
            "depth",
            &fs::read_to_string("src/shaders/depth.wgsl").unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    // Skinning is maintained by the concurrent skeleton slice. Its source stays
    // untouched; every shader assembled by pipeline.rs uses this packaging step.
    for name in [
        "frame",
        "transform",
        "shadow_sample",
        "forward",
        "fade",
        "lights",
        "shadow",
        "shadow_seed",
        "sky",
        "tone",
        "tonemap",
        "forward_early",
        "bloom",
        "model",
        "model_early",
        "model_vertex",
        "outline",
        "model_shadow",
        "no_shadow_sample",
        "cull",
        "ibl",
        "environment",
        "occlusion",
        "occluder",
        "quad",
        "particle",
        "sprite",
        "soft",
        "count",
        "soft_count",
    ] {
        let path = format!("src/shaders/{name}.wgsl");
        println!("cargo:rerun-if-changed={path}");
        let source = fs::read_to_string(path).unwrap();
        let packed = pack(&source);
        fs::write(Path::new(&out).join(format!("{name}.wgsl")), packed).unwrap();
    }
    quads(Path::new(&out));
}

/// Every quad module, assembled here and validated by naga, at one sample a
/// pixel and four: dots (`particle`) and textured quads (`sprite`), each
/// plain, soft, and as a look frame's probe variant (`count`, `soft_count`).
/// The renderer compiles these files as written (`pipeline::quad_module`).
/// The probe variants' counter group is reflected into `count_interface.rs`,
/// and every variant must declare that same group: a binding that drifts is a
/// build error.
fn quads(out: &Path) {
    let piece = |name: &str| pack(&fs::read_to_string(format!("src/shaders/{name}.wgsl")).unwrap());
    let head = [piece("frame"), piece("tone"), piece("quad")].concat();
    fs::create_dir_all(out.join("quad")).unwrap();
    let (mut counters, mut interfaces): (Option<String>, String) = (None, String::new());
    for samples in [1, 4] {
        let depth = |name: &str| {
            let text = piece(name);
            if samples == 1 {
                text.replace("texture_depth_multisampled_2d", "texture_depth_2d")
                    .replace("textureNumSamples(count_depth)", "1u")
            } else {
                text
            }
        };
        for base in ["particle", "sprite"] {
            for (kind, parts) in [
                ("plain", vec![piece(base)]),
                ("soft", vec![piece(base), depth("soft")]),
                ("count", vec![piece(base), depth("count")]),
                (
                    "soft_count",
                    vec![
                        piece(base),
                        depth("soft"),
                        depth("count"),
                        piece("soft_count"),
                    ],
                ),
            ] {
                let name = format!("{base}_{kind}_{samples}");
                let source = [head.clone(), parts.concat()].concat();
                let reflected = exact_gpu_reflect::reflect(&name, &source)
                    .unwrap_or_else(|e| panic!("quad module {name}: {e}"));
                fs::write(out.join(format!("quad/{name}.wgsl")), &source).unwrap();
                if !kind.contains("count") {
                    continue;
                }
                // The counter group: its three binding consts as reflected, the
                // depth's sample count aside.
                let group: String = reflected
                    .lines()
                    .filter(|l| {
                        ["COVERED", "COUNTED", "COUNT_DEPTH"]
                            .iter()
                            .any(|c| l.contains(&format!("pub const {c}:")))
                    })
                    .map(|l| format!("{}\n", l.trim()))
                    .collect::<String>()
                    .replace("multisampled: false", "multisampled: true");
                match &counters {
                    None if group.matches("pub const").count() == 3 => counters = Some(group),
                    None => panic!("quad module {name}: the counter group is not covered, counted and count_depth"),
                    Some(first) => assert_eq!(&group, first, "quad module {name}: its counter group differs"),
                }
                if base == "particle" && samples == 4 {
                    interfaces.push_str(&reflected);
                }
            }
        }
    }
    fs::write(out.join("count_interface.rs"), interfaces).unwrap();
}

// WGSL has no string literals. Replace comments with whitespace (nested block
// comments included), then trim each line. Preserve every newline, so concatenated
// shader diagnostics retain their original line numbers; only columns change.
fn pack(source: &str) -> String {
    let mut chars = source.chars().peekable();
    let mut text = String::with_capacity(source.len());
    let mut block = 0;
    let mut line = false;
    while let Some(c) = chars.next() {
        if c == '\n' {
            text.push(c);
            line = false;
        } else if line {
            continue;
        } else if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            block += 1;
            text.push(' ');
        } else if block != 0 {
            if c == '*' && chars.peek() == Some(&'/') {
                chars.next();
                block -= 1;
            }
        } else if c == '/' && chars.peek() == Some(&'/') {
            chars.next();
            line = true;
        } else {
            text.push(c);
        }
    }
    assert_eq!(block, 0, "unterminated shader comment");
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        out.push_str(line.trim());
        if line.ends_with('\n') {
            out.push('\n');
        }
    }
    out
}
