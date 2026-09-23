use super::*;
fn flat(wave: Wave) -> Synth {
    Synth {
        wave,
        hz: 1.0,
        attack: 0.0,
        decay: 0.0,
        sustain: 1.0,
        release: 0.0,
        seconds: 1.0,
        gain: 1.0,
        ..Synth::default()
    }
}
#[test]
fn waves_match_closed_forms() {
    let cases = [
        (
            Wave::Sine,
            vec![
                0.0,
                math::sqrt(0.5),
                1.0,
                math::sqrt(0.5),
                0.0,
                -math::sqrt(0.5),
                -1.0,
                -math::sqrt(0.5),
            ],
        ),
        // Band-limited edges: the sample on a discontinuity takes its midpoint.
        (Wave::Square, vec![0.0, 1.0, 1.0, 1.0, 0.0, -1.0, -1.0, -1.0]),
        (Wave::Saw, vec![0.0, -0.75, -0.5, -0.25, 0.0, 0.25, 0.5, 0.75]),
        (
            Wave::Triangle,
            vec![-1.0, -0.5, 0.0, 0.5, 1.0, 0.5, 0.0, -0.5],
        ),
    ];
    for (wave, expected) in cases {
        for (a, b) in render(&flat(wave), 8).iter().zip(expected) {
            assert!((a - b).abs() < 0.000001, "{wave:?}: {a} != {b}");
        }
    }
    let noise = render(&flat(Wave::Noise), 8);
    // First two xorshift32 states from the specified seed: 0x87985aa5, 0x155b24a3.
    assert_eq!(noise[0], (0x87985au32 as f32) / 8388608.0 - 1.0);
    assert_eq!(noise[1], (0x155b24u32 as f32) / 8388608.0 - 1.0);
}
#[test]
fn adsr_and_filters() {
    let s = flat(Wave::Square)
        .hz(0.0)
        .attack(0.1)
        .decay(0.2)
        .sustain(0.5)
        .release(0.2);
    for (t, expected) in [
        (0.0, 0.0),
        (0.05, 0.5),
        (0.1, 1.0),
        (0.2, 0.75),
        (0.5, 0.5),
        (0.9, 0.25),
        (1.0, 0.0),
    ] {
        assert!((envelope(&s, t) - expected).abs() < 1e-6);
    }
    let dc = flat(Wave::Square).hz(0.0);
    let lp = render(&dc.clone().lowpass_hz(10.0), 1000);
    let hp = render(&dc.highpass_hz(10.0), 1000);
    assert!((lp[999] - 1.0).abs() < 1e-5);
    assert!(hp[999].abs() < 1e-5);
    // A band-limited square at exactly Nyquist is silent; a triangle alternates ±1.
    let nyquist = flat(Wave::Triangle).hz(500.0);
    let lp = render(&nyquist.clone().lowpass_hz(10.0), 1000);
    let hp = render(&nyquist.highpass_hz(10.0), 1000);
    assert!(lp[999].abs() < 0.04);
    assert!(hp[999].abs() > 0.9);
}
#[test]
fn chime_hash_is_pinned() {
    let chime = Synth::sine(880.0)
        .decay(0.6)
        .seconds(0.8)
        .layer(Synth::sine(1320.0).gain(0.4));
    let pcm = render(&chime, 48000);
    assert_eq!(pcm, render(&chime, 48000));
    assert_eq!(exact_game::hash::of(&pcm), 12744919313992958087);
}

/// Signal-to-alias ratio in dB. A 4,800-sample window holds a whole number of
/// periods, so harmonics fall on exact DFT bins; power anywhere else is aliasing
/// folded back from above Nyquist (in-band amplitude response does not count).
fn signal_to_alias_db(wave: Wave, hz: f32) -> f64 {
    let (rate, n) = (48_000u32, 4_800usize);
    let pcm = render(&flat(wave).hz(hz), rate);
    let window = &pcm[..n];
    let cycles = (hz as f64 * n as f64 / rate as f64).round() as usize;
    let total: f64 = window.iter().map(|x| (*x as f64).powi(2)).sum();
    let harmonic: f64 = (0..=n / 2)
        .step_by(cycles)
        .map(|bin| {
            let (re, im) = window.iter().enumerate().fold((0.0, 0.0), |(re, im), (i, x)| {
                let angle = std::f64::consts::TAU * (bin * i) as f64 / n as f64;
                (re + *x as f64 * angle.cos(), im - *x as f64 * angle.sin())
            });
            let scale = if bin == 0 || bin * 2 == n { 1.0 } else { 2.0 };
            scale * (re * re + im * im) / n as f64
        })
        .sum();
    10.0 * (harmonic / (total - harmonic).max(1e-30)).log10()
}
// Naive edges measured 21.2/15.3/11.5/9.6 dB (square) and 19.4/13.3/9.9/6.8 dB
// (saw) at these pitches; `cargo test -p exact-game-audio alias -- --nocapture`.
#[test]
fn square_and_saw_alias_measurement() {
    for wave in [Wave::Square, Wave::Saw] {
        for hz in [440.0, 1760.0, 3520.0, 7040.0] {
            let db = signal_to_alias_db(wave, hz);
            println!("{wave:?} {hz} Hz: {db:.1} dB");
            assert!(db > 22.0, "{wave:?} {hz} Hz aliases at {db:.1} dB");
        }
    }
}
