use base64::Engine;
use clod_bake::{Mesh, loaders, normalize};
use serde_json::json;
#[test]
fn finite_normals_keep_their_direction() {
    let mut failures = Vec::new();
    for value in [1e20, 1e-30, -1e20, -1e-30] {
        let n = normalize([value, 0.0, 0.0]);
        println!("normal input={value} output={n:?}");
        if n != [value.signum(), 0.0, 0.0] {
            failures.push(format!("authored {value}: {n:?}"));
        }
    }
    for value in [1e20, 1e-30] {
        let mut mesh = Mesh {
            positions: vec![[0.0; 3], [0.0, value, 0.0], [0.0, 0.0, value]],
            indices: vec![0, 1, 2],
            ..Default::default()
        };
        mesh.compute_normals();
        println!("generated extent={value} normals={:?}", mesh.normals);
        if mesh.normals != [[1.0, 0.0, 0.0]; 3] {
            failures.push(format!("generated {value}"));
        }
    }
    println!("normal_cases=6 failures={failures:?}");
    assert!(failures.is_empty(), "{failures:?}");
}
#[test]
fn gltf_preserves_each_primitives_normals() {
    let dir = std::path::PathBuf::from(std::env::var_os("HOME").unwrap())
        .join("Library/Caches/exact2-cluster-lod/out/F1");
    std::fs::create_dir_all(&dir).unwrap();
    let mut bytes = Vec::new();
    for v in [
        0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0,
    ] {
        bytes.extend(v.to_le_bytes());
    }
    let mut doc = json!({"asset":{"version":"2.0"},"buffers":[{"byteLength":72,"uri":format!("data:application/octet-stream;base64,{}",base64::engine::general_purpose::STANDARD.encode(&bytes))}],"bufferViews":[{"buffer":0,"byteOffset":0,"byteLength":36},{"buffer":0,"byteOffset":36,"byteLength":36}],"accessors":[{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,1,0]},{"bufferView":1,"componentType":5126,"count":3,"type":"VEC3"}],"meshes":[{"primitives":[{"attributes":{"POSITION":0,"NORMAL":1}},{"attributes":{"POSITION":0}}]}]});
    let path = dir.join("mixed-normals.gltf");
    std::fs::write(&path, serde_json::to_vec(&doc).unwrap()).unwrap();
    let (mut mesh, _) = loaders::load(&path).unwrap();
    mesh.compute_normals();
    let expected = [
        [1.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
    ];
    let normals_ok = mesh.normals == expected;
    doc["bufferViews"][1]["buffer"] = json!(99);
    std::fs::write(&path, serde_json::to_vec(&doc).unwrap()).unwrap();
    let bad_buffer = matches!(
        std::panic::catch_unwind(|| loaders::load(&path).is_err()),
        Ok(true)
    );
    println!(
        "gltf_primitives=2 authored_normals=3 generated_normals=3 normals_ok={normals_ok} missing_buffer_is_error={bad_buffer}"
    );
    assert!(normals_ok && bad_buffer);
}
