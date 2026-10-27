//! Compare synchronous VM bodies against explicit immutable T3 source contexts.
//! Supply both context paths for this ignored test; missing inputs fail.
//! Mutation completion and native UI need separate acceptance.

use exact_plan::{Plan, TypeKind, TypesId, Value};
use exact_runner::vm::{self, Env, Outcome};
use serde_json::{json, Value as Json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::time::Instant;

const MANIFEST: &str = "8df0b970114e28d0570d13ea997849ea2a8e4fc6e5369cbd5c04ebbe81d176c7";
const BASELINE: &str = "54f35dce4615d82f36cabe91c510ee22e67462aedb38bb102f43504de56a17a4";
const CANDIDATE: &str = "1237ef887d530d8168df37f2705723d503f479ccaa4da007a6572f320092e143";
const GENERATORS: &str = "c0d4b7197378f147a1fb94b5c07d442b0b9187433291fd13a4e654dfcf5ffe2f";
const GENERATED: &str = "d07023312bcdc90d3cbf7cc3b6f4e92945f14575ccb69def230afaface62585e";
const RECEIPT: &str = "inline-shortcut-source-context.json";
const SEEDS: [(&str, &str); 17] = [
    ("composerSelection", "initialComposerSelectionState"),
    ("confirmationState", "initialConfirmationState"),
    ("fieldProtocol", "initialComposerFieldProtocol"),
    ("hoverState", "initialHoverState"),
    ("intentState", "initialComposerIntentState"),
    ("modifierOwnership", "initialModifierOwnershipState"),
    ("noticeExitState", "initialNoticeExitState"),
    ("paletteSession", "initialPaletteSessionState"),
    ("previewState", "initialPromptPreviewState"),
    ("providerWizardState", "initialProviderWizardState"),
    ("sshResponse", "initialSshResponseState"),
    ("streamGeometry", "initialStreamGeometryState"),
    ("transcriptViewport", "initialTranscriptViewportState"),
    ("keybindingEditor", "initialKeybindingEditorState"),
    ("composerMenu", "initialComposerMenuState"),
    ("composerDraft", "initialComposerDraftState"),
    ("settingsPageState", "initialSettingsPageState"),
];

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

struct Context {
    root: PathBuf,
    inventory: BTreeMap<String, String>,
    generated_digest: String,
    verification_seconds: f64,
    #[cfg(unix)]
    identities: BTreeMap<String, (u64, u64)>,
    #[cfg(unix)]
    root_identity: (u64, u64),
}

fn immutable_tree(root: &Path, directory: &Path, inventory: &BTreeMap<String, String>) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        let metadata = std::fs::symlink_metadata(&path).unwrap();
        assert!(!metadata.file_type().is_symlink(), "{}", path.display());
        assert!(metadata.permissions().readonly(), "{}", path.display());
        if metadata.is_dir() {
            immutable_tree(root, &path, inventory);
        } else {
            assert!(metadata.is_file());
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                assert_eq!(metadata.nlink(), 1);
            }
            let relative = path.strip_prefix(root).unwrap().to_str().unwrap();
            assert!(
                relative == RECEIPT || inventory.contains_key(relative),
                "unlisted {relative}"
            );
        }
    }
}

