# fontique 0.11.1 — local patches

Complete crates.io archive, including upstream MIT/Apache licenses and
`.cargo_vcs_info.json`. No feature or dependency change: exact2 builds it
with default features off and `std` on (no `system`: no fontconfig, no
DirectWrite or Core Text; LLP 1085.000 §3).

- Upstream: https://github.com/linebender/parley (the `fontique` crate)
- Exact release archive: https://static.crates.io/crates/fontique/fontique-0.11.1.crate
- Archive SHA256: 6688bc1294fe7117d788937b6c53480169b29c566954af490830d4c09da9516a
- Upstream VCS revision: eea3503dd6cf17130cbb07348e0ff2c918300e94
- Root `[patch.crates-io]` (and `game/`, `snapback4/`) selects this copy.

Every patch below is upstream status **to send**. Remove a patch when a
pinned upstream release supplies the same behaviour and the tests named here
pass.

1. **Font admission (G2).** `FontInfo::from_font_ref` refuses a face with no
   `head` table (some bitmap-only faces have `bhed` instead) or with
   `unitsPerEm` 0, where it already refuses one with no usable cmap: glyph
   metrics are divided by it, and such a face laid out with infinite
   advances. Fallback goes on to the next face. Parity spike: the DejaVu
   fixtures register no face and fall back to the good one, also when the
   damaged copy keeps the family name. Tests:
   `host/linux/src/text/transfer_tests/font_admission.rs`.
2. **Faces described ahead of time.** `FontInfo::from_parts(source, index,
   width, style, weight, axes, charmap_index)`, `CharmapIndex::{to_parts,
   from_parts}` and `Collection::register_described(family, faces)`: a host
   that cached a font directory's listing registers its faces without
   opening a file (the storage-and-fonts spike: 1.05 ms against fontdb's
   0.82 for 269 files on Linux, opening no font file; 6 files opened by the
   first shape). The host's cache (`host/linux/src/text/font_cache.rs`)
   stores each face's family, width, style, weight, every axis and its
   charmap subtable. Tests: every host text test registers faces this way.
3. **`load_fonts_from_paths` registers each file once.** 0.11.1 shared one
   `families` map across every scanned face and merged the whole map into the
   collection on each one, re-adding every earlier face (45,763 faces for
   269 files), and rescanned a collection file once per face. Each file now
   has its own map and is registered once (277 faces for the same files).
4. **A last-resort fallback list.** `FallbackMap::set_last_resort` and
   `Collection::set_last_resort_fallbacks`: the families of every script
   given no list of its own (its default key; a tracked locale of such a
   script still finds none, as before). The host gives each script its own
   families followed by one shared last resort (every installed family);
   the ~125 scripts with none of their own now share that list instead of
   each holding a copy (`fallback::configure` 0.42 to 0.25 ms of a Pixel 10
   Pro XL's boot, LLP 1085.000 stage 3). Tests:
   `host/linux/src/text/fallback.rs`
   (`a_script_without_families_of_its_own_takes_the_last_resort`: every
   script's list is what a copy per script gave).

All other archive files are byte-for-byte upstream.
