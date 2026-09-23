//! Which texture payload a device fetches. The world names each texture by
//! its authored `.tex` name; the host fetches the file this surface asks for.
//! @ref llp/1046.003-game-engine-as-built.explainer.md#compressed-textures-2026-09-23
use exact_game::asset::TextureFamily;
use exact_gpu::wgpu;

/// The family chosen from the device's granted features, and every requested
/// file name still answered or in flight, with the authored name it carries
/// (at most the surface's 256 asset names, so a list).
#[derive(Default)]
pub(crate) struct Payloads {
    pub family: TextureFamily,
    files: Vec<(String, String)>,
    /// Files of a previous family, which the host forgets with the next drain.
    stale: Vec<String>,
    /// Authored names whose family payload was missing: fetch the RGBA8 file.
    fallbacks: Vec<String>,
}

/// BC where granted (desktop GPUs, Apple silicon), else ASTC (phones), else
/// RGBA8. BC is preferred where both exist: BC4 halves one-channel maps and
/// the measured quality of the others is equal.
pub(crate) fn family(features: wgpu::Features) -> TextureFamily {
    if features.contains(wgpu::Features::TEXTURE_COMPRESSION_BC) {
        TextureFamily::Bc
    } else if features.contains(wgpu::Features::TEXTURE_COMPRESSION_ASTC) {
        TextureFamily::Astc
    } else {
        TextureFamily::Rgba8
    }
}

impl Payloads {
    /// Adopt a device's family. On a change every earlier file is retired;
    /// the caller re-requests the textures, which then name the new family.
    pub fn choose(&mut self, features: wgpu::Features) -> bool {
        let next = family(features);
        if next == self.family {
            return false;
        }
        self.family = next;
        self.stale.extend(
            std::mem::take(&mut self.files)
                .into_iter()
                .map(|(file, _)| file),
        );
        true
    }
    fn position(&self, file: &str) -> Option<usize> {
        self.files.iter().position(|(f, _)| f == file)
    }
    /// A family payload was missing (an authored `.tex` without a bake's
    /// variants): the next drain fetches the authored RGBA8 file instead.
    pub fn missing(&mut self, file: &str) -> bool {
        let Some(at) = self.position(file) else {
            return false;
        };
        let authored = self.files[at].1.clone();
        if authored == file {
            return false;
        }
        self.files[at] = (authored.clone(), authored.clone());
        self.fallbacks.push(authored);
        true
    }
    /// Authored names to the files this device fetches.
    pub fn request(&mut self, names: Vec<String>) -> Vec<String> {
        let mut files = std::mem::take(&mut self.fallbacks);
        for name in names {
            if files.contains(&name) {
                continue;
            }
            // A missing family payload keeps fetching the authored file.
            let file = if self.position(&name).is_some() {
                name.clone()
            } else {
                self.family.name(&name)
            };
            if name.ends_with(".tex") && self.position(&file).is_none() {
                self.files.push((file.clone(), name));
            }
            files.push(file);
        }
        files
    }
    /// Authored names to every file requested for them, which the host forgets.
    pub fn retire(&mut self, names: Vec<String>) -> Vec<String> {
        let mut files = std::mem::take(&mut self.stale);
        for name in names {
            let before = files.len();
            self.files.retain(|(file, authored)| {
                let retired = *authored == name;
                if retired {
                    files.push(file.clone());
                }
                !retired
            });
            if files.len() == before {
                files.push(name);
            }
        }
        files
    }
    /// The authored name a delivered file carries.
    pub fn authored<'a>(&'a self, file: &'a str) -> &'a str {
        self.position(file).map_or(file, |at| &self.files[at].1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wgpu::Features as F;
    #[test]
    fn requests_name_the_device_family_and_retire_every_file() {
        let mut p = Payloads::default();
        assert!(p.choose(F::TEXTURE_COMPRESSION_ASTC));
        let files = p.request(vec!["fox.model".into(), "fox/0.tex".into()]);
        assert_eq!(files, ["fox.model", "fox/0.astc.tex"]);
        assert_eq!(p.authored("fox/0.astc.tex"), "fox/0.tex");
        assert_eq!(p.authored("fox.model"), "fox.model");
        // BC is preferred where both exist; the ASTC file is retired at once.
        assert!(p.choose(F::TEXTURE_COMPRESSION_BC | F::TEXTURE_COMPRESSION_ASTC));
        assert!(!p.choose(F::TEXTURE_COMPRESSION_BC));
        assert_eq!(p.request(vec!["fox/0.tex".into()]), ["fox/0.bc.tex"]);
        let retired = p.retire(vec!["fox/0.tex".into(), "fox.model".into()]);
        assert_eq!(retired, ["fox/0.astc.tex", "fox/0.bc.tex", "fox.model"]);
        assert_eq!(p.authored("fox/0.bc.tex"), "fox/0.bc.tex");
        // Without either family the authored RGBA8 file is fetched.
        p.choose(F::empty());
        assert_eq!(p.request(vec!["a.bc.tex".into()]), ["a.bc.tex"]);
        assert_eq!(p.retire(vec!["a.bc.tex".into()]), ["a.bc.tex"]);
    }
    #[test]
    fn a_missing_family_payload_falls_back_to_the_authored_file() {
        let mut p = Payloads::default();
        p.choose(F::TEXTURE_COMPRESSION_BC);
        assert_eq!(p.request(vec!["hand.tex".into()]), ["hand.bc.tex"]);
        assert!(p.missing("hand.bc.tex"));
        assert!(!p.missing("hand.tex"));
        assert_eq!(p.request(Vec::new()), ["hand.tex"]);
        assert_eq!(p.authored("hand.tex"), "hand.tex");
        // Retirement forgets the fallback; a later request tries the family again.
        assert_eq!(p.retire(vec!["hand.tex".into()]), ["hand.tex"]);
        assert_eq!(p.request(vec!["hand.tex".into()]), ["hand.bc.tex"]);
    }
}