impl Context {
    fn load(variable: &str, role: &str, expected: &str) -> Self {
        let started = Instant::now();
        let supplied =
            std::env::var_os(variable).unwrap_or_else(|| panic!("{variable} is required"));
        let supplied = PathBuf::from(supplied);
        assert!(supplied.is_absolute(), "{variable} must be absolute");
        let root = supplied.canonicalize().expect("context root exists");
        assert_eq!(supplied, root, "context root must be canonical");
        assert!(std::fs::metadata(&root).unwrap().permissions().readonly());
        let root_metadata = std::fs::symlink_metadata(&root).unwrap();
        assert!(root_metadata.is_dir() && !root_metadata.file_type().is_symlink());
        #[cfg(unix)]
        let root_identity = {
            use std::os::unix::fs::MetadataExt;
            (root_metadata.dev(), root_metadata.ino())
        };
        #[cfg(unix)]
        let mut identities = BTreeMap::new();
        #[cfg(unix)]
        let mut sealed_device = None;
        let receipt_path = root.join(RECEIPT);
        let metadata = std::fs::symlink_metadata(&receipt_path).unwrap();
        assert!(metadata.is_file() && metadata.permissions().readonly());
        let receipt: Json = serde_json::from_slice(&std::fs::read(receipt_path).unwrap())
            .expect("context receipt JSON");
        assert_eq!(receipt["version"], 1);
        assert_eq!(receipt["role"], role);
        assert_eq!(
            receipt["sourceCommit"],
            if role == "baseline" {
                "0b0361e3e8005a968add954b1453a9e41668a547"
            } else {
                "b914ee67eedbc5d5cf7712f697c1c1ff2031646d"
            }
        );
        #[cfg(unix)]
        assert_eq!(
            root_identity.1,
            receipt["rootIdentityAtCreation"]["ino"].as_u64().unwrap()
        );
        assert_eq!(receipt["stale"], json!([]), "unresolved stale outputs");
        let mut inventory = BTreeMap::new();
        let mut digests = BTreeMap::new();
        for (category, count) in [
            ("sources", 326),
            ("generated", 170),
            ("generatorInputs", 18),
        ] {
            let entries = receipt[category]
                .as_array()
                .expect("complete input inventory");
            assert_eq!(entries.len(), count, "{category} cardinality");
            let mut rows = BTreeMap::new();
            for entry in entries {
                let relative = entry["path"].as_str().expect("input path");
                let path = Path::new(relative);
                assert!(path.components().all(|p| matches!(p, Component::Normal(_))));
                let full = root.join(path);
                let mut parent = full.as_path();
                while parent != root {
                    let metadata = std::fs::symlink_metadata(parent).unwrap();
                    assert!(!metadata.file_type().is_symlink(), "{}", parent.display());
                    parent = parent.parent().unwrap();
                }
                assert_eq!(full.canonicalize().unwrap(), full);
                let metadata = std::fs::metadata(&full).unwrap();
                assert!(metadata.is_file() && metadata.permissions().readonly());
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    let sealed = entry["metadata"]["dev"].as_u64().unwrap();
                    assert_eq!(*sealed_device.get_or_insert(sealed), sealed);
                    assert_eq!(metadata.dev(), root_identity.0);
                    assert_eq!(metadata.ino(), entry["metadata"]["ino"].as_u64().unwrap());
                    assert_eq!(metadata.nlink(), 1);
                    assert!(identities
                        .insert(relative.to_owned(), (metadata.dev(), metadata.ino()))
                        .is_none());
                }
                let bytes = std::fs::read(&full).unwrap();
                assert_eq!(entry["bytes"].as_u64().unwrap(), bytes.len() as u64);
                let digest = sha(&bytes);
                assert_eq!(entry["sha256"].as_str().unwrap(), digest, "{relative}");
                assert!(rows.insert(relative.to_owned(), digest.clone()).is_none());
                assert!(inventory.insert(relative.to_owned(), digest).is_none());
            }
            let joined: String = rows.iter().map(|(p, h)| format!("{p}\0{h}\n")).collect();
            digests.insert(category, sha(joined.as_bytes()));
        }
        assert_eq!(digests["sources"], expected, "actual app source identity");
        assert_eq!(digests["generatorInputs"], GENERATORS);
        assert_eq!(digests["generated"], GENERATED);
        assert_eq!(inventory["app.json"], MANIFEST);
        for name in [
            "diff/diff-glyphs",
            "panel/panel-icons",
            "providers/provider-icons",
            "settings/settings-brands",
            "settings/settings-icons",
            "shared/icons",
            "shared/pages-icons",
            "shared/timeline-files",
            "shell/editor-icons",
            "shell/shell-icons",
            "shell/shell-morph",
            "sidebar/sidebar-icons",
            "timeline/timeline-icons",
        ] {
            assert!(
                inventory.contains_key(&format!("src/{name}.contract")),
                "missing {name}"
            );
        }
        let mut context = Self {
            root,
            inventory,
            generated_digest: digests["generated"].clone(),
            verification_seconds: 0.0,
            #[cfg(unix)]
            identities,
            #[cfg(unix)]
            root_identity,
        };
        immutable_tree(&context.root, &context.root, &context.inventory);
        // Check every captured Contract import, including files the fixture does
        // not reach. A missing generator output fails before compiler execution.
        for relative in context
            .inventory
            .keys()
            .filter(|p| p.ends_with(".contract"))
        {
            context.check_imports(relative, &context.text(relative));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let end = std::fs::symlink_metadata(&context.root).unwrap();
            assert_eq!((end.dev(), end.ino()), context.root_identity);
        }
        context.verification_seconds = started.elapsed().as_secs_f64();
        context
    }

    fn text(&self, relative: &str) -> String {
        assert!(
            self.inventory.contains_key(relative),
            "unlisted source {relative}"
        );
        let bytes = std::fs::read(self.root.join(relative)).unwrap();
        assert_eq!(
            sha(&bytes),
            self.inventory[relative],
            "source changed: {relative}"
        );
        String::from_utf8(bytes).unwrap()
    }

    fn recheck(&self) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let root = std::fs::symlink_metadata(&self.root).unwrap();
            assert_eq!((root.dev(), root.ino()), self.root_identity);
            for (path, identity) in &self.identities {
                let metadata = std::fs::symlink_metadata(self.root.join(path)).unwrap();
                assert_eq!((metadata.dev(), metadata.ino()), *identity);
                assert_eq!(metadata.nlink(), 1);
            }
        }
        immutable_tree(&self.root, &self.root, &self.inventory);
        for (relative, digest) in &self.inventory {
            assert_eq!(
                sha(&std::fs::read(self.root.join(relative)).unwrap()),
                *digest,
                "context changed during verification: {relative}"
            );
        }
    }

    fn check_imports(&self, relative: &str, text: &str) {
        for line in text.lines().filter(|line| line.starts_with("use ")) {
            let (_, target) = line.split_once(" from ").expect("literal local import");
            let target = target.split('"').nth(1).expect("quoted import");
            assert!(
                target.starts_with("./") || target.starts_with("../"),
                "{line}"
            );
            let path = self.root.join(relative).parent().unwrap().join(target);
            let resolved = path
                .canonicalize()
                .unwrap_or_else(|e| panic!("{line}: {e}"));
            let confined = resolved
                .strip_prefix(&self.root)
                .expect("import escaped app root");
            assert!(
                self.inventory.contains_key(confined.to_str().unwrap()),
                "{line}"
            );
            assert!(!std::fs::symlink_metadata(path)
                .unwrap()
                .file_type()
                .is_symlink());
        }
    }
}

fn declaration(source: &str, prefix: &str) -> String {
    let lines: Vec<_> = source.lines().collect();
    let start = lines
        .iter()
        .position(|line| line.starts_with(prefix))
        .expect(prefix);
    let mut end = start + 1;
    while end < lines.len() {
        let line = lines[end];
        if !line.trim().is_empty() && !line.starts_with("    ") && !line.trim().starts_with("//") {
            break;
        }
        end += 1;
    }
    format!("{}\n", lines[start..end].join("\n").trim_end())
}

fn shape_fields(source: &str, name: &str) -> Vec<(String, String)> {
    let mut lines = source
        .lines()
        .skip_while(|line| *line != format!("shape {name}"));
    assert!(lines.next().is_some(), "shape {name}");
    lines
        .take_while(|line| line.is_empty() || line.starts_with(' '))
        .filter_map(|line| line.trim().split_once(": "))
        .map(|(name, ty)| (name.to_owned(), ty.to_owned()))
        .collect()
}

