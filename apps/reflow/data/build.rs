//! Generate `metrics.rs`: per declared face, the horizontal advance of every
//! character the app's prose uses and the GPOS `kern` pairs between them, in
//! font units. The runtime crate then measures text by arithmetic — exactly
//! what Pretext caches from canvas `measureText`, read here from the same
//! bytes the hosts shape with (LLP 1019 D3), so no font is parsed at runtime
//! and the wasm carries a table, not a font.
use std::fmt::Write as _;
use std::{env, fs, path::Path};
use ttf_parser::gpos::{PairAdjustment, PositioningSubtable};
use ttf_parser::{Face, GlyphId, Tag};

/// Every character the app's prose, titles and ASCII ramp may contain.
const EXTRA: &str = "‘’“”—–…·×éèàçñöüï";
const FACES: [(&str, &str); 3] = [
    ("REGULAR", "ExposureSansOriginal03-Regular.ttf"),
    ("ITALIC", "ExposureSansOriginal03-Italic.ttf"),
    ("BOLD", "ExposureSansOriginal03-Bold.ttf"),
];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let mut out = String::new();
    for (name, file) in FACES {
        let path = Path::new("../assets").join(file);
        println!("cargo:rerun-if-changed={}", path.display());
        let bytes = fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let face = Face::parse(&bytes, 0).unwrap_or_else(|e| panic!("{file}: {e:?}"));
        let chars: Vec<char> = (0x20u8..0x7f)
            .map(char::from)
            .chain(EXTRA.chars())
            .collect();
        let glyphs: Vec<(char, GlyphId, u16)> = chars
            .iter()
            .filter_map(|&c| {
                let g = face.glyph_index(c)?;
                Some((c, g, face.glyph_hor_advance(g)?))
            })
            .collect();
        let mut ascii = [0u16; 128];
        let mut extra = Vec::new();
        for &(c, _, adv) in &glyphs {
            if (c as u32) < 128 {
                ascii[c as usize] = adv;
            } else {
                extra.push((c, adv));
            }
        }
        let mut kern: Vec<(u32, i16)> = Vec::new();
        if let Some(gpos) = face.tables().gpos {
            let mut lookups: Vec<u16> = Vec::new();
            for i in 0..gpos.features.len() {
                let Some(feature) = gpos.features.get(i) else {
                    continue;
                };
                if feature.tag == Tag::from_bytes(b"kern") {
                    lookups.extend(feature.lookup_indices);
                }
            }
            lookups.sort_unstable();
            lookups.dedup();
            for li in lookups {
                let Some(lookup) = gpos.lookups.get(li) else {
                    continue;
                };
                for si in 0..lookup.subtables.len() {
                    let Some(PositioningSubtable::Pair(pair)) =
                        lookup.subtables.get::<PositioningSubtable>(si)
                    else {
                        continue;
                    };
                    for &(a, ga, _) in &glyphs {
                        let Some(index) = pair.coverage().get(ga) else {
                            continue;
                        };
                        for &(b, gb, _) in &glyphs {
                            let adjust = match pair {
                                PairAdjustment::Format1 { sets, .. } => sets
                                    .get(index)
                                    .and_then(|s| s.get(gb))
                                    .map(|(v, _)| v.x_advance),
                                PairAdjustment::Format2 {
                                    classes, matrix, ..
                                } => matrix
                                    .get((classes.0.get(ga), classes.1.get(gb)))
                                    .map(|(v, _)| v.x_advance),
                            };
                            if let Some(adjust) = adjust {
                                let key = ((a as u32) << 21) | b as u32;
                                // The first lookup and subtable to cover a pair wins.
                                if adjust != 0 && !kern.iter().any(|&(k, _)| k == key) {
                                    kern.push((key, adjust));
                                }
                            }
                        }
                    }
                }
            }
        }
        kern.sort_unstable_by_key(|&(k, _)| k);
        writeln!(
            out,
            "/// {file}: {} glyphs, {} kern pairs.\npub static {name}: FaceData = FaceData {{\n    upem: {}.0,\n    ascent: {}.0,\n    descent: {}.0,\n    ascii: {ascii:?},\n    extra: &{extra:?},\n    kern: &{kern:?},\n}};",
            glyphs.len(),
            kern.len(),
            face.units_per_em(),
            face.ascender(),
            face.descender(),
        )
        .unwrap();
    }
    let out_dir = env::var("OUT_DIR").unwrap();
    fs::write(Path::new(&out_dir).join("metrics.rs"), out).unwrap();
}
