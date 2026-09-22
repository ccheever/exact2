//! SQLite's JS handles hold already-admitted database capabilities.
//! @ref LLP 0059.000#315-sqlite--host-module-capability-bearing-already-built — provider and typed boundary
use crate::boundary::{HostArg, HostError, HostValue};
use crate::grant::GrantSet;
use crate::stdlib::sqlite::{
    Command, Database, ExecuteResult, Location, Provider, Rows, Statement, Value,
};
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex, OnceLock, Weak,
};

enum ResultData {
    Execute(ExecuteResult),
    Rows(Rows),
    Batch(Vec<ExecuteResult>),
}
#[derive(Default)]
pub(crate) struct Registry {
    provider: OnceLock<Arc<dyn Provider>>,
    databases: Mutex<HashMap<u64, Database>>,
    statements: Mutex<HashMap<u64, Arc<Statement>>>,
    results: Mutex<HashMap<u64, ResultData>>,
    next: AtomicU64,
    closed: AtomicBool,
}
fn invalid(message: &str) -> HostError {
    HostError::InvalidArgument(message.into())
}
fn closed() -> HostError {
    HostError::Failed("SQLite runtime is closed".into())
}
fn handle(n: f64) -> Result<u64, HostError> {
    if n.fract() == 0.0 && (1.0..=9_007_199_254_740_991.0).contains(&n) {
        Ok(n as u64)
    } else {
        Err(invalid("invalid SQLite handle"))
    }
}
impl Registry {
    fn id(&self) -> u64 {
        self.next.fetch_add(1, Ordering::Relaxed) + 1
    }
    pub(crate) fn set_provider(&self, provider: Arc<dyn Provider>) -> Result<(), HostError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(closed());
        }
        self.provider
            .set(provider)
            .map_err(|_| invalid("SQLite provider is already configured"))
    }
    fn database(&self, id: u64) -> Result<Database, HostError> {
        self.databases
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or_else(|| invalid("SQLite database is closed or unknown"))
    }
    fn statement(&self, id: u64) -> Result<Arc<Statement>, HostError> {
        self.statements
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or_else(|| invalid("SQLite statement is closed or unknown"))
    }
    fn result(&self, data: ResultData) -> Result<HostValue, HostError> {
        let mut results = self.results.lock().unwrap();
        if self.closed.load(Ordering::Acquire) {
            return Err(closed());
        }
        let id = self.id();
        results.insert(id, data);
        Ok(HostValue::Number(id as f64))
    }
    fn close_database(&self, id: u64) -> Result<(), HostError> {
        let database = self.databases.lock().unwrap().remove(&id);
        if let Some(database) = database {
            database.close()?;
        }
        Ok(())
    }
    fn close_statement(&self, id: u64) {
        if let Some(statement) = self.statements.lock().unwrap().remove(&id) {
            statement.close();
        }
    }
    pub(crate) fn shutdown(&self) {
        self.closed.store(true, Ordering::Release);
        self.results.lock().unwrap().clear();
        self.statements.lock().unwrap().clear();
        let databases = std::mem::take(&mut *self.databases.lock().unwrap());
        // Native close can wait for an in-flight query. Never block engine destruction.
        crate::pool::run(move || {
            for database in databases.into_values() {
                let _ = database.close();
            }
        });
    }
}
fn number(args: &[HostValue], index: usize) -> Result<f64, HostError> {
    match args.get(index) {
        Some(HostValue::Number(n)) => Ok(*n),
        _ => Err(invalid("SQLite expects a number")),
    }
}
fn string(args: &[HostValue], index: usize) -> Result<&str, HostError> {
    match args.get(index) {
        Some(HostValue::Str(s)) => Ok(s),
        _ => Err(invalid("SQLite expects a string")),
    }
}
fn count(n: f64) -> Result<usize, HostError> {
    if n.fract() == 0.0 && (0.0..=100_000.0).contains(&n) {
        Ok(n as usize)
    } else {
        Err(invalid("invalid SQLite count"))
    }
}
fn parameters(args: &[HostValue]) -> Result<Vec<Value>, HostError> {
    if !args.len().is_multiple_of(2) {
        return Err(invalid("SQLite parameters require type/value pairs"));
    }
    args.chunks_exact(2)
        .map(|pair| {
            Ok(match number(pair, 0)? {
                0.0 => Value::Null,
                1.0 => Value::Integer(
                    string(pair, 1)?
                        .parse()
                        .map_err(|_| invalid("SQLite integer is outside int64"))?,
                ),
                2.0 => {
                    let n = number(pair, 1)?;
                    if !n.is_finite() {
                        return Err(invalid("SQLite real must be finite"));
                    }
                    Value::Real(n)
                }
                3.0 => Value::Text(string(pair, 1)?.to_owned()),
                4.0 => match &pair[1] {
                    HostValue::Bytes(bytes) => Value::Blob(bytes.clone()),
                    _ => return Err(invalid("SQLite blob expects bytes")),
                },
                _ => return Err(invalid("unknown SQLite parameter type")),
            })
        })
        .collect()
}