fn fixture(context: &Context) -> String {
    let root = context.text("app.contract");
    let ui = context.text("src/shell/app-ui-decisions.contract");
    let app = context.text("src/shell/app-decisions.contract");
    let fields = shape_fields(&app, "AppDecision");
    let private = shape_fields(&ui, "UiDecision");
    assert_eq!(fields.len(), 295);
    assert_eq!(private.len(), 24);
    let actions = [
        "applyState",
        "applyUi",
        "applyCommand",
        "applyPanelState",
        "applyHover",
        "applyChatSends",
        "applyLocalChange",
        "applySharedCompletionEffects",
    ];
    let consumers: String = actions
        .iter()
        .map(|name| declaration(&root, &format!("  action {name}(")))
        .collect();
    let window = context.text("src/shell/app-window.contract");
    let private_body = declaration(&window, "  action applyPrivateDecision(");
    let private_names = [
        "composerCommand",
        "composerShift",
        "taskModelPick",
        "providerVisit",
        "settingsThemeRequest",
        "restGestureUntil",
        "restLastPull",
        "restTop",
        "restHeight",
        "restContent",
        "restWidth",
        "restMovement",
    ];
    let mut source: String = root
        .lines()
        .filter(|line| line.starts_with("use "))
        .map(|line| format!("{line}\n"))
        .collect();
    source.push_str(concat!(
        "use UiDecision, beginUiDecision from \"./src/shell/app-ui-decisions.contract\"\n",
        "use initialAppDecision from \"./src/shell/app-decisions.contract\"\n",
        "use UiStart from \"./src/shell/app-ui-start.contract\"\n",
        "use stepInlineDocumentShortcut from \"./src/shell/app-ui-composer-keyboard-transitions.contract\"\n",
        "use needsInlinePaletteFrame from \"./src/shell/app-ui-queries.contract\"\n",
        "use uiInlineDocumentShortcut from \"./src/shell/app-ui-entry.contract\"\n",
    ));
    let initializers = SEEDS
        .iter()
        .map(|(_, function)| *function)
        .collect::<Vec<_>>()
        .join(", ");
    source.push_str(&format!(
        "use {initializers} from \"./src/shell/app-protocols.contract\"\n"
    ));
    source.push_str("component Probe\n  state captured: option<UiDecision> = none\n");
    source.push_str("  state capturedStart: option<UiStart> = none\n  state fixtureRoot = initialAppDecision()\n");
    let stored: BTreeSet<_> = consumers
        .lines()
        .filter_map(|line| {
            let (name, _) = line.trim().split_once(" = ")?;
            (!name.contains(' ')).then_some(name)
        })
        .collect();
    for name in stored {
        assert!(
            fields.iter().any(|(field, _)| field == name),
            "consumer state {name}"
        );
        source.push_str(&format!("  state {name} = fixtureRoot.{name}\n"));
    }
    for name in private_names {
        let prefix = format!("  state {name} =");
        let line = window
            .lines()
            .find(|line| line.starts_with(&prefix))
            .expect(name);
        source.push_str(line);
        source.push('\n');
    }
    // Only synchronous sends/refresh instructions are evaluated here. Their
    // mutation result slots retain production names and result shapes; queue,
    // refresh-on-completion and then-actions require the eventual baked app.
    for line in root.lines().filter(|line| line.starts_with("  mutation ")) {
        let line = line.split(" //").next().unwrap();
        let end = [" queue", " refreshes", " then"]
            .iter()
            .filter_map(|word| line.find(word))
            .min()
            .unwrap_or(line.len());
        source.push_str(&format!("{}\n", &line[..end]));
    }
    source.push_str("  resource data = fixtureData() as shape Snapshot\n");
    source.push_str("  resource shell = fixtureShell() as shape ShellView\n");
    source.push_str(&consumers);
    source.push_str(&private_body);
    source.push_str("  action defaults(s: UiStart)\n    let seeded = UiStart(s,\n");
    source.push_str(
        &SEEDS
            .iter()
            .map(|(field, function)| format!("      {field}={function}()"))
            .collect::<Vec<_>>()
            .join(",\n"),
    );
    source.push_str(
        "\n    )\n    capturedStart = some(seeded)\n    captured = some(beginUiDecision(seeded))\n",
    );
    source.push_str(concat!(
        "  state needed = false\n",
        "  action shortcut(d: UiDecision, s: UiStart, name: string)\n",
        "    captured = some(stepInlineDocumentShortcut(d, s, name))\n",
        "  action entry(s: UiStart, name: string)\n",
        "    captured = some(uiInlineDocumentShortcut(s, name))\n",
        "  action query(s: UiStart, name: string)\n",
        "    needed = needsInlinePaletteFrame(s, name)\n",
        "  action begin(s: UiStart)\n    captured = some(beginUiDecision(s))\n",
        "  action consume(d: UiDecision)\n    applyPrivateDecision(d)\n    applyUi(d.root)\n",
        "  view\n    text \"inline shortcut verification\"\n",
    ));
    // The root repeats imports; merge names while preserving declaration bodies.
    let mut imports = BTreeMap::<String, BTreeSet<String>>::new();
    let mut body = String::new();
    for line in source.lines() {
        if let Some(line) = line.strip_prefix("use ") {
            let (names, target) = line.split_once(" from ").unwrap();
            let target = target.split('"').nth(1).unwrap();
            imports
                .entry(target.to_owned())
                .or_default()
                .extend(names.split(',').map(|name| name.trim().to_owned()));
        } else {
            body.push_str(line);
            body.push('\n');
        }
    }
    let imports: String = imports
        .into_iter()
        .map(|(target, names)| {
            format!(
                "use {} from \"{target}\"\n",
                names.into_iter().collect::<Vec<_>>().join(", ")
            )
        })
        .collect();
    let source = format!("{imports}{body}");
    context.check_imports("app.contract", &source);
    source
}

