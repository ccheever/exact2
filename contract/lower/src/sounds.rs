//! Declared sounds (LLP 1096 D1): each `sound "assets/…wav"` is read here,
//! its RIFF chunks walked to `data`, and refused unless every host can play
//! it: 16-bit integer or 32-bit float PCM, one or two channels, 8–96 kHz, at
//! most 10 s. The plan's `sounds` row carries what the runner needs (frames
//! and rate, for a voice's length) and what the hosts decode (the path).

use super::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::Component as PathComponent;

/// "A sound is a short clip" (LLP 1096 §9 Q4).
const LONGEST_SECONDS: f64 = 10.0;

const CONVERT: &str = "convert it: `afconvert -f WAVE -d LEI16 in.m4a out.wav`, or `ffmpeg -i in.mp3 -c:a pcm_s16le out.wav`";

/// What the header says.
#[derive(Debug, PartialEq)]
pub(crate) struct Wav {
    pub(crate) frames: u32,
    pub(crate) rate: u32,
    pub(crate) channels: u8,
}

impl Lowerer<'_> {
    pub(super) fn declare_sounds(
        &mut self,
        file: &File,
        asset_root: Option<&Path>,
    ) -> Result<(), LowerError> {
        let Some(first) = file.sounds.first() else {
            return Ok(());
        };
        let Some(root) = asset_root else {
            return err(
                "lower-sound-path",
                "a sound source is relative to its app directory; compile this source with `compile_path`",
                first.span,
            );
        };
        let root = root.canonicalize().map_err(|e| LowerError {
            id: "lower-sound-unreadable",
            message: format!("sound asset root `{}` is unreadable: {e}", root.display()),
            span: first.span,
        })?;
        let mut seen = BTreeSet::new();
        for sound in &file.sounds {
            if !seen.insert(sound.source.as_str()) {
                continue;
            }
            let bytes = read(&root, &sound.source, sound.span)?;
            let wav = parse(&bytes).map_err(|why| LowerError {
                id: "lower-sound-format",
                message: format!(
                    "sound `{}` {why}; a sound is a WAV of 16-bit integer or 32-bit float PCM, one or two channels, at 8–96 kHz: {CONVERT}",
                    sound.source
                ),
                span: sound.span,
            })?;
            let seconds = f64::from(wav.frames) / f64::from(wav.rate);
            if seconds > LONGEST_SECONDS {
                return err(
                    "lower-sound-long",
                    format!(
                        "sound `{}` is {seconds:.1} s; a sound is a short clip, at most 10 s: play longer audio with `audio`",
                        sound.source
                    ),
                    sound.span,
                );
            }
            let digest: String = Sha256::digest(&bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            self.b
                .sound(&sound.source, wav.frames, wav.rate, wav.channels, &digest);
        }
        Ok(())
    }
}

/// The file's bytes, after `font`'s path rules (`fonts.rs`): a portable
/// relative path under the app's `assets/`, inside the app directory.
fn read(root: &Path, source: &str, span: Span) -> Result<Vec<u8>, LowerError> {
    let path = Path::new(source);
    let inside = exact_plan::is_portable_asset_path(source)
        && !path.is_absolute()
        && !path.components().any(|c| {
            matches!(
                c,
                PathComponent::ParentDir | PathComponent::RootDir | PathComponent::Prefix(_)
            )
        });
    if !inside {
        return err(
            "lower-sound-path",
            format!(
                "sound source `{source}` must be a portable relative path under the app directory"
            ),
            span,
        );
    }
    if !matches!(path.components().next(), Some(PathComponent::Normal(first)) if first == "assets")
    {
        return err(
            "lower-sound-path",
            format!("sound source `{source}` must be under the app's `assets/` directory"),
            span,
        );
    }
    let unreadable = |e: String| LowerError {
        id: "lower-sound-unreadable",
        message: format!("sound source `{source}` is unreadable: {e}"),
        span,
    };
    let canonical = root
        .join(path)
        .canonicalize()
        .map_err(|e| unreadable(e.to_string()))?;
    if !canonical.starts_with(root) {
        return Err(unreadable("it resolves outside the app directory".into()));
    }
    std::fs::read(&canonical).map_err(|e| unreadable(e.to_string()))
}

/// Walk a RIFF/WAVE file's chunks to `data`, skipping any it does not need
/// (`fact`, `LIST`, `bext`, …). The reason is a phrase after the path.
pub(crate) fn parse(bytes: &[u8]) -> Result<Wav, String> {
    let u16_at = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]);
    let u32_at = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("is not a WAV file (no RIFF/WAVE header)".into());
    }
    let mut at = 12;
    let mut format: Option<(u16, u16, u32, u16, u16)> = None;
    while at + 8 <= bytes.len() {
        let (id, size) = (&bytes[at..at + 4], u32_at(at + 4) as usize);
        let body = at + 8;
        let end = body.saturating_add(size).min(bytes.len());
        match id {
            b"fmt " => {
                if end - body < 16 {
                    return Err("has a `fmt ` chunk too short to read".into());
                }
                let mut code = u16_at(body);
                let bits = u16_at(body + 14);
                // WAVE_FORMAT_EXTENSIBLE: the sub-format GUID's first two
                // bytes are the format it names.
                if code == 0xFFFE {
                    if end - body < 40 {
                        return Err("has an extensible `fmt ` chunk too short to read".into());
                    }
                    code = u16_at(body + 24);
                }
                format = Some((
                    code,
                    u16_at(body + 2),
                    u32_at(body + 4),
                    u16_at(body + 12),
                    bits,
                ));
            }
            b"data" => {
                let Some((code, channels, rate, block, bits)) = format else {
                    return Err("has its `data` before its `fmt ` chunk".into());
                };
                match (code, bits) {
                    (1, 16) | (3, 32) => {}
                    (1, b) => return Err(format!("is {b}-bit integer PCM")),
                    (3, b) => return Err(format!("is {b}-bit float PCM")),
                    (c, _) => return Err(format!("is encoded with WAV format {c}, not PCM")),
                }
                if !(1..=2).contains(&channels) {
                    return Err(format!("has {channels} channels"));
                }
                if !(8_000..=96_000).contains(&rate) {
                    return Err(format!("is sampled at {rate} Hz"));
                }
                if usize::from(block) != usize::from(channels) * usize::from(bits / 8) {
                    return Err("has a block size its channels and bits do not make".into());
                }
                let frames = (end - body) / usize::from(block);
                return Ok(Wav {
                    frames: frames as u32,
                    rate,
                    channels: channels as u8,
                });
            }
            _ => {}
        }
        // Chunks are padded to an even length.
        at = body.saturating_add(size).saturating_add(size & 1);
    }
    Err("has no `data` chunk".into())
}
