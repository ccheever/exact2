//! Every example in the guides compiles (LLP 1086 D10): each fenced
//! `contract` block in `docs/*.md`, the recipes (`docs/recipes/*.md`, LLP
//! 1116 D3) and `README.md` compiles with no diagnostics, and each
//! `contract-test` block parses. A deliberately partial block is fenced
//! `text`. A `tamber` block is checked as a design, and is the design of
//! the `contract` blocks after it in its section (LLP 1121 D2). Every
//! failure is reported in one run. So does every starter
//! pattern (`docs/patterns/<name>/`, LLP 1116.001 D3), and the async lane
//! runs each one's tests in an app `exact new` made.

use std::path::{Path, PathBuf};

/// One fenced block: its document, the line of its opening fence, its info
/// string, its body, and the nearest `contract` block before it in the same
/// section (the one a `ts` block's types come from).
pub(crate) struct Block {
    pub(crate) doc: PathBuf,
    pub(crate) line: usize,
    pub(crate) info: String,
    pub(crate) body: String,
    pub(crate) contract: Option<String>,
    /// The nearest `tamber` block before it in the same section: the app's
    /// design (`app.tamber`, LLP 1121 D2), which a `contract` block names.
    pub(crate) design: Option<String>,
    /// The `<!-- check: … -->` marker on the line before the fence, if any:
    /// `app`, `route <name>` or `file`. Marked blocks are parts of one app
    /// and are compiled assembled, not one by one.
    pub(crate) check: Option<String>,
}

impl Block {
    pub(crate) fn at(&self) -> String {
        format!("{}:{}", self.doc.display(), self.line)
    }
}

/// `README.md`, every `docs/*.md` and every recipe (`docs/recipes/*.md`),
/// by path from the repo root.
pub(crate) fn documents() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut docs: Vec<PathBuf> = ["docs", "docs/recipes"]
        .iter()
        .flat_map(|dir| std::fs::read_dir(root.join(dir)).unwrap())
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .collect();
    docs.sort();
    docs.insert(0, root.join("README.md"));
    docs
}