struct Compiled {
    plan: Plan,
    strings: vm::Strings,
    initial_slots: Vec<Value>,
    ui: TypesId,
    start: TypesId,
    metrics: Json,
    evaluation_seconds: std::cell::Cell<f64>,
    evaluations: std::cell::Cell<usize>,
}

impl Compiled {
    fn new(context: &Context) -> Self {
        let context_started = Instant::now();
        let source = fixture(context);
        let path = context.root.join("app.contract");
        let graph = contract::with_contract_texts(vec![(path.clone(), source.clone())], || {
            contract::source_graph(&path)
        });
        assert!(graph.errors.is_empty(), "source graph: {:?}", graph.errors);
        assert!(graph.packages.is_empty(), "no external package imports");
        for file in graph.sources {
            let relative = file
                .path
                .strip_prefix(&context.root)
                .expect("graph confined");
            assert!(context.inventory.contains_key(relative.to_str().unwrap()));
        }
        let started = Instant::now();
        eprintln!(
            "INLINE_STAGE {}",
            json!({"stage":"public_compile_begin", "context":context.root.display().to_string()})
        );
        let (plan, _) = contract::compile_path_source_all(&path, &source, false)
            .unwrap_or_else(|errors| panic!("{}: {errors:#?}", context.root.display()));
        eprintln!(
            "INLINE_STAGE {}",
            json!({"stage":"public_compile_returned", "context":context.root.display().to_string(), "seconds":started.elapsed().as_secs_f64(), "codePoolBytes":plan.code.len()})
        );
        plan.validate().unwrap();
        let plan = Plan::decode(&plan.encode()).expect("unchanged decoder accepts plan");
        plan.validate().unwrap();
        eprintln!(
            "INLINE_STAGE {}",
            json!({"stage":"roundtrip_validated", "context":context.root.display().to_string(), "seconds":started.elapsed().as_secs_f64()})
        );
        let duration = started.elapsed().as_secs_f64();
        let action = plan
            .actions
            .iter()
            .find(|a| plan.str(a.name) == "shortcut")
            .unwrap();
        let params: Vec<_> = action.params.iter().map(|id| plan.param(id).ty).collect();
        let (ui, start) = (params[0], params[1]);
        assert_eq!(plan.type_(ui).fields.len, 24);
        assert_eq!(plan.type_(field(&plan, ui, "root").1).fields.len, 295);
        for (name, _) in SEEDS {
            field(&plan, field(&plan, ui, "root").1, name);
            field(&plan, start, name);
        }
        let mut metrics = metrics(&plan, duration, &sha(source.as_bytes()));
        metrics["productionMutationDeclarations"] = json!(context
            .text("app.contract")
            .lines()
            .filter(|line| line.starts_with("  mutation "))
            .collect::<Vec<_>>());
        metrics["contextVerificationSeconds"] = json!(context.verification_seconds);
        metrics["contextThroughRoundtripSeconds"] =
            json!(context.verification_seconds + context_started.elapsed().as_secs_f64());
        let strings = vm::intern(&plan);
        let initial_slots = plan
            .slots
            .iter()
            .enumerate()
            .map(|(index, slot)| sample(&plan, slot.ty, index as u32 + 1, false, 0))
            .collect();
        Self {
            plan,
            strings,
            initial_slots,
            ui,
            start,
            metrics,
            evaluation_seconds: std::cell::Cell::new(0.0),
            evaluations: std::cell::Cell::new(0),
        }
    }

    fn slots(&self) -> Vec<Value> {
        self.initial_slots.clone()
    }

    fn evaluate(&self, name: &str, args: &[Value], slots: &[Value], now: f64) -> Outcome {
        let started = Instant::now();
        let plan = &self.plan;
        let action = plan
            .actions
            .iter()
            .find(|a| plan.str(a.name) == name)
            .expect(name);
        let writes: Vec<_> = action
            .writes
            .iter()
            .map(|id| plan.write(id).slot.0)
            .collect();
        let outcome = vm::eval(
            plan.code(action.body),
            &Env {
                plan,
                strings: &self.strings,
                router: None,
                lists: None,
                format: None,
                geometry: None,
                slots,
                derives: &[],
                resources: &[],
                params: args,
                frames: &[],
                now_ms: now,
                pending_resources: &[],
                failed_resources: &[],
                failed_why: &[],
                pending_mutations: &[],
                failed_mutations: &[],
                store_dependent_derives: &[],
                store_dependent_resources: &[],
            },
            &writes,
        )
        .unwrap_or_else(|trap| panic!("{name}: {trap:?}"));
        self.evaluation_seconds
            .set(self.evaluation_seconds.get() + started.elapsed().as_secs_f64());
        self.evaluations.set(self.evaluations.get() + 1);
        outcome
    }

    fn captured(&self, outcome: &Outcome, slot: &str) -> Value {
        let values: Vec<_> = outcome
            .writes
            .iter()
            .filter_map(|(index, value)| {
                (self.plan.str(self.plan.slots[*index as usize].name) == slot).then_some(value)
            })
            .collect();
        assert_eq!(values.len(), 1, "one {slot} write");
        match values[0] {
            Value::Option(Some(record)) => record.as_ref().clone(),
            other => panic!("expected some(record), got {other:?}"),
        }
    }
}

fn field(plan: &Plan, ty: TypesId, name: &str) -> (usize, TypesId) {
    let found: Vec<_> = plan
        .type_(ty)
        .fields
        .iter()
        .enumerate()
        .filter_map(|(i, id)| {
            let field = plan.field(id);
            (plan.str(field.name) == name).then_some((i, field.ty))
        })
        .collect();
    assert_eq!(found.len(), 1, "field {name}");
    found[0]
}

