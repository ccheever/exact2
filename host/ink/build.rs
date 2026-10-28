//! The portable shader is checked in every backend dialect. @ref LLP 1149 D6.1.

use naga::back::{glsl, hlsl, msl, spv};
use naga::valid::{Capabilities, ValidationFlags, Validator};
use std::{fs, path::PathBuf};

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
    for entry in translated.entry_point_names {
        entry.expect("MSL entry point");
    }
    fs::write(out.join("ink.metal"), msl_source).unwrap();

    let mut hlsl_source = String::new();
    let pipeline = hlsl::PipelineOptions::default();
    let translated = hlsl::Writer::new(&mut hlsl_source, &hlsl_options, &pipeline)
        .write(&module, &info, None)
        .expect("HLSL 5.0 translation");
    for entry in translated.entry_point_names {
        entry.expect("HLSL entry point");
    }
    fs::write(out.join("ink.hlsl"), fixed_sampler_hlsl(&hlsl_source)).unwrap();

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
