use exact_game::{
    bin, json, Component, Data, Kind, Material, Mesh, Parent, Transform, Vec3, World,
};
use exact_game_scene::{bake, digest_bytes, SceneIdentity, Types};
use serde_json::{json as value, Value};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU32, Ordering},
};

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static SERIAL: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "exact-scene-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        Self(fs::canonicalize(dir).unwrap())
    }
    fn put(&self, name: &str, text: impl AsRef<[u8]>) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, text).unwrap();
        path
    }
    fn scene(&self, entities: Value) -> PathBuf {
        self.put(
            "scene.json",
            value!({"units":"metres-y-up","entities":entities}).to_string(),
        )
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn scene_entities_bind_to_checked_kinds_without_changing_the_world() {
    #[derive(Kind)]
    struct Prop {
        transform: Transform,
        mesh: Mesh,
        material: Option<Material>,
    }
    let files = Files::new();
    let path = files.scene(value!([
        {"id":"prop","components":{"Transform":{},"Mesh":{"Sphere":{"radius":0.5}}}},
        {"id":"incomplete","components":{"Transform":{}}}
    ]));
    let types = Types::standard();
    let baked = bake::compile(path, &types, &[]).unwrap();
    let mut world = World::new(60, 0);
    types.register(&mut world);
    types
        .prepare(&baked.content, &[])
        .unwrap()
        .instantiate(&mut world)
        .unwrap();
    let before = (world.save(), world.hash(), world.mutation_epoch());
    let prop = world.bind::<Prop>("prop").unwrap();
    assert!(world.row(prop).unwrap().material.is_none());
    let error = world.bind::<Prop>("incomplete").unwrap_err().to_string();
    assert!(error.contains("Prop") && error.contains("Mesh"), "{error}");
    assert_eq!(before, (world.save(), world.hash(), world.mutation_epoch()));
}

#[test]
fn typed_defaults_enums_and_parents_construct_the_identical_world() {
    let files = Files::new();
    let path = files.scene(value!([
        {"id":"child","parent":"root","components":{"Transform":{"position":[1,2,3]},"Mesh":{"Sphere":{"radius":0.5}},"Material":{"roughness":0.8}}},
        {"id":"root","components":{"Transform":{"position":[4,0,0]}}}
    ]));
    let types = Types::standard();
    let baked = bake::compile(path, &types, &[]).unwrap();
    let mut actual = World::new(60, 7);
    types.register(&mut actual);
    types
        .prepare(&baked.content, &[])
        .unwrap()
        .instantiate(&mut actual)
        .unwrap();
    let mut expected = World::new(60, 7);
    let child = expected.spawn_named(
        "child",
        (
            Transform::at(1., 2., 3.),
            Mesh::sphere(0.5),
            Material::default().rough(0.8),
        ),
    );
    let root = expected.spawn_named("root", Transform::at(4., 0., 0.));
    expected.insert(child, Parent(root));
    expected.insert_resource(SceneIdentity {
        digest: baked.digest.clone(),
        assets: Default::default(),
    });
    expected.propagate();
    assert_eq!(actual.hash(), expected.hash());
    assert_eq!(actual.global(child).unwrap().translation.x, 5.0);
    assert_eq!(baked.source_map["sceneDigest"], baked.digest);
    assert_eq!(
        baked.source_map["entities"]["child"]["declaration"]["field"],
        "/entities/0/id"
    );
}

#[test]
fn all_component_errors_include_the_file_and_exact_field_location() {
    let files = Files::new();
    let path = files.put(
        "scene.json",
        r#"{
  "units": "metres-y-up",
  "entities": [
    {"id":"a", "components": {"Transform": {"positon": [0,0,0]}}},
    {"id":"b", "components": {"Material": {"roughness": "bad"}}},
    {"id":"c", "components": {"Mesh": {"Sphere": {"radius": false}}}},
    {"id":"d", "components": {"NotRegistered": {}}}
  ]
}"#,
    );
    let errors = bake::compile(path, &Types::standard(), &[]).err().unwrap();
    for (line, field) in [
        (4, "/entities/0/components/Transform/positon"),
        (5, "/entities/1/components/Material/roughness"),
        (6, "/entities/2/components/Mesh/Sphere/radius"),
        (7, "/entities/3/components/NotRegistered"),
    ] {
        assert!(errors.contains(&format!("scene.json:{line}:")), "{errors}");
        assert!(errors.contains(field), "{errors}");
    }
}

