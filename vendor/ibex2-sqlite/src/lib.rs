//! Optional native SQLite provider. The embedder owns stable database parents;
//! SQLite's maintained VFS owns journaling, locking and crash recovery.
use ibex2::{
    boundary::HostError,
    stdlib::sqlite::{self, Command, ExecuteResult, Location, Rows, StatementInfo, Value},
};
use rusqlite::{
    config::DbConfig,
    fallible_iterator::FallibleIterator,
    hooks::{AuthAction, AuthContext, Authorization},
    limits::Limit,
    types::{Value as SqlValue, ValueRef},
    Batch, Connection, OpenFlags, TransactionBehavior,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

const MAX_RESULT_BYTES: usize = 16 * 1024 * 1024;
const MAX_ROWS: usize = 100_000;
const MAX_COMMANDS: usize = 10_000;
#[derive(Default)]
pub struct SqliteProvider;
struct Session {
    connection: Mutex<Option<Connection>>,
    internal_transaction: Arc<AtomicBool>,
}
fn error(e: impl std::fmt::Display) -> HostError {
    HostError::Failed(format!("SQLite: {e}"))
}

impl sqlite::Provider for SqliteProvider {
    fn open(&self, location: Location) -> Result<Arc<dyn sqlite::Connection>, HostError> {
        let path = location.path;
        if !path.is_absolute()
            || path.to_str().is_none()
            || path.as_os_str().as_encoded_bytes().contains(&0)
        {
            return Err(HostError::InvalidArgument(
                "SQLite requires an absolute native filename".into(),
            ));
        }
        // Reject existing symlinks at every component, including the final file.
        // The host keeps parents stable after this check; this is not an OS sandbox.
        let mut ancestor = std::path::PathBuf::new();
        for part in path.components() {
            if matches!(part, std::path::Component::ParentDir) {
                return Err(error("parent traversal is not allowed"));
            }
            ancestor.push(part);
            match std::fs::symlink_metadata(&ancestor) {
                Ok(meta) if meta.file_type().is_symlink() => {
                    return Err(error("symbolic links are not allowed"))
                }
                Ok(meta) if ancestor == path && !meta.is_file() => {
                    return Err(error("database must be a regular file"));
                }
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound && ancestor == path => {}
                Err(e) => return Err(error(e)),
            }
        }
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .map_err(error)?;
        connection
            .busy_timeout(std::time::Duration::from_secs(5))
            .map_err(error)?;
        connection
            .set_db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true)
            .map_err(error)?;
        connection
            .set_db_config(DbConfig::SQLITE_DBCONFIG_TRUSTED_SCHEMA, false)
            .map_err(error)?;
        connection
            .execute_batch("PRAGMA foreign_keys=ON; PRAGMA temp_store=MEMORY;")
            .map_err(error)?;
        connection
            .set_limit(Limit::SQLITE_LIMIT_LENGTH, MAX_RESULT_BYTES as i32)
            .map_err(error)?;
        connection
            .set_limit(Limit::SQLITE_LIMIT_SQL_LENGTH, 1024 * 1024)
            .map_err(error)?;
        connection
            .set_limit(Limit::SQLITE_LIMIT_ATTACHED, 0)
            .map_err(error)?;
        let internal_transaction = Arc::new(AtomicBool::new(false));
        let internal = internal_transaction.clone();
        connection
            .authorizer(Some(move |ctx: AuthContext<'_>| match ctx.action {
                AuthAction::Attach { .. }
                | AuthAction::Detach { .. }
                | AuthAction::CreateVtable { .. }
                | AuthAction::DropVtable { .. }
                | AuthAction::Unknown { .. }
                | AuthAction::Savepoint { .. } => Authorization::Deny,
                AuthAction::Transaction { .. } if !internal.load(Ordering::Relaxed) => {
                    Authorization::Deny
                }
                AuthAction::Function { function_name }
                    if function_name.eq_ignore_ascii_case("load_extension") =>
                {
                    Authorization::Deny
                }
                AuthAction::Pragma { pragma_name, .. } => {
                    let name = pragma_name.to_ascii_lowercase();
                    if matches!(name.as_str(), "user_version" | "application_id")
                        || matches!(
                            name.as_str(),
                            "table_info"
                                | "table_xinfo"
                                | "index_list"
                                | "index_info"
                                | "foreign_key_list"
                                | "foreign_key_check"
                                | "integrity_check"
                                | "quick_check"
                        )
                    {
                        Authorization::Allow
                    } else {
                        Authorization::Deny
                    }
                }
                _ => Authorization::Allow,
            }))
            .map_err(error)?;
        Ok(Arc::new(Session {
            connection: Mutex::new(Some(connection)),
            internal_transaction,
        }))
    }
}

