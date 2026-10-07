//! LLP 1096 D1, D2, D10, §5: a declared WAV is read and checked before any
//! host plays it, the three commands are checked, and `expect sound` parses
//! and reaches the driver.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A WAV: `code` (1 integer, 3 float, 0xFFFE extensible naming 1 or 3),
/// `bits`, `channels`, `rate`, `frames`, and chunks before `data`.
fn wav(code: u16, bits: u16, channels: u16, rate: u32, frames: u32, before_data: &[u8]) -> Vec<u8> {
    let block = channels * bits / 8;
    let mut fmt = Vec::new();
    for x in [code, channels] {
        fmt.extend_from_slice(&x.to_le_bytes());
    }
    fmt.extend_from_slice(&rate.to_le_bytes());
    fmt.extend_from_slice(&(rate * u32::from(block)).to_le_bytes());
    for x in [block, bits] {
        fmt.extend_from_slice(&x.to_le_bytes());
    }
    if code == 0xFFFE {
        for x in [22u16, bits] {
            fmt.extend_from_slice(&x.to_le_bytes());
        }
        fmt.extend_from_slice(&3u32.to_le_bytes());
        // KSDATAFORMAT_SUBTYPE_IEEE_FLOAT: the format code, then the GUID's tail.
        fmt.extend_from_slice(&3u16.to_le_bytes());
        fmt.extend_from_slice(&[0, 0, 0, 0, 0x10, 0, 0x80, 0, 0, 0xaa, 0, 0x38, 0x9b, 0x71]);
    }
    let data = frames * u32::from(block);
    let mut body = b"WAVEfmt ".to_vec();
    body.extend_from_slice(&(fmt.len() as u32).to_le_bytes());
    body.extend_from_slice(&fmt);
    body.extend_from_slice(before_data);
    body.extend_from_slice(b"data");
    body.extend_from_slice(&data.to_le_bytes());
    body.resize(body.len() + data as usize, 0);
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    out
}

/// A chunk of `id` holding `n` bytes, padded to even.
fn chunk(id: &[u8; 4], n: u32) -> Vec<u8> {
    let mut c = id.to_vec();
    c.extend_from_slice(&n.to_le_bytes());
    c.resize(8 + n as usize + (n as usize & 1), 7);
    c
}

struct App(PathBuf);

impl App {
    fn new(files: &[(&str, Vec<u8>)], source: &str) -> App {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "exact-contract-sound-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(dir.join("assets")).unwrap();
        for (name, bytes) in files {
            std::fs::write(dir.join(name), bytes).unwrap();
        }
        std::fs::write(dir.join("app.contract"), source).unwrap();
        App(dir)
    }

    fn compile(&self) -> Result<exact_plan::Plan, contract::CompileError> {
        contract::compile_path(&self.0.join("app.contract"))
    }
}