pub(crate) fn run(
    op: u32,
    args: &[HostValue],
    state: &crate::task::RuntimeState,
    grants: &GrantSet,
) -> Result<HostValue, HostError> {
    let registry = &state.sqlite;
    if registry.closed.load(Ordering::Acquire) {
        return Err(closed());
    }
    if op == 150 {
        let path = crate::stdlib::app_fs::resolve_sqlite(
            grants,
            state.app_directories(),
            string(args, 0)?,
        )?;
        let provider = registry
            .provider
            .get()
            .ok_or_else(|| HostError::Failed("SQLite provider is not installed".into()))?;
        let database = Database::new(provider.open(Location { path })?);
        let mut databases = registry.databases.lock().unwrap();
        if registry.closed.load(Ordering::Acquire) {
            database.close()?;
            return Err(closed());
        }
        let id = registry.id();
        databases.insert(id, database);
        return Ok(HostValue::Number(id as f64));
    }
    let id = handle(number(args, 0)?)?;
    match op {
        151 => {
            let statement = registry.database(id)?.prepare(string(args, 1)?)?;
            let mut statements = registry.statements.lock().unwrap();
            if registry.closed.load(Ordering::Acquire) {
                return Err(closed());
            }
            let id = registry.id();
            statements.insert(id, Arc::new(statement));
            Ok(HostValue::Number(id as f64))
        }
        152 => registry.result(ResultData::Execute(
            registry
                .database(id)?
                .execute(string(args, 1)?, &parameters(&args[2..])?)?,
        )),
        153 => registry.result(ResultData::Rows(
            registry
                .database(id)?
                .query(string(args, 1)?, &parameters(&args[2..])?)?,
        )),
        154 => registry.result(ResultData::Execute(
            registry.statement(id)?.execute(&parameters(&args[1..])?)?,
        )),
        155 => registry.result(ResultData::Rows(
            registry.statement(id)?.query(&parameters(&args[1..])?)?,
        )),
        156 => {
            let n = count(number(args, 1)?)?;
            let mut offset = 2;
            let mut commands = Vec::new();
            for _ in 0..n {
                let sql = string(args, offset)?.to_owned();
                let n = count(number(args, offset + 1)?)?;
                offset += 2;
                let end = offset
                    .checked_add(n * 2)
                    .ok_or_else(|| invalid("SQLite parameter count overflow"))?;
                let values = args
                    .get(offset..end)
                    .ok_or_else(|| invalid("missing SQLite parameters"))?;
                commands.push(Command {
                    sql,
                    params: parameters(values)?,
                });
                offset = end;
            }
            if offset != args.len() {
                return Err(invalid("unexpected SQLite transaction arguments"));
            }
            registry.result(ResultData::Batch(
                registry.database(id)?.transaction(&commands)?,
            ))
        }
        157 => {
            registry.close_database(id)?;
            Ok(HostValue::Undefined)
        }
        158 => {
            registry.close_statement(id);
            Ok(HostValue::Undefined)
        }
        _ => Err(invalid("unknown SQLite operation")),
    }
}

