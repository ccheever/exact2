//! The in-memory bake a game's hostless tests load (`exact_game::sim_with_art!`).
use exact_game::*;
use std::{fs, path::PathBuf};

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("game-art-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("logic/src")).unwrap();
    fs::write(dir.join("logic/src/lib.rs"), "").unwrap();
    fs::create_dir_all(dir.join("art")).unwrap();
    dir
}
/// One second of mono 16-bit PCM at 8 kHz holding `sample` throughout.
fn wav(sample: i16) -> Vec<u8> {
    let data: Vec<u8> = std::iter::repeat_n(sample.to_le_bytes(), 8000)
        .flatten()
        .collect();
    let mut fmt = Vec::new();
    for field in [
        &1u16.to_le_bytes()[..],
        &1u16.to_le_bytes(),
        &8000u32.to_le_bytes(),
    ] {
        fmt.extend(field);
    }
    fmt.extend(16000u32.to_le_bytes());
    fmt.extend(2u16.to_le_bytes());
    fmt.extend(16u16.to_le_bytes());
    let mut body = b"WAVE".to_vec();
    for (id, chunk) in [(b"fmt ", &fmt[..]), (b"data", &data[..])] {
        body.extend(id);
        body.extend((chunk.len() as u32).to_le_bytes());
        body.extend(chunk);
    }
    let mut out = b"RIFF".to_vec();
    out.extend((body.len() as u32).to_le_bytes());
    out.extend(body);
    out
}

#[test]
fn both_manifest_layouts_find_the_game_and_nothing_else_does() {
    let game = temp("layouts");
    fs::create_dir_all(game.join(".shells/logic")).unwrap();
    // The generated member, two levels below the game, and an authored one, one level.
    assert_eq!(
        exact_game_bake::game_dir(game.join(".shells/logic")).unwrap(),
        game
    );
    assert_eq!(exact_game_bake::game_dir(game.join("logic")).unwrap(), game);
    let outside = std::env::temp_dir().join(format!("game-art-none-{}", std::process::id()));
    fs::create_dir_all(&outside).unwrap();
    let refused = exact_game_bake::game_dir(&outside).unwrap_err();
    assert!(refused.contains("logic/src/lib.rs"), "{refused}");
    fs::remove_dir_all(game).unwrap();
    fs::remove_dir_all(outside).unwrap();
}

#[test]
fn the_memory_bake_is_byte_for_byte_what_bake_art_writes() {
    let game = temp("same-bytes");
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    fs::copy(fixtures.join("crate.gltf"), game.join("art/crate.gltf")).unwrap();
    fs::copy(fixtures.join("tone.ogg"), game.join("art/tone.ogg")).unwrap();
    fs::write(game.join("art/hum.wav"), wav(300)).unwrap();
    let memory = exact_game_bake::art(&game).unwrap();
    assert!(
        !game.join("assets").exists(),
        "the memory bake writes nothing"
    );
    exact_game_bake::bake_art(&game).unwrap();
    fn walk(root: &std::path::Path, dir: &std::path::Path, out: &mut Assets) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let name = path.strip_prefix(root).unwrap().to_str().unwrap();
                out.insert(name.replace('\\', "/"), fs::read(&path).unwrap());
            }
        }
    }
    type Assets = std::collections::BTreeMap<String, Vec<u8>>;
    let mut written = Assets::new();
    walk(&game.join("assets"), &game.join("assets"), &mut written);
    assert!(memory.contains_key("crate.model") && memory.contains_key("tone.sound"));
    assert_eq!(memory, written);
    let empty = temp("no-art");
    fs::remove_dir_all(empty.join("art")).unwrap();
    assert!(exact_game_bake::art(&empty).unwrap().is_empty());
    fs::remove_dir_all(empty).unwrap();
    fs::remove_dir_all(game).unwrap();
}

struct Hum;
impl Game for Hum {
    const ID: &'static str = "game-art-hum";
    const ASSETS: &'static [&'static str] = &["hum.sound"];
    type Args = ();
    fn setup(_: &mut World, _: &()) {}
    fn tick(_: &mut World, _: &Input, _: &()) {}
}
fn hum(game: &std::path::Path) -> Sim<Hum> {
    let art = exact_game_bake::art(game).unwrap();
    Sim::with_assets((), |name| art.get(name).cloned().ok_or(name.to_owned())).unwrap()
}

