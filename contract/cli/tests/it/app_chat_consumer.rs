//! App-private boundary verification over two admitted, independently closed source contexts.
//! The normal baked macOS tests remain the whole-app regression coverage.
use exact_kernel::{Kernel, NodeType, Op, PropId};
use exact_plan::{Plan, TypeKind, TypesId, Value};
use exact_runner::geometry::{GeometryAnswer, GeometryEnv, GeometryLinks, Scrolled};
use exact_runner::held::Held;
use exact_runner::vm::{self, Env};
use serde_json::{Value as Json, json};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::Instant;

const ORIGINAL_INVENTORY: &str = "fe961672f1e81a25120fef6c377c86a1d9537f8a9ef796e453d1477a2057a1a1";
const SELECTED_INVENTORY: &str = "f276b5be801f39b9d8c5314e0295da9adf3aff3b4717b5136f1cbbbe77549e99";
const APP_MANIFEST: &str = "8df0b970114e28d0570d13ea997849ea2a8e4fc6e5369cbd5c04ebbe81d176c7";
const ORIGINAL_REV: &str = "76641c9c20e17d7095069463cce02812db8bd1c1";
const SEEDED: [&str; 17] = [
    "composerSelection",
    "confirmationState",
    "fieldProtocol",
    "hoverState",
    "intentState",
    "modifierOwnership",
    "noticeExitState",
    "paletteSession",
    "previewState",
    "providerWizardState",
    "sshResponse",
    "streamGeometry",
    "transcriptViewport",
    "keybindingEditor",
    "composerMenu",
    "composerDraft",
    "settingsPageState",
];
const CHANNELS: [(&str, &str, &str); 6] = [
    (
        "sendFileCommentChanged",
        "fileComment",
        "fileCommentChanged",
    ),
    ("sendFragmentChanged", "fragment", "fragmentChanged"),
    (
        "sendInlineDocumentChanged",
        "inlineDocument",
        "inlineDocumentChanged",
    ),
    ("sendLineDragChanged", "lineDrag", "lineDragChanged"),
    ("sendPrQuickChanged", "prQuick", "prQuickChanged"),
    ("sendLocalChanged", "local", "localChanged"),
];
const CASES: &str = r##"[
{"name":"inline","op":"inline-doc-ack","id":"item","value":"value","send":"inlineDocumentChanged","fold":false,"panel":false,"atEnd":false,"ownerMatches":true,"tabMode":"absent","floating":false},
{"name":"quick","op":"surface-pr-quick","id":"item","value":"value","send":"prQuickChanged","fold":false,"panel":false,"atEnd":false,"ownerMatches":true,"tabMode":"absent","floating":false},
{"name":"comment","op":"surface-files-comment-add","id":"item","value":"value","send":"fileCommentChanged","fold":false,"panel":false,"atEnd":false,"ownerMatches":true,"tabMode":"absent","floating":false},
{"name":"drag","op":"pr-code-drag","id":"item","value":"value","send":"lineDragChanged","fold":false,"panel":false,"atEnd":false,"ownerMatches":true,"tabMode":"absent","floating":false},
{"name":"begin","op":"pr-code-begin","id":"item","value":"value","send":"lineDragChanged","fold":false,"panel":false,"atEnd":false,"ownerMatches":true,"tabMode":"absent","floating":false},
{"name":"fragment","op":"link-open","id":"item","value":"#heading","send":"fragmentChanged","fold":false,"panel":false,"atEnd":false,"ownerMatches":true,"tabMode":"absent","floating":false},
{"name":"external-link","op":"link-open","id":"item","value":"https://fixture.invalid","send":"localChanged","fold":false,"panel":false,"atEnd":false,"ownerMatches":true,"tabMode":"absent","floating":false},
{"name":"ordinary","op":"ordinary","id":"item","value":"value","send":"localChanged","fold":false,"panel":false,"atEnd":false,"ownerMatches":true,"tabMode":"absent","floating":false},
{"name":"fold-end0-owner0","op":"fold","id":"item","value":"value","send":"localChanged","fold":true,"panel":false,"atEnd":false,"ownerMatches":false,"tabMode":"absent","floating":false},
{"name":"fold-end0-owner1","op":"fold","id":"item","value":"value","send":"localChanged","fold":true,"panel":false,"atEnd":false,"ownerMatches":true,"tabMode":"absent","floating":false},
{"name":"fold-end1-owner0","op":"fold","id":"item","value":"value","send":"localChanged","fold":true,"panel":false,"atEnd":true,"ownerMatches":false,"tabMode":"absent","floating":false},
{"name":"fold-end1-owner1","op":"fold","id":"item","value":"value","send":"localChanged","fold":false,"panel":false,"atEnd":true,"ownerMatches":true,"tabMode":"absent","floating":false},
{"name":"group-end0-owner0","op":"group","id":"item","value":"value","send":"localChanged","fold":true,"panel":false,"atEnd":false,"ownerMatches":false,"tabMode":"absent","floating":false},
{"name":"group-end0-owner1","op":"group","id":"item","value":"value","send":"localChanged","fold":true,"panel":false,"atEnd":false,"ownerMatches":true,"tabMode":"absent","floating":false},
{"name":"group-end1-owner0","op":"group","id":"item","value":"value","send":"localChanged","fold":true,"panel":false,"atEnd":true,"ownerMatches":false,"tabMode":"absent","floating":false},
{"name":"group-end1-owner1","op":"group","id":"item","value":"value","send":"localChanged","fold":false,"panel":false,"atEnd":true,"ownerMatches":true,"tabMode":"absent","floating":false},
{"name":"attempt-end0-owner0","op":"attempt","id":"item","value":"value","send":"localChanged","fold":true,"panel":false,"atEnd":false,"ownerMatches":false,"tabMode":"absent","floating":false},
{"name":"attempt-end0-owner1","op":"attempt","id":"item","value":"value","send":"localChanged","fold":true,"panel":false,"atEnd":false,"ownerMatches":true,"tabMode":"absent","floating":false},
{"name":"attempt-end1-owner0","op":"attempt","id":"item","value":"value","send":"localChanged","fold":true,"panel":false,"atEnd":true,"ownerMatches":false,"tabMode":"absent","floating":false},
{"name":"attempt-end1-owner1","op":"attempt","id":"item","value":"value","send":"localChanged","fold":false,"panel":false,"atEnd":true,"ownerMatches":true,"tabMode":"absent","floating":false},
{"name":"tree-all-end0-owner0","op":"tree-all","id":"item","value":"value","send":"localChanged","fold":true,"panel":false,"atEnd":false,"ownerMatches":false,"tabMode":"absent","floating":false},
{"name":"tree-all-end0-owner1","op":"tree-all","id":"item","value":"value","send":"localChanged","fold":true,"panel":false,"atEnd":false,"ownerMatches":true,"tabMode":"absent","floating":false},
{"name":"tree-all-end1-owner0","op":"tree-all","id":"item","value":"value","send":"localChanged","fold":true,"panel":false,"atEnd":true,"ownerMatches":false,"tabMode":"absent","floating":false},
{"name":"tree-all-end1-owner1","op":"tree-all","id":"item","value":"value","send":"localChanged","fold":false,"panel":false,"atEnd":true,"ownerMatches":true,"tabMode":"absent","floating":false},
{"name":"tree-dir-end0-owner0","op":"tree-dir","id":"item","value":"value","send":"localChanged","fold":true,"panel":false,"atEnd":false,"ownerMatches":false,"tabMode":"absent","floating":false},
{"name":"tree-dir-end0-owner1","op":"tree-dir","id":"item","value":"value","send":"localChanged","fold":true,"panel":false,"atEnd":false,"ownerMatches":true,"tabMode":"absent","floating":false},
{"name":"tree-dir-end1-owner0","op":"tree-dir","id":"item","value":"value","send":"localChanged","fold":true,"panel":false,"atEnd":true,"ownerMatches":false,"tabMode":"absent","floating":false},
{"name":"tree-dir-end1-owner1","op":"tree-dir","id":"item","value":"value","send":"localChanged","fold":false,"panel":false,"atEnd":true,"ownerMatches":true,"tabMode":"absent","floating":false},
{"name":"surface-close-others","op":"surface-close-others","id":"item","value":"value","send":"localChanged","fold":false,"panel":true,"atEnd":false,"ownerMatches":true,"tabMode":"absent","floating":false},
{"name":"surface-close-to-right","op":"surface-close-to-right","id":"item","value":"value","send":"localChanged","fold":false,"panel":true,"atEnd":false,"ownerMatches":true,"tabMode":"absent","floating":false},
{"name":"surface-close-all","op":"surface-close-all","id":"item","value":"value","send":"localChanged","fold":false,"panel":true,"atEnd":false,"ownerMatches":true,"tabMode":"absent","floating":false},
{"name":"close-absent","op":"surface-close","id":"item","value":"value","send":"localChanged","fold":false,"panel":true,"atEnd":false,"ownerMatches":true,"tabMode":"absent","floating":false},
{"name":"close-plain","op":"surface-close","id":"item","value":"value","send":"localChanged","fold":false,"panel":true,"atEnd":false,"ownerMatches":true,"tabMode":"plain","floating":false},
{"name":"close-confirm","op":"surface-close","id":"item","value":"value","send":"localChanged","fold":false,"panel":false,"atEnd":false,"ownerMatches":true,"tabMode":"confirm","floating":false},
{"name":"browser-float-False","op":"surface-browser-float","id":"item","value":"value","send":"localChanged","fold":false,"panel":true,"atEnd":false,"ownerMatches":true,"tabMode":"absent","floating":false},
{"name":"browser-float-True","op":"surface-browser-float","id":"item","value":"value","send":"localChanged","fold":false,"panel":false,"atEnd":false,"ownerMatches":true,"tabMode":"absent","floating":true},
{"name":"session-float","op":"surface-session-float","id":"item","value":"value","send":"localChanged","fold":false,"panel":true,"atEnd":false,"ownerMatches":true,"tabMode":"absent","floating":false},
{"name":"search-sheet-Escape","op":"surface-files-search-key","id":"sheet","value":"Escape","send":"localChanged","fold":false,"panel":true,"atEnd":false,"ownerMatches":true,"tabMode":"absent","floating":false},
{"name":"search-sheet-Enter","op":"surface-files-search-key","id":"sheet","value":"Enter","send":"localChanged","fold":false,"panel":false,"atEnd":false,"ownerMatches":true,"tabMode":"absent","floating":false},
{"name":"search-item-Escape","op":"surface-files-search-key","id":"item","value":"Escape","send":"localChanged","fold":false,"panel":false,"atEnd":false,"ownerMatches":true,"tabMode":"absent","floating":false},
{"name":"search-item-Enter","op":"surface-files-search-key","id":"item","value":"Enter","send":"localChanged","fold":false,"panel":false,"atEnd":false,"ownerMatches":true,"tabMode":"absent","floating":false},
{"name":"media-menu","op":"media-menu","id":"item","value":"value","send":"localChanged","fold":false,"panel":false,"atEnd":false,"ownerMatches":true,"tabMode":"absent","floating":false},
{"name":"table-menu","op":"table-menu","id":"item","value":"value","send":"localChanged","fold":false,"panel":false,"atEnd":false,"ownerMatches":true,"tabMode":"absent","floating":false},
{"name":"unrecognized","op":"unrecognized","id":"item","value":"value","send":"localChanged","fold":false,"panel":false,"atEnd":false,"ownerMatches":true,"tabMode":"absent","floating":false}
]"##;

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

