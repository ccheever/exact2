//! The app's native-module roster (LLP 1024 D1): the tags its module
//! artifact serves, declared in `app.json` as `"modules": ["tag", …]`. A
//! module tag the roster does not name is `bake-unknown-module`, so a typo
//! is a named diagnostic in the dev loop and in every build, never an empty
//! box at runtime. Beside it, the app's `data-*` words (LLP 1075.003 Q2):
//! `"data": ["word", …]`, from which the Apple build also writes the Swift
//! module's typed keys; a word the list lacks is `bake-undeclared-data`.
//! And its `hatch` words (LLP 1075.003.000 Q1): `"hatches": ["word", …]`, the
//! same way, `bake-undeclared-hatch`; or `{"word": ["ios", …]}`, each word
//! with the platforms that handle it (LLP 1075.003.000.001 §5).

use super::{CompileError, Manifest};
use contract_syntax::{File, Span};
use std::path::Path;

/// The roster in `manifest`, each name checked against the tag grammar.
pub fn roster(manifest: &Manifest) -> Result<Vec<String>, String> {
    let Some(value) = manifest.json.get("modules") else {
        return Ok(Vec::new());
    };
    let names = value
        .as_array()
        .ok_or("app.json `modules` is a list of custom-element tags")?;
    names
        .iter()
        .map(|name| match name.as_str() {
            Some(tag) if contract_lower::is_module_tag(tag) => Ok(tag.to_owned()),
            _ => Err(format!(
                "app.json `modules`: {name} is not a lowercase custom-element name (LLP 1024 D1)"
            )),
        })
        .collect()
}

/// The `data-*` words in `manifest`, each checked against HTML's spelling
/// and the words a host already writes.
pub fn data_words(manifest: &Manifest) -> Result<Vec<String>, String> {
    words(manifest, &Words::DATA)
}

/// The `hatch` words in `manifest` (LLP 1075.003.000 Q1): the nodes the
/// app's native code receives, each a word as `data-*`'s are. Either form
/// of the declaration, every platform's words.
pub fn hatch_words(manifest: &Manifest) -> Result<Vec<String>, String> {
    Ok(hatch_table(manifest)?.into_iter().map(|(w, _)| w).collect())
}

/// The platforms a hatch word may name (LLP 1075.003.000.001 §5).
pub const HATCH_PLATFORMS: [&str; 7] =
    ["ios", "tvos", "macos", "web", "linux", "windows", "android"];

/// The words `platform`'s module handles (LLP 1075.003.000.001 §4.3, §5),
/// sorted: every word of a plain list, and of the object form the words
/// that name the platform. The bake reads this for the plan's requirement
/// and the cohort's capability; nothing is scanned or run.
pub fn hatches_on(manifest: &Manifest, platform: &str) -> Result<Vec<String>, String> {
    let mut words: Vec<String> = hatch_table(manifest)?
        .into_iter()
        .filter(|(_, on)| {
            on.as_ref()
                .is_none_or(|on| on.iter().any(|p| p == platform))
        })
        .map(|(w, _)| w)
        .collect();
    words.sort();
    Ok(words)
}

/// The plan's `hatches` rows (LLP 1075.003.000.001 §4.3): each declared
/// word with the mask of the platforms that handle it.
pub fn hatch_rows(manifest: &Manifest) -> Result<Vec<(String, u16)>, String> {
    let mut rows: Vec<(String, u16)> = hatch_table(manifest)?
        .into_iter()
        .map(|(word, on)| {
            let mask = match on {
                None => HATCH_PLATFORMS
                    .iter()
                    .fold(0, |m, p| m | exact_plan::Plan::hatch_platform_bit(p)),
                Some(on) => on
                    .iter()
                    .fold(0, |m, p| m | exact_plan::Plan::hatch_platform_bit(p)),
            };
            (word, mask)
        })
        .collect();
    rows.sort();
    Ok(rows)
}

