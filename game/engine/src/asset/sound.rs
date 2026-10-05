//! Baked sampled sounds. The bake decodes WAV and Ogg Vorbis; a module only
//! undoes this layout, so no audio decoder is linked into a game.
//! @ref LLP 1046.003 §AU4 (where decoding happens)
use crate::Data;
use std::sync::Arc;

/// Resident 16-bit PCM for every delivered sound in one world, counted once per asset.
/// A delivery that would exceed it is refused by name before setup can run.
pub const SOUND_BYTE_BUDGET: usize = 32 * 1024 * 1024;
/// Source rates accepted by the bake and at delivery; outputs resample to the device.
pub const SOUND_RATES: std::ops::RangeInclusive<u32> = 8_000..=192_000;

/// The `.sound` record: 16-bit PCM, channels planar, each channel stored as wrapping
/// first differences, which gzip compresses 1.3–2.2× smaller than interleaved PCM.
/// @ref LLP 1046.003 §AU4 (where decoding happens)
#[derive(Data, Default, Clone, Debug, PartialEq)]
pub struct SoundData {
    /// Frames per second.
    pub rate: u32,
    /// One (mono) or two (left, right).
    pub channels: u32,
    /// Frames per channel.
    pub frames: u32,
    /// `channels * frames` differences as `i16` bit patterns, one channel after another.
    pub deltas: Vec<u16>,
}
impl SoundData {
    /// Encode interleaved 16-bit samples.
    pub fn encode(rate: u32, channels: u32, interleaved: &[i16]) -> Result<Self, String> {
        if !(1..=2).contains(&channels) {
            return Err(format!("{channels} channels; expected mono or stereo"));
        }
        let c = channels as usize;
        if !interleaved.len().is_multiple_of(c) {
            return Err("samples end mid-frame".into());
        }
        let frames = u32::try_from(interleaved.len() / c).map_err(|_| "too many frames")?;
        let mut deltas = Vec::with_capacity(interleaved.len());
        for channel in 0..c {
            let mut previous = 0i16;
            for frame in interleaved.chunks_exact(c) {
                deltas.push(frame[channel].wrapping_sub(previous) as u16);
                previous = frame[channel];
            }
        }
        let data = Self {
            rate,
            channels,
            frames,
            deltas,
        };
        data.validate()?;
        Ok(data)
    }
    /// Resident bytes once decoded.
    pub fn pcm_bytes(&self) -> usize {
        self.frames as usize * self.channels as usize * 2
    }
    /// Refuse malformed or oversized records before allocating PCM.
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=2).contains(&self.channels) {
            return Err(format!(
                "{} channels; expected mono or stereo",
                self.channels
            ));
        }
        if !SOUND_RATES.contains(&self.rate) {
            return Err(format!(
                "{} Hz is outside {}..={} Hz",
                self.rate,
                SOUND_RATES.start(),
                SOUND_RATES.end()
            ));
        }
        if self.frames == 0 {
            return Err("sound has no frames".into());
        }
        if self.pcm_bytes() > SOUND_BYTE_BUDGET {
            return Err(format!(
                "{} of 16-bit PCM exceeds the {} sound residency budget",
                mib(self.pcm_bytes()),
                mib(SOUND_BYTE_BUDGET)
            ));
        }
        if self.deltas.len() != self.frames as usize * self.channels as usize {
            return Err("sample count does not match frames × channels".into());
        }
        Ok(())
    }
    /// Interleaved 16-bit PCM.
    pub fn samples(&self) -> Vec<i16> {
        let (c, frames) = (self.channels as usize, self.frames as usize);
        let mut out = vec![0i16; c * frames];
        for (channel, deltas) in self.deltas.chunks_exact(frames).enumerate() {
            let mut value = 0i16;
            for (frame, delta) in deltas.iter().enumerate() {
                value = value.wrapping_add(*delta as i16);
                out[frame * c + channel] = value;
            }
        }
        out
    }
}
pub(crate) fn mib(bytes: usize) -> String {
    format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
}