struct Context {
    root: PathBuf,
    inventory: Json,
}
impl Context {
    fn load(variable: &str, role: &str, expected: &str) -> Self {
        let supplied = std::env::var_os(variable)
            .unwrap_or_else(|| panic!("{variable} must name its admitted source context"));
        let supplied = PathBuf::from(supplied);
        assert!(
            !fs::symlink_metadata(&supplied)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        let root = supplied.canonicalize().unwrap();
        let inventory: Json = serde_json::from_slice(
            &fs::read(root.join("source-context.json")).expect("admitted context inventory"),
        )
        .unwrap();
        assert_eq!(inventory["role"], role);
        assert_eq!(inventory["inventorySHA256"], expected);
        if role == "original" {
            assert_eq!(inventory["sourceAuthority"], ORIGINAL_REV);
        }
        let files = inventory["files"]
            .as_array()
            .expect("complete input inventory");
        let mut normalized = Vec::new();
        let mut unique = BTreeSet::new();
        for file in files {
            let relative = file["file"].as_str().unwrap();
            assert!(unique.insert(relative));
            let path = Path::new(relative);
            assert!(!path.is_absolute());
            assert!(path.components().all(|p| matches!(p, Component::Normal(_))));
            let full = root.join(path);
            let mut parent = full.parent().unwrap();
            while parent != root {
                let metadata = fs::symlink_metadata(parent).unwrap();
                assert!(metadata.is_dir() && !metadata.file_type().is_symlink());
                parent = parent.parent().unwrap();
            }
            let metadata = fs::symlink_metadata(&full).unwrap();
            assert!(metadata.is_file() && !metadata.file_type().is_symlink());
            assert_eq!(metadata.len(), file["bytes"].as_u64().unwrap());
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                assert_eq!(metadata.dev(), file["dev"].as_u64().unwrap());
                assert_eq!(metadata.ino(), file["ino"].as_u64().unwrap());
            }
            let bytes = fs::read(&full).unwrap();
            assert_eq!(hash(&bytes), file["sha256"].as_str().unwrap(), "{relative}");
            normalized.push(json!({
                "file": relative, "bytes": bytes.len(), "sha256": hash(&bytes)
            }));
            if relative.ends_with(".contract") {
                for line in std::str::from_utf8(&bytes).unwrap().lines() {
                    if line.trim_start().starts_with("use ") {
                        let (_, tail) = line.split_once(" from \"").expect("use source path");
                        let target = tail.split('"').next().unwrap();
                        assert!(target.starts_with('.'), "package import in context");
                        let resolved = full.parent().unwrap().join(target).canonicalize().unwrap();
                        assert!(
                            resolved.starts_with(&root),
                            "context escape: {relative} -> {target}"
                        );
                    }
                }
            }
        }
        assert_eq!(hash(&serde_json::to_vec(&normalized).unwrap()), expected);
        assert_eq!(
            hash(&fs::read(root.join("app.json")).unwrap()),
            APP_MANIFEST
        );
        assert_eq!(inventory["generatedFiles"], 170);
        Self { root, inventory }
    }

