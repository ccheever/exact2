use super::*;
#[test]
fn content_digest_reuses_equal_bytes_and_replaces_changed_names() {
    let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
        return;
    };
    let mut renderer =
        crate::Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
    let mut model = Model {
        meshes: vec![exact_game::asset::MeshData {
            positions: vec![0.; 9],
            normals: vec![0.; 9],
            uvs: vec![0.; 6],
            indices: vec![0, 1, 2],
            ..Default::default()
        }],
        nodes: vec![exact_game::asset::Node {
            mesh: Some(0),
            ..Default::default()
        }],
        materials: vec![MaterialData::default()],
        ..Default::default()
    };
    renderer.prepare_model("resident.model", &model).unwrap();
    let before = renderer.residency_work();
    let old_mesh = renderer.models.loaded["resident.model"].nodes[0].0;
    renderer
        .prepare_model("resident.model", &model.clone())
        .unwrap();
    assert_eq!(before.json(), renderer.residency_work().json());
    model.meshes[0].positions[0] = 0.5;
    renderer.prepare_model("resident.model", &model).unwrap();
    assert_eq!(
        old_mesh, renderer.models.loaded["resident.model"].nodes[0].0,
        "replacement reuses the retired slot after accepting its digest"
    );
    let delta = renderer.residency_work().since(before);
    assert_eq!(delta.mesh_uploads, 1);
    assert_eq!(delta.pipeline_creations, 0);
    assert_eq!(delta.texture_uploads, 0);
    let mut texture = TextureData {
        width: 1,
        height: 1,
        mips: vec![vec![255; 4]],
        ..Default::default()
    };
    renderer.add_texture("resident.tex", &texture).unwrap();
    let before = renderer.residency_work();
    renderer
        .add_texture("resident.tex", &texture.clone())
        .unwrap();
    assert_eq!(before.json(), renderer.residency_work().json());
    texture.mips[0][0] = 0;
    renderer.add_texture("resident.tex", &texture).unwrap();
    assert_eq!(renderer.residency_work().since(before).texture_uploads, 1);
    texture.srgb = !texture.srgb;
    renderer.add_texture("resident.tex", &texture).unwrap();
    texture.wrap[0] = Wrap::Clamp;
    renderer.add_texture("resident.tex", &texture).unwrap();
    assert_eq!(renderer.residency_work().since(before).texture_uploads, 3);
}
#[test]
fn normal_cache_and_rebatch_scratch_follow_the_live_records() {
    let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
        return;
    };
    let mut renderer =
        crate::Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
    renderer
        .prepare_model("empty.model", &Model::default())
        .unwrap();
    let layout = &renderer.pipelines.models.as_ref().unwrap().instance;
    for x in 0..8 {
        let records = [DrawInstance {
            data: 0,
            transform: 0,
            geometry: MeshId(0),
            material: MaterialId(0),
            local: Mat4::from_translation(glam::Vec3::new(x as f32, 0., 0.)),
            skin: None,
            tint: [1.; 4],
            glow: [0.; 3],
        }];
        renderer
            .models
            .set(&gpu.device, &gpu.queue, layout, &renderer.uniform, &records)
            .unwrap();
        assert_eq!(renderer.models.normals.len(), 1);
        let ptr = renderer.models.words.as_ptr();
        renderer
            .models
            .set(&gpu.device, &gpu.queue, layout, &renderer.uniform, &records)
            .unwrap();
        assert_eq!(ptr, renderer.models.words.as_ptr());
    }
}
#[test]
fn material_waits_for_all_textures_before_rebinding() {
    let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
        return;
    };
    let mut renderer =
        crate::Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
    let model = Model {
        meshes: vec![exact_game::asset::MeshData {
            positions: vec![0.; 9],
            normals: vec![0.; 9],
            uvs: vec![0.; 6],
            indices: vec![0, 1, 2],
            ..Default::default()
        }],
        nodes: vec![exact_game::asset::Node {
            mesh: Some(0),
            ..Default::default()
        }],
        materials: vec![MaterialData {
            base_color_texture: Some(0),
            normal_texture: Some(1),
            ..Default::default()
        }],
        textures: vec!["color.tex".into(), "normal.tex".into()],
        ..Default::default()
    };
    renderer.prepare_model("two.model", &model).unwrap();
    let initial = renderer.models.materials[0].bind.clone();
    let texture = TextureData {
        width: 1,
        height: 1,
        mips: vec![vec![255; 4]],
        ..Default::default()
    };
    renderer.add_texture("color.tex", &texture).unwrap();
    assert_eq!(renderer.models.materials[0].bind, initial);
    renderer.add_texture("normal.tex", &texture).unwrap();
    let final_bind = renderer.models.materials[0].bind.clone();
    assert_ne!(final_bind, initial);
    renderer.add_texture("normal.tex", &texture).unwrap();
    assert_eq!(renderer.models.materials[0].bind, final_bind);
}