fn validate(conn: &Connection, sql: &str) -> Result<StatementInfo, HostError> {
    let mut batch = Batch::new(conn, sql);
    let statement = batch
        .next()
        .map_err(error)?
        .ok_or_else(|| error("expected one SQL statement"))?;
    let info = StatementInfo {
        parameter_count: statement.parameter_count(),
        columns: statement
            .column_names()
            .iter()
            .map(|s| s.to_string())
            .collect(),
    };
    if batch.next().map_err(error)?.is_some() {
        return Err(error("expected one SQL statement"));
    }
    Ok(info)
}
fn parameters(params: &[Value]) -> Result<Vec<SqlValue>, HostError> {
    let bytes = params.iter().try_fold(0usize, |size, value| {
        size.checked_add(std::mem::size_of::<Value>())?
            .checked_add(match value {
                Value::Text(v) => v.len(),
                Value::Blob(v) => v.len(),
                _ => 0,
            })
    });
    if bytes.is_none_or(|bytes| bytes > MAX_RESULT_BYTES) {
        return Err(error("parameters exceed 16 MiB limit"));
    }
    params
        .iter()
        .map(|value| {
            Ok(match value {
                Value::Null => SqlValue::Null,
                Value::Integer(v) => SqlValue::Integer(*v),
                Value::Real(v) if v.is_finite() => SqlValue::Real(*v),
                Value::Real(_) => {
                    return Err(HostError::InvalidArgument(
                        "SQLite real must be finite".into(),
                    ))
                }
                Value::Text(v) => SqlValue::Text(v.clone()),
                Value::Blob(v) => SqlValue::Blob(v.clone()),
            })
        })
        .collect()
}
fn execute(conn: &Connection, sql: &str, params: &[Value]) -> Result<ExecuteResult, HostError> {
    validate(conn, sql)?;
    let mut statement = conn.prepare_cached(sql).map_err(error)?;
    if statement.column_count() != 0 {
        return Err(error(
            "execute does not accept a statement returning rows; use query",
        ));
    }
    let changes = statement
        .execute(rusqlite::params_from_iter(parameters(params)?))
        .map_err(error)?;
    Ok(ExecuteResult {
        changes: changes as u64,
        last_insert_rowid: conn.last_insert_rowid(),
    })
}
impl Session {
    fn with<T>(&self, f: impl FnOnce(&Connection) -> Result<T, HostError>) -> Result<T, HostError> {
        let guard = self
            .connection
            .lock()
            .map_err(|_| error("connection lock poisoned"))?;
        f(guard.as_ref().ok_or_else(|| error("database is closed"))?)
    }
}
impl sqlite::Connection for Session {
    fn prepare(&self, sql: &str) -> Result<StatementInfo, HostError> {
        self.with(|conn| validate(conn, sql))
    }
    fn execute(&self, sql: &str, params: &[Value]) -> Result<ExecuteResult, HostError> {
        self.with(|conn| execute(conn, sql, params))
    }
    fn query(&self, sql: &str, params: &[Value]) -> Result<Rows, HostError> {
        self.with(|conn| {
            validate(conn, sql)?;
            let mut statement = conn.prepare_cached(sql).map_err(error)?;
            // Read-only queries prevent a result-limit failure from committing a
            // partially consumed INSERT/UPDATE ... RETURNING.
            if !statement.readonly() {
                return Err(error("query requires a read-only statement"));
            }
            let columns: Vec<String> = statement
                .column_names()
                .iter()
                .map(|s| s.to_string())
                .collect();
            let mut budget = columns.iter().map(String::len).sum::<usize>();
            let mut cursor = statement
                .query(rusqlite::params_from_iter(parameters(params)?))
                .map_err(error)?;
            let mut rows = Vec::new();
            while let Some(row) = cursor.next().map_err(error)? {
                if rows.len() == MAX_ROWS {
                    return Err(error("query result exceeds row limit"));
                }
                let mut values = Vec::with_capacity(columns.len());
                for i in 0..columns.len() {
                    let value = row.get_ref(i).map_err(error)?;
                    budget = budget.saturating_add(
                        std::mem::size_of::<Value>()
                            + match value {
                                ValueRef::Text(v) | ValueRef::Blob(v) => v.len(),
                                _ => 0,
                            },
                    );
                    if budget > MAX_RESULT_BYTES {
                        return Err(error("query result exceeds 16 MiB limit"));
                    }
                    values.push(match value {
                        ValueRef::Null => Value::Null,
                        ValueRef::Integer(v) => Value::Integer(v),
                        ValueRef::Real(v) => Value::Real(v),
                        ValueRef::Text(v) => {
                            Value::Text(std::str::from_utf8(v).map_err(error)?.to_string())
                        }
                        ValueRef::Blob(v) => Value::Blob(v.to_vec()),
                    });
                }
                rows.push(values);
            }
            Ok(Rows { columns, rows })
        })
    }
    fn transaction(&self, commands: &[Command]) -> Result<Vec<ExecuteResult>, HostError> {
        if commands.len() > MAX_COMMANDS {
            return Err(error("transaction exceeds 10000 commands"));
        }
        let mut guard = self
            .connection
            .lock()
            .map_err(|_| error("connection lock poisoned"))?;
        let conn = guard.as_mut().ok_or_else(|| error("database is closed"))?;
        self.internal_transaction.store(true, Ordering::Relaxed);
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate);
        self.internal_transaction.store(false, Ordering::Relaxed);
        let transaction = transaction.map_err(error)?;
        let results: Result<Vec<_>, _> = commands
            .iter()
            .map(|command| execute(&transaction, &command.sql, &command.params))
            .collect();
        self.internal_transaction.store(true, Ordering::Relaxed);
        let outcome = match results {
            Ok(values) => transaction.commit().map(|()| values).map_err(error),
            Err(e) => {
                drop(transaction);
                Err(e)
            }
        };
        self.internal_transaction.store(false, Ordering::Relaxed);
        outcome
    }
    fn close(&self) -> Result<(), HostError> {
        let mut guard = self
            .connection
            .lock()
            .map_err(|_| error("connection lock poisoned"))?;
        if let Some(conn) = guard.take() {
            if let Err((conn, e)) = conn.close() {
                *guard = Some(conn);
                return Err(error(e));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