    fn check_production_blocks(&self) {
        let source = fs::read_to_string(self.root.join("app.contract")).unwrap();
        let window = fs::read_to_string(self.root.join("src/shell/app-window.contract")).unwrap();
        let entry = fs::read_to_string(self.root.join("production-boundary.contract")).unwrap();
        for name in ["applyChatSends", "applyLocalChange", "applyChat"] {
            let body = action(&source, name);
            assert!(
                entry.contains(body.trim_end()),
                "actual production consumer {name}"
            );
            assert_eq!(
                hash(format!("{}\n", body.trim_end()).as_bytes()),
                self.inventory["entrySourceMapping"]["actions"][name]
                    .as_str()
                    .unwrap()
            );
        }
        let wrapper = action(&window, "actChatLocal");
        assert!(
            entry.contains(wrapper.trim_end()),
            "actual production window wrapper"
        );
        assert_eq!(
            hash(format!("{}\n", wrapper.trim_end()).as_bytes()),
            self.inventory["entrySourceMapping"]["windowWrapperSHA256"]
                .as_str()
                .unwrap()
        );
        for declaration in self.inventory["entrySourceMapping"]["mutationDeclarations"]
            .as_array()
            .unwrap()
        {
            let declaration = declaration.as_str().unwrap();
            assert!(source.lines().any(|line| line == declaration));
            assert!(entry.lines().any(|line| line == declaration));
        }
        assert!(entry.contains("  action applyChatRoot("));
        assert!(entry.contains("\n    applyChat(d)\n"));
    }