/// A delivered sound, resident once in its world's asset store. Presentation
/// executors share this allocation; simulation reads only registered metadata.
#[derive(Clone, Debug)]
pub struct SoundAsset {
    /// Frames per second.
    pub rate: u32,
    /// One or two.
    pub channels: u32,
    /// Frames per channel.
    pub frames: u32,
    /// Interleaved 16-bit PCM.
    pub samples: Arc<[i16]>,
    /// Content identity of the delivered record.
    pub digest: u64,
}
impl SoundAsset {
    pub(crate) fn new(data: &SoundData) -> Self {
        Self {
            rate: data.rate,
            channels: data.channels,
            frames: data.frames,
            samples: data.samples().into(),
            digest: crate::hash::of(data),
        }
    }
    /// Resident bytes.
    pub fn bytes(&self) -> usize {
        self.samples.len() * 2
    }
}

impl<G: crate::Game> crate::Sim<G> {
    /// `replacing`, as `deliver_level`'s: a development reload's new bytes.
    pub(crate) fn deliver_sound(
        &mut self,
        name: &str,
        data: SoundData,
        replacing: bool,
    ) -> Result<(), String> {
        if !G::ASSETS.contains(&name) {
            return Err(format!("sound `{name}` is not declared by Game::ASSETS"));
        }
        let sound = SoundAsset::new(&data);
        let assets = &mut self.world_mut().assets;
        if !replacing
            && assets
                .identities
                .get(name)
                .is_some_and(|old| *old != sound.digest)
        {
            return Err(format!(
                "sound `{name}` cannot change after delivery; restart with the new sound"
            ));
        }
        let resident: usize = assets
            .sounds
            .iter()
            .filter(|(other, _)| *other != name)
            .map(|(_, s)| s.bytes())
            .sum();
        if resident + sound.bytes() > SOUND_BYTE_BUDGET {
            return Err(format!(
                "{} of 16-bit PCM with {} already resident exceeds the {} sound residency budget",
                mib(sound.bytes()),
                mib(resident),
                mib(SOUND_BYTE_BUDGET)
            ));
        }
        assets.identify(name, sound.digest);
        assets.sounds.insert(name.into(), Arc::new(sound));
        assets.states.insert(name.into(), super::AssetState::Loaded);
        Ok(())
    }
}
impl crate::World {
    /// A declared, delivered sound's PCM, for presentation executors.
    pub fn sound_asset(&self, name: &str) -> Option<&SoundAsset> {
        self.assets
            .declared
            .contains(name)
            .then(|| self.assets.sounds.get(name).map(|s| s.as_ref()))
            .flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn planar_differences_round_trip_extremes() {
        let interleaved = [0, i16::MAX, i16::MIN, -1, i16::MAX, i16::MIN, 7, 7];
        let data = SoundData::encode(48_000, 2, &interleaved).unwrap();
        assert_eq!(data.frames, 4);
        assert_eq!(data.samples(), interleaved);
        let copy: SoundData = crate::bin::from_slice(&crate::bin::to_vec(&data)).unwrap();
        assert_eq!(copy, data);
        assert_eq!(copy.samples(), interleaved);
    }
    #[test]
    fn records_refuse_by_reason() {
        let bad = |d: SoundData| d.validate().unwrap_err();
        let ok = SoundData::encode(8_000, 1, &[1, 2, 3]).unwrap();
        assert!(bad(SoundData {
            channels: 3,
            ..ok.clone()
        })
        .contains("mono or stereo"));
        assert!(bad(SoundData {
            rate: 7_999,
            ..ok.clone()
        })
        .contains("outside"));
        assert!(bad(SoundData {
            frames: 4,
            ..ok.clone()
        })
        .contains("frames × channels"));
        assert!(bad(SoundData {
            frames: 0,
            deltas: vec![],
            ..ok.clone()
        })
        .contains("no frames"));
        let huge = SoundData {
            frames: (SOUND_BYTE_BUDGET / 2 + 1) as u32,
            ..ok
        };
        assert!(bad(huge).contains("exceeds the 32.0 MiB sound residency budget"));
    }
}
