//! `cargo run -p exact-game-audio --example demo -- /tmp/audio-demo [--play]`
use exact_game::{audio::Synth, hash};
use exact_game_audio::render;
#[cfg(target_os = "macos")]
use exact_game_audio::Output;
use std::{fs, io::Write, path::Path};
fn wav(path: &Path, pcm: &[f32], rate: u32) -> std::io::Result<()> {
    let mut f = fs::File::create(path)?;
    let bytes = pcm.len() as u32 * 2;
    f.write_all(b"RIFF")?;
    f.write_all(&(36 + bytes).to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?;
    f.write_all(&rate.to_le_bytes())?;
    f.write_all(&(rate * 2).to_le_bytes())?;
    f.write_all(&2u16.to_le_bytes())?;
    f.write_all(&16u16.to_le_bytes())?;
    f.write_all(b"data")?;
    f.write_all(&bytes.to_le_bytes())?;
    for &sample in pcm {
        f.write_all(&((sample.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())?;
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let dir = args
        .iter()
        .find(|s| !s.starts_with("--"))
        .ok_or("usage: demo DIRECTORY [--play]")?;
    fs::create_dir_all(dir)?;
    let play = args.iter().any(|s| s == "--play");
    #[cfg(target_os = "macos")]
    let mut output = if play {
        Some(exact_game_audio::AppleOutput::new()?)
    } else {
        None
    };
    #[cfg(not(target_os = "macos"))]
    if play {
        return Err("--play is supported on macOS only; Linux uses NullOutput".into());
    }
    let sounds = [
        (
            "chime",
            Synth::sine(880.0)
                .decay(0.6)
                .seconds(0.8)
                .layer(Synth::sine(1320.0).gain(0.4)),
        ),
        (
            "footstep",
            Synth::noise()
                .attack(0.002)
                .decay(0.08)
                .seconds(0.12)
                .lowpass_hz(1800.0)
                .highpass_hz(120.0)
                .gain(0.7),
        ),
        (
            "thud",
            Synth::sine(100.0)
                .slide(-36.0)
                .attack(0.002)
                .decay(0.25)
                .seconds(0.35)
                .gain(0.8)
                .layer(
                    Synth::noise()
                        .decay(0.025)
                        .seconds(0.05)
                        .lowpass_hz(600.0)
                        .gain(0.3),
                ),
        ),
        (
            "wind",
            Synth::noise()
                .attack(0.0)
                .decay(0.0)
                .sustain(1.0)
                .release(0.0)
                .looped()
                .seconds(2.0)
                .lowpass_hz(500.0)
                .highpass_hz(60.0)
                .gain(0.5),
        ),
        (
            "night-sting",
            Synth::triangle(220.0)
                .slide(-8.0)
                .attack(0.02)
                .decay(0.8)
                .seconds(1.2)
                .lowpass_hz(2200.0)
                .vibrato_hz(6.0)
                .vibrato_depth(0.4)
                .gain(0.4)
                .layer(Synth::sine(233.08).decay(0.7).seconds(1.0).gain(0.3)),
        ),
    ];
    for (name, synth) in sounds {
        let pcm = render(&synth, 48000);
        let rms = (pcm.iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / pcm.len() as f64).sqrt();
        let peak = pcm.iter().fold(0.0f32, |a, x| a.max(x.abs()));
        println!(
            "{name}: samples={} rms={rms:.6} peak={peak:.6} hash={:016x}",
            pcm.len(),
            hash::of(&pcm)
        );
        wav(&Path::new(dir).join(format!("{name}.wav")), &pcm, 48000)?;
        #[cfg(target_os = "macos")]
        if let Some(output) = &mut output {
            let pcm = exact_game_audio::Pcm::F32(pcm.into());
            assert!(output.start(0, &pcm, 48000, name == "wind", 0, 1.0));
            output.set(0, 0.7, 0.7);
            output.flush();
            std::thread::sleep(std::time::Duration::from_secs(2));
            output.stop(0);
            output.flush();
        }
    }
    Ok(())
}