    fn compile(&self) -> Plan {
        self.check_production_blocks();
        let path = self.root.join("production-boundary.contract");
        let source = fs::read_to_string(&path).unwrap();
        let start = Instant::now();
        let (plan, _) = contract::compile_path_source_all(&path, &source, false)
            .unwrap_or_else(|errors| panic!("{}: {errors:#?}", path.display()));
        let compile_seconds = start.elapsed().as_secs_f64();
        let roundtrip = Instant::now();
        plan.validate().expect("public plan validation");
        let bytes = plan.encode();
        let decoded = Plan::decode(&bytes).expect("public plan decode");
        println!(
            "CONTEXT {} compile_seconds={compile_seconds} roundtrip_seconds={} code={} data={} encoded={}",
            self.root.display(),
            roundtrip.elapsed().as_secs_f64(),
            decoded.code.len(),
            decoded.data.len(),
            bytes.len()
        );
        let ranges: BTreeSet<_> = decoded
            .actions
            .iter()
            .map(|a| (a.body.offset, a.body.len))
            .collect();
        let sum: u64 = ranges.iter().map(|(_, len)| u64::from(*len)).sum();
        let mut end = 0_u64;
        let mut union = 0_u64;
        for (offset, len) in &ranges {
            let low = u64::from(*offset);
            let high = low + u64::from(*len);
            union += high.saturating_sub(end.max(low));
            end = end.max(high);
        }
        println!(
            "RANGES rows={} unique={} sum={sum} union={union}",
            decoded.actions.len(),
            ranges.len()
        );
        for body in &decoded.actions {
            let mut counts = BTreeMap::<String, usize>::new();
            for instruction in vm::instructions(decoded.code(body.body)) {
                *counts
                    .entry(format!(
                        "{:?}",
                        instruction.expect("instruction decoder").op
                    ))
                    .or_default() += 1;
            }
            println!(
                "ACTION {} bytes={} ops={counts:?}",
                decoded.str(body.name),
                body.body.len
            );
        }
        decoded
    }
}

fn action<'a>(source: &'a str, name: &str) -> &'a str {
    let prefix = format!("  action {name}");
    let start = source
        .lines()
        .scan(0, |offset, line| {
            let at = *offset;
            *offset += line.len() + 1;
            Some((at, line))
        })
        .find(|(_, line)| {
            line.strip_prefix(&prefix)
                .is_some_and(|tail| tail.is_empty() || tail.starts_with('('))
        })
        .unwrap()
        .0;
    let rest = &source[start..];
    let end = rest.find('\n').map_or(rest.len(), |newline| {
        let mut offset = newline + 1;
        for line in rest[offset..].lines() {
            if line.starts_with("  ")
                && !line.starts_with("    ")
                && [
                    "action ",
                    "state ",
                    "derive ",
                    "task ",
                    "resource ",
                    "mutation ",
                    "view",
                    "provide",
                ]
                .iter()
                .any(|kind| line[2..].starts_with(kind))
            {
                return offset;
            }
            offset += line.len() + 1;
        }
        rest.len()
    });
    &rest[..end]
}