fn sample(plan: &Plan, ty: TypesId, seed: u32, filled: bool, depth: usize) -> Value {
    assert!(depth < 64, "recursive fixture type");
    let row = plan.type_(ty);
    match row.kind {
        TypeKind::Number => Value::Number(if filled { f64::from(seed) + 0.25 } else { 0.0 }),
        TypeKind::Bool => Value::Bool(filled),
        TypeKind::String => Value::str(&if filled {
            format!("sentinel-{seed}")
        } else {
            String::new()
        }),
        TypeKind::Unit => Value::Unit,
        TypeKind::Option => Value::Option(if filled {
            Some(std::rc::Rc::new(sample(
                plan,
                row.elem.unwrap(),
                seed + 1,
                filled,
                depth + 1,
            )))
        } else {
            None
        }),
        TypeKind::List => Value::list(if filled && depth < 5 {
            vec![sample(plan, row.elem.unwrap(), seed + 1, filled, depth + 1)]
        } else {
            vec![]
        }),
        TypeKind::Record => Value::record(
            row.fields
                .iter()
                .enumerate()
                .map(|(i, id)| {
                    sample(
                        plan,
                        plan.field(id).ty,
                        seed.wrapping_mul(7).wrapping_add(i as u32 + 1),
                        filled,
                        depth + 1,
                    )
                })
                .collect(),
        ),
    }
}

fn tagged(name: &str, value: Json) -> Json {
    Json::Object([(name.to_owned(), value)].into_iter().collect())
}

fn normalize(plan: &Plan, ty: TypesId, value: &Value) -> Json {
    let row = plan.type_(ty);
    match (row.kind, value) {
        (TypeKind::Number, Value::Number(number)) => {
            json!({"numberBits": format!("{:016x}", number.to_bits())})
        }
        (TypeKind::Bool, Value::Bool(boolean)) => json!({"bool": boolean}),
        (TypeKind::String, value) if value.as_str().is_some() => {
            json!({"string": value.as_str().unwrap()})
        }
        (TypeKind::Unit, Value::Unit) => json!({"unit": true}),
        (TypeKind::Option, Value::Option(None)) => json!({"none": true}),
        (TypeKind::Option, Value::Option(Some(inner))) => {
            tagged("some", normalize(plan, row.elem.unwrap(), inner))
        }
        (TypeKind::List, Value::List(items)) => tagged(
            "list",
            Json::Array(
                items
                    .iter()
                    .map(|v| normalize(plan, row.elem.unwrap(), v))
                    .collect(),
            ),
        ),
        (TypeKind::Record, Value::Record(items)) => {
            assert_eq!(
                items.len(),
                row.fields.len as usize,
                "complete record arity"
            );
            let mut result = BTreeMap::new();
            for (i, id) in row.fields.iter().enumerate() {
                let field = plan.field(id);
                assert!(result
                    .insert(plan.str(field.name), normalize(plan, field.ty, &items[i]))
                    .is_none());
            }
            tagged(
                "record",
                Json::Object(
                    result
                        .into_iter()
                        .map(|(name, value)| (name.to_owned(), value))
                        .collect(),
                ),
            )
        }
        _ => panic!("typed normalization mismatch: {:?}, {value:?}", row.kind),
    }
}

fn raw(value: &Value) -> Json {
    match value {
        Value::Number(number) => json!({"numberBits": format!("{:016x}", number.to_bits())}),
        Value::Bool(boolean) => json!({"bool": boolean}),
        Value::Unit => json!({"unit": true}),
        Value::Option(None) => json!({"none": true}),
        Value::Option(Some(inner)) => json!({"some": raw(inner)}),
        Value::List(items) => json!({"list": items.iter().map(raw).collect::<Vec<_>>()}),
        Value::Record(items) => {
            json!({"recordPositions": items.iter().map(raw).collect::<Vec<_>>()})
        }
        value => json!({"string": value.as_str().expect("text")}),
    }
}

fn effects(plan: &Plan, outcome: &Outcome) -> Json {
    assert!(
        outcome.row_writes.is_empty(),
        "fixture has no row-scoped state"
    );
    let writes: Vec<_> = outcome
        .writes
        .iter()
        .map(|(slot, value)| {
            let slot = &plan.slots[*slot as usize];
            json!([plan.str(slot.name), normalize(plan, slot.ty, value)])
        })
        .collect();
    let sends: Vec<_> = outcome
        .sends
        .iter()
        .map(|(mutation, source, args)| {
            let signatures: Vec<_> = plan
                .sources
                .iter()
                .filter(|s| plan.str(s.name) == source)
                .collect();
            assert_eq!(signatures.len(), 1, "source {source}");
            let signature = signatures[0];
            assert_eq!(signature.params.len as usize, args.len());
            let args: Vec<_> = signature
                .params
                .iter()
                .enumerate()
                .map(|(i, id)| normalize(plan, plan.source_param(id).ty, &args[i]))
                .collect();
            json!([
                plan.str(plan.mutations[*mutation as usize].name),
                source,
                args
            ])
        })
        .collect();
    json!({
        "value": raw(&outcome.value), "writes": writes, "rowWrites": [], "sends": sends,
        "commands": outcome.commands.iter().map(|(name, args)| json!([name, args.iter().map(raw).collect::<Vec<_>>()])).collect::<Vec<_>>(),
        "refreshes": outcome.refreshes.iter().map(|id| plan.str(plan.resources[*id as usize].name)).collect::<Vec<_>>(),
        "storeDependent": outcome.store_dependent,
    })
}

