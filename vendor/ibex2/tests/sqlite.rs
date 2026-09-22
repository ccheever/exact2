//! App storage through the actual Hermes bindings and native SQLite provider.
#![cfg(all(feature = "hermes", feature = "loader"))]
mod common;
use common::Project;
use ibex2::{
    engine::hermes::{DynamicCode, Hermes},
    loader::{ModuleGrants, Root},
    stdlib::app_fs::AppDirectories,
};
use std::{sync::Arc, time::Duration};

fn runtime(
    project: &Project,
    manifest: &str,
    provider: Arc<dyn ibex2::stdlib::sqlite::Provider>,
) -> Hermes {
    let data = project.0.join("data");
    let cache = project.0.join("cache");
    let temporary = project.0.join("tmp");
    for path in [&data, &cache, &temporary] {
        std::fs::create_dir_all(path).unwrap();
    }
    let mut runtime = Hermes::new(DynamicCode::Closed).unwrap();
    assert!(runtime.install_stdlib());
    runtime.install_bindings().unwrap();
    runtime
        .set_app_directories(AppDirectories::new(data, cache, temporary).unwrap())
        .unwrap();
    runtime.set_sqlite_provider(provider).unwrap();
    runtime
        .set_loader(
            Root::Declared(project.0.clone()),
            ModuleGrants::parse(manifest).unwrap(),
        )
        .unwrap();
    runtime.harden().unwrap();
    runtime
}

fn run(project: &Project, manifest: &str) -> Vec<String> {
    let mut runtime = runtime(project, manifest, Arc::new(ibex2_sqlite::SqliteProvider));
    runtime.run_entry("./index.js").unwrap();
    runtime.run_to_quiescence(Duration::from_secs(10));
    runtime
        .drain_console()
        .into_iter()
        .map(|record| record.message)
        .collect()
}

#[test]
fn sqlite_refuses_tampering_with_an_admitted_intl_intrinsic() {
    let project = Project::new("sqlite-intl-integrity");
    project.file(
        "index.js",
        r#"(async function () {
          try { await sqlite.open('app:/data/test.db'); }
          catch (error) { console.log(error.message); }
        })();"#,
    );
    let data = project.0.join("data");
    let cache = project.0.join("cache");
    let temporary = project.0.join("tmp");
    for path in [&data, &cache, &temporary] {
        std::fs::create_dir_all(path).unwrap();
    }
    let mut runtime = Hermes::new(DynamicCode::Closed).unwrap();
    assert!(runtime.install_stdlib());
    runtime.install_bindings().unwrap();
    // Linux trusted bootstrap replaces this exact method and narrowly updates
    // SQLite's construction-time integrity snapshot. A later replacement must
    // still be refused after freezing; admitting the name is not a whitelist.
    runtime
        .eval("Number.prototype.toLocaleString = function () { return 'tampered'; }")
        .unwrap();
    runtime
        .set_app_directories(AppDirectories::new(data.clone(), cache, temporary).unwrap())
        .unwrap();
    runtime
        .set_sqlite_provider(Arc::new(ibex2_sqlite::SqliteProvider))
        .unwrap();
    runtime
        .set_loader(
            Root::Declared(project.0.clone()),
            ModuleGrants::parse("[*]\nsqlite.open app:/data/test.db\n").unwrap(),
        )
        .unwrap();
    runtime.harden().unwrap();
    runtime.run_entry("./index.js").unwrap();
    runtime.run_to_quiescence(Duration::from_secs(10));
    let output: Vec<_> = runtime
        .drain_console()
        .into_iter()
        .map(|record| record.message)
        .collect();
    assert_eq!(output.len(), 1, "{output:?}");
    assert!(output[0].contains("unchanged, hardened intrinsics"));
    assert!(!data.join("test.db").exists());
}