/// The fenced blocks of every document, in order. A heading outside a fence
/// ends a section, so a `ts` block never pairs with an earlier section's
/// Contract. A `ts` block after a split app's marked blocks pairs with the
/// app they assemble, not with the last fragment.
pub(crate) fn blocks() -> Vec<Block> {
    let mut out = Vec::new();
    for doc in documents() {
        let text = std::fs::read_to_string(&doc).unwrap();
        let shown = doc
            .strip_prefix(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .unwrap_or(&doc)
            .to_path_buf();
        let mut open: Option<(usize, String, String)> = None;
        let mut contract: Option<String> = None;
        let mut design: Option<String> = None;
        let mut marker: Option<String> = None;
        let mut pending: Option<String> = None;
        let mut split: Vec<(String, String)> = Vec::new();
        let mut after_split = false;
        for (index, line) in text.lines().enumerate() {
            match &mut open {
                Some((start, info, body)) => {
                    if line.trim() == "```" {
                        let mut block = Block {
                            doc: shown.clone(),
                            line: *start,
                            info: std::mem::take(info),
                            body: std::mem::take(body),
                            contract: contract.clone(),
                            design: design.clone(),
                            check: pending.take(),
                        };
                        if block.info == "ts" && after_split {
                            let parts: Vec<(&str, &str)> = split
                                .iter()
                                .map(|(c, b)| (c.as_str(), b.as_str()))
                                .collect();
                            block.contract = Some(assemble(&parts).0);
                        }
                        if block.info == "tamber" {
                            design = Some(block.body.clone());
                        }
                        if block.info == "contract" {
                            contract = Some(block.body.clone());
                            after_split = block.check.is_some();
                            if let Some(check) = &block.check {
                                split.push((check.clone(), block.body.clone()));
                            }
                        }
                        out.push(block);
                        open = None;
                    } else {
                        body.push_str(line);
                        body.push('\n');
                    }
                }
                None => {
                    if let Some(info) = line.strip_prefix("```") {
                        pending = marker.take();
                        open = Some((index + 1, info.trim().to_string(), String::new()));
                    } else {
                        marker = line
                            .trim()
                            .strip_prefix("<!-- check:")
                            .and_then(|rest| rest.strip_suffix("-->"))
                            .map(|m| m.trim().to_string());
                        if line.starts_with('#') {
                            contract = None;
                            design = None;
                            after_split = false;
                        }
                    }
                }
            }
        }
        assert!(open.is_none(), "{}: unclosed fence", shown.display());
    }
    out
}

#[test]
fn the_paging_columns_recipe_ignores_leave_and_loads_until_the_last_page() {
    use exact_kernel::{Kernel, Offer};
    use exact_plan::Value;
    use exact_runner::{Event, Runner};

    let source = blocks()
        .into_iter()
        .find(|block| {
            block.doc.ends_with("recipes/responsive-columns.md")
                && block.info == "contract"
                && block.body.starts_with("component Feed\n")
        })
        .expect("paging columns example")
        .body;
    let mut runner = Runner::boot(
        contract::compile(&source).unwrap(),
        (),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    runner
        .act(
            "loadMore",
            vec![Value::record(vec![Value::Bool(false), Value::Number(0.0)])],
        )
        .unwrap();
    let cards = |count| Value::list((0..count).map(|n| Value::Number(n as f64)).collect());
    assert_eq!(runner.slot("cards"), Some(&cards(3)));
    for count in [6, 9, 12] {
        let root = runner.kernel().roots()[0];
        runner
            .kernel_mut()
            .compute_layout(root, Offer::definite(390.0, 844.0))
            .unwrap();
        let due = runner.intersect_due();
        assert_eq!(due.len(), 1, "one marker after all columns");
        let entry = &due[0];
        assert!(entry.is_intersecting);
        assert!(runner.intersect_delivered(entry));
        runner
            .dispatch(
                entry.view,
                Event::Intersect {
                    is_intersecting: entry.is_intersecting,
                    intersection_ratio: entry.intersection_ratio,
                },
            )
            .unwrap();
        assert_eq!(runner.slot("cards"), Some(&cards(count)));
    }
    assert!(runner.intersect_due().is_empty());
    assert!(runner.kernel().find_by_test_id("load-more").is_empty());
}

#[test]
fn every_contract_example_in_the_guides_compiles_and_every_test_parses() {
    let dir = std::env::temp_dir().join(format!("exact-docs-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("app.contract");
    let (mut compiled, mut parsed) = (0, 0);
    let mut failures = Vec::new();
    for block in blocks() {
        // A diagnostic's line is the block's, so the document line is the
        // fence's plus it.
        let at = |line: u32| format!("{}:{}", block.doc.display(), block.line + line as usize);
        match block.info.as_str() {
            "contract" if block.check.is_some() => {}
            "contract" => {
                compiled += 1;
                // A declared sound the example names is a short WAV here
                // (LLP 1096 D1: the compiler reads it).
                for line in block.body.lines() {
                    if let Some(src) = line
                        .strip_prefix("sound \"")
                        .and_then(|r| r.strip_suffix('"'))
                    {
                        let file = dir.join(src);
                        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
                        std::fs::write(&file, wav()).unwrap();
                    }
                }
                // A design shown before it in its section is the app's.
                let tamber = dir.join("app.tamber");
                match &block.design {
                    Some(design) => std::fs::write(&tamber, design).unwrap(),
                    None => {
                        let _ = std::fs::remove_file(&tamber);
                    }
                }
                if let Err(all) = contract::compile_path_source_all(&path, &block.body, false) {
                    for e in all {
                        failures.push(format!("{}: [{}] {}", at(e.span.line), e.id, e.message));
                    }
                }
            }
            "tamber" => {
                parsed += 1;
                if let Err(all) = exact_tamber::check(&block.body, &contract_lower::tamber_vocab())
                {
                    for d in all {
                        failures.push(format!("{}: [{}] {}", at(d.line), d.id, d.message));
                    }
                }
            }
            "contract-test" => {
                parsed += 1;
                if let Err(e) = contract::tests(&block.body) {
                    failures.push(format!("{}: [{}] {}", at(e.span.line), e.id, e.message));
                }
            }
            _ => {}
        }
    }
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(compiled > 0 && parsed > 0, "no examples found");
    assert!(
        failures.is_empty(),
        "{} of {compiled} contract and {parsed} contract-test blocks fail; fence a deliberately partial block as `text`:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Blocks marked `<!-- check: app -->`, `<!-- check: route <name> -->` and
/// `<!-- check: file -->` are one app split across a guide (LLP 1115 D7;
/// docs/recipes/navigation-stack.md): the `app` blocks in order, each `route` block in
/// place of the placeholder under `if e.name == "<name>"`, and each `file`
/// block appended. The assembled app compiles with no diagnostics.
#[test]
fn every_split_app_in_the_guides_compiles_assembled() {
    let dir = std::env::temp_dir().join(format!("exact-docs-app-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("app.contract");
    let mut docs: Vec<(PathBuf, Vec<Block>)> = Vec::new();
    for block in blocks() {
        if block.info != "contract" || block.check.is_none() {
            continue;
        }
        match docs.last_mut() {
            Some((doc, list)) if *doc == block.doc => list.push(block),
            _ => docs.push((block.doc.clone(), vec![block])),
        }
    }
    assert!(
        !docs.is_empty(),
        "no split apps found (docs/recipes/navigation-stack.md has one)"
    );
    let mut failures = Vec::new();
    for (doc, list) in &docs {
        let parts: Vec<(&str, &str)> = list
            .iter()
            .map(|b| (b.check.as_deref().unwrap_or(""), b.body.as_str()))
            .collect();
        let (app, wrong) = assemble(&parts);
        for (i, why) in wrong {
            failures.push(format!("{}: {why}", list[i].at()));
        }
        if let Err(all) = contract::compile_path_source_all(&path, &app, false) {
            let lines: Vec<&str> = app.lines().collect();
            for e in all {
                let at = lines
                    .get(e.span.line.saturating_sub(1) as usize)
                    .copied()
                    .unwrap_or("");
                failures.push(format!(
                    "{} (assembled line {}: `{}`): [{}] {}",
                    doc.display(),
                    e.span.line,
                    at.trim(),
                    e.id,
                    e.message
                ));
            }
        }
    }
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(
        failures.is_empty(),
        "{} failures in assembled guide apps:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// The starter patterns (LLP 1116.001 D3): each directory under
/// `docs/patterns` holding an `app.contract`, by path from the repo root.
pub(crate) fn patterns() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/patterns");
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|dir| dir.join("app.contract").is_file())
        .collect();
    dirs.sort();
    dirs
}

/// Every pattern compiles with only its documented diagnostics, and its tests parse.
#[test]
fn every_pattern_compiles_with_expected_diagnostics_and_its_tests_parse() {
    let mut failures = Vec::new();
    let all = patterns();
    assert!(all.len() >= 5, "docs/patterns has {} patterns", all.len());
    for dir in &all {
        let path = dir.join("app.contract");
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        let at = |e: &contract::CompileError| {
            format!(
                "{name}/app.contract:{}: [{}] {}",
                e.span.line, e.id, e.message
            )
        };
        if let Err(all) = contract::compile_path_all(&path, false) {
            failures.extend(all.iter().map(at));
        }
        let warnings = contract::compile_path_all_warned(&path, false).1;
        failures.extend(warnings.iter().map(at));
        match std::fs::read_to_string(dir.join("app.test.contract")) {
            Ok(tests) => match contract::tests(&tests) {
                Ok(decls) if decls.is_empty() => {
                    failures.push(format!("{name}/app.test.contract: no tests"))
                }
                Ok(_) => {}
                Err(e) => failures.push(format!(
                    "{name}/app.test.contract:{}: [{}] {}",
                    e.span.line, e.id, e.message
                )),
            },
            Err(e) => failures.push(format!("{name}/app.test.contract: {e}")),
        }
        if !dir.join("app.ts").is_file() {
            failures.push(format!("{name}: no app.ts"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures in docs/patterns:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Each pattern, written by `bun exact.mjs pattern <name>` into one app
/// `exact new` made, passes its own tests on the web: `test web`, or the
/// command its pattern.json adds (rest's `api-test`, which starts its server).
#[test]
#[ignore = "async lane: makes an app with Bun and Cargo and drives every pattern's tests in Chrome; bun scripts/async.mjs runs it"]
fn every_pattern_passes_its_tests_on_the_web() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let parent = std::env::temp_dir().join(format!("exact-patterns-{}", std::process::id()));
    std::fs::create_dir_all(&parent).unwrap();
    let app = parent.join("patterns-check");
    let run = |cwd: &Path, args: &[&str]| {
        let output =
            std::process::Command::new(std::env::var_os("BUN").unwrap_or_else(|| "bun".into()))
                .args(args)
                .current_dir(cwd)
                .env_remove("EXACT_APP_DIR")
                .output()
                .expect("bun runs");
        let said = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        (output.status.success(), said)
    };
    let script = root.join("scripts/exact.mjs");
    let (made, said) = run(
        &root,
        &[script.to_str().unwrap(), "new", app.to_str().unwrap()],
    );
    assert!(made, "exact new: {said}");
    let mut failures = Vec::new();
    for dir in patterns() {
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        let (wrote, said) = run(&app, &["exact.mjs", "pattern", &name, "--force"]);
        if !wrote {
            failures.push(format!("pattern {name}: {said}"));
            continue;
        }
        let verb = if dir.join("pattern.json").is_file() {
            "api-test"
        } else {
            "test"
        };
        let (passed, said) = run(&app, &["exact.mjs", verb, "web"]);
        if !passed {
            failures.push(format!("{name} ({verb} web):\n{said}"));
        }
    }
    let _ = std::fs::remove_dir_all(&parent);
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// A recipe builders copy carries no tells (LLP 1116 D3 §6.5): `no-tells`
/// passes over every recipe's `contract` blocks and start-here's, over
/// every pattern's (LLP 1116.001 D3), and over `README.md`'s, the first
/// Contract a reader meets (Charlie, 2026-10-12).
#[test]
#[ignore = "async lane: launches Bun for scripts/no-tells.mjs; bun scripts/async.mjs runs it"]
fn the_readme_the_recipes_and_start_here_say_no_literal_colour_size_or_weight() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let readme = root.join("README.md");
    let guides: Vec<PathBuf> = documents()
        .into_iter()
        .filter(|doc| {
            *doc == readme
                || doc.parent().is_some_and(|dir| dir.ends_with("recipes"))
                || doc.ends_with("start-here.md")
        })
        .collect();
    assert!(guides.contains(&readme), "README.md is not a guide");
    assert!(guides.len() > 2, "no recipes found under docs/recipes");
    let output =
        std::process::Command::new(std::env::var_os("BUN").unwrap_or_else(|| "bun".into()))
            .arg(root.join("scripts/no-tells.mjs"))
            .args(&guides)
            .args(patterns().iter().map(|dir| dir.join("app.contract")))
            .output()
            .expect("bun runs scripts/no-tells.mjs");
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// The app a guide's marked blocks make, in order: the `app` blocks, each
/// `route <name>` block in place of its placeholder, each `file` block
/// appended; and what is wrong with a part, by its index.
pub(crate) fn assemble(parts: &[(&str, &str)]) -> (String, Vec<(usize, String)>) {
    let mut wrong = Vec::new();
    let mut app = String::new();
    for (_, body) in parts.iter().filter(|(check, _)| *check == "app") {
        app.push_str(body);
    }
    let mut filled = std::collections::HashSet::new();
    for (i, (check, body)) in parts.iter().enumerate() {
        let known = *check == "app"
            || *check == "file"
            || check
                .strip_prefix("route ")
                .is_some_and(|n| !n.trim().is_empty());
        if !known {
            wrong.push((
                i,
                format!("unknown check marker `{check}` (use `app`, `file` or `route <name>`)"),
            ));
            continue;
        }
        if let Some(name) = check.strip_prefix("route ") {
            let name = name.trim();
            if !filled.insert(name.to_string()) {
                wrong.push((i, format!("route `{name}` is filled twice")));
                continue;
            }
            match fill_route(&app, name, body) {
                Some(filled) => app = filled,
                None => wrong.push((
                    i,
                    format!("no `if e.name == \"{name}\"` placeholder in the app blocks"),
                )),
            }
        }
    }
    for (_, body) in parts.iter().filter(|(check, _)| *check == "file") {
        app.push('\n');
        app.push_str(body);
    }
    (app, wrong)
}

/// The app with the route's block in place of the lines under
/// `if e.name == "<name>"` (the placeholder screen and its comment).
fn fill_route(app: &str, name: &str, route: &str) -> Option<String> {
    let lines: Vec<&str> = app.lines().collect();
    let head = format!("if e.name == \"{name}\"");
    let at = lines.iter().position(|l| l.trim() == head)?;
    let depth = lines[at].len() - lines[at].trim_start().len();
    let mut end = at + 1;
    while end < lines.len() {
        let l = lines[end];
        if !l.trim().is_empty() && l.len() - l.trim_start().len() <= depth {
            break;
        }
        end += 1;
    }
    let pad = " ".repeat(depth + 2);
    let mut out: Vec<String> = lines[..=at].iter().map(|l| l.to_string()).collect();
    for l in route.lines() {
        out.push(if l.trim().is_empty() {
            String::new()
        } else {
            format!("{pad}{l}")
        });
    }
    out.extend(lines[end..].iter().map(|l| l.to_string()));
    let mut s = out.join("\n");
    s.push('\n');
    Some(s)
}

/// A 10 ms, 48 kHz, 16-bit mono WAV of silence.
fn wav() -> Vec<u8> {
    let data = 480u32 * 2;
    let mut b = b"RIFF".to_vec();
    b.extend_from_slice(&(36 + data).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&[1, 0, 1, 0]);
    b.extend_from_slice(&48_000u32.to_le_bytes());
    b.extend_from_slice(&96_000u32.to_le_bytes());
    b.extend_from_slice(&[2, 0, 16, 0]);
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data.to_le_bytes());
    b.resize(b.len() + data as usize, 0);
    b
}
