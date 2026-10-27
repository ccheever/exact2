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
const BASELINE: &str = "a087cf8d57c479481b6d9088204521b4a291b7081f52eb4d599ab9702ca1c5f4";
const CANDIDATE: &str = "54f35dce4615d82f36cabe91c510ee22e67462aedb38bb102f43504de56a17a4";
const GENERATORS: &str = "c0d4b7197378f147a1fb94b5c07d442b0b9187433291fd13a4e654dfcf5ffe2f";
const RECEIPT: &str = "eng28617-source-context.json";
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
        let receipt_path = root.join(RECEIPT);
        let metadata = std::fs::symlink_metadata(&receipt_path).unwrap();
        assert!(metadata.is_file() && metadata.permissions().readonly());
        let receipt: Json = serde_json::from_slice(&std::fs::read(receipt_path).unwrap())
            .expect("context receipt JSON");
        assert_eq!(receipt["version"], 1);
        assert_eq!(receipt["role"], role);
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
    let mut source: String = root
        .lines()
        .filter(|line| line.starts_with("use "))
        .map(|line| format!("{line}\n"))
        .collect();
    source.push_str(concat!(
        "use UiDecision, beginUiDecision from \"./src/shell/app-ui-decisions.contract\"\n",
        "use initialAppDecision from \"./src/shell/app-decisions.contract\"\n",
        "use UiStart from \"./src/shell/app-ui-start.contract\"\n",
        "use stepCommand from \"./src/shell/app-ui-dispatch-transitions.contract\"\n",
        "use stepPaletteRun, stepPaletteShow from \"./src/shell/app-ui-palette-transitions-2.contract\"\n",
        "use stepPaletteGo, stepPaletteFocusBack from \"./src/shell/app-ui-palette-transitions.contract\"\n",
        "use stepPaletteFlag, stepPaletteHover from \"./src/shell/app-ui-palette-transitions.contract\"\n",
        "use stepPaletteClose, stepPaletteEdit, stepPaletteKey from \"./src/shell/app-ui-navigation-transitions-2.contract\"\n",
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
    for (action, signature, call) in [
        (
            "command",
            "op: string, id: string, value: string, n: number",
            "stepCommand(d, s, op, id, value, n)",
        ),
        (
            "run",
            "op: string, arg: string, arg2: string",
            "stepPaletteRun(d, s, op, arg, arg2)",
        ),
        (
            "show",
            "mode: string, page: string",
            "stepPaletteShow(d, s, mode, page)",
        ),
        (
            "go",
            "page: string, query: string, back: bool",
            "stepPaletteGo(d, s, page, query, back)",
        ),
        (
            "focusBack",
            "page: string",
            "stepPaletteFocusBack(d, s, page)",
        ),
        ("flag", "name: string", "stepPaletteFlag(d, s, name)"),
        (
            "hover",
            "index: number, over: bool",
            "stepPaletteHover(d, s, index, over)",
        ),
        ("edit", "value: string", "stepPaletteEdit(d, s, value)"),
        ("key", "name: string", "stepPaletteKey(d, s, name)"),
    ] {
        source.push_str(&format!("  action {action}(d: UiDecision, s: UiStart, {signature})\n    captured = some({call})\n"));
    }
    source.push_str(concat!(
        "  action begin(s: UiStart)\n    captured = some(beginUiDecision(s))\n",
        "  action close(d: UiDecision, s: UiStart)\n    captured = some(stepPaletteClose(d, s))\n",
        "  action consumeCommand(d: UiDecision)\n    applyCommand(d.root)\n",
        "  action consumePalette(d: UiDecision)\n    applyUi(d.root)\n",
        "  action consumeState(d: UiDecision)\n    applyState(d.root)\n",
        "  view\n    text \"command and palette verification\"\n",
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
        let (plan, _) = contract::compile_path_source_all(&path, &source, false)
            .unwrap_or_else(|errors| panic!("{}: {errors:#?}", context.root.display()));
        plan.validate().unwrap();
        let plan = Plan::decode(&plan.encode()).expect("unchanged decoder accepts plan");
        plan.validate().unwrap();
        let duration = started.elapsed().as_secs_f64();
        let action = plan
            .actions
            .iter()
            .find(|a| plan.str(a.name) == "command")
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
        Self {
            plan,
            ui,
            start,
            metrics,
            evaluation_seconds: std::cell::Cell::new(0.0),
            evaluations: std::cell::Cell::new(0),
        }
    }

    fn slots(&self) -> Vec<Value> {
        self.plan
            .slots
            .iter()
            .enumerate()
            .map(|(index, slot)| sample(&self.plan, slot.ty, index as u32 + 1, false, 0))
            .collect()
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
        let strings = vm::intern(plan);
        let outcome = vm::eval(
            plan.code(action.body),
            &Env {
                plan,
                strings: &strings,
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
            json!({"some": normalize(plan, row.elem.unwrap(), inner)})
        }
        (TypeKind::List, Value::List(items)) => {
            json!({"list": items.iter().map(|v| normalize(plan, row.elem.unwrap(), v)).collect::<Vec<_>>()})
        }
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
            json!({"record": result})
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
        "codePoolBytes": plan.code.len(), "actions": actions, "distinctBodies": bodies.len(),
        "distinctRanges": ranges.len(), "distinctRangeSum": ranges.iter().map(|(_, n)| u64::from(*n)).sum::<u64>(),
        "rangeUnionBytes": union})
}

#[derive(Clone)]
struct Call {
    action: &'static str,
    args: Vec<Value>,
}

fn call(action: &'static str, args: Vec<Value>) -> Call {
    Call { action, args }
}

fn texts(values: &[&str]) -> Vec<Value> {
    values.iter().map(|v| Value::str(v)).collect()
}

fn command_cases() -> Vec<Call> {
    let mut cases = Vec::new();
    let mut add = |op: &str, id: &str, n: f64| {
        let mut args = texts(&[op, id, "distinct value"]);
        args.push(Value::Number(n));
        cases.push(call("command", args));
    };
    for op in [
        "secretlocal:answer",
        "secretlocal:answer:focus-in",
        "secretlocal:answer:focus-out",
        "chatlocal:item-detail",
        "chatlocal:item-detail:focus-in",
        "chatlocal:item-detail-extra",
    ] {
        add(op, "id", 17.0);
    }
    for id in [
        "drag:a",
        "gutter:a",
        "comment:a",
        "to",
        "end",
        "",
        "cite:a",
        "drag",
        "gutter",
        "comment",
    ] {
        add("diffreview", id, 17.0);
    }
    for op in [
        "terminallocal:resize",
        "terminalpanellocal:resize",
        "terminallocal:resize:focus-in",
        "terminalpanellocal:resize:focus-out",
        "other:focus-in",
        "other:focus-out",
        "other:focus-in-extra",
        "draft",
        "search",
        "sidebar",
        "answer",
        "choice",
        "dismiss-error",
        "close-diff",
        "diff-view",
        "favorite-model",
        "draft-extra",
    ] {
        add(op, "id", 17.0);
    }
    for id in ["", "undo-key"] {
        add("sidebar:undo", id, 17.0);
    }
    for op in ["chat:send", "chat:", "chatlocal:other", "other", ""] {
        for n in [0.0, 19.0, -7.0] {
            add(op, "id", n);
        }
    }
    assert_eq!(cases.len(), 50);
    cases
}

fn palette_cases() -> Vec<Call> {
    let mut result = Vec::new();
    for op in [
        "page",
        "browse",
        "browse-to",
        "mode",
        "device",
        "copy-thread",
        "theme-editor",
        "settings",
        "project-settings",
        "usage",
        "pull-requests",
        "thread-pull-requests",
        "flow",
        "open-project",
        "finder",
        "open-file",
        "thread",
        "new-thread",
        "unknown",
        "",
    ] {
        result.push(call("run", texts(&[op, "ancestor", "distinct-arg2"])));
    }
    for mode in ["content", "files", "command", ""] {
        result.push(call("run", texts(&["mode", mode, "distinct-arg2"])));
    }
    for op in ["page", "browse"] {
        for page in ["current", "new-page", ""] {
            result.push(call("run", texts(&[op, page, "distinct-arg2"])));
        }
    }
    for page in [
        "ancestor",
        "new-page",
        "current",
        "",
        "pr-link-thread",
        "pr-linked|id",
    ] {
        for back in [false, true] {
            let mut args = texts(&[page, "replacement-query"]);
            args.push(Value::Bool(back));
            result.push(call("go", args));
        }
        result.push(call("focusBack", texts(&[page])));
    }
    for mode in ["command", "content", "files", "other"] {
        for page in ["", "page"] {
            result.push(call("show", texts(&[mode, page])));
        }
    }
    for value in ["", "changed query"] {
        result.push(call("edit", texts(&[value])));
    }
    for flag in ["case", "word", "regex", "unknown", ""] {
        result.push(call("flag", texts(&[flag])));
    }
    for (index, over) in [(3.0, true), (-1.0, true), (4.0, false)] {
        result.push(call("hover", vec![Value::Number(index), Value::Bool(over)]));
    }
    for key in [
        "ArrowDown",
        "ArrowUp",
        "Back",
        "Backspace",
        "Enter",
        "unknown",
    ] {
        result.push(call("key", texts(&[key])));
    }
    result.push(call("close", vec![]));
    result
}

struct StartCase {
    group: &'static str,
    value: Value,
}

impl StartCase {
    fn covers(&self, action: &str) -> bool {
        match self.group {
            "base" => true,
            "command" => action == "command",
            "trail" => ["go", "run", "edit", "key"].contains(&action),
            "mode" => ["run", "show", "close", "edit"].contains(&action),
            "busy" => ["run", "show", "close", "focusBack"].contains(&action),
            "theme" | "utility" => action == "run",
            "focus" => ["focusBack", "show", "close"].contains(&action),
            group => group == action,
        }
    }
}

fn start_cases(compiled: &Compiled, original: &Value) -> Vec<StartCase> {
    let (plan, ty) = (&compiled.plan, compiled.start);
    let mut result = Vec::new();
    let mut add = |group, patches: Vec<(&str, Value)>| {
        let mut start = original.clone();
        for (path, value) in patches {
            set(plan, ty, &mut start, path, value);
        }
        result.push(StartCase {
            group,
            value: start,
        });
    };
    add("base", vec![]);
    let nondefault = SEEDS
        .iter()
        .enumerate()
        .map(|(index, (name, _))| {
            let member = field(plan, ty, name).1;
            (*name, sample(plan, member, index as u32 + 100, true, 0))
        })
        .collect();
    add("base", nondefault);
    for pending in [false, true] {
        add("command", vec![("commandPending", Value::Bool(pending))]);
    }
    for (pages, queries) in [
        (vec![], vec![]),
        (vec!["ancestor", "middle"], vec!["saved", "middle-query"]),
        (vec!["ancestor", "middle"], vec![]),
        (vec!["ancestor", "ancestor"], vec!["first", "second"]),
    ] {
        add(
            "trail",
            vec![
                ("paletteSession.page", Value::str("current")),
                ("paletteSession.query", Value::str("current-query")),
                ("paletteSession.trailPages", Value::list(texts(&pages))),
                ("paletteSession.trailQueries", Value::list(texts(&queries))),
            ],
        );
    }
    for mode in ["command", "content", "files", "other"] {
        for open in [false, true] {
            add(
                "mode",
                vec![
                    ("paletteSession.mode", Value::str(mode)),
                    ("paletteSession.open", Value::Bool(open)),
                    ("paletteSession.returnQuery", Value::str("return-query")),
                ],
            );
        }
    }
    for busy in [false, true] {
        for open in [false, true] {
            add(
                "busy",
                vec![
                    ("paletteSession.busy", Value::Bool(busy)),
                    ("settingsPageState.open", Value::Bool(open)),
                    ("utilityPage", Value::str("old-utility")),
                    ("rightPanelAt", Value::str("old-panel")),
                    ("shell.panel.key", Value::str("current-panel")),
                ],
            );
        }
    }
    for dialog in ["create", "edit", "duplicate", "other"] {
        add(
            "theme",
            vec![
                ("settingsThemeDialog", Value::str(dialog)),
                ("settingsThemeSubject", Value::str("old-subject")),
            ],
        );
    }
    let frame_ty = field(plan, ty, "paletteFrame").1;
    for width in [0.0, 320.0] {
        let inner = plan.type_(frame_ty).elem.unwrap();
        let mut geometry = sample(plan, inner, 1, false, 0);
        set(plan, inner, &mut geometry, "width", Value::Number(width));
        set(plan, inner, &mut geometry, "height", Value::Number(48.0));
        let geometry = Value::some(geometry);
        add("focus", vec![("paletteFrame", geometry.clone())]);
        if width > 0.0 {
            for suppress in ["settingsPageState.open", "pageCover", "promptLinkPage.show"] {
                add(
                    "focus",
                    vec![
                        ("paletteFrame", geometry.clone()),
                        (suppress, Value::Bool(true)),
                    ],
                );
            }
            for page in ["pr-link-thread", "pr-linked|id"] {
                add(
                    "focus",
                    vec![
                        ("paletteFrame", geometry.clone()),
                        ("paletteSession.page", Value::str(page)),
                        ("paletteSession.open", Value::Bool(true)),
                    ],
                );
            }
        }
    }
    for pop in [false, true] {
        add("edit", vec![("palette.popOnEmpty", Value::Bool(pop))]);
    }
    for mode in ["content", "files"] {
        for queries in [vec![], vec!["saved-query"]] {
            add(
                "edit",
                vec![
                    ("palette.popOnEmpty", Value::Bool(true)),
                    ("palette.parent", Value::str("ancestor")),
                    ("paletteSession.mode", Value::str(mode)),
                    (
                        "paletteSession.trailPages",
                        Value::list(texts(&["ancestor"])),
                    ),
                    ("paletteSession.trailQueries", Value::list(texts(&queries))),
                ],
            );
        }
    }
    add(
        "flag",
        vec![
            ("paletteSession.caseSensitive", Value::Bool(true)),
            ("paletteSession.wholeWord", Value::Bool(true)),
            ("paletteSession.regex", Value::Bool(true)),
        ],
    );
    add(
        "hover",
        vec![("paletteSession.cursor", Value::Number(-1.0))],
    );
    add("hover", vec![("paletteSession.cursor", Value::Number(3.0))]);
    for page in ["", "usage", "pull-requests"] {
        add(
            "utility",
            vec![
                ("utilityPage", Value::str(page)),
                ("pageTrail", Value::list(texts(&["old-page"]))),
            ],
        );
    }
    let palette_ty = field(plan, ty, "palette").1;
    let rows_ty = field(plan, palette_ty, "rows").1;
    let row_ty = plan.type_(rows_ty).elem.unwrap();
    let mut rows = Vec::new();
    for index in 0..3 {
        let mut row = sample(plan, row_ty, 1, false, 0);
        for (path, number) in [
            ("index", index as f64),
            ("top", index as f64 * 60.0),
            ("height", 48.0),
        ] {
            set(plan, row_ty, &mut row, path, Value::Number(number));
        }
        rows.push(row);
    }
    for count in [0.0, 1.0, 3.0] {
        for highlight in [-2.0, -1.0, 0.0, 1.0, 2.0, 5.0] {
            for matching in [false, true] {
                add(
                    "key",
                    vec![
                        ("palette.count", Value::Number(count)),
                        ("paletteHighlight", Value::Number(highlight)),
                        (
                            "palette.rows",
                            Value::list(if matching { rows.clone() } else { vec![] }),
                        ),
                        ("palette.back", Value::Bool(true)),
                        ("palette.parent", Value::str("ancestor")),
                        ("paletteSession.scroll", Value::Number(70.0)),
                        ("paletteListHeight", Value::Number(80.0)),
                    ],
                );
            }
        }
    }
    add(
        "key",
        vec![
            ("palette.back", Value::Bool(true)),
            ("paletteSession.query", Value::str("nonempty")),
            ("paletteSession.cursor", Value::Number(3.0)),
            ("paletteListHeight", Value::Number(0.0)),
            ("palette.count", Value::Number(3.0)),
            ("palette.rows", Value::list(rows)),
            ("paletteHighlight", Value::Number(1.0)),
        ],
    );
    result
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

fn sequences() -> Vec<Vec<Call>> {
    let mut sequences = Vec::new();
    for op in [
        "settings",
        "project-settings",
        "usage",
        "pull-requests",
        "thread-pull-requests",
    ] {
        sequences.push(vec![
            call(
                "command",
                vec![
                    Value::str("sidebar:undo"),
                    Value::str("undo"),
                    Value::str(""),
                    Value::Number(0.0),
                ],
            ),
            call("run", texts(&[op, "project", "focus"])),
            call(
                "command",
                vec![
                    Value::str("chat:send"),
                    Value::str("id"),
                    Value::str("text"),
                    Value::Number(0.0),
                ],
            ),
        ]);
    }
    sequences
}

fn consumer(action: &str) -> &'static str {
    match action {
        "command" => "consumeCommand",
        "edit" | "flag" | "hover" => "consumeState",
        _ => "consumePalette",
    }
}

fn compare_step(
    a: &Compiled,
    b: &Compiled,
    starts: (&Value, &Value),
    values: (&Value, &Value),
    call: &Call,
    now: f64,
    label: &str,
) -> (Value, Value) {
    let mut returned = Vec::new();
    let mut traces = Vec::new();
    let mut consumed = Vec::new();
    for (compiled, start, value) in [(a, starts.0, values.0), (b, starts.1, values.1)] {
        let mut args = vec![value.clone(), start.clone()];
        args.extend(call.args.clone());
        let slots = compiled.slots();
        let result = compiled.evaluate(call.action, &args, &slots, now);
        traces.push(effects(&compiled.plan, &result));
        let value = compiled.captured(&result, "captured");
        normalize(&compiled.plan, compiled.ui, &value);
        let applied = compiled.evaluate(
            consumer(call.action),
            std::slice::from_ref(&value),
            &slots,
            now,
        );
        consumed.push(effects(&compiled.plan, &applied));
        returned.push(value);
    }
    assert_eq!(traces[0], traces[1], "returned record/VM effects {label}");
    assert_eq!(
        consumed[0], consumed[1],
        "actual production consumer body {label}"
    );
    (returned.remove(0), returned.remove(0))
}

fn guarded_clock(compiled: &Compiled, start: &Value, incoming: &Value) {
    let plan = &compiled.plan;
    for pending in [false, true] {
        let mut start = start.clone();
        set(
            plan,
            compiled.start,
            &mut start,
            "commandPending",
            Value::Bool(pending),
        );
        set(
            plan,
            compiled.start,
            &mut start,
            "wallTime.epochAtZero",
            Value::Number(1000.0),
        );
        for (op, n, timed) in [
            ("chat:send", 0.0, true),
            ("chat:send", 17.0, false),
            ("chatlocal:other", 0.0, false),
            ("other", 0.0, false),
            ("secretlocal:answer", 0.0, false),
            ("terminallocal:focus-in", 0.0, false),
            ("sidebar:undo", 0.0, false),
        ] {
            let mut outcomes = Vec::new();
            for now in [0.0, 137.25] {
                let mut args = vec![incoming.clone(), start.clone()];
                args.extend(texts(&[op, "undo-key", "value"]));
                args.push(Value::Number(n));
                let output = compiled.evaluate("command", &args, &compiled.slots(), now);
                let value = compiled.captured(&output, "captured");
                if timed && !pending {
                    assert_eq!(
                        at(plan, compiled.ui, &value, "root.command.p3").1,
                        &Value::Number(1000.0 + now)
                    );
                }
                if op == "sidebar:undo" {
                    assert_eq!(
                        at(plan, compiled.ui, &value, "root.writeModifierOwnership").1,
                        &Value::Bool(true)
                    );
                    assert_eq!(
                        at(
                            plan,
                            compiled.ui,
                            &value,
                            "root.modifierOwnership.undoHeldKey"
                        )
                        .1,
                        &Value::str("undo-key")
                    );
                }
                outcomes.push(normalize(plan, compiled.ui, &value));
            }
            if !timed || pending {
                assert_eq!(
                    outcomes[0], outcomes[1],
                    "clock-independent {op}, pending={pending}"
                );
            }
        }
    }
}

#[test]
#[ignore = "requires admitted original/selected T3 source contexts"]
fn actual_app_command_palette_records_effects_and_size() {
    let baseline = Context::load("EXACT_T3_COMMAND_PALETTE_BASELINE", "baseline", BASELINE);
    let candidate = Context::load("EXACT_T3_COMMAND_PALETTE_CANDIDATE", "candidate", CANDIDATE);
    assert!(
        !baseline.root.starts_with(&candidate.root) && !candidate.root.starts_with(&baseline.root)
    );
    assert_eq!(
        baseline.generated_digest, candidate.generated_digest,
        "same generated inputs"
    );
    let (a, b) = (Compiled::new(&baseline), Compiled::new(&candidate));
    assert_eq!(
        a.metrics["productionMutationDeclarations"],
        b.metrics["productionMutationDeclarations"]
    );
    let mut defaults = Vec::new();
    for compiled in [&a, &b] {
        let start = sample(&compiled.plan, compiled.start, 1, false, 0);
        let output = compiled.evaluate("defaults", &[start], &compiled.slots(), 0.0);
        defaults.push((
            compiled.captured(&output, "capturedStart"),
            compiled.captured(&output, "captured"),
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
    guarded_clock(&a, &defaults[0].0, &defaults[0].1);
    guarded_clock(&b, &defaults[1].0, &defaults[1].1);
    let (starts_a, starts_b) = (
        start_cases(&a, &defaults[0].0),
        start_cases(&b, &defaults[1].0),
    );
    assert_eq!(starts_a.len(), starts_b.len());
    let mut cases = command_cases();
    cases.extend(palette_cases());
    let started = Instant::now();
    let mut comparisons = 0usize;
    for profile in 0..5 {
        for (index, (sa, sb)) in starts_a.iter().zip(&starts_b).enumerate() {
            assert_eq!(sa.group, sb.group);
            let (input_a, input_b) = if profile == 0 {
                let initial = |compiled: &Compiled, start: &Value| {
                    let output = compiled.evaluate(
                        "begin",
                        std::slice::from_ref(start),
                        &compiled.slots(),
                        0.0,
                    );
                    compiled.captured(&output, "captured")
                };
                (initial(&a, &sa.value), initial(&b, &sb.value))
            } else {
                (
                    incoming(&a, &defaults[0].1, profile),
                    incoming(&b, &defaults[1].1, profile),
                )
            };
            assert_eq!(
                normalize(&a.plan, a.ui, &input_a),
                normalize(&b.plan, b.ui, &input_b)
            );
            for now in [0.0, 137.25] {
                for case in cases.iter().filter(|case| sa.covers(case.action)) {
                    compare_step(
                        &a,
                        &b,
                        (&sa.value, &sb.value),
                        (&input_a, &input_b),
                        case,
                        now,
                        &format!(
                            "profile={profile} start={index} now={now} {} {:?}",
                            case.action, case.args
                        ),
                    );
                    comparisons += 1;
                }
                // Shared-start public composition is distinct from applying
                // state and rebuilding UiStart in the eventual running app.
                if now == 0.0 || !["base", "busy"].contains(&sa.group) {
                    continue;
                }
                let (mut prefix_a, mut prefix_b) = (input_a.clone(), input_b.clone());
                for case in [
                    call("show", texts(&["command", ""])),
                    call("run", texts(&["browse", "ancestor", "query"])),
                    call("edit", texts(&[""])),
                    call("key", texts(&["Back"])),
                    call("close", vec![]),
                ] {
                    (prefix_a, prefix_b) = compare_step(
                        &a,
                        &b,
                        (&sa.value, &sb.value),
                        (&prefix_a, &prefix_b),
                        &case,
                        now,
                        "shared palette prefix",
                    );
                    comparisons += 1;
                }
                // Every command starts from the same evaluated prefix. Repeating
                // the identical pure prefix for each command adds no coverage.
                for case in command_cases() {
                    compare_step(
                        &a,
                        &b,
                        (&sa.value, &sb.value),
                        (&prefix_a, &prefix_b),
                        &case,
                        now,
                        "command after palette lifecycle",
                    );
                    comparisons += 1;
                }
                for sequence in sequences() {
                    let (mut da, mut db) = (input_a.clone(), input_b.clone());
                    for case in sequence {
                        (da, db) = compare_step(
                            &a,
                            &b,
                            (&sa.value, &sb.value),
                            (&da, &db),
                            &case,
                            now,
                            "public composition, shared start",
                        );
                        comparisons += 1;
                    }
                }
            }
        }
    }
    // Check result times and effects; Env exposes no clock-read-count trace.
    let actual_metrics = |compiled: &Compiled| {
        let mut metrics = compiled.metrics.clone();
        metrics["vmEvaluationSeconds"] = json!(compiled.evaluation_seconds.get());
        metrics["vmEvaluations"] = json!(compiled.evaluations.get());
        metrics
    };
    eprintln!(
        "{}",
        json!({"baseline": actual_metrics(&a), "candidate": actual_metrics(&b),
        "comparisons": comparisons, "evaluationSeconds": started.elapsed().as_secs_f64(),
        "clockReadCountObserved": false, "asyncCompletionCovered": false,
        "geometryInputCasesOnly": true})
    );
    baseline.recheck();
    candidate.recheck();
    assert!(
        b.plan.code.len() < a.plan.code.len(),
        "code pool must shrink"
    );
    assert!(
        b.metrics["rangeUnionBytes"].as_u64().unwrap()
            < a.metrics["rangeUnionBytes"].as_u64().unwrap(),
        "unique action byte union must shrink"
    );
}