fn set(plan: &Plan, ty: TypesId, value: &mut Value, path: &str, replacement: Value) {
    let (head, tail) = path
        .split_once('.')
        .map_or((path, None), |(a, b)| (a, Some(b)));
    let (index, member) = field(plan, ty, head);
    let Value::Record(items) = value else {
        panic!("record at {path}")
    };
    let mut items = items.iter().cloned().collect::<Vec<_>>();
    if let Some(tail) = tail {
        set(plan, member, &mut items[index], tail, replacement);
    } else {
        normalize(plan, member, &replacement);
        items[index] = replacement;
    }
    *value = Value::record(items);
}

fn at<'a>(plan: &Plan, mut ty: TypesId, mut value: &'a Value, path: &str) -> (TypesId, &'a Value) {
    for part in path.split('.') {
        let (index, member) = field(plan, ty, part);
        let Value::Record(items) = value else {
            panic!("record at {path}")
        };
        value = &items[index];
        ty = member;
    }
    (ty, value)
}

fn flags(plan: &Plan, ty: TypesId, value: &mut Value, active: bool) {
    let members: Vec<_> = plan
        .type_(ty)
        .fields
        .iter()
        .filter_map(|id| {
            let field = plan.field(id);
            let name = plan.str(field.name);
            (plan.type_(field.ty).kind == TypeKind::Bool
                && ["write", "send", "host"]
                    .iter()
                    .any(|p| name.starts_with(p)))
            .then(|| name.to_owned())
        })
        .collect();
    for name in members {
        set(plan, ty, value, &name, Value::Bool(active));
    }
}

fn metrics(plan: &Plan, seconds: f64, fixture_sha: &str) -> Json {
    let mut ranges = BTreeSet::new();
    let mut bodies = BTreeSet::new();
    let actions: Vec<_> = plan
        .actions
        .iter()
        .map(|action| {
            let code = plan.code(action.body);
            ranges.insert((action.body.offset, action.body.len));
            bodies.insert(sha(code));
            let mut ops = BTreeMap::<String, usize>::new();
            for instruction in vm::instructions(code) {
                *ops.entry(format!("{:?}", instruction.unwrap().op))
                    .or_default() += 1;
            }
            json!({"name": plan.str(action.name), "offset": action.body.offset,
            "bytes": code.len(), "sha256": sha(code), "opcounts": ops})
        })
        .collect();
    let mut end = 0;
    let mut union = 0u64;
    for (offset, len) in &ranges {
        let next = offset.checked_add(*len).unwrap();
        union += u64::from(next.saturating_sub(end.max(*offset)));
        end = end.max(next);
    }
    json!({"compileValidateRoundtripSeconds": seconds, "fixtureSha256": fixture_sha,
        "codePoolBytes": plan.code.len(), "encodedPlanBytes": plan.encode().len(), "encodedPlanSHA256": sha(&plan.encode()), "actions": actions, "distinctBodies": bodies.len(),
        "distinctRanges": ranges.len(), "distinctRangeSum": ranges.iter().map(|(_, n)| u64::from(*n)).sum::<u64>(),
        "rangeUnionBytes": union})
}

fn incoming(compiled: &Compiled, original: &Value, profile: usize) -> Value {
    if profile == 0 {
        return original.clone();
    }
    let plan = &compiled.plan;
    let mut value = sample(plan, compiled.ui, 71, true, 0);
    flags(plan, compiled.ui, &mut value, profile > 1);
    let (root_ty, root) = at(plan, compiled.ui, &value, "root");
    let mut root = root.clone();
    flags(plan, root_ty, &mut root, profile > 1);
    set(
        plan,
        root_ty,
        &mut root,
        "changedKind",
        Value::str(match profile {
            2 => "command",
            3 => "providerChange",
            4 => "providerAdd",
            _ => "unrelated",
        }),
    );
    set(plan, compiled.ui, &mut value, "root", root);
    value
}

