//! The generated seam is checked by the actual pinned TypeScript compiler.
//! @ref LLP 1027 D5 — a wrong-shaped answer is a build refusal.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

const CONTRACT: &str = r#"
shape Item
  id: string
  enabled: bool
  scores: list<number>
  child: option<string>

component App
  state query = ""
  resource items = search(query) as shape list<Item>
  mutation outcome as shape option<Item>
  action save writes outcome
    send outcome = save(query, true)
  view
    text `${length(items)}`
"#;

const VALID: &str = r#"
import type { Answer, Args, Result, Source, Sources, Store, Storage } from './app.contract.d.ts';
const sources: Sources = {
  search: ([query], store) => [{ id: query, enabled: store.get('token') !== null, scores: [1], child: null }],
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
const nullable: Result<'save'> = null;
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
        Command::new("bun")
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
                "ES2022",
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
    assert!(invoke(&["types", "app.contract", "-o", "app.d.ts"])
        .status
        .success());
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
