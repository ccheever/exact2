//! Chromium's pair table for Latin-1 line breaks.
//! @ref LLP 1043 §4 C — Pretext's corpora showed where the walker and Chrome disagree.
//!
//! Blink decides a break between two characters in U+0021..=U+00FF from a
//! generated table, `kFastLineBreakTable` (Chromium
//! `third_party/blink/renderer/platform/text/character_property_data_generator.cc`,
//! BSD-3-Clause, © The Chromium Authors), before it asks ICU. The rows below are
//! that table as Pretext decoded it (`src/generated/engine-break-data.ts`, MIT,
//! © Pretext contributors), factored into 14 distinct rows and 13 distinct
//! columns; WebKit's table differs from it in 14 of 6,244 bytes.

/// Row class of the character before a break, indexed by `char - 0x21`.
const BEFORE: [u8; 223] = [
    0, 1, 2, 3, 4, 2, 5, 5, 4, 2, 4, 4, 6, 4, 7, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 4, 4, 8, 2, 2, 9, 8,
    8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 5, 4, 4, 8, 8, 8,
    8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 5, 10, 0, 2, 8,
    8, 8, 8, 8, 8, 11, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8,
    8, 5, 5, 12, 12, 12, 12, 8, 8, 8, 8, 8, 5, 8, 13, 8, 8, 12, 12, 8, 8, 5, 8, 8, 8, 8, 8, 8, 5,
    8, 8, 8, 5, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8,
    8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8,
    8, 8, 8, 8,
];
/// Column class of the character after a break, indexed by `char - 0x21`.
const AFTER: [u8; 223] = [
    0, 1, 2, 3, 4, 2, 1, 5, 0, 2, 4, 0, 6, 0, 0, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 0, 0, 8, 2, 2, 0, 2,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 5, 4, 0, 2, 2, 2,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 5, 6, 0, 2, 6, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 9,
    10, 10, 10, 10, 10, 11, 11, 11, 11, 11, 0, 11, 0, 11, 11, 10, 10, 11, 11, 12, 11, 11, 11, 11,
    11, 11, 0, 11, 11, 11, 10, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11,
    11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11,
    11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11,
];
/// Bit `after` of row `before`: a line may start between the two characters.
const PAIRS: [u16; 14] = [
    0b1110100100000,
    0b0000100100000,
    0b1000100100000,
    0b1010000000000,
    0b1010100100000,
    0b0000000000000,
    0b1011101110110,
    0b1110000000000,
    0b1000000000000,
    0b1110111111100,
    0b1111100100000,
    0b1111111111111,
    0b1010000111000,
    0b1111110111100,
];

/// Chrome's answer for a pair of characters in U+0021..=U+00FF.
pub(crate) fn breaks(before: char, after: char) -> bool {
    let (b, a) = (before as usize - 0x21, after as usize - 0x21);
    PAIRS[BEFORE[b] as usize] >> AFTER[a] & 1 == 1
}