pub(crate) fn result_field(
    args: &[HostArg],
    state: Option<&crate::task::RuntimeState>,
) -> Result<HostValue, HostError> {
    let registry = &state.ok_or_else(|| invalid("missing runtime"))?.sqlite;
    let number = |index| match args.get(index) {
        Some(HostArg::Number(n)) => Ok(*n),
        _ => Err(invalid("SQLite result expects numeric arguments")),
    };
    let id = handle(number(0)?)?;
    let field = count(number(1)?)?;
    let mut results = registry.results.lock().unwrap();
    if field == 8 {
        results.remove(&id);
        return Ok(HostValue::Undefined);
    }
    let data = results
        .get(&id)
        .ok_or_else(|| invalid("SQLite result is released or unknown"))?;
    if field == 0 {
        return Ok(HostValue::Number(match data {
            ResultData::Execute(_) => 0.0,
            ResultData::Rows(_) => 1.0,
            ResultData::Batch(_) => 2.0,
        }));
    }
    let index = || count(number(2)?);
    if field == 6 || field == 7 {
        let result = match data {
            ResultData::Execute(result) => result,
            ResultData::Batch(batch) => batch
                .get(index()?)
                .ok_or_else(|| invalid("SQLite batch index out of bounds"))?,
            _ => return Err(invalid("SQLite result is not an execution result")),
        };
        return Ok(if field == 6 {
            HostValue::Number(result.changes as f64)
        } else {
            HostValue::Str(result.last_insert_rowid.to_string())
        });
    }
    match (field, data) {
        (1, ResultData::Rows(rows)) => Ok(HostValue::Number(rows.rows.len() as f64)),
        (1, ResultData::Batch(batch)) => Ok(HostValue::Number(batch.len() as f64)),
        (2, ResultData::Rows(rows)) => Ok(HostValue::Number(rows.columns.len() as f64)),
        (3, ResultData::Rows(rows)) => rows
            .columns
            .get(index()?)
            .cloned()
            .map(HostValue::Str)
            .ok_or_else(|| invalid("SQLite column out of bounds")),
        (4 | 5, ResultData::Rows(rows)) => {
            let column = count(number(3)?)?;
            let value = rows
                .rows
                .get(index()?)
                .and_then(|row| row.get(column))
                .ok_or_else(|| invalid("SQLite cell out of bounds"))?;
            Ok(if field == 4 {
                HostValue::Number(match value {
                    Value::Null => 0.0,
                    Value::Integer(_) => 1.0,
                    Value::Real(_) => 2.0,
                    Value::Text(_) => 3.0,
                    Value::Blob(_) => 4.0,
                })
            } else {
                match value {
                    Value::Null => HostValue::Null,
                    Value::Integer(n) => HostValue::Str(n.to_string()),
                    Value::Real(n) => HostValue::Number(*n),
                    Value::Text(s) => HostValue::Str(s.clone()),
                    Value::Blob(b) => HostValue::Bytes(b.clone()),
                }
            })
        }
        _ => Err(invalid("invalid SQLite result field")),
    }
}

struct Owner {
    state: Weak<crate::task::RuntimeState>,
    id: u64,
    kind: i32,
}
/// # Safety
/// `queue` must be a live runtime queue pointer.
#[no_mangle]
pub unsafe extern "C" fn ibex2_sqlite_owner_create(
    queue: *const crate::task::RuntimeState,
    id: f64,
    kind: i32,
) -> *mut std::ffi::c_void {
    let Some(state) = crate::task::clone_queue(queue) else {
        return std::ptr::null_mut();
    };
    let Ok(id) = handle(id) else {
        return std::ptr::null_mut();
    };
    if kind != 0 && kind != 1 {
        return std::ptr::null_mut();
    }
    Box::into_raw(Box::new(Owner {
        state: Arc::downgrade(&state),
        id,
        kind,
    }))
    .cast()
}
/// # Safety
/// `owner` must be null or an unfreed owner returned by `ibex2_sqlite_owner_create`.
#[no_mangle]
pub unsafe extern "C" fn ibex2_sqlite_owner_destroy(owner: *mut std::ffi::c_void) {
    if owner.is_null() {
        return;
    }
    let owner = Box::from_raw(owner.cast::<Owner>());
    if let Some(state) = owner.state.upgrade() {
        crate::pool::run(move || {
            if owner.kind == 0 {
                let _ = state.sqlite.close_database(owner.id);
            } else {
                state.sqlite.close_statement(owner.id);
            }
        });
    }
}