impl Drop for App {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const PLAY: &str = "component A\n  state hat = \"assets/b.wav\"\n  action go\n    playSound(\"assets/a.wav\", at=now() + 100, gain=0.8, group=\"kick\")\n    playSound(hat)\n    stopSounds(group=\"hat\")\n    stopSounds()\n  view\n    button \"go\" press=go\n";

#[test]
fn integer_float_and_extensible_wavs_are_read_to_the_plan() {
    let mut chunks = chunk(b"fact", 4);
    chunks.extend(chunk(b"LIST", 5));
    let app = App::new(
        &[
            ("assets/a.wav", wav(1, 16, 1, 44_100, 4_410, &[])),
            ("assets/b.wav", wav(3, 32, 2, 48_000, 960, &chunks)),
            ("assets/c.wav", wav(0xFFFE, 32, 2, 96_000, 9_600, &[])),
        ],
        &format!("sound \"assets/a.wav\"\nsound \"assets/b.wav\"\nsound \"assets/c.wav\"\n{PLAY}"),
    );
    let plan = app.compile().unwrap();
    let rows: Vec<_> = plan
        .sounds
        .iter()
        .map(|r| {
            (
                plan.str(r.src).to_string(),
                r.frames,
                r.rate,
                r.channels,
                plan.str(r.digest).len(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            ("assets/a.wav".to_string(), 4_410, 44_100, 1, 64),
            ("assets/b.wav".to_string(), 960, 48_000, 2, 64),
            ("assets/c.wav".to_string(), 9_600, 96_000, 2, 64),
        ]
    );
    // Without its directory a declaration has nothing to read.
    let e = contract::compile(&format!("sound \"assets/a.wav\"\n{PLAY}")).unwrap_err();
    assert_eq!(e.id, "lower-sound-path");
}

#[test]
fn every_refusal_names_the_file_and_what_to_do() {
    let cases: [(&str, Vec<u8>, &str, &str); 5] = [
        (
            "assets/a.mp3",
            b"ID3\x04\x00\x00\x00\x00\x00\x00".to_vec(),
            "lower-sound-format",
            "sound `assets/a.mp3` is not a WAV file (no RIFF/WAVE header); a sound is a WAV of 16-bit integer or 32-bit float PCM, one or two channels, at 8–96 kHz: convert it: `afconvert -f WAVE -d LEI16 in.m4a out.wav`, or `ffmpeg -i in.mp3 -c:a pcm_s16le out.wav`",
        ),
        (
            "assets/a.wav",
            wav(1, 24, 1, 48_000, 10, &[]),
            "lower-sound-format",
            "sound `assets/a.wav` is 24-bit integer PCM; a sound is a WAV of 16-bit integer or 32-bit float PCM, one or two channels, at 8–96 kHz: convert it: `afconvert -f WAVE -d LEI16 in.m4a out.wav`, or `ffmpeg -i in.mp3 -c:a pcm_s16le out.wav`",
        ),
        (
            "assets/a.wav",
            wav(1, 16, 3, 48_000, 10, &[]),
            "lower-sound-format",
            "sound `assets/a.wav` has 3 channels; a sound is a WAV of 16-bit integer or 32-bit float PCM, one or two channels, at 8–96 kHz: convert it: `afconvert -f WAVE -d LEI16 in.m4a out.wav`, or `ffmpeg -i in.mp3 -c:a pcm_s16le out.wav`",
        ),
        (
            "assets/a.wav",
            wav(1, 16, 1, 8_000, 88_000, &[]),
            "lower-sound-long",
            "sound `assets/a.wav` is 11.0 s; a sound is a short clip, at most 10 s: play longer audio with `audio`",
        ),
        (
            "a.wav",
            wav(1, 16, 1, 48_000, 10, &[]),
            "lower-sound-path",
            "sound source `a.wav` must be under the app's `assets/` directory",
        ),
    ];
    for (path, bytes, id, message) in cases {
        let app = App::new(
            &[(path, bytes)],
            &format!("sound \"{path}\"\ncomponent A\n  view\n    text \"a\"\n"),
        );
        let e = app.compile().unwrap_err();
        assert_eq!((e.id.as_str(), e.message.as_str()), (id, message), "{path}");
    }
    let app = App::new(
        &[],
        "sound \"assets/gone.wav\"\ncomponent A\n  view\n    text \"a\"\n",
    );
    assert_eq!(app.compile().unwrap_err().id, "lower-sound-unreadable");
}

#[test]
fn the_checker_reads_literal_sources_and_named_arguments() {
    let declared = "sound \"assets/kit/kick.wav\"\nsound \"assets/kit/hat.wav\"\n";
    let files = [
        ("assets/kit/kick.wav", wav(1, 16, 1, 48_000, 480, &[])),
        ("assets/kit/hat.wav", wav(1, 16, 1, 48_000, 480, &[])),
    ];
    let app = |body: &str| {
        let dir = App::new(&[], "");
        std::fs::create_dir_all(dir.0.join("assets/kit")).unwrap();
        for (name, bytes) in &files {
            std::fs::write(dir.0.join(name), bytes).unwrap();
        }
        std::fs::write(
            dir.0.join("app.contract"),
            format!("{declared}component A\n  state src = \"assets/kit/kick.wav\"\n  action go\n    {body}\n  view\n    button \"go\" press=go\n"),
        )
        .unwrap();
        dir
    };
    for ok in [
        "playSound(\"assets/kit/hat.wav\", at=now() + 100, gain=0.8, group=\"hat\")",
        "playSound(src)",
        "stopSounds(group=\"hat\")",
        "stopSounds()",
    ] {
        app(ok).compile().unwrap_or_else(|e| panic!("{ok}: {e}"));
    }
    let e = app("playSound(\"assets/kit/hats.wav\")")
        .compile()
        .unwrap_err();
    assert_eq!(e.id, "type-sound-undeclared");
    assert_eq!(
        e.message,
        "`assets/kit/hats.wav` is not a declared sound (declared, nearest first: `assets/kit/hat.wav`, `assets/kit/kick.wav`); declare it at the top level: `sound \"assets/kit/hats.wav\"`"
    );
    for bad in [
        "playSound(\"assets/kit/hat.wav\", gain=1.5)",
        "playSound(\"assets/kit/hat.wav\", pan=1)",
        "playSound(\"assets/kit/hat.wav\", at=\"soon\")",
        "playSound(at=0)",
    ] {
        assert_eq!(
            app(bad).compile().unwrap_err().id,
            "type-play-sound",
            "{bad}"
        );
    }
}

#[test]
fn sound_is_a_word_only_at_the_top_level() {
    contract::compile("component A\n  state sound = true\n  action flip\n    sound = not sound\n  view\n    button \"s\" press=flip\n")
        .unwrap();
}

#[test]
fn expect_sound_parses_and_reaches_the_driver() {
    let src = "test \"kicks\"\n  tap \"play\"\n  expect sound has \"assets/kit/kick.wav\" at 0\n  expect sound missing \"assets/kit/kick.wav\" at 125\n  expect sound has \"assets/kit/kick.wav\" at 500 gain 0.8 ends 550 by cancelled\n";
    let tests = contract::tests(src).unwrap();
    let json = contract::tests_json(&tests);
    for want in [
        r#"{"op":"expect-sound","src":"assets/kit/kick.wav","present":true,"at":0,"line":3}"#,
        r#"{"op":"expect-sound","src":"assets/kit/kick.wav","present":false,"at":125,"line":4}"#,
        r#"{"op":"expect-sound","src":"assets/kit/kick.wav","present":true,"at":500,"gain":0.8,"ends":550,"by":"cancelled","line":5}"#,
    ] {
        assert!(json.contains(want), "{want}\n{json}");
    }
    for (bad, why) in [
        ("expect sound \"assets/k.wav\"", "or `missing`"),
        (
            "expect sound has \"assets/k.wav\" by soon",
            "a voice ends by",
        ),
        ("expect sound has \"assets/k.wav\" at 1 at 2", "given twice"),
        (
            "expect sound has \"assets/k.wav\" pitch 1",
            "no clause `pitch`",
        ),
    ] {
        let e = contract::tests(&format!("test \"t\"\n  {bad}\n")).unwrap_err();
        assert!(e.message.contains(why), "{bad}: {e}");
    }
}
