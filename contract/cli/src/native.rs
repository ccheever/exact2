//! The app's native-module roster (LLP 1024 D1): the tags its module
//! artifact serves, declared in `app.json` as `"modules": ["tag", …]`, and
//! the views its packages serve (`package_views`). A module tag neither
//! names is `bake-unknown-module`, so a typo
//! is a named diagnostic in the dev loop and in every build, never an empty
//! box at runtime. Beside it, the app's `data-*` words (LLP 1075.003 Q2):
//! `"data": ["word", …]`, from which the Apple build also writes the Swift
//! module's typed keys; a word the list lacks is `bake-undeclared-data`.
//! And its `hatch` words (LLP 1075.003.000 Q1): `"hatches": ["word", …]`, the
//! same way, `bake-undeclared-hatch`; or `{"word": ["ios", …]}`, each word
//! with the platforms that handle it (LLP 1075.003.000.001 §5).

use super::{CompileError, Manifest};
use crate::package_views::PackageView;
use contract_syntax::{File, Span};
use std::path::Path;

mod packages;
mod slot;
pub use packages::{bound, plan_packages, rust_packages, write_desktop_packages, Bound};
pub use slot::{native_slot, NativeSlot};

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

/// What an app's Linux crate adds to its generated entry for its hatches,
/// and the host call its `main` makes (LLP 1075.003.000.001 §5): the text of
/// [`rust_hatches`] and `run_with_hatches`, or nothing and plain `run` when
/// the app has no hatch files. The platform is the build target's own, so
/// an Android or Windows build of the crate handles that platform's words.
pub fn rust_hatch_entry(
    app_root: &Path,
    manifest: &Manifest,
    target: &str,
) -> Result<(String, &'static str), String> {
    let platform = if target.contains("android") {
        "android"
    } else if target.contains("windows") {
        "windows"
    } else {
        "linux"
    };
    Ok(match rust_hatches(app_root, manifest, platform)? {
        Some(text) => (
            text,
            "run_with_hatches::<AppData, hatches::ExactHatches>(PLAN, COMPAT, HatchKey::WORDS)",
        ),
        None => {
            // A first hatch file is a new one under `modules`: build again then.
            let modules = app_root.join("modules");
            if modules.is_dir() {
                println!("cargo:rerun-if-changed={}", modules.display());
            }
            (String::new(), "run::<AppData>(PLAN, COMPAT)")
        }
    })
}

/// What an app's Android crate (the Canvas host's library) writes beside
/// its plan (LLP 1115.001.000 D2, D4): `hatches.rs`, which defines
/// `exact_hatches()`, the install `canvas_jni!` hands the host (the app's
/// Rust hatches, `modules/android/*.rs` or `modules/linux/*.rs`, with the
/// reader's Kotlin ones told beside them; the reader's alone when the app
/// has no Rust ones; none when it gives `android` no word); and what
/// [`android_reach`] writes.
pub fn android_entry(
    app_root: &Path,
    manifest: &Manifest,
    plan: &exact_plan::Plan,
    out_dir: &Path,
) -> Result<(), String> {
    let install = "Option<::exact_linux::hatches::Install>";
    let text = match rust_hatches(app_root, manifest, "android")? {
        Some(rust) => format!(
            "{rust}fn exact_hatches() -> {install} {{\n    Some(::exact_linux::hatches::install_with_reader::<hatches::ExactHatches>(HatchKey::WORDS))\n}}\n"
        ),
        None => {
            let modules = app_root.join("modules");
            if modules.is_dir() {
                println!("cargo:rerun-if-changed={}", modules.display());
            }
            let words = hatches_on(manifest, "android")?;
            if words.is_empty() {
                format!("fn exact_hatches() -> {install} {{\n    None\n}}\n")
            } else {
                format!(
                    "fn exact_hatches() -> {install} {{\n    Some(::exact_linux::hatches::install_with_reader::<()>(&{words:?}))\n}}\n"
                )
            }
        }
    };
    std::fs::write(out_dir.join("hatches.rs"), text).map_err(|e| e.to_string())?;
    android_reach(app_root, plan, out_dir)
}

/// What the APK's build reads beside an Android plan, from an app's
/// Android crate or a game's bake: `uses.txt`, the plan's capabilities by
/// name, one a line (a plan with a date or time control links Compose
/// Material 3), and `permissions.txt`, the Android permissions its grant
/// lines and module names derive beyond the compatibility id's reach.
pub fn android_reach(
    app_root: &Path,
    plan: &exact_plan::Plan,
    out_dir: &Path,
) -> Result<(), String> {
    let uses: Vec<&str> = exact_runner::uses(plan).iter().map(|c| c.name()).collect();
    std::fs::write(out_dir.join("uses.txt"), uses.join("\n")).map_err(|e| e.to_string())?;
    // The grant ceiling of the compatibility id the build script wrote
    // beside it, when it wrote one.
    let compat = std::fs::read_to_string(out_dir.join("compat.json")).ok();
    let ceiling = compat
        .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
        .and_then(|c| c["inputs"]["grantCeiling"].as_str().map(str::to_owned));
    let text = app_text(app_root);
    let used = |name: &str| names(&text, name);
    let mut permissions: Vec<String> = android_write_permissions(plan, ceiling.as_deref(), used)
        .into_iter()
        .map(str::to_owned)
        .collect();
    for (line, permission) in derived(plan, &text, "android") {
        let granted = |g: &str| {
            (line.strip_prefix("device.")).is_none_or(|n| exact_runner::device::granted(g, n))
        };
        if ceiling.as_deref().is_none_or(granted) && !permissions.contains(&permission) {
            permissions.push(permission);
        }
    }
    permissions.sort_unstable();
    std::fs::write(out_dir.join("permissions.txt"), permissions.join("\n"))
        .map_err(|e| e.to_string())
}