#[test]
fn strict_authoring_does_not_change_lenient_save_decoding() {
    assert!(json::from_str::<Transform>(r#"{"typo":1,"position":[1,2]}"#).is_ok());
    for text in [
        r#"{"typo":1}"#,
        r#"{"position":[1,2]}"#,
        r#"{"position":[1,2,3,4]}"#,
        r#"{"position":[1,true,3]}"#,
    ] {
        assert!(json::from_str_strict::<Transform>(text).is_err(), "{text}");
    }
    assert!(json::from_str_strict::<(u32, u32)>("[1]").is_err());
    assert!(json::from_str_strict::<(u32, u32)>("[1,2,3]").is_err());
    assert!(json::from_str_strict::<Mesh>(r#"{"Asset":[]}"#).is_err());
    assert!(json::from_str_strict::<Mesh>(r#"{"Asset":["one","two"]}"#).is_err());
    let mesh: Mesh = json::from_str_strict(r#"{"Plane":{"width":4,"depth":7}}"#).unwrap();
    assert_eq!(mesh, Mesh::plane(4., 7.));
}

#[test]
fn identity_parent_and_dependency_errors_are_bake_errors() {
    let files = Files::new();
    for (entities, want) in [
        (
            value!([{"id":"a","components":{}},{"id":"a","components":{}}]),
            "duplicate identity",
        ),
        (
            value!([{"id":"a","parent":"missing","components":{}}]),
            "missing parent",
        ),
        (
            value!([{"id":"a","parent":"b","components":{}},{"id":"b","parent":"a","components":{}}]),
            "parent cycle",
        ),
        (value!([{"id":"a/b","components":{}}]), "local key"),
    ] {
        let error = bake::compile(files.scene(entities), &Types::standard(), &[])
            .err()
            .unwrap();
        assert!(
            error.contains(want) && error.contains("scene.json:"),
            "{error}"
        );
    }
    let path = files.put("scene.json", r#"{"units":"metres-y-up","fragments":{"recursive":"scene.json"},"entities":[{"id":"x","fragment":"recursive"}]}"#);
    let error = bake::compile(path, &Types::standard(), &[]).err().unwrap();
    assert!(
        error.contains("fragment dependency cycle") && error.contains("/entities/0"),
        "{error}"
    );
    let path = files.put(
        "duplicate.json",
        r#"{"units":"metres-y-up","entities":[],"entities":[]}"#,
    );
    assert!(bake::compile(path, &Types::standard(), &[])
        .err()
        .unwrap()
        .contains("duplicate field entities"));
}

fn fragment_scene(files: &Files, parameters: Value, overrides: Value) -> PathBuf {
    files.put("object.json", value!({"units":"metres-y-up","parameters":["at"],"entities":[
        {"id":"","components":{"Transform":{"position":{"$param":"at"}},"Material":{"color":[0,0,0,1],"roughness":0.9}}},
        {"id":"light","parent":"$root","components":{"Transform":{"position":[0,1,0]}}}
    ]}).to_string());
    files.put(
        "scene.json",
        value!({"units":"metres-y-up","fragments":{"object":"object.json"},"entities":[
            {"id":"one","fragment":"object","parameters":parameters,"overrides":overrides},
            {"id":"two","fragment":"object","parameters":{"at":[8,0,0]}}
        ]})
        .to_string(),
    )
}

#[test]
fn fragments_qualify_ids_substitute_values_and_replace_whole_components() {
    let files = Files::new();
    let path = fragment_scene(
        &files,
        value!({"at":[2,0,0]}),
        value!({"":{"Material":{"roughness":0.2}}}),
    );
    let types = Types::standard();
    let baked = bake::compile(path, &types, &[]).unwrap();
    let mut world = World::new(60, 0);
    types
        .prepare(&baked.content, &[])
        .unwrap()
        .instantiate(&mut world)
        .unwrap();
    assert_eq!(world.get::<Material>("one").unwrap().color, [1.; 4]); // no hidden deep merge
    assert_eq!(
        world.get::<Material>("two").unwrap().color,
        [0., 0., 0., 1.]
    );
    assert_eq!(
        world.get::<Parent>("one/light").unwrap().0,
        world.named("one").unwrap()
    );
    assert_eq!(
        world.get::<Parent>("two/light").unwrap().0,
        world.named("two").unwrap()
    );
    let link = &baked.source_map["entities"]["one"];
    assert!(link["declaration"]["file"]
        .as_str()
        .unwrap()
        .ends_with("object.json"));
    assert_eq!(link["instances"][0]["field"], "/entities/0");
    assert_eq!(
        link["components"]["Material"]["field"],
        "/entities/0/overrides//Material"
    );
}

#[test]
fn fragment_diagnostics_point_to_parameter_call_sites_and_unknown_overrides() {
    let files = Files::new();
    for (params, overrides, expected) in [
        (
            value!({"at":[1,"bad",3]}),
            value!({}),
            "/entities/0/parameters/at/1",
        ),
        (value!({}), value!({}), "parameters differ"),
        (
            value!({"at":[0,0,0],"typo":3}),
            value!({}),
            "parameters differ",
        ),
        (
            value!({"at":[0,0,0]}),
            value!({"missing":{"Material":{}}}),
            "override target one/missing",
        ),
    ] {
        let path = fragment_scene(&files, params, overrides);
        let error = bake::compile(path, &Types::standard(), &[]).err().unwrap();
        assert!(error.contains(expected), "{error}");
    }
}

#[test]
fn assets_bind_the_exact_baked_bytes_at_bake_and_runtime() {
    let files = Files::new();
    let digest = exact_game_scene::digest_bytes(b"fixture asset bytes");
    let asset = exact_game_scene::Asset::new("model.model", &digest);
    files.put("model.model", b"fixture asset bytes");
    let path = files.put("scene.json", value!({"units":"metres-y-up","assets":{"model":{"name":"model.model","path":"model.model"}},"entities":[
        {"id":"model","components":{"Mesh":{"Asset":[{"$asset":"model"}]}}}
    ]}).to_string());
    let types = Types::standard();
    let baked = bake::compile(&path, &types, &[asset]).unwrap();
    assert!(types
        .prepare(&baked.content, &[])
        .err()
        .unwrap()
        .contains("missing scene asset"));
    assert!(types
        .prepare(
            &baked.content,
            &[exact_game_scene::Asset::new(
                "model.model",
                &exact_game_scene::digest_bytes(b"different")
            )]
        )
        .err()
        .unwrap()
        .contains("digest differs"));
    assert!(types.prepare(&baked.content, &[asset]).is_ok());
    files.put("model.model", b"changed bytes");
    assert!(bake::compile(&path, &types, &[asset])
        .err()
        .unwrap()
        .contains("differs from the behavior module"));
    let raw = files.scene(value!([{"id":"model","components":{"Mesh":{"Asset":["model.model"]}}}]));
    assert!(bake::compile(raw, &types, &[asset])
        .err()
        .unwrap()
        .contains("undeclared asset"));
}

#[test]
fn no_source_paths_ship_and_content_identity_is_independent_of_file_location() {
    let first = Files::new();
    let second = Files::new();
    let entities = value!([{"id":"object","components":{"Transform":{"position":[1,2,3]}}}]);
    let a = bake::compile(first.scene(entities.clone()), &Types::standard(), &[]).unwrap();
    let b = bake::compile(second.scene(entities), &Types::standard(), &[]).unwrap();
    assert_eq!(a.digest, b.digest);
    assert_eq!(a.content, b.content);
    assert_ne!(a.source_map, b.source_map);
    assert!(
        a.source_map["files"][first.0.join("scene.json").to_str().unwrap()]
            .as_str()
            .is_some()
    );
    let decoded: Vec<_> = a
        .content
        .as_bytes()
        .chunks_exact(2)
        .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16).unwrap())
        .collect();
    assert!(!String::from_utf8_lossy(&decoded).contains(first.0.to_str().unwrap()));
    assert_eq!(digest_bytes(&decoded[9..]), a.digest);
}

#[derive(Default, Component)]
struct Bulk {
    samples: Vec<f32>,
}
#[derive(Default, Data)]
struct BulkInput {
    points: Vec<Vec3>,
}
#[test]
fn opaque_bulk_data_uses_an_explicit_typed_rust_constructor() {
    let files = Files::new();
    let path =
        files.scene(value!([{"id":"bulk","components":{"Bulk":{"points":[[1,2,3],[4,5,6]]}}}]));
    let mut types = Types::new();
    types.constructed::<BulkInput, Bulk>(
        |input| {
            Ok(Bulk {
                samples: input.points.iter().flat_map(|p| p.to_array()).collect(),
            })
        },
        |_| Ok(()),
    );
    let baked = bake::compile(path, &types, &[]).unwrap();
    let mut world = World::new(60, 0);
    types
        .prepare(&baked.content, &[])
        .unwrap()
        .instantiate(&mut world)
        .unwrap();
    assert_eq!(
        world.get::<Bulk>("bulk").unwrap().samples,
        [1., 2., 3., 4., 5., 6.]
    );
    let refused = files.scene(value!([{"id":"bulk","components":{"Bulk":{"samples":[1,2,3]}}}]));
    assert!(bake::compile(refused, &types, &[])
        .err()
        .unwrap()
        .contains("unknown field"));
}

#[test]
fn instantiation_conflicts_and_incompatible_saved_types_retain_the_world() {
    let files = Files::new();
    let path =
        files.scene(value!([{"id":"object","components":{"Transform":{"position":[1,2,3]}}}]));
    let types = Types::standard();
    let scene = bake::compile(path, &types, &[]).unwrap();
    let mut world = World::new(60, 0);
    types.register(&mut world);
    types
        .prepare(&scene.content, &[])
        .unwrap()
        .instantiate(&mut world)
        .unwrap();
    let before = world.hash();
    assert!(types
        .prepare(&scene.content, &[])
        .unwrap()
        .instantiate(&mut world)
        .is_err());
    assert_eq!(world.hash(), before);
    assert!(types.prepare("bad", &[]).is_err());
    let saved = world.save();
    let mut incompatible = World::new(60, 0);
    incompatible.spawn_named("retained", Material::default());
    let before = incompatible.hash();
    assert!(incompatible
        .load(&saved)
        .unwrap_err()
        .to_string()
        .contains("unregistered"));
    assert_eq!(incompatible.hash(), before);
    assert_eq!(
        bin::to_vec(&*world.get::<Transform>("object").unwrap()),
        bin::to_vec(&Transform::at(1., 2., 3.))
    );
}

#[test]
fn authoring_default_metadata_comes_from_the_registered_rust_writer() {
    let defaults = Types::standard().defaults().unwrap();
    assert_eq!(
        defaults["Transform"],
        json::to_string(&Transform::default()).unwrap()
    );
    assert_eq!(
        defaults["Material"],
        json::to_string(&Material::default()).unwrap()
    );
    assert_eq!(defaults["Mesh"], json::to_string(&Mesh::default()).unwrap());
}

#[derive(Default, Component)]
struct Animated {
    spring: exact_game::Spring,
    tween: exact_game::Tween,
}
#[test]
fn handwritten_nested_data_readers_cannot_silently_discard_authoring_fields() {
    let files = Files::new();
    let path = files.scene(value!([
        {"id":"a","components":{"Animated":{"spring":{"typo":1}}}},
        {"id":"b","components":{"Animated":{"tween":{"typo":1}}}}
    ]));
    let mut types = Types::new();
    types.component::<Animated>();
    let errors = bake::compile(path, &types, &[]).err().unwrap();
    assert!(
        errors.contains("/entities/0/components/Animated/spring/typo"),
        "{errors}"
    );
    assert!(
        errors.contains("/entities/1/components/Animated/tween/typo"),
        "{errors}"
    );
    assert!(json::from_str::<Animated>(r#"{"spring":{"typo":1},"tween":{"typo":1}}"#).is_ok());
}

fn scene_artifact(payload: &[u8]) -> String {
    b"EXSCENE\0\x01"
        .iter()
        .chain(payload)
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[derive(Default, Data)]
struct WireScene {
    entities: Vec<WireRow>,
}
#[derive(Default, Data)]
struct WireRow {
    id: String,
    components: std::collections::BTreeMap<String, Vec<u8>>,
}
fn component_scene(name: &str, component: Vec<u8>, count: usize) -> String {
    scene_artifact(&bin::to_vec(&WireScene {
        entities: (0..count)
            .map(|i| WireRow {
                id: format!("object-{i}"),
                components: [(name.into(), component.clone())].into(),
            })
            .collect(),
    }))
}

#[test]
fn compiled_scene_overcount_refuses_before_reading_the_first_row() {
    use exact_game::Writer;
    let mut writer = bin::Encoder::default();
    writer.begin_struct();
    writer.field("entities");
    writer.begin_seq(100_001);
    let mut payload = writer.finish();
    // Enough wire bytes for the declared count, but no valid Row at all. The
    // cardinality error must win before any Row is read or allocated.
    payload.extend(std::iter::repeat_n(255, 100_001));
    let error = Types::new()
        .prepare(&scene_artifact(&payload), &[])
        .err()
        .unwrap();
    assert!(error.contains("scene exceeds entity limit"), "{error}");
}

#[derive(Default, Component)]
struct NestedCollection {
    values: Vec<[u64; 32]>,
}
#[test]
fn opaque_component_collection_refuses_before_decoding_oversized_storage() {
    use exact_game::Writer;
    let count = 64 * 1024 * 1024 / std::mem::size_of::<[u64; 32]>() + 1;
    let mut writer = bin::Encoder::default();
    writer.begin_struct();
    writer.field("values");
    writer.begin_seq(count);
    let mut component = writer.finish();
    // A compact invalid first element proves the storage preflight happens
    // before element decoding, without constructing the large destination.
    component.extend(std::iter::repeat_n(255, count));
    let content = component_scene("NestedCollection", component, 1);
    let mut types = Types::new();
    types.component::<NestedCollection>();
    let error = types.prepare(&content, &[]).err().unwrap();
    assert!(
        error.contains("decoded size exceeds load budget"),
        "{error}"
    );
    assert!(error.contains("object-0.NestedCollection"), "{error}");
}

#[derive(Default)]
struct AccountedComponent;
impl Component for AccountedComponent {
    const NAME: &'static str = "AccountedComponent";
}
impl Data for AccountedComponent {
    fn write(&self, writer: &mut dyn exact_game::Writer) {
        writer.boolean(true);
    }
    fn read(&mut self, reader: &mut dyn exact_game::Reader) -> Result<(), exact_game::DataError> {
        reader.boolean()?;
        // Exercise shared accounting without allocating a large test payload.
        reader.claim(40 * 1024 * 1024)
    }
}
#[test]
fn separate_component_payloads_share_one_scene_allocation_allowance() {
    let mut types = Types::new();
    types.component::<AccountedComponent>();
    let component = bin::to_vec(&AccountedComponent);
    assert!(types
        .prepare(
            &component_scene("AccountedComponent", component.clone(), 1),
            &[]
        )
        .is_ok());
    let error = types
        .prepare(&component_scene("AccountedComponent", component, 2), &[])
        .err()
        .unwrap();
    assert!(
        error.contains("decoded size exceeds load budget"),
        "{error}"
    );
    assert!(error.contains("object-1.AccountedComponent"), "{error}");
}

#[test]
fn repeated_large_empty_fragments_share_a_read_budget_and_refuse() {
    let files = Files::new();
    let fragment = format!(
        "{}{}",
        " ".repeat(1024 * 1024),
        value!({"units":"metres-y-up","entities":[]})
    );
    files.put("empty.json", fragment);
    let path = files.put("scene.json", value!({"units":"metres-y-up",
        "fragments":{"empty":"empty.json"},
        "entities":(0..70).map(|i| value!({"id":format!("empty-{i}"),"fragment":"empty"})).collect::<Vec<_>>()
    }).to_string());
    let error = bake::compile(&path, &Types::standard(), &[]).err().unwrap();
    assert!(error.contains("scene work limit"), "{error}");
    // Removing the repeated work must accept the same empty fragment.
    files.put(
        "scene.json",
        value!({"units":"metres-y-up",
            "fragments":{"empty":"empty.json"},"entities":[{"id":"one","fragment":"empty"}]
        })
        .to_string(),
    );
    assert!(bake::compile(path, &Types::standard(), &[]).is_ok());
}
