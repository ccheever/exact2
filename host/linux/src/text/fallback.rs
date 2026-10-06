//! Per-script font fallback (LLP 1085.000 G7): the lists cosmic-text kept
//! for each platform (its `unix.rs`, `windows.rs` and `macos.rs`), given to
//! fontique for every script. After a script's own families come the
//! platform's common ones, then every other script's, then every installed
//! family: cosmic-text's last resort was every face, and a character Parley
//! resolves to Common or Latin text (an Arabic `،` after Latin words) must
//! be able to reach any of them (the parity spike's third surprise).
use fontique::{Collection, FallbackKey, FamilyId, Script, ScriptExt};

/// Set every script's fallback families on `collection` for `locale` (the
/// document language picks the Han face, as cosmic-text's did).
pub(super) fn configure(collection: &mut Collection, locale: &str) {
    let names = super::fonts::family_names(collection);
    let every_script = SCRIPTS
        .iter()
        .flat_map(|(_, f)| f.iter())
        .chain(han(locale))
        .chain(han("ko"))
        .chain(han("ja"))
        .copied();
    let mut ordered: Vec<FamilyId> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for name in COMMON
        .iter()
        .copied()
        .chain(every_script)
        .chain(names.iter().map(String::as_str))
    {
        if let Some(id) = collection.family_id(name) {
            if seen.insert(id) {
                ordered.push(id);
            }
        }
    }
    let mut keys: Vec<Script> = Vec::new();
    for script in <Script as ScriptExt>::all_samples()
        .iter()
        .map(|(s, _)| *s)
        .chain(["Zyyy", "Zinh", "Zzzz", "Latn"].map(Script::from_str_unchecked))
    {
        if !keys.contains(&script) {
            keys.push(script);
        }
    }
    for script in keys {
        let own: Vec<FamilyId> = families(script.as_str(), locale)
            .iter()
            .filter_map(|n| collection.family_id(n))
            .collect();
        let list: Vec<FamilyId> = own
            .iter()
            .copied()
            .chain(ordered.iter().copied().filter(|id| !own.contains(id)))
            .collect();
        collection.set_fallbacks(FallbackKey::new(script, None), list.into_iter());
    }
}

/// A script's own families (ISO 15924 tag).
fn families(tag: &str, locale: &str) -> &'static [&'static str] {
    match tag {
        "Hani" | "Bopo" => han(locale),
        "Hang" => han("ko"),
        "Hira" | "Kana" => han("ja"),
        _ => SCRIPTS
            .iter()
            .find(|(t, _)| *t == tag)
            .map_or(&[], |(_, f)| f),
    }
}

#[cfg(target_os = "windows")]
const COMMON: &[&str] = &[
    "Segoe UI",
    "Segoe UI Emoji",
    "Segoe UI Symbol",
    "Segoe UI Historic",
];

#[cfg(target_os = "windows")]
fn han(locale: &str) -> &'static [&'static str] {
    match locale {
        "ja" => &["Yu Gothic"],
        "ko" => &["Malgun Gothic"],
        "zh-HK" => &["MingLiU_HKSCS"],
        "zh-TW" => &["Microsoft JhengHei UI"],
        _ => &["Microsoft YaHei UI"],
    }
}

#[cfg(target_os = "windows")]
const SCRIPTS: &[(&str, &[&str])] = &[
    ("Adlm", &["Ebrima"]),
    ("Beng", &["Nirmala UI"]),
    ("Cans", &["Gadugi"]),
    ("Cakm", &["Nirmala UI"]),
    ("Cher", &["Gadugi"]),
    ("Deva", &["Nirmala UI"]),
    ("Ethi", &["Ebrima"]),
    ("Gujr", &["Nirmala UI"]),
    ("Guru", &["Nirmala UI"]),
    ("Java", &["Javanese Text"]),
    ("Knda", &["Nirmala UI"]),
    ("Khmr", &["Leelawadee UI"]),
    ("Laoo", &["Leelawadee UI"]),
    ("Mlym", &["Nirmala UI"]),
    ("Mong", &["Mongolian Baiti"]),
    ("Mymr", &["Myanmar Text"]),
    ("Orya", &["Nirmala UI"]),
    ("Sinh", &["Nirmala UI"]),
    ("Taml", &["Nirmala UI"]),
    ("Telu", &["Nirmala UI"]),
    ("Thaa", &["MV Boli"]),
    ("Thai", &["Leelawadee UI"]),
    ("Tibt", &["Microsoft Himalaya"]),
    ("Tfng", &["Ebrima"]),
    ("Vaii", &["Ebrima"]),
    ("Yiii", &["Microsoft Yi Baiti"]),
];

#[cfg(target_os = "macos")]
const COMMON: &[&str] = &[
    ".SF NS",
    "Menlo",
    "Apple Color Emoji",
    "Geneva",
    "Arial Unicode MS",
];

#[cfg(target_os = "macos")]
fn han(locale: &str) -> &'static [&'static str] {
    match locale {
        "ja" => &["Hiragino Sans"],
        "ko" => &["Apple SD Gothic Neo"],
        "zh-HK" => &["PingFang HK"],
        "zh-TW" => &["PingFang TC"],
        _ => &["PingFang SC"],
    }
}

