// SPDX-License-Identifier: MIT OR Apache-2.0

use unicode_script::Script;

use super::Fallback;

/// A platform-specific font fallback list, for Android: the system's own
/// families (`/system/etc/fonts.xml`'s fallback chain). Without one, a
/// character the primary face lacks is searched for in every installed
/// font, each file opened and parsed — on a phone's ~230 fonts, the first
/// emoji or script a page shows costs tens of milliseconds.
#[derive(Debug)]
pub struct PlatformFallback;

impl Fallback for PlatformFallback {
    fn common_fallback(&self) -> &'static [&'static str] {
        common_fallback()
    }

    fn forbidden_fallback(&self) -> &'static [&'static str] {
        forbidden_fallback()
    }

    fn script_fallback(
        &self,
        script: unicode_script::Script,
        locale: &str,
    ) -> &'static [&'static str] {
        script_fallback(script, locale)
    }
}

// Fallbacks to use after any script specific fallbacks
const fn common_fallback() -> &'static [&'static str] {
    &[
        "Roboto",
        "Noto Color Emoji",
        "Noto Color Emoji Flags",
        "Noto Sans Symbols",
        "Noto Sans Math",
    ]
}

// Fallbacks to never use
const fn forbidden_fallback() -> &'static [&'static str] {
    &[]
}

fn han_unification(locale: &str) -> &'static [&'static str] {
    match locale {
        // Japan
        "ja" => &["Noto Sans CJK JP"],
        // Korea
        "ko" => &["Noto Sans CJK KR"],
        // Hong Kong
        "zh-HK" => &["Noto Sans CJK HK"],
        // Taiwan
        "zh-TW" => &["Noto Sans CJK TC"],
        // Simplified Chinese is the default (also catches "zh-CN" for China)
        _ => &["Noto Sans CJK SC"],
    }
}

// Fallbacks to use per script
fn script_fallback(script: Script, locale: &str) -> &'static [&'static str] {
    //TODO: abstract style (sans/serif/monospaced)
    match script {
        Script::Adlam => &["Noto Sans Adlam", "Noto Sans Adlam Unjoined"],
        Script::Arabic => &["Noto Naskh Arabic", "Noto Sans Arabic"],
        Script::Armenian => &["Noto Sans Armenian"],
        Script::Bengali => &["Noto Sans Bengali"],
        Script::Bopomofo => han_unification(locale),
        Script::Braille => &["Noto Sans Symbols"],
        Script::Buhid => &["Noto Sans Buhid"],
        Script::Chakma => &["Noto Sans Chakma"],
        Script::Cherokee => &["Noto Sans Cherokee"],
        Script::Deseret => &["Noto Sans Deseret"],
        Script::Devanagari => &["Noto Sans Devanagari"],
        Script::Ethiopic => &["Noto Sans Ethiopic"],
        Script::Georgian => &["Noto Sans Georgian"],
        Script::Gothic => &["Noto Sans Gothic"],
        Script::Grantha => &["Noto Sans Grantha"],
        Script::Gujarati => &["Noto Sans Gujarati"],
        Script::Gurmukhi => &["Noto Sans Gurmukhi"],
        Script::Han => han_unification(locale),
        Script::Hangul => han_unification("ko"),
        Script::Hanunoo => &["Noto Sans Hanunoo"],
        Script::Hebrew => &["Noto Sans Hebrew"],
        Script::Hiragana => han_unification("ja"),
        Script::Javanese => &["Noto Sans Javanese"],
        Script::Kannada => &["Noto Sans Kannada"],
        Script::Katakana => han_unification("ja"),
        Script::Khmer => &["Noto Sans Khmer"],
        Script::Lao => &["Noto Sans Lao"],
        Script::Malayalam => &["Noto Sans Malayalam"],
        Script::Mongolian => &["Noto Sans Mongolian"],
        Script::Myanmar => &["Noto Sans Myanmar"],
        Script::Oriya => &["Noto Sans Oriya"],
        Script::Runic => &["Noto Sans Runic"],
        Script::Sinhala => &["Noto Sans Sinhala"],
        Script::Syriac => &["Noto Sans Syriac"],
        Script::Tagalog => &["Noto Sans Tagalog"],
        Script::Tagbanwa => &["Noto Sans Tagbanwa"],
        Script::Tai_Le => &["Noto Sans Tai Le"],
        Script::Tai_Tham => &["Noto Sans Tai Tham"],
        Script::Tai_Viet => &["Noto Sans Tai Viet"],
        Script::Tamil => &["Noto Sans Tamil"],
        Script::Telugu => &["Noto Sans Telugu"],
        Script::Thaana => &["Noto Sans Thaana"],
        Script::Thai => &["Noto Sans Thai"],
        //TODO: no sans script?
        Script::Tibetan => &["Noto Serif Tibetan"],
        Script::Tifinagh => &["Noto Sans Tifinagh"],
        Script::Vai => &["Noto Sans Vai"],
        //TODO: Use han_unification?
        Script::Yi => &["Noto Sans Yi", "Noto Sans CJK SC"],
        _ => &[],
    }
}
