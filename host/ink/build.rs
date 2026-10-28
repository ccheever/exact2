//! The portable shader is checked in every backend dialect. @ref LLP 1149 D6.1.

use naga::back::{glsl, hlsl, msl, spv};
use naga::valid::{Capabilities, ValidationFlags, Validator};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    println!("cargo:rerun-if-changed=shaders/ink.wgsl");
    println!("cargo:rerun-if-changed=build.rs");
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let source = fs::read_to_string("shaders/ink.wgsl").unwrap();
    let module = naga::front::wgsl::parse_str(&source)
        .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&source)));
    let info = Validator::new(ValidationFlags::all(), Capabilities::empty())
        .validate(&module)
        .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&source)));

    let mut metal = msl::Options {
        lang_version: (2, 0),
        fake_missing_bindings: false,
        ..Default::default()
    };
    let mut hlsl_options = hlsl::Options {
        shader_model: hlsl::ShaderModel::V5_0,
        fake_missing_bindings: false,
        ..Default::default()
    };
    hlsl_options.sampler_buffer_binding_map.insert(
        hlsl::SamplerIndexBufferKey { group: 0 },
        hlsl::BindTarget {
            register: 5,
            ..Default::default()
        },
    );
    let mut resources = msl::EntryPointResources::default();
    for binding in 0..9 {
        let key = naga::ResourceBinding { group: 0, binding };
        let mut target = msl::BindTarget::default();
        match binding {
            0 => target.buffer = Some(0),
            6 => target.sampler = Some(msl::BindSamplerTarget::Resource(0)),
            _ => target.texture = Some((binding - 1) as u8),
        }
        resources.resources.insert(key, target);
        hlsl_options.binding_map.insert(
            key,
            hlsl::BindTarget {
                register: if binding == 6 {
                    0
                } else {
                    binding.saturating_sub(1)
                },
                ..Default::default()
            },
        );
    }
    for entry in &module.entry_points {
        metal
            .per_entry_point_map
            .insert(entry.name.clone(), resources.clone());
    }
    let (msl_source, translated) =
        msl::write_string(&module, &info, &metal, &msl::PipelineOptions::default())
            .expect("MSL translation");
    let entries: Vec<_> = translated
        .entry_point_names
        .into_iter()
        .map(|entry| entry.expect("MSL entry point"))
        .collect();
    fs::write(
        out.join("metal_entries.rs"),
        format!(
            "const VERTEX: &str = {:?};\nconst FRAGMENT: &str = {:?};\n",
            entries[0], entries[1]
        ),
    )
    .unwrap();
    fs::write(out.join("ink.metal"), msl_source).unwrap();
    build_metal(&out);

    let mut hlsl_source = String::new();
    let pipeline = hlsl::PipelineOptions::default();
    let translated = hlsl::Writer::new(&mut hlsl_source, &hlsl_options, &pipeline)
        .write(&module, &info, None)
        .expect("HLSL 5.0 translation");
    let entries: Vec<_> = translated
        .entry_point_names
        .into_iter()
        .map(|entry| entry.expect("HLSL entry point"))
        .collect();
    fs::write(out.join("ink.hlsl"), fixed_sampler_hlsl(&hlsl_source)).unwrap();
    build_dxbc(&out, &entries);

    for entry in &module.entry_points {
        let pipeline = spv::PipelineOptions {
            shader_stage: entry.stage,
            entry_point: entry.name.clone(),
        };
        let words = spv::write_vec(&module, &info, &spv::Options::default(), Some(&pipeline))
            .expect("SPIR-V translation");
        let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        fs::write(out.join(format!("{}.spv", entry.name)), bytes).unwrap();
        let options = glsl::Options {
            version: glsl::Version::new_gles(300),
            ..Default::default()
        };
        let pipeline = glsl::PipelineOptions {
            shader_stage: entry.stage,
            entry_point: entry.name.clone(),
            multiview: None,
        };
        let mut source = String::new();
        glsl::Writer::new(
            &mut source,
            &module,
            &info,
            &options,
            &pipeline,
            naga::proc::BoundsCheckPolicies::default(),
        )
        .expect("GLSL ES 3.00 writer")
        .write()
        .expect("GLSL ES 3.00 translation");
        assert!(
            source.contains("precision highp float;") && source.contains("precision highp int;"),
            "GLSL ES positions, distances and derivative operands require highp defaults"
        );
        fs::write(out.join(format!("{}.glsl", entry.name)), source).unwrap();
    }
}