fn seed(plan: &Plan, ty: TypesId, rich: bool, path: &str, depth: usize) -> Value {
    let row = plan.type_(ty);
    match row.kind {
        TypeKind::Number => Value::Number(if rich { 37.0 } else { 0.0 }),
        TypeKind::Bool => Value::Bool(rich),
        TypeKind::String => Value::str(&if rich {
            format!("seed:{path}")
        } else {
            String::new()
        }),
        TypeKind::Unit => Value::Unit,
        TypeKind::Option => {
            if rich && depth < 2 {
                Value::some(seed(plan, row.elem.unwrap(), rich, path, depth + 1))
            } else {
                Value::NONE
            }
        }
        TypeKind::List => Value::list(if rich && depth < 2 {
            vec![seed(plan, row.elem.unwrap(), rich, path, depth + 1)]
        } else {
            Vec::new()
        }),
        TypeKind::Record => Value::record(
            row.fields
                .iter()
                .map(|id| {
                    let field = plan.field(id);
                    seed(
                        plan,
                        field.ty,
                        rich,
                        &format!("{path}.{}", plan.str(field.name)),
                        depth,
                    )
                })
                .collect(),
        ),
    }
}
fn field_type(plan: &Plan, mut ty: TypesId, path: &[&str]) -> TypesId {
    for name in path {
        ty = plan
            .field(
                plan.type_(ty)
                    .fields
                    .iter()
                    .find(|id| plan.str(plan.field(*id).name) == *name)
                    .expect(name),
            )
            .ty;
    }
    ty
}
fn set(plan: &Plan, ty: TypesId, value: &mut Value, path: &[&str], replacement: Value) {
    let (index, field) = plan
        .type_(ty)
        .fields
        .iter()
        .enumerate()
        .find(|(_, id)| plan.str(plan.field(*id).name) == path[0])
        .expect(path[0]);
    let Value::Record(fields) = value else {
        panic!("record at {}", path[0])
    };
    let mut fields: Vec<_> = fields.iter().cloned().collect();
    if path.len() == 1 {
        fields[index] = replacement;
    } else {
        set(
            plan,
            plan.field(field).ty,
            &mut fields[index],
            &path[1..],
            replacement,
        );
    }
    *value = Value::record(fields);
}
fn normalized(plan: &Plan, ty: TypesId, value: &Value) -> Json {
    let row = plan.type_(ty);
    match row.kind {
        TypeKind::Number => match value {
            Value::Number(v) => json!(v),
            _ => panic!("number"),
        },
        TypeKind::Bool => match value {
            Value::Bool(v) => json!(v),
            _ => panic!("bool"),
        },
        TypeKind::String => json!(value.as_str().expect("string")),
        TypeKind::Unit => json!({"unit": true}),
        TypeKind::Option => match value {
            Value::Option(None) => json!({"none": true}),
            Value::Option(Some(v)) => json!({"some": normalized(plan, row.elem.unwrap(), v)}),
            _ => panic!("option"),
        },
        TypeKind::List => match value {
            Value::List(v) => json!(
                v.iter()
                    .map(|v| normalized(plan, row.elem.unwrap(), v))
                    .collect::<Vec<_>>()
            ),
            _ => panic!("list"),
        },
        TypeKind::Record => match value {
            Value::Record(v) => {
                assert_eq!(v.len(), row.fields.len as usize);
                Json::Object(
                    row.fields
                        .iter()
                        .enumerate()
                        .map(|(index, id)| {
                            let field = plan.field(id);
                            (
                                plan.str(field.name).to_owned(),
                                normalized(plan, field.ty, &v[index]),
                            )
                        })
                        .collect(),
                )
            }
            _ => panic!("record"),
        },
    }
}
fn untyped(value: &Value) -> Json {
    match value {
        Value::Number(v) => json!(v),
        Value::Bool(v) => json!(v),
        Value::Unit => json!({"unit": true}),
        Value::Option(None) => json!({"none": true}),
        Value::Option(Some(v)) => json!({"some": untyped(v)}),
        Value::List(v) => json!({"list": v.iter().map(untyped).collect::<Vec<_>>()}),
        Value::Record(v) => json!({"record": v.iter().map(untyped).collect::<Vec<_>>()}),
        v => json!(v.as_str().expect("text")),
    }
}