/// A painting host's hatches for an app's own crate (LLP 1075.003.000.001
/// §5): Rust for its generated entry, or `None` when the app has no
/// `modules/linux/*.rs`. The text is the typed `HatchKey`, with only the
/// words `platform` (`linux`, `windows` or `android`) handles, so module code that
/// names another platform's word does not compile; then a module `hatches`
/// that includes each file, in name order. The files name their one type
/// once (`pub type ExactHatches = App;`), and the entry hands the host
/// `hatches::ExactHatches` and `HatchKey::WORDS`. Nothing here constructs it.
pub fn rust_hatches(
    app_root: &Path,
    manifest: &Manifest,
    platform: &str,
) -> Result<Option<String>, String> {
    // A painting host's own directory when the app has one (`modules/android`,
    // `modules/windows`), else the presenter's, `modules/linux`.
    let own = app_root.join("modules").join(platform);
    let directory = if own.is_dir() {
        own
    } else {
        app_root.join("modules/linux")
    };
    let mut files: Vec<_> = std::fs::read_dir(&directory)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == "rs"))
        .collect();
    if files.is_empty() {
        return Ok(None);
    }
    files.sort();
    let words = hatches_on(manifest, platform)?;
    let variant = |word: &str| {
        let name: String = word
            .split('-')
            .map(|part| part[..1].to_ascii_uppercase() + &part[1..])
            .collect();
        match name.as_str() {
            "Self" => "Self_".to_owned(),
            _ => name,
        }
    };
    let list = |each: &dyn Fn(&String) -> String| words.iter().map(each).collect::<String>();
    let mut text = String::from(
        "/// One of the app's `hatch` words this platform handles (app.json `hatches`).\n#[allow(dead_code)]\n#[derive(Clone, Copy, Debug, PartialEq, Eq)]\npub enum HatchKey {\n",
    );
    text += &list(&|w| format!("    /// `hatch=\"{w}\"`.\n    {},\n", variant(w)));
    text += "}\n#[allow(dead_code)]\nimpl HatchKey {\n    /// The words this platform's hatches were built to handle.\n    pub const WORDS: &'static [&'static str] = &[";
    text += &list(&|w| format!("{w:?}, "));
    text += "];\n    /// The key of a node's word (`HatchKey::of(element.hatch())`).\n    pub fn of(word: &str) -> Option<HatchKey> {\n        match word {\n";
    text += &list(&|w| format!("            {w:?} => Some(HatchKey::{}),\n", variant(w)));
    text += "            _ => None,\n        }\n    }\n    /// The word as the Contract writes it.\n    pub fn word(self) -> &'static str {\n        match self {\n";
    text += &list(&|w| format!("            HatchKey::{} => {w:?},\n", variant(w)));
    text += "        }\n    }\n}\n/// The app's hatches: `modules/linux/*.rs`.\n#[allow(dead_code, missing_docs)]\nmod hatches {\n    #[allow(unused_imports)]\n    use super::HatchKey;\n";
    for file in &files {
        println!("cargo:rerun-if-changed={}", file.display());
        text += &format!("    include!({:?});\n", file.display().to_string());
    }
    println!("cargo:rerun-if-changed={}", directory.display());
    text += "}\n";
    Ok(Some(text))
}

/// A hatch word and the platforms that handle it; `None` is every platform.
type HatchRow = (String, Option<Vec<String>>);

/// `app.json` `hatches`: `["word", …]`, every platform the app builds, or
/// `{"word": ["ios", "web"], …}`, each word with the platforms that handle it.
fn hatch_table(manifest: &Manifest) -> Result<Vec<HatchRow>, String> {
    let Some(table) = manifest.json.get("hatches").and_then(|v| v.as_object()) else {
        return Ok(words(manifest, &Words::HATCHES)?
            .into_iter()
            .map(|w| (w, None))
            .collect());
    };
    table
        .iter()
        .map(|(word, on)| {
            if !contract_lower::dataset::is_word(word) {
                return Err(format!(
                    "app.json `hatches`: \"{word}\" is not a word: lowercase words of letters and digits joined by `-` (LLP 1075.003 Q2)"
                ));
            }
            let listed = on.as_array().ok_or_else(|| {
                format!("app.json `hatches`: `{word}` names its platforms as a list, as [\"ios\", \"web\"]")
            })?;
            let mut platforms = Vec::new();
            for p in listed {
                match p.as_str() {
                    Some(p) if platforms.iter().any(|had| had == p) => {
                        return Err(format!("app.json `hatches`: `{word}` lists `{p}` twice"))
                    }
                    Some(p) if HATCH_PLATFORMS.contains(&p) => platforms.push(p.to_owned()),
                    _ => {
                        return Err(format!(
                            "app.json `hatches`: `{word}` names {p}, which is not a platform ({})",
                            HATCH_PLATFORMS.join(", ")
                        ))
                    }
                }
            }
            Ok((word.clone(), Some(platforms)))
        })
        .collect()
}

/// A list of words an app declares in `app.json`, and how the bake names
/// one it lacks.
struct Words {
    key: &'static str,
    noun: &'static str,
    /// How a use is written in Contract, around the word.
    written: (&'static str, &'static str),
    refusal: &'static str,
    /// Whether a host already writes the word itself (`data-*` only).
    reserved: fn(&str) -> bool,
}

impl Words {
    const DATA: Words = Words {
        key: "data",
        noun: "data-*",
        written: ("data-", ""),
        refusal: "bake-undeclared-data",
        reserved: contract_lower::dataset::reserved,
    };
    const HATCHES: Words = Words {
        key: "hatches",
        noun: "hatch",
        written: ("hatch=\"", "\""),
        refusal: "bake-undeclared-hatch",
        reserved: |_| false,
    };
    fn written(&self, word: &str) -> String {
        format!("`{}{word}{}`", self.written.0, self.written.1)
    }
}

