//! Every shader this module runs, compiled here to Metal AIR (`rules/
//! DEFERRED.md`: the GPU "compiles no shaders at runtime").
//!
//! vello's WGSL (from `vello_shaders`, with the fine stage patched: a
//! backdrop it starts from and premultiplied output) and this crate's own
//! (`shaders/*.wgsl`) are preprocessed and validated by `vello_shaders`,
//! written as MSL by naga with the binding slots wgpu-hal's Metal pipeline
//! layout assigns (buffers, then textures, each in binding order), then
//! compiled by `xcrun metal` for the target's SDK. The module loads each
//! `.metallib` through wgpu's passthrough shaders.
//!
//! Workgroup memory: naga passes `var<workgroup>` as `threadgroup` entry
//! arguments whose lengths wgpu sets from its own reflection, which a
//! passthrough shader has none of; they are rewritten here as `threadgroup`
//! variables in the kernel's scope, which Metal sizes itself.

use naga::back::msl;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use vello_shaders::compile::ShaderInfo;
use vello_shaders::BindType;

/// The kernels the module creates pipelines for, by `vello_shaders` name.
const VELLO: &[&str] = &[
    "pathtag_reduce",
    "pathtag_reduce2",
    "pathtag_scan1",
    "pathtag_scan_small",
    "pathtag_scan_large",
    "bbox_clear",
    "flatten",
    "draw_reduce",
    "draw_leaf",
    "clip_reduce",
    "clip_leaf",
    "binning",
    "tile_alloc",
    "path_count_setup",
    "path_count",
    "backdrop_dyn",
    "coarse",
    "path_tiling_setup",
    "path_tiling",
    "fine_area",
];

