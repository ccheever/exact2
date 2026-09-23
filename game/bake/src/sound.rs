//! Build-time audio import. WAV (8/16/24/32-bit integer PCM, 32/64-bit float) and
//! Ogg Vorbis become 16-bit `.sound` records; no game module links a decoder.
//! @ref LLP 1046.003 §AU4 (where decoding happens)
use exact_game::asset::{SoundData, SOUND_BYTE_BUDGET, SOUND_RATES};
use std::path::Path;

/// Decode one `.wav` or `.ogg` file, refusing unsupported encodings by name.
pub fn sound(path: impl AsRef<Path>) -> Result<SoundData, String> {
    let path = path.as_ref();
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let decoded = match path.extension().and_then(|v| v.to_str()) {
        Some("wav") => wav(&bytes),
        Some("ogg") => vorbis(&bytes),
        _ => Err("expected a .wav or .ogg file".into()),
    };
    decoded
        .and_then(|(rate, channels, samples)| {
            if !SOUND_RATES.contains(&rate) {
                return Err(format!(
                    "{rate} Hz is outside {}..={} Hz",
                    SOUND_RATES.start(),
                    SOUND_RATES.end()
                ));
            }
            SoundData::encode(rate, channels, &samples)
        })
        .map_err(|e| format!("{}: {e}", path.display()))
}

fn quantize(x: f64) -> i16 {
    if x.is_finite() {
        (x * 32768.0).round().clamp(-32768.0, 32767.0) as i16
    } else {
        0
    }
}

fn channels(n: u32) -> Result<u32, String> {
    if (1..=2).contains(&n) {
        Ok(n)
    } else {
        Err(format!("{n} channels; expected mono or stereo"))
    }
}

fn wav(bytes: &[u8]) -> Result<(u32, u32, Vec<i16>), String> {
    let u16le = |b: &[u8], at: usize| u16::from_le_bytes([b[at], b[at + 1]]);
    let u32le = |b: &[u8], at: usize| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("not a RIFF/WAVE file".into());
    }
    let (mut at, mut format, mut data) = (12, None, None);
    while at + 8 <= bytes.len() {
        let size = u32le(bytes, at + 4) as usize;
        let body = bytes.get(at + 8..at + 8 + size).ok_or_else(|| {
            format!(
                "chunk `{}` is truncated",
                String::from_utf8_lossy(&bytes[at..at + 4])
            )
        })?;
        match &bytes[at..at + 4] {
            b"fmt " => format = Some(body),
            b"data" => data = Some(body),
            _ => {}
        }
        at += 8 + size + (size & 1);
    }
    let fmt = format.ok_or("missing fmt chunk")?;
    let data = data.ok_or("missing data chunk")?;
    if fmt.len() < 16 {
        return Err("fmt chunk is too short".into());
    }
    let mut tag = u16le(fmt, 0);
    if tag == 0xFFFE {
        // WAVE_FORMAT_EXTENSIBLE: the subformat GUID's first two bytes are the tag.
        const GUID_TAIL: [u8; 14] = [0, 0, 0, 0, 0x10, 0, 0x80, 0, 0, 0xAA, 0, 0x38, 0x9B, 0x71];
        if fmt.len() < 40 || fmt[26..40] != GUID_TAIL {
            return Err("extensible fmt chunk has an unknown subformat".into());
        }
        tag = u16le(fmt, 24);
    }
    let channels = channels(u32::from(u16le(fmt, 2)))?;
    let rate = u32le(fmt, 4);
    let bits = u16le(fmt, 14);
    let width = match (tag, bits) {
        (1, 8 | 16 | 24 | 32) | (3, 32 | 64) => usize::from(bits / 8),
        (1 | 3, _) => {
            return Err(format!(
                "{bits}-bit {} samples are unsupported",
                if tag == 1 { "integer" } else { "float" }
            ))
        }
        _ => {
            return Err(format!(
                "WAVE format {tag:#06x} is compressed; export PCM or float"
            ))
        }
    };
    let frame = width * channels as usize;
    if usize::from(u16le(fmt, 12)) != frame {
        return Err("block alignment does not match channels × sample width".into());
    }
    if !data.len().is_multiple_of(frame) {
        return Err("data chunk ends mid-frame".into());
    }
    if data.len() / width * 2 > SOUND_BYTE_BUDGET {
        return Err(budget(data.len() / width * 2, true));
    }
    let samples = data
        .chunks_exact(width)
        .map(|s| match (tag, width) {
            (1, 1) => (i16::from(s[0]) - 128) << 8,
            (1, 2) => i16::from_le_bytes([s[0], s[1]]),
            (1, 3) => {
                let v = i32::from_le_bytes([0, s[0], s[1], s[2]]) >> 8;
                ((v + 128) >> 8).clamp(-32768, 32767) as i16
            }
            (1, 4) => {
                let v = i64::from(i32::from_le_bytes(s.try_into().unwrap()));
                ((v + 32768) >> 16).clamp(-32768, 32767) as i16
            }
            (3, 4) => quantize(f64::from(f32::from_le_bytes(s.try_into().unwrap()))),
            _ => quantize(f64::from_le_bytes(s.try_into().unwrap())),
        })
        .collect();
    Ok((rate, channels, samples))
}

fn vorbis(bytes: &[u8]) -> Result<(u32, u32, Vec<i16>), String> {
    let mut reader = lewton::inside_ogg::OggStreamReader::new(std::io::Cursor::new(bytes))
        .map_err(|e| format!("Ogg Vorbis: {e}"))?;
    let channels = channels(u32::from(reader.ident_hdr.audio_channels))?;
    let rate = reader.ident_hdr.audio_sample_rate;
    let mut samples = Vec::new();
    while let Some(packet) = reader
        .read_dec_packet_itl()
        .map_err(|e| format!("Ogg Vorbis: {e}"))?
    {
        samples.extend(packet);
        if samples.len() * 2 > SOUND_BYTE_BUDGET {
            return Err(budget(samples.len() * 2, false));
        }
    }
    // The final page's granule position is the stream's length in frames; the
    // last packet decodes past it. lewton leaves this trim to its caller.
    if let Some(frames) = reader.get_last_absgp() {
        samples.truncate(samples.len().min(frames as usize * channels as usize));
    }
    Ok((rate, channels, samples))
}

fn budget(bytes: usize, exact: bool) -> String {
    let mib = |b: usize| format!("{:.1} MiB", b as f64 / (1024.0 * 1024.0));
    format!(
        "{}{} of 16-bit PCM exceeds the {} sound residency budget",
        if exact { "" } else { "more than " },
        mib(bytes),
        mib(SOUND_BYTE_BUDGET)
    )
}
