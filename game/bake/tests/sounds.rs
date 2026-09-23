//! WAV and Ogg Vorbis become 16-bit `.sound` records at bake time.
use exact_game::asset::{Content, SoundData};
use std::fs;

fn temp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "sound-bake-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("art")).unwrap();
    dir
}
/// A RIFF/WAVE file; `extensible` wraps the tag in WAVE_FORMAT_EXTENSIBLE.
fn wav(tag: u16, channels: u16, rate: u32, bits: u16, data: &[u8], extensible: bool) -> Vec<u8> {
    let align = channels * bits / 8;
    let mut fmt = Vec::new();
    fmt.extend((if extensible { 0xFFFE } else { tag }).to_le_bytes());
    fmt.extend(channels.to_le_bytes());
    fmt.extend(rate.to_le_bytes());
    fmt.extend((rate * u32::from(align)).to_le_bytes());
    fmt.extend(align.to_le_bytes());
    fmt.extend(bits.to_le_bytes());
    if extensible {
        fmt.extend(22u16.to_le_bytes());
        fmt.extend(bits.to_le_bytes());
        fmt.extend(3u32.to_le_bytes());
        fmt.extend(tag.to_le_bytes());
        fmt.extend([0, 0, 0, 0, 0x10, 0, 0x80, 0, 0, 0xAA, 0, 0x38, 0x9B, 0x71]);
    }
    let mut body = b"WAVE".to_vec();
    for (id, chunk) in [(b"fmt ", &fmt[..]), (b"LIST", &b"odd"[..]), (b"data", data)] {
        body.extend(id);
        body.extend((chunk.len() as u32).to_le_bytes());
        body.extend(chunk);
        if chunk.len() % 2 == 1 {
            body.push(0);
        }
    }
    let mut out = b"RIFF".to_vec();
    out.extend((body.len() as u32).to_le_bytes());
    out.extend(body);
    out
}
fn bake(name: &str, bytes: &[u8]) -> Result<SoundData, String> {
    let dir = temp(name);
    let path = dir.join(format!("art/{name}"));
    fs::write(&path, bytes).unwrap();
    let result = exact_game_bake::sound(&path);
    fs::remove_dir_all(dir).unwrap();
    result
}

#[test]
fn integer_and_float_wav_quantize_to_sixteen_bits() {
    let pcm16: Vec<u8> = [0i16, 1, -1, i16::MAX, i16::MIN, 1234]
        .iter()
        .flat_map(|s| s.to_le_bytes())
        .collect();
    let mono = bake("a.wav", &wav(1, 1, 22_050, 16, &pcm16, false)).unwrap();
    assert_eq!((mono.rate, mono.channels, mono.frames), (22_050, 1, 6));
    assert_eq!(mono.samples(), [0, 1, -1, i16::MAX, i16::MIN, 1234]);
    let ext = bake("b.wav", &wav(1, 2, 44_100, 16, &pcm16, true)).unwrap();
    assert_eq!((ext.channels, ext.frames), (2, 3));
    assert_eq!(ext.samples(), [0, 1, -1, i16::MAX, i16::MIN, 1234]);

    let u8s = bake("c.wav", &wav(1, 1, 8_000, 8, &[0, 128, 255], false)).unwrap();
    assert_eq!(u8s.samples(), [-32768, 0, 32512]);

    // 24-bit rounds to nearest 16-bit value, half up, and clamps at full scale.
    let pcm24: Vec<u8> = [0x007F_FFFFi32, -0x0080_0000, 0x80, 0x7F, -0x81, 0x0012_3456]
        .iter()
        .flat_map(|s| s.to_le_bytes()[..3].to_vec())
        .collect();
    let stereo = bake("d.wav", &wav(1, 2, 48_000, 24, &pcm24, true)).unwrap();
    assert_eq!((stereo.channels, stereo.frames), (2, 3));
    assert_eq!(stereo.samples(), [32767, -32768, 1, 0, -1, 0x1234]);

    let pcm32: Vec<u8> = [i32::MAX, i32::MIN, 0x8000]
        .iter()
        .flat_map(|s| s.to_le_bytes())
        .collect();
    let wide = bake("e.wav", &wav(1, 1, 96_000, 32, &pcm32, false)).unwrap();
    assert_eq!(wide.samples(), [32767, -32768, 1]);

    let f32s: Vec<u8> = [0.5f32, 1.0, -1.0, f32::NAN, -0.25, 2.0]
        .iter()
        .flat_map(|s| s.to_le_bytes())
        .collect();
    let float = bake("f.wav", &wav(3, 1, 32_000, 32, &f32s, false)).unwrap();
    assert_eq!(float.samples(), [16384, 32767, -32768, 0, -8192, 32767]);
    let f64s: Vec<u8> = [0.5f64, -1.5]
        .iter()
        .flat_map(|s| s.to_le_bytes())
        .collect();
    let double = bake("g.wav", &wav(3, 1, 32_000, 64, &f64s, true)).unwrap();
    assert_eq!(double.samples(), [16384, -32768]);
}