thread_local! { static GEOMETRY: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) }; }
fn frame(_: &Kernel, _: &Scrolled, view: u32) -> GeometryAnswer {
    GEOMETRY.with(|reads| reads.borrow_mut().push(view));
    GeometryAnswer {
        x: 7.0,
        y: 11.0,
        width: 13.0,
        height: 17.0,
        ..Default::default()
    }
}
fn measure(_: &mut Kernel, _: &Scrolled, _: u32) -> GeometryAnswer {
    panic!("unexpected measurement")
}
fn start_value(plan: &Plan, ty: TypesId, case: &Json, rich: bool) -> Value {
    let mut start = seed(plan, ty, rich, "start", 0);
    for (path, value) in [
        (
            vec!["transcriptViewport", "atEnd"],
            Value::Bool(case["atEnd"].as_bool().unwrap()),
        ),
        (
            vec!["transcriptViewport", "owner"],
            Value::str(if case["ownerMatches"].as_bool().unwrap() {
                "owner"
            } else {
                "other"
            }),
        ),
        (vec!["composerOwner"], Value::str("owner")),
        (
            vec!["shell", "panel", "browser", "capture", "floating"],
            Value::Bool(case["floating"].as_bool().unwrap()),
        ),
        (vec!["viewport", "width"], Value::Number(800.0)),
        (vec!["viewport", "height"], Value::Number(600.0)),
    ] {
        set(plan, ty, &mut start, &path, value);
    }
    let tabs_ty = field_type(plan, ty, &["shell", "panel", "tabs"]);
    let tab_ty = plan.type_(tabs_ty).elem.unwrap();
    let tabs = if case["tabMode"] == "absent" {
        Vec::new()
    } else {
        let mut tab = seed(plan, tab_ty, rich, "tab", 0);
        set(
            plan,
            tab_ty,
            &mut tab,
            &["id"],
            Value::str(case["id"].as_str().unwrap()),
        );
        set(
            plan,
            tab_ty,
            &mut tab,
            &["closeTitle"],
            Value::str(if case["tabMode"] == "confirm" {
                "Confirm"
            } else {
                ""
            }),
        );
        vec![tab]
    };
    set(
        plan,
        ty,
        &mut start,
        &["shell", "panel", "tabs"],
        Value::list(tabs),
    );
    start
}
fn incoming(plan: &Plan, ty: TypesId, profile: u8) -> Value {
    assert_eq!(
        plan.type_(ty).fields.len,
        24,
        "UiDecision root plus 23 private fields"
    );
    assert_eq!(plan.type_(field_type(plan, ty, &["root"])).fields.len, 295);
    let mut value = seed(plan, ty, profile != 0, "before", 0);
    if profile == 2 {
        for (flag, _, _) in CHANNELS {
            set(
                plan,
                ty,
                &mut value,
                &["root", flag],
                Value::Bool(flag == "sendFragmentChanged" || flag == "sendLocalChanged"),
            );
        }
        set(
            plan,
            ty,
            &mut value,
            &["root", "writeFoldHold"],
            Value::Bool(false),
        );
        set(
            plan,
            ty,
            &mut value,
            &["root", "writeRightPanelAt"],
            Value::Bool(false),
        );
    }
    value
}
fn shared_input(plan: &Plan, ty: TypesId, mask: u8) -> Value {
    let mut value = seed(plan, ty, true, "shared", 0);
    for (index, (flag, field, _)) in CHANNELS.iter().enumerate() {
        set(
            plan,
            ty,
            &mut value,
            &[*flag],
            Value::Bool(mask & (1 << index) != 0),
        );
        for (part, v) in [
            ("p0", Value::str(&format!("channel:{field}"))),
            ("p1", Value::str(&format!("id:{field}"))),
            ("p2", Value::str(&format!("value:{field}"))),
            ("p3", Value::Number(index as f64 + 0.25)),
        ] {
            set(plan, ty, &mut value, &[*field, part], v);
        }
    }
    value
}