#[cfg(target_os = "macos")]
const SCRIPTS: &[(&str, &[&str])] = &[
    ("Adlm", &["Noto Sans Adlam"]),
    ("Arab", &["Geeza Pro"]),
    ("Armn", &["Noto Sans Armenian"]),
    ("Beng", &["Bangla Sangam MN"]),
    ("Buhd", &["Noto Sans Buhid"]),
    ("Cans", &["Euphemia UCAS"]),
    ("Cakm", &["Noto Sans Chakma"]),
    ("Deva", &["Devanagari Sangam MN"]),
    ("Ethi", &["Kefa"]),
    ("Goth", &["Noto Sans Gothic"]),
    ("Gran", &["Grantha Sangam MN"]),
    ("Gujr", &["Gujarati Sangam MN"]),
    ("Guru", &["Gurmukhi Sangam MN"]),
    ("Hano", &["Noto Sans Hanunoo"]),
    ("Hebr", &["Arial"]),
    ("Java", &["Noto Sans Javanese"]),
    ("Knda", &["Noto Sans Kannada"]),
    ("Khmr", &["Khmer Sangam MN"]),
    ("Laoo", &["Lao Sangam MN"]),
    ("Mlym", &["Malayalam Sangam MN"]),
    ("Mong", &["Noto Sans Mongolian"]),
    ("Mymr", &["Noto Sans Myanmar"]),
    ("Orya", &["Noto Sans Oriya"]),
    ("Sinh", &["Sinhala Sangam MN"]),
    ("Syrc", &["Noto Sans Syriac"]),
    ("Tglg", &["Noto Sans Tagalog"]),
    ("Tagb", &["Noto Sans Tagbanwa"]),
    ("Tale", &["Noto Sans Tai Le"]),
    ("Lana", &["Noto Sans Tai Tham"]),
    ("Tavt", &["Noto Sans Tai Viet"]),
    ("Taml", &["InaiMathi"]),
    ("Telu", &["Telugu Sangam MN"]),
    ("Thaa", &["Noto Sans Thaana"]),
    ("Thai", &["Ayuthaya"]),
    ("Tibt", &["Kailasa"]),
    ("Tfng", &["Noto Sans Tifinagh"]),
    ("Vaii", &["Noto Sans Vai"]),
    ("Yiii", &["Noto Sans Yi", "PingFang SC"]),
];

// Linux, the BSDs and Android: the Noto names, which Android's own
// /system/fonts also uses (cosmic-text gave Android no list and fell to
// every face; the last resort here keeps that).
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const COMMON: &[&str] = &[
    "Noto Sans",
    "DejaVu Sans",
    "FreeSans",
    "Noto Sans Mono",
    "DejaVu Sans Mono",
    "FreeMono",
    "Noto Sans Symbols",
    "Noto Sans Symbols2",
    "Noto Color Emoji",
];

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn han(locale: &str) -> &'static [&'static str] {
    match locale {
        "ja" => &["Noto Sans CJK JP"],
        "ko" => &["Noto Sans CJK KR"],
        "zh-HK" => &["Noto Sans CJK HK"],
        "zh-TW" => &["Noto Sans CJK TC"],
        _ => &["Noto Sans CJK SC"],
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const SCRIPTS: &[(&str, &[&str])] = &[
    ("Adlm", &["Noto Sans Adlam", "Noto Sans Adlam Unjoined"]),
    ("Arab", &["Noto Sans Arabic", "Noto Naskh Arabic"]),
    ("Armn", &["Noto Sans Armenian"]),
    ("Beng", &["Noto Sans Bengali"]),
    // DejaVu Sans would take braille and break alignment beside monospace.
    ("Brai", &["FreeMono"]),
    ("Buhd", &["Noto Sans Buhid"]),
    ("Cakm", &["Noto Sans Chakma"]),
    ("Cher", &["Noto Sans Cherokee"]),
    ("Dsrt", &["Noto Sans Deseret"]),
    ("Deva", &["Noto Sans Devanagari"]),
    ("Ethi", &["Noto Sans Ethiopic"]),
    ("Geor", &["Noto Sans Georgian"]),
    ("Goth", &["Noto Sans Gothic"]),
    ("Gran", &["Noto Sans Grantha"]),
    ("Gujr", &["Noto Sans Gujarati"]),
    ("Guru", &["Noto Sans Gurmukhi"]),
    ("Hano", &["Noto Sans Hanunoo"]),
    ("Hebr", &["Noto Sans Hebrew"]),
    ("Java", &["Noto Sans Javanese"]),
    ("Knda", &["Noto Sans Kannada"]),
    ("Khmr", &["Noto Sans Khmer"]),
    ("Laoo", &["Noto Sans Lao"]),
    ("Mlym", &["Noto Sans Malayalam"]),
    ("Mong", &["Noto Sans Mongolian"]),
    ("Mymr", &["Noto Sans Myanmar"]),
    ("Orya", &["Noto Sans Oriya"]),
    ("Runr", &["Noto Sans Runic"]),
    ("Sinh", &["Noto Sans Sinhala"]),
    ("Syrc", &["Noto Sans Syriac"]),
    ("Tglg", &["Noto Sans Tagalog"]),
    ("Tagb", &["Noto Sans Tagbanwa"]),
    ("Tale", &["Noto Sans Tai Le"]),
    ("Lana", &["Noto Sans Tai Tham"]),
    ("Tavt", &["Noto Sans Tai Viet"]),
    ("Taml", &["Noto Sans Tamil"]),
    ("Telu", &["Noto Sans Telugu"]),
    ("Thaa", &["Noto Sans Thaana"]),
    ("Thai", &["Noto Sans Thai"]),
    ("Tibt", &["Noto Serif Tibetan"]),
    ("Tfng", &["Noto Sans Tifinagh"]),
    ("Vaii", &["Noto Sans Vai"]),
    ("Yiii", &["Noto Sans Yi", "Noto Sans CJK SC"]),
];