#[derive(Clone)]
struct Shortcut {
    command: &'static str,
    kind: &'static str,
    target: &'static str,
    extra: &'static str,
}
struct Case {
    name: &'static str,
    rows: Vec<Shortcut>,
    patches: Vec<(&'static str, Value)>,
    unchanged: bool,
}
fn row(command: &'static str, kind: &'static str) -> Shortcut {
    Shortcut {
        command,
        kind,
        target: "arbitrary:target|α",
        extra: "extra value\nβ",
    }
}
fn case(name: &'static str, rows: Vec<Shortcut>) -> Case {
    Case {
        name,
        rows,
        patches: vec![],
        unchanged: false,
    }
}
fn cases() -> Vec<Case> {
    let mut result = Vec::new();
    for name in ["", "unknown", "missing"] {
        let mut c = case(name, vec![]);
        c.unchanged = true;
        result.push(c);
    }
    let mut unknown = case("unknown", vec![row("unknown", "command")]);
    unknown.unchanged = true;
    result.push(unknown);
    for modal in [false, true] {
        for palette in [false, true] {
            for settings in [false, true] {
                for open in [false, true] {
                    let mut c = case("sidebar.toggle", vec![]);
                    c.patches = vec![
                        ("modal", Value::Bool(modal)),
                        ("paletteSession.open", Value::Bool(palette)),
                        ("settingsPageState.open", Value::Bool(settings)),
                        ("data.sidebarOpen", Value::Bool(open)),
                    ];
                    c.unchanged = modal || palette || settings;
                    result.push(c);
                }
            }
        }
    }
    for (name, mode) in [
        ("commandPalette.toggle", "command"),
        ("filePicker.toggle", "files"),
        ("projectSearch.toggle", "content"),
        ("chat.find", "content"),
    ] {
        for open in [false, true] {
            for page in ["", "arbitrary page"] {
                for suppress in [
                    "",
                    "settingsPageState.open",
                    "pageCover",
                    "promptLinkPage.show",
                ] {
                    let mut r = row(name, "palette");
                    r.target = mode;
                    r.extra = page;
                    let mut c = case(name, vec![r]);
                    c.patches = vec![
                        ("paletteSession.open", Value::Bool(open)),
                        ("paletteSession.mode", Value::str(mode)),
                    ];
                    if !suppress.is_empty() {
                        c.patches.push((suppress, Value::Bool(true)));
                    }
                    result.push(c);
                }
            }
        }
    }
    for name in [
        "rightPanel.toggle",
        "rightPanel.toggleMaximized",
        "preview.toggle",
    ] {
        for kind in ["diff", "command", "panel-close", "unknown"] {
            let mut c = case(name, vec![row(name, kind)]);
            c.unchanged = kind == "unknown";
            result.push(c);
        }
    }
    for popover in [false, true] {
        for shown in [false, true] {
            let mut c = case(
                "threadPanel.toggle",
                vec![row("threadPanel.toggle", "details")],
            );
            c.patches = vec![
                ("detailsPopover", Value::Bool(popover)),
                ("detailsShown", Value::Bool(shown)),
            ];
            result.push(c);
        }
    }
    for name in [
        "terminal.toggle",
        "thread.previous",
        "thread.next",
        "thread.jump.7",
    ] {
        result.push(case(name, vec![row(name, "command")]));
    }
    for target in [
        "",
        "task-model:task",
        "source-control-writer-model:|scope",
        "review-guide-model:|scope",
        "other-setting:scope",
    ] {
        for kind in ["model", "wrong-kind"] {
            let mut c = case("modelPicker.jump.3", vec![row("modelPicker.jump.3", kind)]);
            c.patches
                .push(("composerMenu.modelTarget.id", Value::str(target)));
            c.unchanged = kind != "model";
            result.push(c);
        }
    }
    for name in ["navigation.back", "navigation.forward"] {
        for kind in ["thread", "back", "palette-run", "new-thread-in", "unknown"] {
            let mut c = case(name, vec![row(name, kind)]);
            c.unchanged = kind == "unknown";
            result.push(c);
        }
    }
    let mut first = row("terminal.toggle", "command");
    first.target = "first-target";
    first.extra = "first-extra";
    result.push(case(
        "terminal.toggle",
        vec![first, row("terminal.toggle", "command")],
    ));
    let mut c = case(
        "modelPicker.jump.3",
        vec![
            row("modelPicker.jump.3", "wrong-kind"),
            row("modelPicker.jump.3", "model"),
        ],
    );
    c.unchanged = true;
    result.push(c);
    result
}
fn start_for(
    compiled: &Compiled,
    initial: &Value,
    c: &Case,
    pending: bool,
    blocked: bool,
    frame: usize,
    seed: bool,
) -> Value {
    let (plan, ty) = (&compiled.plan, compiled.start);
    let mut start = initial.clone();
    if seed {
        for (index, (name, _)) in SEEDS.iter().enumerate() {
            let member = field(plan, ty, name).1;
            set(
                plan,
                ty,
                &mut start,
                name,
                sample(plan, member, index as u32 + 100, true, 0),
            );
        }
    }
    for (name, value) in &c.patches {
        set(plan, ty, &mut start, name, value.clone());
    }
    set(plan, ty, &mut start, "commandPending", Value::Bool(pending));
    set(
        plan,
        ty,
        &mut start,
        "inlineDocumentKeysBlocked",
        Value::Bool(blocked),
    );
    let list_ty = field(plan, ty, "shortcuts").1;
    let item_ty = plan.type_(list_ty).elem.unwrap();
    let rows = c
        .rows
        .iter()
        .map(|r| {
            let mut value = sample(plan, item_ty, 1, false, 0);
            for (name, text) in [
                ("command", r.command),
                ("kind", r.kind),
                ("target", r.target),
                ("extra", r.extra),
            ] {
                set(plan, item_ty, &mut value, name, Value::str(text));
            }
            value
        })
        .collect();
    set(plan, ty, &mut start, "shortcuts", Value::list(rows));
    let option_ty = field(plan, ty, "paletteFrame").1;
    let geometry_ty = plan.type_(option_ty).elem.unwrap();
    let bounds = if frame == 0 {
        Value::Option(None)
    } else {
        let mut geometry = sample(plan, geometry_ty, 1, false, 0);
        set(
            plan,
            geometry_ty,
            &mut geometry,
            "width",
            Value::Number(if frame == 1 { 0.0 } else { 320.0 }),
        );
        set(
            plan,
            geometry_ty,
            &mut geometry,
            "height",
            Value::Number(48.0),
        );
        Value::some(geometry)
    };
    set(plan, ty, &mut start, "paletteFrame", bounds);
    start
}
#[test]
#[ignore = "requires admitted immutable InlineShortcut original/candidate source contexts"]
fn actual_inline_shortcut_records_private_effects_and_size() {
    let baseline = Context::load("EXACT_T3_INLINE_BASELINE", "baseline", BASELINE);
    let candidate = Context::load("EXACT_T3_INLINE_CANDIDATE", "candidate", CANDIDATE);
    assert!(
        !baseline.root.starts_with(&candidate.root) && !candidate.root.starts_with(&baseline.root)
    );
    assert_eq!(baseline.generated_digest, candidate.generated_digest);
    let (a, b) = (Compiled::new(&baseline), Compiled::new(&candidate));
    assert_eq!(
        a.metrics["productionMutationDeclarations"],
        b.metrics["productionMutationDeclarations"]
    );
    let mut defaults = Vec::new();
    for compiled in [&a, &b] {
        let s = sample(&compiled.plan, compiled.start, 1, false, 0);
        let outcome = compiled.evaluate("defaults", &[s], &compiled.slots(), 0.0);
        defaults.push((
            compiled.captured(&outcome, "capturedStart"),
            compiled.captured(&outcome, "captured"),
        ));
    }
    assert_eq!(
        normalize(&a.plan, a.start, &defaults[0].0),
        normalize(&b.plan, b.start, &defaults[1].0)
    );
    assert_eq!(
        normalize(&a.plan, a.ui, &defaults[0].1),
        normalize(&b.plan, b.ui, &defaults[1].1)
    );
    let cases = cases();
    let evaluation_started = Instant::now();
    let mut comparisons = 0usize;
    for (index, c) in cases.iter().enumerate() {
        for pending in [false, true] {
            for blocked in [false, true] {
                for frame in 0..3 {
                    for seed in [false, true] {
                        let sa = start_for(&a, &defaults[0].0, c, pending, blocked, frame, seed);
                        let sb = start_for(&b, &defaults[1].0, c, pending, blocked, frame, seed);
                        assert_eq!(
                            normalize(&a.plan, a.start, &sa),
                            normalize(&b.plan, b.start, &sb)
                        );
                        for now in [0.0, 137.25] {
                            let qa = a.evaluate(
                                "query",
                                &[sa.clone(), Value::str(c.name)],
                                &a.slots(),
                                now,
                            );
                            let qb = b.evaluate(
                                "query",
                                &[sb.clone(), Value::str(c.name)],
                                &b.slots(),
                                now,
                            );
                            assert_eq!(
                                effects(&a.plan, &qa),
                                effects(&b.plan, &qb),
                                "frame predicate case {index}"
                            );
                            for profile in 0..5 {
                                let initial =
                                    |compiled: &Compiled, start: &Value, original: &Value| {
                                        if profile == 0 {
                                            let output = compiled.evaluate(
                                                "begin",
                                                std::slice::from_ref(start),
                                                &compiled.slots(),
                                                now,
                                            );
                                            let input = compiled.captured(&output, "captured");
                                            for (name, _) in SEEDS {
                                                let (st, sv) =
                                                    at(&compiled.plan, compiled.start, start, name);
                                                let (rt, rv) = at(
                                                    &compiled.plan,
                                                    compiled.ui,
                                                    &input,
                                                    &format!("root.{name}"),
                                                );
                                                assert_eq!(
                                                    normalize(&compiled.plan, st, sv),
                                                    normalize(&compiled.plan, rt, rv)
                                                );
                                            }
                                            input
                                        } else {
                                            incoming(compiled, original, profile)
                                        }
                                    };
                                let ia = initial(&a, &sa, &defaults[0].1);
                                let ib = initial(&b, &sb, &defaults[1].1);
                                assert_eq!(
                                    normalize(&a.plan, a.ui, &ia),
                                    normalize(&b.plan, b.ui, &ib)
                                );
                                let mut returned = Vec::new();
                                let mut consumed = Vec::new();
                                let mut traces = Vec::new();
                                for (compiled, start, input) in [(&a, &sa, &ia), (&b, &sb, &ib)] {
                                    let out = compiled.evaluate(
                                        "shortcut",
                                        &[input.clone(), start.clone(), Value::str(c.name)],
                                        &compiled.slots(),
                                        now,
                                    );
                                    let value = compiled.captured(&out, "captured");
                                    if profile == 0 {
                                        let entry = compiled.evaluate(
                                            "entry",
                                            &[start.clone(), Value::str(c.name)],
                                            &compiled.slots(),
                                            now,
                                        );
                                        let entry_value = compiled.captured(&entry, "captured");
                                        assert_eq!(
                                            normalize(&compiled.plan, compiled.ui, &value),
                                            normalize(&compiled.plan, compiled.ui, &entry_value),
                                            "actual public entry composition"
                                        );
                                    }
                                    if blocked || c.unchanged {
                                        assert_eq!(
                                            normalize(&compiled.plan, compiled.ui, &value),
                                            normalize(&compiled.plan, compiled.ui, input),
                                            "unchanged case {index}"
                                        );
                                    }
                                    returned.push(normalize(&compiled.plan, compiled.ui, &value));
                                    traces.push(effects(&compiled.plan, &out));
                                    let applied = compiled.evaluate(
                                        "consume",
                                        &[value],
                                        &compiled.slots(),
                                        now,
                                    );
                                    consumed.push(effects(&compiled.plan, &applied));
                                }
                                assert_eq!(
                                    returned[0], returned[1],
                                    "all295+24 case{index} profile{profile}"
                                );
                                assert_eq!(traces[0], traces[1], "VM return/slots case{index}");
                                assert_eq!(
                                    consumed[0], consumed[1],
                                    "actual private-before-root consumer case{index}"
                                );
                                comparisons += 1;
                            }
                        }
                    }
                }
            }
        }
        if index == 0 || (index + 1) % 16 == 0 || index + 1 == cases.len() {
            eprintln!(
                "INLINE_PROGRESS {}",
                json!({"completedCases":index+1, "pairedComparisons":comparisons, "baselineVM":a.evaluations.get(), "candidateVM":b.evaluations.get(), "seconds":evaluation_started.elapsed().as_secs_f64()})
            );
        }
    }
    eprintln!(
        "{}",
        json!({"baseline":a.metrics,"candidate":b.metrics,"explicitCases":cases.len(),
        "comparisons":comparisons,"baselineVM":a.evaluations.get(),"candidateVM":b.evaluations.get(),
        "categoryOrderingOnly":true,"asyncRepliesCovered":false,"nativeUICovered":false,
        "clockReadCountObserved":false,"geometryInputCasesOnly":true})
    );
    baseline.recheck();
    candidate.recheck();
    assert!(
        b.plan.code.len() < a.plan.code.len(),
        "actual code pool must shrink"
    );
    assert!(
        b.metrics["rangeUnionBytes"].as_u64().unwrap()
            < a.metrics["rangeUnionBytes"].as_u64().unwrap(),
        "actual unique action byte union must shrink"
    );
}
