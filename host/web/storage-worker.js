// @ref LLP 1027 D10 — SQLite runs off the presentation thread.
import sqlite3InitModule from './sqlite3.mjs';
import { createFileStore, lockPaths } from './storage-fs.js';

// No shared memory or isolation headers: keep the database in WASM memory and
// atomically publish its complete SQLite file after each successful mutation.
// This costs O(database size) per write; a connection exclusively locks its file.
const ready = sqlite3InitModule({ locateFile: file => new URL(file, import.meta.url).href });
const databases = new Map();
let nextDatabase = 0, nextStatement = 0, queue = Promise.resolve();
const limit = 16 * 1024 * 1024;
const fail = message => { throw Object.assign(new Error(message), { kind: 'Unavailable' }); };
const allowedPragmas = new Set(['user_version', 'application_id', 'table_info', 'table_xinfo',
  'index_list', 'index_info', 'foreign_key_list', 'foreign_key_check', 'integrity_check', 'quick_check']);

function configure(sqlite, session) {
  const { capi: c } = sqlite, db = session.db;
  db.checkRc(c.sqlite3_db_config(db.pointer, c.SQLITE_DBCONFIG_DEFENSIVE, 1, 0));
  db.checkRc(c.sqlite3_db_config(db.pointer, c.SQLITE_DBCONFIG_TRUSTED_SCHEMA, 0, 0));
  db.exec('PRAGMA foreign_keys=ON; PRAGMA temp_store=MEMORY;');
  c.sqlite3_limit(db.pointer, c.SQLITE_LIMIT_LENGTH, limit);
  c.sqlite3_limit(db.pointer, c.SQLITE_LIMIT_SQL_LENGTH, 1024 * 1024);
  c.sqlite3_limit(db.pointer, c.SQLITE_LIMIT_ATTACHED, 0);
  const denied = new Set([c.SQLITE_ATTACH, c.SQLITE_DETACH, c.SQLITE_CREATE_VTABLE,
    c.SQLITE_DROP_VTABLE, c.SQLITE_SAVEPOINT]);
  db.checkRc(c.sqlite3_set_authorizer(db.pointer, (_unused, action, first, second) => {
    if (denied.has(action) || (action === c.SQLITE_TRANSACTION && !session.internal)) return c.SQLITE_DENY;
    if (action === c.SQLITE_FUNCTION && String(second).toLowerCase() === 'load_extension') return c.SQLITE_DENY;
    if (action === c.SQLITE_PRAGMA && !allowedPragmas.has(String(first).toLowerCase())
      && !(session.serializing && first === 'page_count')) return c.SQLITE_DENY;
    return c.SQLITE_OK;
  }, 0));
}

function validate(sqlite, session, sql) {
  if (typeof sql !== 'string' || sql.includes('\0') || new TextEncoder().encode(sql).length > 1024 * 1024) fail('invalid SQL string');
  const { wasm: w, capi: c } = sqlite;
  const scope = w.scopedAllocPush();
  let count = 0;
  try {
    let cursor = w.scopedAllocCString(sql);
    const [statement, tail] = w.scopedAllocPtr(2);
    while (cursor && w.peek8(cursor)) {
      w.pokePtr(statement, 0);
      const rc = c.sqlite3_prepare_v3(session.db.pointer, cursor, -1, 0, statement, tail);
      const pointer = w.peekPtr(statement);
      if (pointer) { c.sqlite3_finalize(pointer); count++; }
      session.db.checkRc(rc);
      cursor = w.peekPtr(tail);
    }
    if (count !== 1) fail('expected one SQL statement');
  } finally { w.scopedAllocPop(scope); }
}

function params(values) {
  if (!Array.isArray(values)) fail('SQLite parameters must be an array');
  let budget = 0;
  for (const value of values) {
    budget += 24;
    if (typeof value === 'number') {
      if (!Number.isFinite(value) || (Number.isInteger(value) && !Number.isSafeInteger(value))) fail('SQLite number must be finite and integral numbers safe');
    } else if (typeof value === 'bigint') {
      if (value < -(1n << 63n) || value >= 1n << 63n) fail('SQLite integer is outside signed 64-bit range');
    } else if (typeof value === 'string') budget += new TextEncoder().encode(value).length;
    else if (value instanceof Uint8Array) budget += value.byteLength;
    else if (value !== null) fail('invalid SQLite parameter');
    if (budget > limit) fail('SQLite parameters exceed 16 MiB limit');
  }
  return values;
}