#[test]
fn unsupported_wav_is_refused_by_name() {
    let two = [0u8; 4];
    let refuse = |bytes: Vec<u8>, expected: &str| {
        let error = bake("r.wav", &bytes).unwrap_err();
        assert!(error.contains(expected), "{error} lacks {expected}");
    };
    refuse(
        wav(1, 3, 48_000, 16, &[0; 6], false),
        "3 channels; expected mono or stereo",
    );
    refuse(
        wav(2, 1, 48_000, 4, &two, false),
        "format 0x0002 is compressed",
    );
    refuse(
        wav(1, 1, 48_000, 12, &two, false),
        "12-bit integer samples are unsupported",
    );
    refuse(
        wav(3, 1, 48_000, 16, &two, false),
        "16-bit float samples are unsupported",
    );
    refuse(
        wav(1, 1, 4_000, 16, &two, false),
        "4000 Hz is outside 8000..=192000 Hz",
    );
    refuse(
        wav(1, 2, 48_000, 16, &[0; 6], false),
        "data chunk ends mid-frame",
    );
    refuse(wav(1, 1, 48_000, 16, &[], false), "sound has no frames");
    refuse(b"RIFX0000WAVE".to_vec(), "not a RIFF/WAVE file");
    let mut truncated = wav(1, 1, 48_000, 16, &two, false);
    truncated.truncate(truncated.len() - 1);
    refuse(truncated, "chunk `data` is truncated");
    let mut unknown = wav(1, 1, 48_000, 16, &two, true);
    unknown[20 + 26] ^= 1; // corrupt the subformat GUID's tail
    refuse(unknown, "unknown subformat");
}

#[test]
fn oversized_pcm_is_refused_before_decoding() {
    let data = vec![0u8; exact_game::asset::SOUND_BYTE_BUDGET + 2];
    let error = bake("big.wav", &wav(1, 1, 48_000, 16, &data, false)).unwrap_err();
    assert!(
        error.contains("exceeds the 32.0 MiB sound residency budget"),
        "{error}"
    );
}

#[test]
fn ogg_vorbis_decodes_at_bake_time() {
    // Stereo 22,050 Hz, 0.25 s: left 440 Hz at 0.5, right 660 Hz at 0.25 (libvorbis).
    let sound = exact_game_bake::sound(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/tone.ogg"
    ))
    .unwrap();
    assert_eq!(
        (sound.rate, sound.channels, sound.frames),
        (22_050, 2, 5_512)
    );
    let samples = sound.samples();
    let channel = |c: usize| {
        samples
            .iter()
            .skip(c)
            .step_by(2)
            .map(|s| f64::from(*s) / 32768.0)
    };
    let rms = |c: usize| (channel(c).map(|x| x * x).sum::<f64>() / 5_512.0).sqrt();
    let crossings = |c: usize| {
        let values: Vec<f64> = channel(c).collect();
        values
            .windows(2)
            .filter(|w| w[0] < 0.0 && w[1] >= 0.0)
            .count()
    };
    assert!((rms(0) - 0.5 / 2f64.sqrt()).abs() < 0.02, "{}", rms(0));
    assert!((rms(1) - 0.25 / 2f64.sqrt()).abs() < 0.02, "{}", rms(1));
    // 0.25 s of 440 Hz and 660 Hz: 110 and 165 upward crossings.
    assert!((109..=111).contains(&crossings(0)), "{}", crossings(0));
    assert!((164..=166).contains(&crossings(1)), "{}", crossings(1));
    let error = bake("bad.ogg", b"OggS but not really").unwrap_err();
    assert!(error.contains("Ogg Vorbis"), "{error}");
}

#[test]
fn art_bake_writes_sound_assets_and_refuses_duplicate_stems() {
    let app = temp("art");
    let pcm: Vec<u8> = (0..64i16).flat_map(|s| (s * 100).to_le_bytes()).collect();
    fs::write(app.join("art/blip.wav"), wav(1, 1, 22_050, 16, &pcm, false)).unwrap();
    fs::copy(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/tone.ogg"),
        app.join("art/tone.ogg"),
    )
    .unwrap();
    exact_game_bake::bake_art(&app).unwrap();
    for name in ["blip.sound", "tone.sound"] {
        let bytes = fs::read(app.join("assets").join(name)).unwrap();
        let Ok(Content::Sound(sound)) = Content::decode::<false>(name, &bytes) else {
            panic!("{name} does not decode in a primitive module");
        };
        assert!(sound.frames > 0);
    }
    fs::copy(app.join("art/blip.wav"), app.join("art/tone.wav")).unwrap();
    let error = exact_game_bake::bake_art(&app).unwrap_err();
    assert!(error.contains("duplicate art stem tone.sound"), "{error}");
    fs::remove_file(app.join("art/tone.wav")).unwrap();
    fs::remove_file(app.join("art/tone.ogg")).unwrap();
    exact_game_bake::bake_art(&app).unwrap();
    assert!(
        !app.join("assets/tone.sound").exists(),
        "pruned with its art"
    );
    fs::remove_dir_all(app).unwrap();
}
