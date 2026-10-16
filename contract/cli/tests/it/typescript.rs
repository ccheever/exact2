//! The generated seam is checked by the actual pinned TypeScript compiler.
//! @ref LLP 1027 D5 — a wrong-shaped answer is a build refusal.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

const CONTRACT: &str = r#"
shape Item
  id: string
  enabled: bool
  scores: number[]
  child: option<string>

component App
  state query = ""
  resource items = search(query) as shape Item[]
  resource found = find(query) as shape option<Item>
  mutation outcome as shape Item
  action save
    send outcome = save(query, true)
  view
    text `${items.length}`
"#;

const VALID: &str = r#"
import type { Answer, Args, Result, Source, Sources, Store, Storage } from './app.contract.d.ts';
const sources: Sources = {
  search: ([query], store) => [{ id: query, enabled: store.get('token') !== null, scores: [1], child: null }],
  find: () => null,
  save: async ([query, enabled], store) => {
    store.set('token', query);
    store.forget('old');
    return { id: query, enabled, scores: [], child: query };
  },
};
export const answer: Answer = (source, args, store, storage) => sources[source](args, store, storage);
declare const store: Store;
declare const storage: Storage;
answer('search', ['Palo'], store, storage);
answer('save', ['Palo', true], store, storage);
const nullable: Result<'find'> = null;
// @ts-expect-error argument order is the Contract's
const swapped: Args<'save'> = [true, 'Palo'];
// @ts-expect-error the caller must supply every argument
answer('save', ['Palo'], store, storage);
// @ts-expect-error unknown sources are not widened to strings
const unknown: Source = 'invented';
// @ts-expect-error a result is not another source's result
const wrongResult: Result<'save'> = [];
// @ts-expect-error store values remain strings
store.set('token', 4);
// @ts-expect-error reads may be absent
const missing: string = store.get('missing');
async function storageTypes() {
  const path = storage.fs.directories.data + '/notes.db';
  await storage.fs.atomicWriteFile(storage.fs.directories.cache + '/bytes', new Uint8Array([1, 2]));
  const bytes: ArrayBuffer = await storage.fs.readFile(storage.fs.directories.cache + '/bytes');
  const database = await storage.sqlite.open(path);
  const statement = await database.prepare('SELECT ?');
  const rows = await statement.query([1n, 'text', null, new Uint8Array(bytes)]);
  const name: string = rows.columns[0];
  const batch = await database.transaction([{sql: 'INSERT INTO notes VALUES (?)', params: [1n]}]);
  const id: bigint = batch[0].lastInsertRowid;
  await statement.close();
  await database.close();
  // @ts-expect-error filesystem writes require bytes, not strings
  await storage.fs.writeFile(path, 'wrong');
  // @ts-expect-error SQL parameters do not accept arbitrary records
  await database.query('SELECT ?', [{unexpected: true}]);
  // @ts-expect-error SQLite row IDs are bigint, never lossy numbers
  const lossy: number = batch[0].lastInsertRowid;
  // `fs.compressImage` as the docs give it (LLP 1069.002 A1): options in, the record out.
  const image = await storage.fs.compressImage(path, storage.fs.directories.cache + '/out.jpg', {maxDimension: 1024, maxBytes: 1_000_000});
  const shrunk: {path: string; type: 'image/jpeg'; size: number; width: number; height: number} = image;
  // @ts-expect-error the engine's positional binding is not the app interface
  await storage.fs.compressImage(path, path, 1024, 1_000_000);
}
// @ts-expect-error store.set has no success value
const success: boolean = store.set('token', 'value');
"#;

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "exact-types-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn write(&self, file: &str, text: &str) {
        std::fs::write(self.0.join(file), text).unwrap();
    }

    fn check(&self, text: &str) -> Output {
        self.write("app.ts", text);
        Command::new(std::env::var_os("BUN").unwrap_or_else(|| "bun".into()))
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../node_modules/.bin/tsc"))
            .args([
                "--noEmit",
                "--strict",
                "--target",
                "ES2022",
                "--module",
                "ESNext",
                "--moduleResolution",
                "bundler",
                "--lib",
                // The seam names the platform-standard URL and CryptoKeyPair.
                "ES2022,DOM",
                "--pretty",
                "false",
                "app.ts",
            ])
            .current_dir(&self.0)
            .output()
            .expect("run bun install --frozen-lockfile at the repo root to install the pinned TypeScript compiler")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

/// The app interface of `storage.fs.compressImage` (LLP 1069.002 A1) as
/// docs/reference.md gives it: the declaration apps type-check against says
/// options in and the record out. Kept beside the tsc check (the async lane),
/// which the default checks skip: an Ibex refresh once replaced the
/// declaration with the engine's positional binding and nothing failed.
#[test]
fn compress_image_is_declared_as_the_docs_give_it() {
    let declared = include_str!("../../../../vendor/ibex/crates/ibex2/src/bindings/storage.d.ts");
    for line in [
        "compressImage(from: string, to: string, options: CompressImageOptions): Promise<CompressedImage>;",
        "readonly maxDimension: number;",
        "readonly maxBytes: number;",
        "readonly type: \"image/jpeg\";",
        "readonly width: number;",
        "readonly height: number;",
        "readonly size: number;",
        "readonly path: string;",
    ] {
        assert!(declared.contains(line), "storage.d.ts lost `{line}`");
    }
    // The engine's positional binding is the prelude's to adapt, never the app's.
    assert!(!declared.contains("compressImage(from: string, to: string, maxDimension"));
    let reference = include_str!("../../../../docs/reference.md");
    assert!(reference.contains("`storage.fs.compressImage(from, to, {maxDimension, maxBytes})`"));
}

#[test]
#[ignore = "async lane: typechecks generated signatures with tsc; bun scripts/async.mjs runs it"]
fn generated_signatures_check_real_sync_and_async_providers_and_the_dispatcher() {
    let plan = contract::compile(CONTRACT).unwrap();
    let declarations = contract::typescript(&plan).unwrap();
    assert_eq!(
        declarations,
        contract::typescript(&exact_plan::Plan::decode(&plan.encode()).unwrap()).unwrap()
    );
    let fixture = Fixture::new();
    fixture.write("app.contract.d.ts", &declarations);
    let output = fixture.check(VALID);
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // No @ts-expect-error escape: each changed provider must actually fail.
    for bad in [
        VALID.replace("scores: [1]", "scores: ['wrong']"),
        VALID.replace("child: query", "child: 42"),
        VALID.replace(
            "export const answer: Answer = (source, args, store, storage) => sources[source](args, store, storage)",
            "export const answer: Answer = () => 42",
        ),
    ] {
        let output = fixture.check(&bad);
        assert!(!output.status.success(), "wrong-shaped logic passed tsc");
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("error TS"),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
#[ignore = "async lane: typechecks Caltrain with tsc; bun scripts/async.mjs runs it"]
fn caltrain_types_follow_its_real_plan_without_requiring_the_host_owned_source() {
    let plan = contract::compile_path(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/caltrain/app.contract"),
    )
    .unwrap();
    let fixture = Fixture::new();
    fixture.write("app.contract.d.ts", &contract::typescript(&plan).unwrap());
    let output = fixture.check(
        r#"
import type { Args, Result, Source } from './app.contract.d.ts';
const args: Args<'board'> = ['mv', 'north', 123];
const board: Result<'board'> = [{ id: '1', train: 123, service: 'Local', headsign: 'SF', at: 456 }];
const location: Result<'defaultLocation'> = { lat: 0, lon: 0 };
// @ts-expect-error the runner owns delivery, not the app module
const delivery: Source = 'exactDelivery';
// @ts-expect-error a station is not a departure
const wrong: Result<'board'> = [{ id: 'mv', name: 'Mountain View', zone: 4, distance: 0 }];
"#,
    );
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "async lane: typechecks the guides' TypeScript examples with tsc; bun scripts/async.mjs runs it"]
fn every_typescript_example_in_the_guides_checks_against_its_contracts_types() {
    // LLP 1086 D10: each `ts` block after a `contract` block in the same
    // section is checked against that Contract's `contract types` output.
    let mut failures = Vec::new();
    let mut checked = 0;
    for block in crate::docs::blocks() {
        let Some(source) = block.contract.as_deref().filter(|_| block.info == "ts") else {
            continue;
        };
        checked += 1;
        let fixture = Fixture::new();
        fixture.write("app.contract", source);
        let plan = match contract::compile_path(&fixture.0.join("app.contract")) {
            Ok(plan) => plan,
            Err(e) => {
                failures.push(format!("{}: its contract: {e}", block.at()));
                continue;
            }
        };
        fixture.write("app.contract.d.ts", &contract::typescript(&plan).unwrap());
        let output = fixture.check(&block.body);
        if !output.status.success() {
            failures.push(format!(
                "{}:\n{}{}",
                block.at(),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }
    assert!(
        checked > 0,
        "no TypeScript examples follow a contract block"
    );
    assert!(
        failures.is_empty(),
        "{} of {checked} TypeScript examples fail tsc:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn the_cli_emits_types_and_refuses_bad_inputs_without_touching_last_good_output() {
    let fixture = Fixture::new();
    fixture.write("app.contract", CONTRACT);
    let invoke = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_contract"))
            .args(args)
            .current_dir(&fixture.0)
            .output()
            .unwrap()
    };
    let stdout = invoke(&["types", "app.contract"]);
    assert!(stdout.status.success());
    let written = invoke(&["types", "app.contract", "-o", "app.d.ts"]);
    assert!(written.status.success());
    assert!(written.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&written.stderr),
        "types: wrote app.d.ts\n"
    );
    assert_eq!(
        stdout.stdout,
        std::fs::read(fixture.0.join("app.d.ts")).unwrap()
    );
    for args in [
        vec!["types"],
        vec!["types", "app.contract", "-o"],
        vec!["types", "app.contract", "--unknown"],
    ] {
        assert_eq!(invoke(&args).status.code(), Some(2));
    }
    fixture.write("app.contract", "not a contract");
    assert_eq!(
        invoke(&["types", "app.contract", "-o", "app.d.ts"])
            .status
            .code(),
        Some(1)
    );
    assert_eq!(
        stdout.stdout,
        std::fs::read(fixture.0.join("app.d.ts")).unwrap()
    );
}

#[test]
fn malformed_plan_tables_are_refused_before_generating_types() {
    let mut plan = contract::compile(CONTRACT).unwrap();
    plan.sources[0].params.len = u32::MAX;
    assert!(contract::typescript(&plan).is_err());
}

#[test]
fn router_shapes_are_already_named_by_the_plan_type_generator() {
    let plan = contract::compile(include_str!("../../../corpus/routes.contract")).unwrap();
    let declaration = contract::typescript(&plan).unwrap();
    for name in ["Router", "Tab", "Entry", "Params"] {
        let (id, row) = plan
            .types
            .iter()
            .enumerate()
            .find(|(_, r)| plan.str(r.name) == name)
            .unwrap();
        let prefix = format!("type T{id} = {{ ");
        let line = declaration
            .lines()
            .find(|l| l.starts_with(&prefix))
            .unwrap();
        for field in
            &plan.fields[row.fields.start as usize..(row.fields.start + row.fields.len) as usize]
        {
            assert!(line.contains(&format!("\"{}\": T{};", plan.str(field.name), field.ty.0)));
        }
    }
}

// LLP 1116.006 D7: a builder reads the top of app.contract.d.ts, not its declarations.
#[test]
fn the_file_opens_with_what_app_ts_writes_one_line_each() {
    let declaration = contract::typescript(&contract::compile(CONTRACT).unwrap()).unwrap();
    let top: Vec<&str> = declaration.lines().take(30).collect();
    for line in [
        "//   find: (args: [string]) => Item | null",
        "//   save: (args: [string, boolean]) => Item",
        "//   search: (args: [string]) => Item[]",
        "// The shapes. None is exported: name one by its source, `type Item = Result<'save'>`.",
        "//   Item = { id: string; enabled: boolean; scores: number[]; child: string | null }",
    ] {
        assert!(
            top.contains(&line),
            "{line} is not in the top:\n{}",
            top.join("\n")
        );
    }
    // Comments only, ahead of the declarations, which are as they were.
    let summary = declaration.split("\n\n").next().unwrap();
    assert!(summary.lines().all(|l| l.starts_with("//")), "{summary}");
    assert!(declaration.contains("\nexport interface SourceMap {\n  \"find\": { args: [T"));

    // The router's shapes are the runner's, so they are left out; a shape names the shapes inside it.
    let routes = contract::typescript(
        &contract::compile(include_str!("../../../corpus/routes.contract")).unwrap(),
    )
    .unwrap();
    let summary = routes.split("\n\n").next().unwrap();
    for name in ["Router", "Tab", "Entry", "Params"] {
        assert!(!summary.contains(&format!("//   {name} = ")), "{summary}");
    }
    assert!(
        summary
            .lines()
            .any(|l| l == "//   loadPeople: (args: [string[]]) => Person[]"),
        "{summary}"
    );
    assert!(
        summary
            .lines()
            .any(|l| l == "//   Person = { id: string; name: string; post: string }"),
        "{summary}"
    );
    assert!(
        summary.contains("`type Person = Result<'loadPeople'>[number]`"),
        "{summary}"
    );
}

#[test]
fn a_contract_without_sources_has_no_summary() {
    let declaration = contract::typescript(
        &contract::compile("component App\n  view\n    text \"hi\"\n").unwrap(),
    )
    .unwrap();
    assert!(!declaration.contains("What app.ts writes"));
    assert!(declaration.starts_with(
        "// @generated from Contract source signatures; do not edit.\n\
         // Static types do not replace runtime shape, identity, or grant checks.\n\n"
    ));
}

#[test]
#[ignore = "async lane: checks the stream handle with tsc"]
fn generated_socket_handle_types_text_binary_and_close_events() {
    let plan = contract::compile(CONTRACT).unwrap();
    let fixture = Fixture::new();
    fixture.write("app.contract.d.ts", &contract::typescript(&plan).unwrap());
    let output = fixture.check(
        r#"
import type { StreamSocket } from './app.contract.d.ts';
let kept: StreamSocket | undefined;
fetch('wss://example.test', { exactStream(event, socket) {
  kept = socket;
  const payload: string | ArrayBuffer = event.data;
  const code: number | undefined = event.code;
  const reason: string | undefined = event.reason;
  const clean: boolean | undefined = event.wasClean;
  return typeof payload === 'string' ? payload : new TextDecoder().decode(payload);
}});
function mutation() {
  if (!kept) return;
  kept.send('subscribe');
  kept.send(new Uint8Array([1, 2]));
  kept.send(new ArrayBuffer(2));
  kept.close(4001, 'finished');
  const state: number = kept.readyState;
  const bytes: number = kept.bufferedAmount;
  // @ts-expect-error the handle cannot send arbitrary records
  kept.send({ secret: true });
  // @ts-expect-error the state is owned by the transport
  kept.readyState = 1;
}
"#,
    );
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn build_reports_all_reachable_ambient_apis_before_bake() {
    let fixture = Fixture::new();
    fixture.write(
        "app.contract",
        "component App\n  view\n    text \"Hello\"\n",
    );
    fixture.write(
        "app.ts",
        "import './helper.ts';\nexport const answer = () => Date.now();\n",
    );
    fixture.write("helper.ts", "export const value = Math.random();\n");
    let output = Command::new(env!("CARGO_BIN_EXE_contract"))
        .args(["build", "app.contract", "--json"])
        .current_dir(&fixture.0)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let found: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(found.iter().all(|d| d["id"] == "analyze-ambient-api"));
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("Date.now()"), "{text}");
    assert!(text.contains("Math.random()"), "{text}");
}

#[test]
fn ambient_check_resolves_extensionless_mounted_modules() {
    let fixture = Fixture::new();
    fixture.write(
        "app.contract",
        "component App\n  view\n    text \"Hello\"\n",
    );
    fixture.write(
        "app.json",
        r#"{"typescript":{"sources":{"backend":"generated"}}}"#,
    );
    std::fs::create_dir_all(fixture.0.join("generated")).unwrap();
    fixture.write("generated/api.ts", "export const value = 42;\n");
    for import in ["./backend/api", "./backend/api.ts"] {
        fixture.write(
            "app.ts",
            &format!("import {{value}} from '{import}';\nexport const answer = () => value;\n"),
        );
        let output = Command::new(env!("CARGO_BIN_EXE_contract"))
            .args(["build", "app.contract", "--json"])
            .current_dir(&fixture.0)
            .output()
            .unwrap();
        assert!(output.status.success(), "{import}: {output:?}");
        assert_eq!(output.stdout, b"[]\n");
    }
}

#[test]
fn unknown_handlers_continue_through_siblings_children_and_timer_targets() {
    let source="component App\n  action save\n    let done = true\n  task first mount\n    every(1000, saev)\n  task second mount\n    every(2000, svae)\n  view\n    column\n      button \"Save\" press=saev\n      column\n        button \"Save again\" press=svae\n";
    let timers = source
        .replace("press=saev", "press=save")
        .replace("press=svae", "press=save");
    let errors = contract::compile_path_source_all(Path::new("app.contract"), &timers, false)
        .err()
        .expect("unknown actions refuse");
    let unknown: Vec<_> = errors
        .iter()
        .filter(|e| e.id == "analyze-unknown-action")
        .collect();
    assert_eq!(unknown.len(), 2, "{errors:?}");
    assert!(
        unknown.iter().all(
            |e| e.message.contains("save") || e.related.iter().any(|r| r.note.contains("save"))
        ),
        "{unknown:?}"
    );
}

#[test]
fn checked_backend_projection_requires_every_contract_key_and_narrows_nullable_values() {
    let fixture = Fixture::new();
    fixture.write(
        "projection.ts",
        include_str!("../../../../snapback4/online/projection.ts"),
    );
    let checked = fixture.check(
        r#"
import {pick} from './projection';
type Note={id:string;body:string;parent:string|null};
const project=pick<Note>();
const row={id:'one',body:'hello',parent:null,at:123};
const note:Note=project(row,'id','body','parent');
// @ts-expect-error all target keys are required
project(row,'id','body');
// @ts-expect-error extra server fields are not Contract keys
project(row,'id','body','parent','at');
// @ts-expect-error wrong field type
project({...row,body:3},'id','body','parent');
// @ts-expect-error a hidden-parent projection is nullable
pick<{parent:string}>()(row,'parent');
"#,
    );
    assert!(checked.status.success(), "{checked:?}");
}

#[test]
fn types_check_is_read_only_and_build_reports_only_changed_declarations() {
    let fixture = Fixture::new();
    fixture.write("app.contract", CONTRACT);
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_contract"))
            .args(args)
            .current_dir(&fixture.0)
            .output()
            .unwrap()
    };
    let check = || run(&["types", "--check", "app.contract"]);
    assert_eq!(check().status.code(), Some(1));
    assert!(!fixture.0.join("app.contract.d.ts").exists());
    fixture.write("app.contract.d.ts", "stale");
    assert_eq!(check().status.code(), Some(1));
    assert_eq!(
        std::fs::read_to_string(fixture.0.join("app.contract.d.ts")).unwrap(),
        "stale"
    );
    let built = run(&["build", "app.contract", "--json"]);
    assert!(built.status.success(), "{built:?}");
    assert_eq!(built.stdout, b"[]\n");
    assert!(String::from_utf8_lossy(&built.stderr)
        .contains("app.contract.d.ts: regenerated, since a source's signature changed"));
    assert!(check().status.success());
    let repeated = run(&["build", "app.contract", "--json"]);
    assert!(repeated.status.success());
    assert!(repeated.stderr.is_empty());
    assert!(run(&["types", "app.contract", "-o", "other.d.ts"])
        .status
        .success());
    assert!(
        run(&["types", "app.contract", "--check", "-o", "other.d.ts"])
            .status
            .success()
    );
}
