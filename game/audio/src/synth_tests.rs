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
        (
            Wave::Square,
            vec![1.0, 1.0, 1.0, 1.0, -1.0, -1.0, -1.0, -1.0],
        ),
        (
            Wave::Saw,
            vec![-1.0, -0.75, -0.5, -0.25, 0.0, 0.25, 0.5, 0.75],
        ),
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
    let nyquist = flat(Wave::Square).hz(500.0);
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
    assert_eq!(exact_game::hash::of(&pcm), 16834850980301692708);
}