#[test]
fn sqlite_typed_parameters_transactions_ordering_and_explicit_lifetime() {
    let project = Project::new("sqlite-values");
    project.file("index.js", r#"
(async function () {
  const db = await sqlite.open('app:/data/test.db');
  await db.execute('CREATE TABLE t (id INTEGER PRIMARY KEY, text TEXT UNIQUE, n REAL, b BLOB, empty)');
  const insert = await db.prepare('INSERT INTO t VALUES (?, ?, ?, ?, ?)');
  const bytes = new Uint8Array([99, 1, 2, 99]);
  const first = await insert.execute([BigInt('9223372036854775807'), 'hello\u0000world', 1.25, bytes.subarray(1, 3), null]);
  console.log(first.changes, String(first.lastInsertRowid));
  const found = await db.query('SELECT * FROM t');
  console.log(found.columns.join(','), typeof found.rows[0][0], String(found.rows[0][0]), found.rows[0][1].length, found.rows[0][2], Array.from(found.rows[0][3]).join(','), found.rows[0][4] === null);
  await db.transaction([{sql:'INSERT INTO t(id,text) VALUES (?,?)', params:[1,'one']}, {sql:'INSERT INTO t(id,text) VALUES (?,?)', params:[2,'two']}]);
  try { await db.transaction([{sql:'INSERT INTO t(id,text) VALUES (?,?)',params:[3,'three']}, {sql:'INSERT INTO t(id,text) VALUES (?,?)',params:[4,'one']}]); }
  catch (e) { console.log('rollback'); }
  console.log(String((await db.query('SELECT count(*) FROM t')).rows[0][0]));
  const reader = await db.prepare('SELECT text FROM t WHERE id = ?');
  console.log((await reader.query([1])).rows[0][0]);
  const queued = db.execute('INSERT INTO t(id,text) VALUES (?,?)',[5,'five']);
  const close = db.close();
  await queued; await close; await db.close();
  try { await reader.query([1]); } catch (e) { console.log('closed'); }
  await reader.close(); await insert.close();
  const reopened = await sqlite.open('app:/data/test.db');
  console.log(String((await reopened.query('SELECT count(*) FROM t')).rows[0][0]));
  for (const params of [[undefined], [Infinity], [9007199254740992], [BigInt('9223372036854775808')], new Array(1)]) {
    try { await reopened.query('SELECT ?',params); } catch (e) { console.log('invalid'); }
  }
  await reopened.close();
})().catch(e => console.error('FAILED', e.message));
"#);
    assert_eq!(
        run(&project, "[*]\nsqlite.open app:/data/test.db\n"),
        [
            "1 9223372036854775807",
            "id,text,n,b,empty bigint 9223372036854775807 11 1.25 1,2 true",
            "rollback",
            "3",
            "one",
            "closed",
            "4",
            "invalid",
            "invalid",
            "invalid",
            "invalid",
            "invalid",
        ]
    );
}

#[test]
fn sqlite_bindings_have_no_global_authority_and_handoff_is_explicit() {
    let project = Project::new("sqlite-authority");
    project.file("index.js", r#"
(async function () {
  console.log(typeof globalThis.sqlite, typeof globalThis.__ibex2_sqlite_result, typeof globalThis.__ibex2_sqlite_own);
  try { await sqlite.open('app:/data/private.db'); } catch(e) { console.log('denied'); }
  const owner = require('./owner');
  const db = await owner.open();
  console.log(Object.keys(db).length, String((await db.query('SELECT 42')).rows[0][0]));
  try { await db.query.call({}, 'SELECT 1'); } catch(e) { console.log('forgery refused'); }
  try { await db.execute("ATTACH DATABASE 'app:/data/other.db' AS other"); } catch(e) { console.log('attach refused'); }
  await db.close();
})().catch(e => console.error('FAILED',e.message));
"#).file("owner.js", "exports.open = () => sqlite.open('app:/data/private.db');");
    assert_eq!(
        run(
            &project,
            "[*]\n[./owner.js]\nsqlite.open app:/data/private.db\n"
        ),
        [
            "undefined undefined undefined",
            "denied",
            "0 42",
            "forgery refused",
            "attach refused"
        ]
    );
}

#[test]
fn app_filesystem_descriptors_atomic_replace_and_grants_reach_the_js_binding() {
    let project = Project::new("app-filesystem-js");
    project.file("index.js", r#"
(async function () {
  console.log(Object.isFrozen(fs.directories), fs.directories.data, fs.directories.cache, fs.directories.temporary);
  await fs.atomicWriteFile(fs.directories.data + '/note', new TextEncoder().encode('first'));
  await fs.atomicWriteFile(fs.directories.data + '/note', new TextEncoder().encode('replacement'));
  console.log(new TextDecoder().decode(await fs.readFile('app:/data/note')));
  console.log(await fs.realpath('app:/data/note'));
  try { await fs.readFile('app:/cache/note'); } catch(e) { console.log('cache denied'); }
})().catch(e => console.error('FAILED', e.message));
"#);
    assert_eq!(
        run(&project, "[*]\nfs.read app:/data\nfs.write app:/data\n"),
        [
            "true app:/data app:/cache app:/tmp",
            "replacement",
            "app:/data/note",
            "cache denied"
        ]
    );
}

use ibex2::{
    boundary::HostError,
    stdlib::sqlite::{self, Provider},
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Mutex,
};

struct QueryGate {
    entered: mpsc::Sender<()>,
    release: Mutex<mpsc::Receiver<()>>,
}
struct TrackingProvider {
    closed: mpsc::Sender<()>,
    query_gate: Option<Arc<QueryGate>>,
}
struct TrackingConnection {
    inner: Arc<dyn sqlite::Connection>,
    closed: mpsc::Sender<()>,
    notified: AtomicBool,
    query_gate: Option<Arc<QueryGate>>,
}
impl Provider for TrackingProvider {
    fn open(&self, location: sqlite::Location) -> Result<Arc<dyn sqlite::Connection>, HostError> {
        Ok(Arc::new(TrackingConnection {
            inner: ibex2_sqlite::SqliteProvider.open(location)?,
            closed: self.closed.clone(),
            notified: AtomicBool::new(false),
            query_gate: self.query_gate.clone(),
        }))
    }
}
impl sqlite::Connection for TrackingConnection {
    fn prepare(&self, sql: &str) -> Result<sqlite::StatementInfo, HostError> {
        self.inner.prepare(sql)
    }
    fn execute(
        &self,
        sql: &str,
        params: &[sqlite::Value],
    ) -> Result<sqlite::ExecuteResult, HostError> {
        self.inner.execute(sql, params)
    }
    fn query(&self, sql: &str, params: &[sqlite::Value]) -> Result<sqlite::Rows, HostError> {
        if let Some(gate) = &self.query_gate {
            gate.entered.send(()).unwrap();
            gate.release
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .unwrap();
        }
        self.inner.query(sql, params)
    }
    fn transaction(
        &self,
        commands: &[sqlite::Command],
    ) -> Result<Vec<sqlite::ExecuteResult>, HostError> {
        self.inner.transaction(commands)
    }
    fn close(&self) -> Result<(), HostError> {
        self.inner.close()?;
        if !self.notified.swap(true, Ordering::AcqRel) {
            let _ = self.closed.send(());
        }
        Ok(())
    }
}

#[test]
fn live_statements_retain_databases_and_gc_closes_unreachable_connections() {
    let project = Project::new("sqlite-gc");
    project.file("index.js", "sqlite.open('app:/data/test.db').then(db => db.prepare('SELECT 42')).then(statement => globalThis.statement = statement);");
    let (closed, received) = mpsc::channel();
    let mut runtime = runtime(
        &project,
        "[*]\nsqlite.open app:/data/test.db\n",
        Arc::new(TrackingProvider {
            closed,
            query_gate: None,
        }),
    );
    runtime.run_entry("./index.js").unwrap();
    runtime.run_to_quiescence(Duration::from_secs(5));
    assert!(runtime.collect_garbage());
    runtime.eval("statement.query().then(result => console.log(String(result.rows[0][0]))).catch(e => console.error(e.message))").unwrap();
    runtime.run_to_quiescence(Duration::from_secs(5));
    assert_eq!(
        runtime
            .drain_console()
            .iter()
            .map(|r| r.message.as_str())
            .collect::<Vec<_>>(),
        ["42"]
    );
    assert!(
        received.try_recv().is_err(),
        "a live statement lost its database"
    );
    runtime.eval("globalThis.statement = undefined").unwrap();
    assert!(runtime.collect_garbage());
    received
        .recv_timeout(Duration::from_secs(5))
        .expect("GC must close the native SQLite connection");
}

#[test]
fn runtime_destruction_closes_a_database_still_reachable_from_javascript() {
    let project = Project::new("sqlite-runtime-drop");
    project.file(
        "index.js",
        "sqlite.open('app:/data/test.db').then(db => globalThis.database = db);",
    );
    let (closed, received) = mpsc::channel();
    let mut runtime = runtime(
        &project,
        "[*]\nsqlite.open app:/data/test.db\n",
        Arc::new(TrackingProvider {
            closed,
            query_gate: None,
        }),
    );
    runtime.run_entry("./index.js").unwrap();
    runtime.run_to_quiescence(Duration::from_secs(5));
    assert_eq!(runtime.eval("typeof database.query").unwrap(), "function");
    drop(runtime);
    received
        .recv_timeout(Duration::from_secs(5))
        .expect("runtime destruction must close native SQLite connections");
}

#[test]
fn pending_query_retains_its_owner_until_the_promise_settles() {
    let project = Project::new("sqlite-pending-gc");
    project.file(
        "index.js",
        "sqlite.open('app:/data/test.db').then(db => globalThis.database = db);",
    );
    let (closed, received) = mpsc::channel();
    let (entered, entered_query) = mpsc::channel();
    let (release_query, release) = mpsc::channel();
    let provider = TrackingProvider {
        closed,
        query_gate: Some(Arc::new(QueryGate {
            entered,
            release: Mutex::new(release),
        })),
    };
    let mut runtime = runtime(
        &project,
        "[*]\nsqlite.open app:/data/test.db\n",
        Arc::new(provider),
    );
    runtime.run_entry("./index.js").unwrap();
    runtime.run_to_quiescence(Duration::from_secs(5));
    runtime.eval("database.query('SELECT 123').then(result => console.log(String(result.rows[0][0]))).catch(e => console.error(e.message)); globalThis.database = undefined;").unwrap();
    runtime.pump().unwrap();
    entered_query
        .recv_timeout(Duration::from_secs(5))
        .expect("query started");
    assert!(runtime.collect_garbage());
    release_query.send(()).unwrap();
    runtime.run_to_quiescence(Duration::from_secs(5));
    assert_eq!(
        runtime
            .drain_console()
            .iter()
            .map(|r| r.message.as_str())
            .collect::<Vec<_>>(),
        ["123"]
    );
    assert!(runtime.collect_garbage());
    received
        .recv_timeout(Duration::from_secs(5))
        .expect("settled query must release its database owner");
}