fn words(manifest: &Manifest, kind: &Words) -> Result<Vec<String>, String> {
    let key = kind.key;
    let Some(value) = manifest.json.get(key) else {
        return Ok(Vec::new());
    };
    let words = value
        .as_array()
        .ok_or_else(|| format!("app.json `{key}` is a list of words"))?;
    let mut seen = std::collections::BTreeSet::new();
    words
        .iter()
        .map(|word| match word.as_str() {
            // Each word is one member of the Swift module's typed keys.
            Some(w) if !seen.insert(w) => Err(format!("app.json `{key}`: `{w}` is listed twice")),
            Some(w) if contract_lower::dataset::is_word(w) && !(kind.reserved)(w) => {
                Ok(w.to_owned())
            }
            Some(w) if contract_lower::dataset::is_word(w) => Err(format!(
                "app.json `{key}`: `{w}` is written by the web host on its own elements (LLP 1075.003 §3.3)"
            )),
            _ => Err(format!(
                "app.json `{key}`: {word} is not a word: lowercase words of letters and digits joined by `-` (LLP 1075.003 Q2)"
            )),
        })
        .collect()
}

/// Every module tag, `data-` word and `hatch` word in `file` against what the
/// app at `app_root` declares.
pub(super) fn check(file: &File, app_root: &Path) -> Result<(), Vec<CompileError>> {
    let mut errors = check_modules(file, app_root).err().unwrap_or_default();
    errors.extend(check_words(
        contract_lower::data_words(file),
        app_root,
        &Words::DATA,
    ));
    errors.extend(check_words(
        contract_lower::hatch_words(file),
        app_root,
        &Words::HATCHES,
    ));
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn check_words(used: Vec<(String, Span)>, app_root: &Path, kind: &Words) -> Vec<CompileError> {
    if used.is_empty() {
        return Vec::new();
    }
    let refusal = |id: &str, message: String, span: Span| CompileError {
        pass: "bake",
        id: id.into(),
        message,
        span,
        file: None,
        related: Box::new([]),
    };
    // `hatches` has two forms (a list, or each word with its platforms).
    let read = |m: Manifest| match kind.key {
        "hatches" => hatch_words(&m),
        _ => words(&m, kind),
    };
    let declared = match Manifest::read(app_root).and_then(read) {
        Ok(declared) => declared,
        Err(message) => return vec![refusal("app-manifest", message, Span::default())],
    };
    let mut seen = std::collections::BTreeSet::new();
    used.into_iter()
        .filter(|(word, _)| {
            contract_lower::dataset::is_word(word)
                && !(kind.reserved)(word)
                && !declared.contains(word)
                && seen.insert(word.clone())
        })
        .map(|(word, span)| {
            let hint = contract_syntax::suggestion(&word, declared.iter().map(String::as_str))
                .map(|n| format!("; did you mean {}?", kind.written(n)))
                .unwrap_or_default();
            let listed = if declared.is_empty() {
                format!(
                    "the app declares no {} words (app.json `{}`)",
                    kind.noun, kind.key
                )
            } else {
                format!("the app's words are {}", declared.join(", "))
            };
            refusal(
                kind.refusal,
                format!("{} is not declared: {listed}{hint}", kind.written(&word)),
                span,
            )
        })
        .collect()
}

fn check_modules(file: &File, app_root: &Path) -> Result<(), Vec<CompileError>> {
    let used = contract_lower::module_tags(file);
    if used.is_empty() {
        return Ok(());
    }
    let refusal = |id: &str, message: String, span: Span| CompileError {
        pass: "bake",
        id: id.into(),
        message,
        span,
        file: None,
        related: Box::new([]),
    };
    let roster = Manifest::read(app_root)
        .and_then(|m| roster(&m))
        .map_err(|message| vec![refusal("app-manifest", message, Span::default())])?;
    let errors: Vec<CompileError> = used
        .into_iter()
        .filter(|(tag, _)| !roster.contains(tag))
        .map(|(tag, span)| {
            let hint = contract_syntax::suggestion(&tag, roster.iter().map(String::as_str))
                .map(|n| format!("; did you mean `{n}`?"))
                .unwrap_or_default();
            let listed = if roster.is_empty() {
                "the app declares no native modules (app.json `modules`)".to_owned()
            } else {
                format!("the app's modules are {}", roster.join(", "))
            };
            refusal(
                "bake-unknown-module",
                format!("`{tag}` is not a native module of this app: {listed}{hint}"),
                span,
            )
        })
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