struct Evaluation {
    observed: Json,
    start: Json,
    before: Option<Json>,
    argument: Option<Json>,
}
fn evaluate(plan: &Plan, name: &str, case: &Json, rich: bool, profile: u8, mask: u8) -> Evaluation {
    let action = plan
        .actions
        .iter()
        .find(|a| plan.str(a.name) == name)
        .expect(name);
    let ui_ty = plan
        .resources
        .iter()
        .find(|r| plan.str(r.name) == "uiStart")
        .unwrap()
        .ty;
    let start = start_value(plan, ui_ty, case, rich);
    let mut before = None;
    let mut argument = None;
    let params: Vec<_> = action
        .params
        .iter()
        .map(|id| {
            let param = plan.param(id);
            match plan.str(param.name) {
                "start" => start.clone(),
                "before" => {
                    let v = incoming(plan, param.ty, profile);
                    before = Some(normalized(plan, param.ty, &v));
                    v
                }
                "d" => {
                    let v = shared_input(plan, param.ty, mask);
                    argument = Some(normalized(plan, param.ty, &v));
                    v
                }
                key @ ("op" | "id" | "value") => Value::str(case[key].as_str().unwrap()),
                key => panic!("unexpected parameter {key}"),
            }
        })
        .collect();
    let mut slots: Vec<_> = plan
        .slots
        .iter()
        .map(|slot| seed(plan, slot.ty, false, "slot", 0))
        .collect();
    for (name, value) in [
        ("rightPanelAt", Value::str("panel-before")),
        ("untouched", Value::str("keep")),
    ] {
        let id = plan
            .slots
            .iter()
            .position(|s| plan.str(s.name) == name)
            .unwrap();
        slots[id] = value;
    }
    let resources: Vec<_> = plan
        .resources
        .iter()
        .map(|resource| {
            Some(Held::new(if plan.str(resource.name) == "uiStart" {
                start.clone()
            } else {
                seed(plan, resource.ty, false, "resource", 0)
            }))
        })
        .collect();
    let mut kernel = Kernel::with_monospace();
    kernel
        .apply(
            0,
            1,
            &[
                Op::CreateView {
                    id: 1,
                    node_type: NodeType::View,
                },
                Op::SetProp {
                    id: 1,
                    prop: PropId::Id,
                    value: format!("table-copy-{}", case["id"].as_str().unwrap()).into(),
                },
            ],
        )
        .unwrap();
    let scrolled = Scrolled::default();
    let links = GeometryLinks::new(
        frame,
        measure,
        |_, _, _, _| panic!("unexpected hit test"),
        |_, _, _, _| panic!("unexpected hit tests"),
        |_, _, _, _| true,
    );
    let geometry = GeometryEnv::new(&kernel, &scrolled, &links, &[]);
    let strings = vm::Strings::new(plan);
    let derives = vec![None; plan.derives.len()];
    let pending_resources = vec![false; plan.resources.len()];
    let failed_resources = vec![None; plan.resources.len()];
    let failed_why = vec![None; plan.resources.len()];
    let pending_mutations = vec![false; plan.mutations.len()];
    let failed_mutations = vec![None; plan.mutations.len()];
    let env = Env {
        plan,
        strings: &strings,
        router: None,
        lists: None,
        format: None,
        geometry: Some(&geometry),
        slots: &slots,
        derives: &derives,
        resources: &resources,
        params: &params,
        frames: &[],
        now_ms: 123.0,
        pending_resources: &pending_resources,
        failed_resources: &failed_resources,
        failed_why: &failed_why,
        pending_mutations: &pending_mutations,
        failed_mutations: &failed_mutations,
        store_dependent_derives: &[],
        store_dependent_resources: &[],
    };
    let allowed: Vec<_> = action
        .writes
        .iter()
        .map(|id| plan.write(id).slot.0)
        .collect();
    GEOMETRY.with(|reads| reads.borrow_mut().clear());
    let out = vm::eval(plan.code(action.body), &env, &allowed)
        .unwrap_or_else(|error| panic!("{name} {}: {error:?}", case["name"]));
    assert!(out.row_writes.is_empty() && out.refreshes.is_empty() && !out.store_dependent);
    assert_eq!(out.value, Value::Unit);
    let writes: Vec<_> = out
        .writes
        .iter()
        .map(|(id, value)| {
            let slot = &plan.slots[*id as usize];
            slots[*id as usize] = value.clone();
            json!([plan.str(slot.name), normalized(plan, slot.ty, value)])
        })
        .collect();
    let state: Json = Json::Object(
        plan.slots
            .iter()
            .enumerate()
            .map(|(index, slot)| {
                (
                    plan.str(slot.name).to_owned(),
                    normalized(plan, slot.ty, &slots[index]),
                )
            })
            .collect(),
    );
    assert_eq!(state["untouched"], "keep");
    assert_eq!(
        state["replyObserved"], false,
        "no asynchronous continuation ran"
    );
    let sends: Vec<_> = out
        .sends
        .iter()
        .map(|(slot, source, args)| {
            json!([
                plan.str(plan.mutations[*slot as usize].name),
                source,
                args.iter().map(untyped).collect::<Vec<_>>()
            ])
        })
        .collect();
    let commands: Vec<_> = out
        .commands
        .iter()
        .map(|(name, args)| json!([name, args.iter().map(untyped).collect::<Vec<_>>()]))
        .collect();
    let reads = GEOMETRY.with(|reads| reads.borrow().clone());
    Evaluation {
        observed: json!({"writes": writes, "state": state, "sends": sends,
            "commands": commands, "geometry": reads, "rowWrites": [], "refreshes": []}),
        start: normalized(plan, ui_ty, &start),
        before,
        argument,
    }
}

