//! Executor-independent SQLite contracts. Embedders supply a separate provider.
use crate::boundary::HostError;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}
#[derive(Debug, Clone, PartialEq)]
pub struct Rows {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecuteResult {
    pub changes: u64,
    pub last_insert_rowid: i64,
}
#[derive(Debug, Clone)]
pub struct Command {
    pub sql: String,
    pub params: Vec<Value>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatementInfo {
    pub parameter_count: usize,
    pub columns: Vec<String>,
}

/// An admitted native filename. The host keeps its parent directories stable
/// while open; SQLite owns its native journal and locking protocol.
pub struct Location {
    pub path: PathBuf,
}
pub trait Provider: Send + Sync {
    fn open(&self, location: Location) -> Result<Arc<dyn Connection>, HostError>;
}
pub trait Connection: Send + Sync {
    fn prepare(&self, sql: &str) -> Result<StatementInfo, HostError>;
    fn execute(&self, sql: &str, params: &[Value]) -> Result<ExecuteResult, HostError>;
    fn query(&self, sql: &str, params: &[Value]) -> Result<Rows, HostError>;
    fn transaction(&self, commands: &[Command]) -> Result<Vec<ExecuteResult>, HostError>;
    fn close(&self) -> Result<(), HostError>;
}

#[derive(Clone)]
pub struct Database {
    connection: Arc<dyn Connection>,
}
impl Database {
    pub fn new(connection: Arc<dyn Connection>) -> Self {
        Self { connection }
    }
    pub fn prepare(&self, sql: &str) -> Result<Statement, HostError> {
        let info = self.connection.prepare(sql)?;
        Ok(Statement {
            database: self.clone(),
            sql: sql.into(),
            info,
            closed: AtomicBool::new(false),
        })
    }
    pub fn execute(&self, sql: &str, params: &[Value]) -> Result<ExecuteResult, HostError> {
        self.connection.execute(sql, params)
    }
    pub fn query(&self, sql: &str, params: &[Value]) -> Result<Rows, HostError> {
        self.connection.query(sql, params)
    }
    pub fn transaction(&self, commands: &[Command]) -> Result<Vec<ExecuteResult>, HostError> {
        self.connection.transaction(commands)
    }
    pub fn close(&self) -> Result<(), HostError> {
        self.connection.close()
    }
}
/// An owned prepared operation. The provider caches the native plan; closing
/// the database invalidates all its statements, including outstanding clones.
pub struct Statement {
    database: Database,
    sql: String,
    pub info: StatementInfo,
    closed: AtomicBool,
}
impl Statement {
    fn check(&self) -> Result<(), HostError> {
        if self.closed.load(Ordering::Acquire) {
            Err(HostError::Failed("SQLite statement is closed".into()))
        } else {
            Ok(())
        }
    }
    pub fn execute(&self, params: &[Value]) -> Result<ExecuteResult, HostError> {
        self.check()?;
        self.database.execute(&self.sql, params)
    }
    pub fn query(&self, params: &[Value]) -> Result<Rows, HostError> {
        self.check()?;
        self.database.query(&self.sql, params)
    }
    pub fn close(&self) {
        self.closed.store(true, Ordering::Release);
    }
}