function run(sqlite, session, sql, values, query) {
  validate(sqlite, session, sql);
  const statement = session.db.prepare(sql), c = sqlite.capi;
  try {
    values = params(values);
    if (values.length !== statement.parameterCount) fail('SQLite parameter count mismatch');
    if (query && !c.sqlite3_stmt_readonly(statement.pointer)) fail('query requires a read-only statement');
    if (!query && statement.columnCount) fail('execute does not accept a statement returning rows; use query');
    if (values.length) statement.bind(values);
    if (!query) {
      statement.step();
      return { changes: c.sqlite3_changes(session.db.pointer), lastInsertRowid: c.sqlite3_last_insert_rowid(session.db.pointer) };
    }
    const columns = statement.getColumnNames([]), rows = [];
    let budget = columns.reduce((n, s) => n + new TextEncoder().encode(s).length, 0);
    while (statement.step()) {
      if (rows.length >= 100000) fail('query result exceeds row limit');
      const row = [];
      for (let i = 0; i < columns.length; i++) {
        const value = c.sqlite3_column_type(statement.pointer, i) === c.SQLITE_INTEGER
          ? c.sqlite3_column_int64(statement.pointer, i) : statement.get(i);
        budget += 24 + (typeof value === 'string' ? new TextEncoder().encode(value).length : value?.byteLength || 0);
        if (budget > limit) fail('query result exceeds 16 MiB limit');
        row.push(value);
      }
      rows.push(row);
    }
    return { columns, rows };
  } finally { statement.finalize(); }
}

function internal(session, sql) {
  session.internal = true;
  try { session.db.exec(sql); } finally { session.internal = false; }
}
async function close(id) {
  const session = databases.get(id);
  if (!session) return;
  databases.delete(id);
  try { session.db.close(); } finally { session.files.close(); await session.release(); }
}
async function persist(sqlite, id, session) {
  try {
    let bytes;
    session.serializing = true;
    try {
      // The official exporter itself executes page_count. Only this synchronous
      // trusted section may use it; app SQL cannot widen the pragma allowlist.
      bytes = session.db.selectValue('PRAGMA page_count')
        ? sqlite.capi.sqlite3_js_db_export(session.db.pointer) : new Uint8Array();
    } finally { session.serializing = false; }
    await session.files.atomicWriteFile(session.path, bytes);
  }
  catch (error) {
    // Memory already committed. Invalidate every handle rather than exposing a
    // successful-looking connection whose state differs from the persisted file.
    await close(id);
    throw error;
  }
}

async function dispatch({ appId, op, args }) {
  const sqlite = await ready;
  if (op === 'open') {
    const path = args[0], release = await lockPaths(appId, [path]);
    const files = createFileStore(appId);
    const session = { path, release, files, db: new sqlite.oo1.DB(':memory:'), statements: new Map(), internal: false };
    try {
      let bytes;
      try { bytes = new Uint8Array(await files.readFile(path)); }
      catch (error) { if (error.code !== 'ENOENT') throw error; }
      if (bytes?.length) {
        const pointer = sqlite.wasm.allocFromTypedArray(bytes);
        session.db.checkRc(sqlite.capi.sqlite3_deserialize(session.db.pointer, 'main', pointer,
          BigInt(bytes.length), BigInt(bytes.length), sqlite.capi.SQLITE_DESERIALIZE_FREEONCLOSE | sqlite.capi.SQLITE_DESERIALIZE_RESIZEABLE));
      }
      configure(sqlite, session);
      const id = ++nextDatabase;
      databases.set(id, session);
      if (!bytes) await persist(sqlite, id, session);
      return id;
    } catch (error) {
      for (const [id, candidate] of databases) if (candidate === session) databases.delete(id);
      session.db.close(); files.close(); await release(); throw error;
    }
  }
  const [id, ...rest] = args;
  if (op === 'close') return close(id);
  if (op === 'statementClose') { databases.get(id)?.statements.delete(rest[0]); return; }
  const session = databases.get(id);
  if (!session) fail('database is closed');
  if (op === 'prepare') {
    validate(sqlite, session, rest[0]);
    const statement = ++nextStatement;
    session.statements.set(statement, rest[0]);
    return statement;
  }
  if (op === 'transaction') {
    const commands = rest[0];
    if (!Array.isArray(commands) || commands.length > 10000) fail('transaction exceeds 10000 commands or is not an array');
    internal(session, 'BEGIN IMMEDIATE');
    let results;
    try {
      results = commands.map(command => run(sqlite, session, command.sql, command.params ?? [], false));
      internal(session, 'COMMIT');
    } catch (error) { internal(session, 'ROLLBACK'); throw error; }
    await persist(sqlite, id, session);
    return results;
  }
  let sql = rest[0], values = rest[1];
  if (op.startsWith('statement')) {
    sql = session.statements.get(rest[0]);
    if (sql === undefined) fail('statement is closed');
  }
  const query = op === 'query' || op === 'statementQuery';
  const result = run(sqlite, session, sql, values, query);
  if (!query) await persist(sqlite, id, session);
  return result;
}

onmessage = ({ data }) => {
  queue = queue.then(async () => {
    try { postMessage({ id: data.id, value: await dispatch(data) }); }
    catch (error) { postMessage({ id: data.id, error: { kind: 'Unavailable', code: error.code, message: String(error.message || error) } }); }
  });
};