fn main() {
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=shaders");
    println!("cargo:rustc-check-cfg=cfg(exact_no_metal_toolchain)");
    let target = std::env::var("TARGET").unwrap();
    // Off Apple the crate is empty (`src/lib.rs`): nothing to compile.
    if !target.contains("-apple-") {
        return;
    }
    // Without Xcode's Metal toolchain the module is a stub whose `ecg_abi`
    // answers 0, so the host draws with Core Graphics; host/apple/build.mjs
    // checks for the toolchain and says how to install it before it embeds
    // a module. This keeps `cargo build --workspace` working on a Mac
    // without it.
    if !Command::new("xcrun")
        .args(["-sdk", "macosx", "-f", "metal"])
        .output()
        .is_ok_and(|o| o.status.success())
        || !Command::new("xcrun")
            .args(["-sdk", "macosx", "metal", "--version"])
            .output()
            .is_ok_and(|o| o.status.success())
    {
        println!("cargo:warning=exact-canvas-vello: no Metal toolchain (`xcodebuild -downloadComponent MetalToolchain`); the module is a stub");
        println!("cargo:rustc-cfg=exact_no_metal_toolchain");
        std::fs::write(out.join("kernels.bin"), []).unwrap();
        std::fs::write(
            out.join("kernels.rs"),
            "pub static KERNELS: &[Kernel] = &[];\n",
        )
        .unwrap();
        return;
    }
    let sdk = if target.ends_with("apple-ios-sim") {
        Some(("iphonesimulator", "-mios-simulator-version-min=17.0"))
    } else if target.ends_with("apple-ios") {
        Some(("iphoneos", "-mios-version-min=17.0"))
    } else if target.ends_with("apple-darwin") {
        Some(("macosx", "-mmacosx-version-min=14.0"))
    } else {
        None
    };

    // vello's shader directory, copied so the fine stage can be patched.
    let dir = out.join("shader");
    let _ = std::fs::remove_dir_all(&dir);
    copy_dir(vello_shaders::compile::shader_dir(), &dir);
    let fine = dir.join("fine.wgsl");
    let src = std::fs::read_to_string(&fine).unwrap();
    std::fs::write(&fine, patch_fine(&src)).unwrap();
    let coarse = dir.join("coarse.wgsl");
    let src = std::fs::read_to_string(&coarse).unwrap();
    std::fs::write(&coarse, patch_coarse(&src)).unwrap();
    let ptcl = dir.join("shared").join("ptcl.wgsl");
    let src = std::fs::read_to_string(&ptcl).unwrap();
    std::fs::write(
        &ptcl,
        src.replace(
            "const CMD_BLUR_RECT = 13u;",
            "const CMD_BLUR_RECT = 13u;\nconst CMD_BEGIN_CLIP_KEEP = 14u;",
        ),
    )
    .unwrap();
    for e in std::fs::read_dir(manifest.join("shaders")).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_some_and(|x| x == "wgsl") {
            std::fs::copy(&p, dir.join(p.file_name().unwrap())).unwrap();
        }
    }
    let infos = match ShaderInfo::from_dir(&dir) {
        Ok(i) => i,
        Err(e) => panic!("shaders: {e}"),
    };
    let mut names: Vec<String> = VELLO.iter().map(|s| s.to_string()).collect();
    for e in std::fs::read_dir(manifest.join("shaders")).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_some_and(|x| x == "wgsl") {
            names.push(p.file_stem().unwrap().to_str().unwrap().to_string());
        }
    }

    // Translate every kernel to MSL, then compile them in parallel.
    let mut jobs = Vec::new();
    for name in &names {
        let info = infos
            .get(name)
            .unwrap_or_else(|| panic!("no shader {name}"));
        let (source, entry) = translate(info);
        let path = out.join(format!("{name}.metal"));
        std::fs::write(&path, &source).unwrap();
        jobs.push((
            name.clone(),
            entry,
            info.workgroup_size,
            bind_kinds(info),
            path,
        ));
    }
    let libs: Vec<Vec<u8>> = std::thread::scope(|s| {
        let handles: Vec<_> = jobs
            .iter()
            .map(|(name, _, _, _, path)| {
                let out = &out;
                s.spawn(move || match sdk {
                    Some((sdk, min)) => {
                        metallib(sdk, min, path, &out.join(format!("{name}.metallib")))
                    }
                    None => Vec::new(),
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    // One blob: every metallib back to back; an index says where each is.
    let mut blob = Vec::new();
    let mut index = String::new();
    writeln!(
        index,
        "/// Every kernel: name, entry point, workgroup size, bindings, byte range in `BLOB`."
    )
    .unwrap();
    writeln!(index, "pub static KERNELS: &[Kernel] = &[").unwrap();
    for ((name, entry, wg, binds, _), lib) in jobs.iter().zip(&libs) {
        let start = blob.len();
        blob.extend_from_slice(lib);
        writeln!(
            index,
            "    Kernel {{ name: {name:?}, entry: {entry:?}, workgroup: {wg:?}, bindings: &{binds:?}, range: ({start}, {}) }},",
            blob.len()
        )
        .unwrap();
    }
    writeln!(index, "];").unwrap();
    std::fs::write(out.join("kernels.bin"), &blob).unwrap();
    std::fs::write(out.join("kernels.rs"), index).unwrap();
}

/// The fine stage, patched: each pixel starts from `backdrop` (the canvas's
/// kept pixels, premultiplied; a 1×1 transparent texture when there are
/// none) instead of the base colour, and is written premultiplied (the
/// IOSurface's layout). The area variant only; the MSAA ones are unused.
fn patch_fine(src: &str) -> String {
    let mut s = src.to_string();
    let once = |s: &mut String, from: &str, to: &str| {
        assert_eq!(s.matches(from).count(), 1, "fine.wgsl patch site {from:?}");
        *s = s.replace(from, to);
    };
    once(
        &mut s,
        "var image_atlas: texture_2d<f32>;\n",
        "var image_atlas: texture_2d<f32>;\n\n#ifndef msaa\n@group(0) @binding(8)\nvar backdrop: texture_2d<f32>;\n#endif\n",
    );
    once(
        &mut s,
        "    for (var i = 0u; i < PIXELS_PER_THREAD; i += 1u) {\n        rgba[i] = base_color;\n    }\n",
        "#ifdef msaa\n    for (var i = 0u; i < PIXELS_PER_THREAD; i += 1u) {\n        rgba[i] = base_color;\n    }\n#else\n    let backdrop_max = vec2<i32>(textureDimensions(backdrop)) - vec2(1);\n    for (var i = 0u; i < PIXELS_PER_THREAD; i += 1u) {\n        let at = min(vec2<i32>(i32(xy.x) + i32(i), i32(xy.y)), backdrop_max);\n        let bd = textureLoad(backdrop, at, 0);\n        rgba[i] = bd + base_color * (1.0 - bd.a);\n    }\n#endif\n",
    );
    once(
        &mut s,
        "            let rgba_sep = vec4(fg.rgb * a_inv, fg.a);\n            textureStore(output, vec2<i32>(coords), rgba_sep);\n",
        "#ifdef msaa\n            let rgba_sep = vec4(fg.rgb * a_inv, fg.a);\n            textureStore(output, vec2<i32>(coords), rgba_sep);\n#else\n            textureStore(output, vec2<i32>(coords), vec4(min(fg.rgb, vec3(fg.a)), fg.a));\n#endif\n",
    );
    // Canvas clips (see `patch_coarse`).
    let begin = s
        .find("            case CMD_BEGIN_CLIP: {")
        .expect("begin clip");
    let end = s[begin..]
        .find("            case CMD_END_CLIP: {")
        .expect("end clip")
        + begin;
    let block = s[begin..end]
        .replace("case CMD_BEGIN_CLIP: {", "case CMD_BEGIN_CLIP, CMD_BEGIN_CLIP_KEEP: {")
        .replace("rgba[i] = vec4(0.0);", "if tag == CMD_BEGIN_CLIP {\n                            rgba[i] = vec4(0.0);\n                        }");
    s.replace_range(begin..end, &block);
    once(
        &mut s,
        "                    if end_clip.blend == LUMINANCE_MASK_LAYER {",
        "                    if end_clip.blend == ((128u << 8u) | 3u) {\n                        rgba[i] = mix(bg, rgba[i], area[i] * end_clip.alpha);\n                        continue;\n                    }\n                    if end_clip.blend == LUMINANCE_MASK_LAYER {",
    );
    s
}

/// Canvas clips are not isolated groups (the web's and Core Graphics'
/// model): a pure clip layer (`push_clip_layer`, blend `BLEND_CLIP`) starts
/// from the pixels under it rather than transparent, and its end restores
/// them outside the clip (a lerp by coverage). So an operator inside a clip
/// (destination-out, source-in, copy, `clearRect`) acts on the canvas, and
/// the clip bounds it. Coarse marks such a begin with `CMD_BEGIN_CLIP_KEEP`.
fn patch_coarse(src: &str) -> String {
    let mut s = src.to_string();
    let once = |s: &mut String, from: &str, to: &str| {
        assert_eq!(
            s.matches(from).count(),
            1,
            "coarse.wgsl patch site {from:?}"
        );
        *s = s.replace(from, to);
    };
    once(
        &mut s,
        "fn write_begin_clip() {\n    alloc_cmd(1u);\n    ptcl[cmd_offset] = CMD_BEGIN_CLIP;",
        "fn write_begin_clip(keep: bool) {\n    alloc_cmd(1u);\n    ptcl[cmd_offset] = select(CMD_BEGIN_CLIP, CMD_BEGIN_CLIP_KEEP, keep);",
    );
    once(
        &mut s,
        "                            write_begin_clip();",
        "                            write_begin_clip(scene[dd] == ((128u << 8u) | 3u));",
    );
    s
}

/// A kernel's bindings as `B` (read-write buffer), `R` (read-only buffer),
/// `U` (uniform), `I` (storage texture, written) and `T` (sampled texture).
fn bind_kinds(info: &ShaderInfo) -> Vec<char> {
    info.bindings
        .iter()
        .map(|b| match b.ty {
            BindType::Buffer => 'B',
            BindType::BufReadOnly => 'R',
            BindType::Uniform => 'U',
            BindType::Image => 'I',
            BindType::ImageRead => 'T',
        })
        .collect()
}

/// MSL for one kernel, with wgpu-hal's slots; the source and its entry name.
fn translate(info: &ShaderInfo) -> (String, String) {
    let mut resources = msl::BindingMap::default();
    let (mut buffers, mut textures) = (0u8, 0u8);
    for b in &info.bindings {
        let mut t = msl::BindTarget::default();
        match b.ty {
            BindType::Buffer | BindType::BufReadOnly | BindType::Uniform => {
                t.buffer = Some(buffers);
                buffers += 1;
            }
            BindType::Image | BindType::ImageRead => {
                t.texture = Some(textures);
                textures += 1;
            }
        }
        t.mutable = b.ty.is_mutable();
        resources.insert(
            naga::ResourceBinding {
                group: b.location.0,
                binding: b.location.1,
            },
            t,
        );
    }
    let mut map = msl::EntryPointResourceMap::default();
    map.insert(
        "main".to_string(),
        msl::EntryPointResources {
            resources,
            immediates_buffer: None,
            sizes_buffer: Some(30),
        },
    );
    // As vello asks of wgpu: unchecked, no zeroed workgroup memory, no loop
    // bounding (`ShaderRuntimeChecks::unchecked()`).
    let options = msl::Options {
        lang_version: (3, 0),
        per_entry_point_map: map,
        inline_samplers: vec![],
        spirv_cross_compatibility: false,
        fake_missing_bindings: false,
        bounds_check_policies: naga::proc::BoundsCheckPolicies::default(),
        zero_initialize_workgroup_memory: false,
        force_loop_bounding: false,
    };
    let pipeline = msl::PipelineOptions {
        entry_point: Some((naga::ShaderStage::Compute, "main".to_string())),
        ..Default::default()
    };
    let (source, tr) = msl::write_string(&info.module, &info.module_info, &options, &pipeline)
        .unwrap_or_else(|e| panic!("msl: {e:?}"));
    let entry = tr.entry_point_names[0].clone().unwrap();
    (threadgroup_locals(&source, &entry), entry)
}

/// Move the kernel's `threadgroup` arguments into its body.
fn threadgroup_locals(src: &str, entry: &str) -> String {
    let head = format!("kernel void {entry}(");
    let at = src
        .find(&head)
        .unwrap_or_else(|| panic!("no kernel {entry}"));
    let open = src[at..].find(") {").unwrap() + at;
    let args = &src[at + head.len()..open];
    let mut kept = Vec::new();
    let mut locals = String::new();
    for arg in args.split("\n,") {
        let a = arg.trim().trim_start_matches(',').trim();
        if let Some(rest) = a.strip_prefix("threadgroup ") {
            // `threadgroup T& name` → `threadgroup T name;`
            let decl = rest.split(" [[").next().unwrap().replacen('&', "", 1);
            writeln!(locals, "    threadgroup {decl};").unwrap();
        } else if !a.is_empty() {
            kept.push(a.to_string());
        }
    }
    format!(
        "{}{}\n  {}\n) {{\n{}{}",
        &src[..at],
        head,
        kept.join("\n, "),
        locals,
        &src[open + 3..]
    )
}

fn metallib(sdk: &str, min: &str, src: &Path, out: &Path) -> Vec<u8> {
    let run = |args: &[&str]| {
        let o = Command::new("xcrun")
            .arg("-sdk")
            .arg(sdk)
            .args(args)
            .output();
        match o {
            Ok(o) if o.status.success() => {}
            Ok(o) => panic!(
                "xcrun -sdk {sdk} {}: {}\n(the Metal toolchain: `xcodebuild -downloadComponent MetalToolchain`)",
                args.join(" "),
                String::from_utf8_lossy(&o.stderr)
            ),
            Err(e) => panic!("xcrun: {e}"),
        }
    };
    let air = out.with_extension("air");
    run(&[
        "metal",
        "-std=metal3.0",
        min,
        "-c",
        src.to_str().unwrap(),
        "-o",
        air.to_str().unwrap(),
    ]);
    run(&[
        "metallib",
        air.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
    ]);
    std::fs::read(out).unwrap()
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let p = e.path();
        if p.is_dir() {
            copy_dir(&p, &to.join(e.file_name()));
        } else {
            std::fs::copy(&p, to.join(e.file_name())).unwrap();
        }
    }
}