/// What the app's use of its module names declares to `target`'s platform
/// beside its grant lines (`derives`, LLP 1133.026 D19 item 4: `ios` usage
/// keys, `android` permissions), each with the grant line whose purpose a
/// usage key takes: a name's own list where the app's source names it; per
/// option, the permission of each value the source writes as a string
/// literal, or of a flag it names. A computed value of such an option is
/// refused at bake (`contract-module-literal`), so a Contract caller's
/// values are in its text; data code's are its own literals.
pub fn derived(plan: &exact_plan::Plan, text: &str, target: &str) -> Vec<(String, String)> {
    use serde_json::Value;
    // A package's specifier (`"@exact/pedometer"`) names no module name.
    let mut code = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(['"', '\'']) {
        let quote = &rest[at..at + 1];
        let close = rest[at + 1..].find(quote).map(|c| at + 1 + c);
        let spec = rest[at + 1..].starts_with('@');
        code.push_str(&rest[..at]);
        match (close, spec) {
            (Some(close), true) => rest = &rest[close + 1..],
            _ => {
                code.push_str(quote);
                rest = &rest[at + 1..];
            }
        }
    }
    code.push_str(rest);
    let text = code.as_str();
    let quoted = |v: &str| text.contains(&format!("\"{v}\"")) || text.contains(&format!("'{v}'"));
    let mut out: Vec<(String, String)> = Vec::new();
    for row in plan.modules.iter() {
        let sub: Value = serde_json::from_str(plan.str(row.substitute)).unwrap_or_default();
        let declares = &sub["derives"][target];
        if declares.is_null() || !names(text, plan.str(row.declared)) {
            continue;
        }
        let lines = match &sub["nameGrants"] {
            Value::Array(own) => own.clone(),
            _ => sub["grants"].as_array().cloned().unwrap_or_default(),
        };
        let line = (lines.first().and_then(Value::as_str))
            .unwrap_or_default()
            .to_owned();
        let mut keys: Vec<&str> = Vec::new();
        match declares {
            Value::Array(list) => keys.extend(list.iter().filter_map(Value::as_str)),
            Value::Object(by) => {
                for (option, d) in by {
                    let flag = option.rsplit('.').next().unwrap_or(option);
                    match d {
                        Value::String(k) if names(text, flag) => keys.push(k),
                        Value::Object(values) => keys.extend(
                            (values.iter())
                                .filter(|(v, _)| quoted(v))
                                .filter_map(|(_, k)| k.as_str()),
                        ),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        for key in keys {
            let pair = (line.clone(), key.to_owned());
            if !out.contains(&pair) {
                out.push(pair);
            }
        }
    }
    out
}

/// The Android permissions the plan's module names derive beside what
/// their grant lines do (LLP 1133.018 D11): a name that writes the person's
/// store (`"writes": true`) and that the app `uses` derives each of its
/// lines' `android_writes` where `grants` grants that line (every line, with
/// no `grants`). So an app that only reads its calendar declares no
/// `WRITE_CALENDAR`. The plan holds every name of a module the app uses, so
/// which the app calls is read from its source, as the Apple build finds the
/// strings keys an option names (LLP 1133.002 D10).
pub fn android_write_permissions(
    plan: &exact_plan::Plan,
    grants: Option<&str>,
    uses: impl Fn(&str) -> bool,
) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for row in plan.modules.iter() {
        let sub: serde_json::Value =
            serde_json::from_str(plan.str(row.substitute)).unwrap_or_default();
        if sub["writes"] != true || !uses(plan.str(row.declared)) {
            continue;
        }
        let lines = match &sub["nameGrants"] {
            serde_json::Value::Array(own) => own.clone(),
            _ => sub["grants"].as_array().cloned().unwrap_or_default(),
        };
        for name in lines
            .iter()
            .filter_map(|l| l.as_str()?.strip_prefix("device."))
        {
            if grants.is_some_and(|g| !exact_runner::device::granted(g, name)) {
                continue;
            }
            let row = exact_runner::device::DEVICES
                .iter()
                .find(|d| d.name == name);
            out.extend(row.map_or(&[][..], |d| d.android_writes));
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// The app's source that can call a module name: its `.contract` and
/// TypeScript files, and its data crate's Rust, where the generated binding
/// names it in snake case.
pub fn app_text(app_root: &Path) -> String {
    fn walk(dir: &Path, out: &mut String) {
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if path.is_dir() {
                if !(name.starts_with('.') || name == "node_modules" || name == "target") {
                    walk(&path, out);
                }
            } else if [".contract", ".ts", ".rs"]
                .iter()
                .any(|e| name.ends_with(e))
            {
                out.push_str(&std::fs::read_to_string(&path).unwrap_or_default());
                out.push('\n');
            }
        }
    }
    let mut out = String::new();
    walk(app_root, &mut out);
    out
}

/// Whether `text` names module name `name` as a word, as written or as the
/// binding's snake-case function.
fn names(text: &str, name: &str) -> bool {
    let snake: String = name
        .chars()
        .flat_map(|c| match c.is_ascii_uppercase() {
            true => vec!['_', c.to_ascii_lowercase()],
            false => vec![c],
        })
        .collect();
    let word = |w: &str| {
        text.match_indices(w).any(|(at, _)| {
            let ident = |c: char| c.is_alphanumeric() || c == '_';
            !text[..at].ends_with(ident) && !text[at + w.len()..].starts_with(ident)
        })
    };
    word(name) || word(&snake)
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
    // A painting host's own Rust when the app has some (`modules/android`,
    // `modules/windows`), else the presenter's, `modules/linux`. Android's
    // directory may hold only the reader's Kotlin (LLP 1115.001.000 D4).
    let rust = |directory: &Path| {
        std::fs::read_dir(directory)
            .into_iter()
            .flatten()
            .flatten()
            .any(|entry| entry.path().extension().is_some_and(|e| e == "rs"))
    };
    let own = app_root.join("modules").join(platform);
    let directory = if rust(&own) {
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
/// app at `app_root` declares; the package views the file places, for the
/// plan's `views` table.
pub(super) fn check(file: &File, app_root: &Path) -> Result<Vec<PackageView>, Vec<CompileError>> {
    let (views, mut errors) = match check_modules(file, app_root) {
        Ok(views) => (views, Vec::new()),
        Err(errors) => (Vec::new(), errors),
    };
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
        Ok(views)
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
            let stale = if Manifest::generated_stale(app_root) {
                "; .shells/app.json is stale, generated from an earlier app.json: \
                 `bun exact.mjs shells` regenerates it, as the web and test builds do"
            } else {
                ""
            };
            refusal(
                kind.refusal,
                format!(
                    "{} is not declared: {listed}{hint}{stale}",
                    kind.written(&word)
                ),
                span,
            )
        })
        .collect()
}

/// Each module tag `file` places against the app's roster and the views its
/// packages serve (`crate::package_views`); the package views it places.
fn check_modules(file: &File, app_root: &Path) -> Result<Vec<PackageView>, Vec<CompileError>> {
    let used = contract_lower::module_tags(file);
    if used.is_empty() {
        return Ok(Vec::new());
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
    let packages = crate::package_views::declared(app_root)
        .map_err(|message| vec![refusal("bake-view-twice", message, Span::default())])?;
    if let Some(view) = packages.values().find(|v| roster.contains(&v.tag)) {
        return Err(vec![refusal(
            "bake-view-twice",
            format!(
                "`{}` is declared by app.json `modules` and by {}: a tag is one per document, so take it out of `modules` or depend on another package",
                view.tag, view.package
            ),
            used.iter().find(|(t, _)| *t == view.tag).map_or_else(Span::default, |u| u.1),
        )]);
    }
    let known: Vec<&str> = (roster.iter().map(String::as_str))
        .chain(packages.keys().map(String::as_str))
        .collect();
    let mut placed = Vec::new();
    let errors: Vec<CompileError> = used
        .into_iter()
        .filter(|(tag, _)| {
            if let Some(view) = packages.get(tag) {
                if !placed.contains(view) {
                    placed.push(view.clone());
                }
            }
            !known.contains(&tag.as_str())
        })
        .map(|(tag, span)| {
            let hint = contract_syntax::suggestion(&tag, known.iter().copied())
                .map(|n| format!("; did you mean `{n}`?"))
                .unwrap_or_default();
            let listed = if known.is_empty() {
                "the app declares no native modules (app.json `modules`) and depends on no package that serves a view".to_owned()
            } else {
                format!("the app's modules and its packages' views are {}", known.join(", "))
            };
            let add = crate::package_views::standard_with(&tag)
                .map(|m| format!("; exact2's `{m}` module serves it: `exact add {m}` adds it to the app's package.json"))
                .unwrap_or_default();
            refusal(
                "bake-unknown-module",
                format!("`{tag}` is not a native module of this app: {listed}{hint}{add}"),
                span,
            )
        })
        .collect();
    if errors.is_empty() {
        Ok(placed)
    } else {
        Err(errors)
    }
}