#[test]
fn a_changed_sound_refuses_the_old_save_by_name() {
    let game = temp("changed");
    fs::write(game.join("art/hum.wav"), wav(300)).unwrap();
    let mut before = hum(&game);
    before.run(100.);
    let saved = before.save().unwrap();
    hum(&game).restore(&saved).unwrap();
    fs::write(game.join("art/hum.wav"), wav(-300)).unwrap();
    let refused = hum(&game).restore(&saved).unwrap_err().to_string();
    assert!(
        refused.contains("asset `hum.sound` identity differs"),
        "{refused}"
    );
    fs::remove_dir_all(game).unwrap();
}

// game/recipes/blender-export.md: the two export faults the bake passes through, as a
// Blender export writes them, read back from the in-memory bake.
#[test]
fn a_double_sided_material_and_black_vertex_colours_bake_as_exported() {
    let game = temp("blender");
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let source: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(fixtures.join("crate.gltf")).unwrap()).unwrap();
    // Backface Culling unticked: the exporter writes doubleSided.
    let mut sided = source.clone();
    sided["materials"][0]["doubleSided"] = serde_json::json!(true);
    fs::write(game.join("art/sided.gltf"), sided.to_string()).unwrap();
    // A part joined without a colour attribute: COLOR_0 is (0, 0, 0, 1) on its 24 vertices.
    let black: Vec<u8> = [0f32, 0., 0., 1.]
        .repeat(24)
        .iter()
        .flat_map(|f| f.to_le_bytes())
        .collect();
    fs::write(game.join("art/joined.bin"), &black).unwrap();
    let mut joined = source;
    joined["buffers"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"byteLength": black.len(), "uri": "joined.bin"}));
    joined["bufferViews"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"buffer": 1, "byteLength": black.len()}));
    joined["accessors"].as_array_mut().unwrap().push(
        serde_json::json!({"bufferView": 4, "componentType": 5126, "count": 24, "type": "VEC4"}),
    );
    joined["meshes"][0]["primitives"][0]["attributes"]["COLOR_0"] = serde_json::json!(4);
    fs::write(game.join("art/joined.gltf"), joined.to_string()).unwrap();
    let art = exact_game_bake::art(&game).unwrap();
    let model = |name: &str| asset::decode_model(&art[name]).unwrap();
    let (sided, joined) = (model("sided.model"), model("joined.model"));
    assert!(sided.materials[0].double_sided);
    assert!(
        sided.meshes[0].colors.is_empty(),
        "no attribute draws white"
    );
    assert!(!joined.materials[0].double_sided);
    assert_eq!(joined.meshes[0].colors, [0., 0., 0., 1.].repeat(24));
    fs::remove_dir_all(game).unwrap();
}

struct Crates;
impl Game for Crates {
    const ID: &'static str = "game-art-crates";
    const ASSETS: &'static [&'static str] = &["crate.model", "pair.model"];
    type Args = ();
    fn setup(_: &mut World, _: &()) {}
    fn tick(_: &mut World, _: &Input, _: &()) {}
}

// LLP 1139.004 D3: a baked material keeps its glTF name, read through
// `world().model(..).materials`; the index stays what selects one.
#[test]
fn a_baked_material_keeps_its_gltf_name_and_its_index() {
    let game = temp("names");
    let scaffold = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../new-assets/art/crate.gltf");
    fs::copy(&scaffold, game.join("art/crate.gltf")).unwrap();
    // Two materials of one name and one with none, each on its own primitive.
    let mut pair: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&scaffold).unwrap()).unwrap();
    let crate_material = pair["materials"][0].clone();
    pair["materials"] = serde_json::json!([crate_material, crate_material, {}]);
    let primitive = pair["meshes"][0]["primitives"][0].clone();
    pair["meshes"][0]["primitives"] = (0..3)
        .map(|i| {
            let mut p = primitive.clone();
            p["material"] = i.into();
            p
        })
        .collect();
    fs::write(game.join("art/pair.gltf"), pair.to_string()).unwrap();
    let art = exact_game_bake::art(&game).unwrap();
    let sim = Sim::<Crates>::with_assets((), |name| art.get(name).cloned().ok_or(name.to_owned()))
        .unwrap();
    let names = |model: &str| -> Vec<String> {
        let model = sim.world().model(model).unwrap();
        model.materials.iter().map(|m| m.name.clone()).collect()
    };
    assert_eq!(names("crate.model"), ["Crate"]);
    assert_eq!(names("pair.model"), ["Crate", "Crate", ""]);
    let pair = sim.world().model("pair.model").unwrap();
    let used: Vec<u32> = pair.meshes.iter().map(|m| m.material).collect();
    assert_eq!(used, [0, 1, 2], "indices select, whatever the names");
    fs::remove_dir_all(game).unwrap();
}
