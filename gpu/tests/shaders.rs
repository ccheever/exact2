//! The shader registry (LLP 1030 D8): a shader is admitted at the interface
//! the binary binds and refused off it, by name, with the registry left as
//! it was; the generation moves on every admission; the module names what
//! is missing.

use exact_gpu::shaders::{missing, set_shader, shader_generation, shader_module, shader_source};
use exact_gpu::wgpu;

const FS: &str = "@fragment fn fs() -> @location(0) vec4<f32> { return vec4<f32>(1.0); }";

#[test]
fn a_shader_is_registered_at_its_interface_and_refused_off_it() {
    let digest = exact_gpu::shaders::interface_digest(FS).unwrap();
    let before = shader_generation();
    set_shader("t_ok", FS.into(), Some(digest)).unwrap();
    assert!(
        shader_generation() > before,
        "an admission moves the generation"
    );
    assert_eq!(shader_source("t_ok").as_deref(), Some(FS));
    let generation = shader_generation();
    // Another output location is another interface.
    let e = set_shader(
        "t_ok",
        "@fragment fn fs() -> @location(1) vec4<f32> { return vec4<f32>(1.0); }".into(),
        Some(digest),
    )
    .unwrap_err();
    assert!(e.contains("`t_ok`") && e.contains("interface"), "{e}");
    assert_eq!(
        shader_source("t_ok").as_deref(),
        Some(FS),
        "the refusal left the registry as it was"
    );
    assert_eq!(shader_generation(), generation, "and moved nothing");
    // A shader that does not validate names its line.
    let e = set_shader(
        "t_bad",
        "@fragment fn fs() -> @location(0) vec4<f32> {\n    return 1;\n}".into(),
        None,
    )
    .unwrap_err();
    assert!(e.contains("does not validate") && e.contains(":2:"), "{e}");
    assert!(shader_source("t_bad").is_none());
    // With no expectation, any valid text is admitted.
    set_shader("t_free", FS.into(), None).unwrap();
    // What a module refuses to create surfaces over.
    assert!(missing(&[("t_ok", digest)]).is_none());
    assert!(missing(&[("t_ok", digest), ("t_none", 1)])
        .unwrap()
        .contains("`t_none`"));
    assert!(missing(&[("t_ok", digest ^ 1)]).unwrap().contains("`t_ok`"));
    // The descriptor a generated `module()` returns is the registered text.
    match shader_module("t_ok").source {
        wgpu::ShaderSource::Wgsl(text) => assert_eq!(&*text, FS),
        _ => panic!("WGSL"),
    }
    match shader_module("t_never").source {
        wgpu::ShaderSource::Wgsl(text) => assert_eq!(&*text, "", "empty until registered"),
        _ => panic!("WGSL"),
    }
}
