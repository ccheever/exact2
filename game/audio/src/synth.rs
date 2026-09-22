use exact_game::{
    audio::{Synth, Wave},
    math,
};

fn held(s: &Synth, t: f32) -> f32 {
    if t < s.attack {
        t / s.attack
    } else if t < s.attack + s.decay {
        math::lerp(1.0, s.sustain, (t - s.attack) / s.decay)
    } else {
        s.sustain
    }
}
fn envelope(s: &Synth, t: f32) -> f32 {
    let off = (s.seconds - s.release).max(0.0);
    if t >= s.seconds {
        0.0
    } else if t < off {
        held(s, t)
    } else if s.release > 0.0 {
        held(s, off) * (s.seconds - t) / (s.seconds - off)
    } else {
        held(s, t)
    }
}
pub(crate) fn sample_count(s: &Synth, rate: u32) -> usize {
    let len = math::ceil(s.duration() * rate as f32) as usize;
    if s.looping && len > 3 {
        len - (rate as usize / 100).max(2).min(len / 2)
    } else {
        len
    }
}
/// Pure mono PCM, using only libm math, fixed operation order and a local noise seed.
/// No normalization: authored layers can exceed ±1; device outputs handle clipping.
pub fn render(s: &Synth, sample_rate: u32) -> Vec<f32> {
    assert!(sample_rate > 0, "sample rate must be positive");
    s.validate();
    let mut out = vec![0.0; math::ceil(s.duration() * sample_rate as f32) as usize];
    render_into(s, sample_rate, &mut out);
    for sample in &mut out {
        *sample = if sample.is_finite() {
            sample.clamp(-4.0, 4.0)
        } else {
            0.0
        };
    }
    if s.looping && out.len() > 3 {
        // Overlap the tail onto the head, then remove that tail. The end of the
        // overlap joins the untouched head; the wrap joins adjacent tail samples.
        let n = (sample_rate as usize / 100).max(2).min(out.len() / 2);
        let end = out.len() - n;
        for i in 0..n {
            let angle = i as f32 / (n - 1) as f32 * std::f32::consts::FRAC_PI_2;
            out[i] = (out[end + i] * math::cos(angle) + out[i] * math::sin(angle)).clamp(-4.0, 4.0);
        }
        out.truncate(end);
    }
    out
}
fn render_into(s: &Synth, rate: u32, out: &mut [f32]) {
    let tau = std::f32::consts::TAU;
    let dt = 1.0 / rate as f32;
    let lp = if s.lowpass_hz > 0.0 {
        1.0 - math::exp(-tau * s.lowpass_hz * dt)
    } else {
        1.0
    };
    let hp = if s.highpass_hz > 0.0 {
        math::exp(-tau * s.highpass_hz * dt)
    } else {
        0.0
    };
    let (mut phase, mut low, mut high, mut previous) = (0.0, 0.0, 0.0, 0.0);
    let mut noise = 0x12345678u32;
    for (i, sample) in out
        .iter_mut()
        .enumerate()
        .take(math::ceil(s.seconds * rate as f32) as usize)
    {
        let t = i as f32 * dt;
        let x = match s.wave {
            Wave::Sine => math::sin(tau * phase),
            Wave::Square => {
                if phase < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            Wave::Saw => 2.0 * phase - 1.0,
            Wave::Triangle => 1.0 - 4.0 * (phase - 0.5).abs(),
            Wave::Noise => {
                noise ^= noise << 13;
                noise ^= noise >> 17;
                noise ^= noise << 5;
                (noise >> 8) as f32 / 8388608.0 - 1.0
            }
        };
        low += lp * (x - low);
        let filtered = if hp != 0.0 {
            high = hp * (high + low - previous);
            previous = low;
            high
        } else {
            low
        };
        *sample += filtered * envelope(s, t) * s.gain;
        let semitones = s.slide * t + s.vibrato_depth * math::sin(tau * s.vibrato_hz * t);
        let frequency = (s.hz * math::powf(2.0, semitones / 12.0)).min(rate as f32 * 0.5);
        phase += frequency * dt;
        phase -= math::floor(phase);
    }
    for layer in &s.layers {
        render_into(layer, rate, out);
    }
}

#[cfg(test)]
#[path = "synth_tests.rs"]
mod tests;