// SDKs and minima follow canvas/vello/build.rs. @ref LLP 1149 D6.1.
fn build_metal(out: &Path) {
    println!("cargo:rustc-check-cfg=cfg(exact_ink_metal)");
    let target = std::env::var("TARGET").unwrap();
    let sdk = if target.ends_with("apple-ios-sim") || target == "x86_64-apple-ios" {
        Some(("iphonesimulator", "-mios-simulator-version-min=17.0"))
    } else if target.ends_with("apple-ios") {
        Some(("iphoneos", "-mios-version-min=17.0"))
    } else if target.ends_with("apple-tvos-sim") || target == "x86_64-apple-tvos" {
        Some(("appletvsimulator", "-mtvos-simulator-version-min=17.0"))
    } else if target.ends_with("apple-tvos") {
        Some(("appletvos", "-mtvos-version-min=17.0"))
    } else if target.ends_with("apple-darwin") {
        Some(("macosx", "-mmacosx-version-min=14.0"))
    } else {
        None
    };
    let Some((sdk, min)) = sdk else { return };
    if !Command::new("xcrun")
        .args(["-sdk", "macosx", "-f", "metal"])
        .output()
        .is_ok_and(|o| o.status.success())
        || !Command::new("xcrun")
            .args(["-sdk", "macosx", "metal", "--version"])
            .output()
            .is_ok_and(|o| o.status.success())
    {
        println!("cargo:warning=exact-ink: no Metal toolchain (`xcodebuild -downloadComponent MetalToolchain`); the direct Metal device is unavailable");
        return;
    }
    metallib(sdk, min, &out.join("ink.metal"), &out.join("ink.metallib"));
    println!("cargo:rustc-cfg=exact_ink_metal");
}

fn metallib(sdk: &str, min: &str, src: &Path, out: &Path) {
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
                args.join(" "), String::from_utf8_lossy(&o.stderr)
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
}

/// Naga 30 emits SM5.1 sampler heaps even with V5_0. Ink has exactly one
/// statically bound sampler, so lower that generated indirection to s0.
/// @ref LLP 1149 D6.2 — a translation correction, checked again by fxc in C6.
fn fixed_sampler_hlsl(source: &str) -> String {
    let mut declarations = 0;
    let mut sampler = 0;
    let mut out = String::new();
    for line in source.lines() {
        if line.starts_with("SamplerState nagaSamplerHeap[")
            || line.starts_with("SamplerComparisonState nagaComparisonSamplerHeap[")
            || line.starts_with("StructuredBuffer<uint> nagaGroup0SamplerIndexArray ")
        {
            declarations += 1;
            continue;
        }
        if line.starts_with("static const SamplerState ")
            && line.contains(" = nagaSamplerHeap[nagaGroup0SamplerIndexArray[0]];")
        {
            let name = line
                .strip_prefix("static const SamplerState ")
                .unwrap()
                .split(" = ")
                .next()
                .unwrap();
            out.push_str(&format!("SamplerState {name} : register(s0);\n"));
            sampler += 1;
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    assert_eq!(
        (declarations, sampler),
        (3, 1),
        "naga's sampler lowering changed"
    );
    assert!(
        !out.contains("nagaSamplerHeap")
            && !out.contains("nagaComparisonSamplerHeap")
            && !out.contains("nagaGroup0SamplerIndexArray")
            && !out.contains(", space"),
        "HLSL 5.0 must not contain descriptor heaps or register spaces"
    );
    out
}

// The cross-check has no Windows SDK. Empty binaries refuse at device creation.
// @ref LLP 1149 D6.1.
fn build_dxbc(out: &Path, entries: &[String]) {
    println!("cargo:rerun-if-env-changed=PATH");
    let windows = std::env::var("TARGET").unwrap().contains("windows");
    let mut fxc = None;
    if windows {
        for dir in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
            // Installation into an existing PATH directory must invalidate the stub.
            if dir.is_dir() {
                println!("cargo:rerun-if-changed={}", dir.display());
            }
            for name in ["fxc.exe", "fxc"] {
                let candidate = dir.join(name);
                if candidate.is_file() {
                    println!("cargo:rerun-if-changed={}", candidate.display());
                    if fxc.is_none() {
                        fxc = Some(candidate);
                    }
                }
            }
            if fxc.is_some() {
                break;
            }
        }
    }
    let available = fxc.is_some();
    for (entry, profile, file) in [(0, "vs_5_0", "ink.vs.dxbc"), (1, "ps_5_0", "ink.ps.dxbc")] {
        let path = out.join(file);
        if !available {
            fs::write(path, []).unwrap();
            continue;
        }
        let result = Command::new(fxc.as_ref().unwrap())
            .args([
                "/nologo",
                "/O3",
                "/T",
                profile,
                "/E",
                &entries[entry],
                "/Fo",
            ])
            .arg(&path)
            .arg(out.join("ink.hlsl"))
            .output()
            .expect("fxc");
        assert!(
            result.status.success(),
            "fxc {profile}: {}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
    if !available && std::env::var("TARGET").unwrap().contains("windows") {
        println!("cargo:warning=exact-ink: fxc is not on PATH; the Direct3D 11 device will refuse to open");
    }
}
