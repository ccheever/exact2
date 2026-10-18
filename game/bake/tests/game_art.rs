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
    assert!(exact_game_bake::art(temp("no-art").join("missing"))
        .unwrap()
        .is_empty());
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