fn check_return(evaluation: &Evaluation, composed: bool) {
    let value = &evaluation.observed["state"]["output"]["some"];
    let object = value.as_object().expect("entire UiDecision return");
    assert_eq!(object.len(), 24);
    let root = value["root"]
        .as_object()
        .expect("entire AppDecision return");
    assert_eq!(root.len(), 295);
    if composed {
        let before = evaluation.before.as_ref().unwrap();
        for (key, v) in before.as_object().unwrap() {
            if key != "root" {
                assert_eq!(&value[key], v, "private field {key}");
            }
        }
        let changed: BTreeSet<_> = CHANNELS
            .iter()
            .flat_map(|(flag, arg, _)| [*flag, *arg])
            .chain([
                "writeFoldHold",
                "foldHold",
                "writeRightPanelAt",
                "rightPanelAt",
            ])
            .collect();
        for (key, v) in before["root"].as_object().unwrap() {
            if !changed.contains(key.as_str()) {
                assert_eq!(&value["root"][key], v, "{key}");
            }
        }
    } else {
        for key in SEEDED {
            assert_eq!(
                value["root"][key], evaluation.start[key],
                "seeded state {key}"
            );
        }
    }
    assert!(
        evaluation.observed["commands"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(evaluation.observed["sends"].as_array().unwrap().is_empty());
    assert!(
        evaluation.observed["geometry"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let writes = evaluation.observed["writes"].as_array().unwrap();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0][0], "output");
}
fn check_consumer(evaluation: &Evaluation, case: &Json) {
    assert_eq!(
        evaluation.observed["geometry"],
        if case["op"] == "table-menu" {
            json!([1, 1, 1, 1])
        } else {
            json!([])
        }
    );
    assert_eq!(evaluation.observed["commands"], json!([]));
    let sent_value = if case["op"] == "table-menu" {
        format!("7|11|13|17|800|600|{}", case["value"].as_str().unwrap())
    } else {
        case["value"].as_str().unwrap().to_owned()
    };
    assert_eq!(
        evaluation.observed["sends"],
        json!([[
            case["send"],
            "command",
            [
                format!("chatlocal:{}", case["op"].as_str().unwrap()),
                case["id"],
                sent_value,
                0.0
            ]
        ]])
    );
    let mut writes = Vec::new();
    if case["fold"].as_bool().unwrap() {
        writes.push(json!(["foldHold", true]));
    }
    if case["panel"].as_bool().unwrap() {
        writes.push(json!(["rightPanelAt", ""]));
    }
    assert_eq!(evaluation.observed["writes"], json!(writes));
    assert_eq!(evaluation.observed["state"]["foldHold"], case["fold"]);
    assert_eq!(
        evaluation.observed["state"]["rightPanelAt"],
        if case["panel"].as_bool().unwrap() {
            json!("")
        } else {
            json!("panel-before")
        }
    );
    assert_eq!(
        evaluation.observed["state"]["output"],
        json!({"none": true})
    );
    assert_eq!(
        evaluation.observed["state"]["projection"],
        json!({"none": true})
    );
}
fn check_shared(evaluation: &Evaluation, mask: u8, projection: bool) {
    let input = evaluation.argument.as_ref().unwrap();
    assert_eq!(evaluation.observed["geometry"], json!([]));
    assert_eq!(evaluation.observed["commands"], json!([]));
    if projection {
        let actual = &evaluation.observed["state"]["projection"]["some"];
        assert_eq!(actual.as_object().unwrap().len(), 10);
        for (flag, field, _) in CHANNELS.iter().take(5) {
            assert_eq!(actual[*flag], input[*flag], "independent flag {flag}");
            assert_eq!(
                actual[*field], input[*field],
                "inactive/active arguments {field}"
            );
        }
        assert_eq!(evaluation.observed["sends"], json!([]));
        assert_eq!(evaluation.observed["writes"].as_array().unwrap().len(), 1);
        assert_eq!(evaluation.observed["writes"][0][0], "projection");
    } else {
        let expected: Vec<_> = CHANNELS
            .iter()
            .enumerate()
            .filter(|(index, _)| mask & (1 << index) != 0)
            .map(|(_, (_, field, mutation))| {
                json!([
                    mutation,
                    "command",
                    [
                        input[*field]["p0"],
                        input[*field]["p1"],
                        input[*field]["p2"],
                        input[*field]["p3"]
                    ]
                ])
            })
            .collect();
        assert_eq!(
            evaluation.observed["sends"],
            json!(expected),
            "six-channel order mask={mask}"
        );
        assert_eq!(evaluation.observed["writes"], json!([]));
    }
}

#[test]
#[ignore = "requires admitted original/selected T3 source contexts"]
fn app_chat_consumer_boundary() {
    let original = Context::load("EXACT_T3_CHAT_ORIGINAL_APP", "original", ORIGINAL_INVENTORY);
    let selected = Context::load("EXACT_T3_CHAT_SELECTED_APP", "selected", SELECTED_INVENTORY);
    assert_ne!(
        original.root, selected.root,
        "independently closed contexts"
    );
    let original = original.compile();
    let selected = selected.compile();
    let cases: Vec<Json> = serde_json::from_str(CASES).unwrap();
    assert_eq!(cases.len(), 44);
    let mut pairs = 0;
    for case in &cases {
        for rich in [false, true] {
            for profile in 0..3 {
                let before = evaluate(&original, "runStep", case, rich, profile, 0);
                let after = evaluate(&selected, "runStep", case, rich, profile, 0);
                check_return(&before, true);
                check_return(&after, true);
                assert_eq!(
                    before.observed, after.observed,
                    "composed {} {rich} {profile}",
                    case["name"]
                );
                pairs += 1;
            }
            for (name, composed) in [("runEntry", false), ("actChatLocal", false)] {
                let before = evaluate(&original, name, case, rich, 0, 0);
                let after = evaluate(&selected, name, case, rich, 0, 0);
                if name == "runEntry" {
                    check_return(&before, composed);
                    check_return(&after, composed);
                } else {
                    check_consumer(&before, case);
                    check_consumer(&after, case);
                }
                assert_eq!(
                    before.observed, after.observed,
                    "{} {name} {rich}",
                    case["name"]
                );
                pairs += 1;
            }
        }
    }
    assert_eq!(pairs, 440);
    for mask in 0..64 {
        for (name, projection) in [("runShared", false), ("runProjection", true)] {
            let before = evaluate(&original, name, &cases[0], false, 0, mask);
            let after = evaluate(&selected, name, &cases[0], false, 0, mask);
            check_shared(&before, mask, projection);
            check_shared(&after, mask, projection);
            assert_eq!(before.observed, after.observed, "{name} mask={mask}");
            pairs += 1;
        }
    }
    assert_eq!(pairs, 568);
    println!(
        "PASS scenarios=44 shared_masks=64 paired_comparisons={pairs} vm_evaluations={}",
        pairs * 2
    );
    // Outcome vectors establish ordering within each category, not interleaving
    // across commands, writes and sends. Reply continuations are not dispatched.
}
