//! Per-script font fallback (LLP 1085.000 G7): the lists cosmic-text kept
//! for each platform (its `unix.rs`, `windows.rs` and `macos.rs`), given to
//! fontique for every script. After a script's own families come the
//! platform's common ones ([`common`]), then every other script's, then
//! every installed family: cosmic-text's last resort was every face, and a
//! character Parley resolves to Common or Latin text (an Arabic `،` after
//! Latin words) must be able to reach any of them (the parity spike's third
//! surprise).
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
    for name in common(locale)
        .into_iter()
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
fn common(_locale: &str) -> Vec<&'static str> {
    COMMON.to_vec()
}

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
fn common(_locale: &str) -> Vec<&'static str> {
    COMMON.to_vec()
}

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
fn common(locale: &str) -> Vec<&'static str> {
    unix_common(cfg!(target_os = "android"), locale)
}

/// What a character none of a run's own families has (`→`, `≤` beside
/// Latin) falls back to first, in the order the platform's browser reaches
/// it, so a symbol takes the face and the advance it takes there:
///
/// - **Linux.** Chrome asks fontconfig for a pattern of the character and
///   the content language alone (`gfx::GetFallbackFontForChar`); the config
///   adds the generic `sans-serif` and expands it to the families its
///   aliases prefer, and `FcFontSort` ranks every face that has the
///   character by its family's place in that list. The list is the sans
///   faces (`60-latin.conf`: DejaVu Sans; Noto Sans, FreeSans where their
///   packages alias them) and then the CJK sans faces, which `fonts-noto-cjk`
///   aliases into `sans-serif` per language. The symbol, emoji and
///   monospace families are in no `sans-serif` alias and come after it; a
///   monospace face is never reached before a CJK one.
/// - **Android.** Chrome (Skia's `SkFontMgr_Android`) and the platform's
///   Minikin both walk `fonts.xml`'s fallback chain, a family covering the
///   content language first, then in the file's order: … Noto Sans Symbols
///   (the subset listed with no language, ahead of CJK), the CJK faces
///   (Simplified, Traditional, Japanese, Korean), Noto Color Emoji, Noto
///   Sans Symbols2. Named families such as `monospace` are not in the
///   chain at all.
///
/// The document language's Han face leads the CJK faces on both.
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn unix_common(android: bool, locale: &str) -> Vec<&'static str> {
    let mut cjk: Vec<&'static str> = han(locale).to_vec();
    for name in ["zh-Hans", "zh-TW", "ja", "ko"]
        .iter()
        .flat_map(|l| han(l).iter())
    {
        if !cjk.contains(name) {
            cjk.push(name);
        }
    }
    let mut list: Vec<&'static str> = Vec::new();
    if android {
        if han_language(locale) {
            list.push(cjk[0]);
        }
        list.push("Noto Sans Symbols");
        list.extend(&cjk);
        list.extend(["Noto Color Emoji", "Noto Sans Symbols2"]);
    } else {
        list.extend(["Noto Sans", "DejaVu Sans", "FreeSans"]);
        list.extend(&cjk);
        list.extend([
            "Noto Sans Symbols",
            "Noto Sans Symbols2",
            "Noto Color Emoji",
            "Noto Sans Mono",
            "DejaVu Sans Mono",
            "FreeMono",
        ]);
    }
    let mut seen = std::collections::HashSet::new();
    list.retain(|name| seen.insert(*name));
    list
}

/// Whether `locale` is a language a Han face is chosen for.
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn han_language(locale: &str) -> bool {
    let language = locale.split(['-', '_']).next().unwrap_or("");
    matches!(language, "ja" | "ko" | "zh")
}

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

#[cfg(all(test, not(any(target_os = "windows", target_os = "macos"))))]
mod tests {
    use super::*;

    fn at(list: &[&str], name: &str) -> usize {
        list.iter().position(|n| *n == name).expect(name)
    }

    #[test]
    fn a_symbol_falls_back_to_a_cjk_face_before_a_monospace_one_on_linux() {
        // `→` is in Noto Sans Mono and Noto Sans CJK, not in Noto Sans:
        // Chrome's fontconfig fallback takes the CJK face (sans-serif's
        // alias), never the monospace one first.
        let list = unix_common(false, "en");
        assert_eq!(&list[..3], ["Noto Sans", "DejaVu Sans", "FreeSans"]);
        for cjk in ["Noto Sans CJK SC", "Noto Sans CJK JP", "Noto Sans CJK KR"] {
            assert!(at(&list, cjk) < at(&list, "Noto Sans Symbols"), "{cjk}");
            assert!(at(&list, cjk) < at(&list, "Noto Sans Mono"), "{cjk}");
        }
        assert!(at(&list, "Noto Color Emoji") < at(&list, "Noto Sans Mono"));
        // The document language's Han face leads.
        assert_eq!(unix_common(false, "ja")[3], "Noto Sans CJK JP");
    }

    #[test]
    fn android_follows_its_fonts_xml_chain() {
        // fonts.xml lists Noto Sans Symbols (the subset, which has `→`)
        // before the CJK faces and Symbols2 after the emoji; monospace is a
        // named family and not in the chain.
        let list = unix_common(true, "en");
        assert_eq!(
            list,
            [
                "Noto Sans Symbols",
                "Noto Sans CJK SC",
                "Noto Sans CJK TC",
                "Noto Sans CJK JP",
                "Noto Sans CJK KR",
                "Noto Color Emoji",
                "Noto Sans Symbols2",
            ]
        );
        // A family covering the content language comes first.
        assert_eq!(
            unix_common(true, "ja")[..2],
            ["Noto Sans CJK JP", "Noto Sans Symbols"]
        );
        assert!(!list.iter().any(|n| n.contains("Mono")));
    }
}
