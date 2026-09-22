// SQLite authority is carried by module parameters and opaque database objects.
// @ref LLP 0067#3-the-check — handles stay inside bindings; handoff carries authority
(function (field, retain) {
  "use strict";
  var databases = new WeakMap(), statements = new WeakMap();
  function own(map, value, name) {
    var state = map.get(value);
    if (!state) throw new TypeError("not a SQLite " + name);
    return state;
  }
  function text(value, name) {
    if (typeof value !== "string") throw new TypeError(name + " must be a string");
    return value;
  }
  function parameters(values) {
    if (values === undefined) values = [];
    if (!Array.isArray(values)) throw new TypeError("SQLite parameters must be an array");
    var out = [];
    for (var index = 0; index < values.length; index++) {
      var value = values[index];
      if (value === null) out.push(0, undefined);
      else if (typeof value === "bigint") {
        if (value < BigInt("-9223372036854775808") || value > BigInt("9223372036854775807"))
          throw new RangeError("SQLite integer exceeds signed 64 bits");
        out.push(1, String(value));
      } else if (typeof value === "number") {
        if (!Number.isFinite(value)) throw new TypeError("SQLite numbers must be finite");
        if (Number.isInteger(value)) {
          if (!Number.isSafeInteger(value)) throw new RangeError("use BigInt for SQLite integers outside the safe range");
          out.push(1, String(value));
        } else out.push(2, value);
      } else if (typeof value === "string") out.push(3, value);
      else if (value instanceof Uint8Array) out.push(4, new Uint8Array(value));
      else throw new TypeError("unsupported SQLite parameter");
    }
    return out;
  }
  function queued(state, work, owner) {
    if (state.closed) return Promise.reject(new Error("SQLite database is closed"));
    var result = state.pending.then(work).then(function (value) {
      // Keep the JS owner (and its native finalizer) alive until settlement.
      if (!databases.has(owner) && !statements.has(owner)) throw new TypeError("invalid SQLite owner");
      return value;
    }, function (error) {
      if (!databases.has(owner) && !statements.has(owner)) throw new TypeError("invalid SQLite owner");
      throw error;
    });
    state.pending = result.catch(function () {});
    return result;
  }
  function result(handle) {
    function execution(index) {
      return { changes: field(handle, 6, index), lastInsertRowid: BigInt(field(handle, 7, index)) };
    }
    try {
      var kind = field(handle, 0);
      if (kind === 0) return execution(0);
      var count = field(handle, 1), rows = [], columns = [], i, j;
      if (kind === 2) {
        for (i = 0; i < count; i++) rows.push(execution(i));
        return rows;
      }
      for (i = 0; i < field(handle, 2); i++) columns.push(field(handle, 3, i));
      for (i = 0; i < count; i++) {
        var row = [];
        for (j = 0; j < columns.length; j++) {
          var type = field(handle, 4, i, j);
          var value = type === 0 ? null : field(handle, 5, i, j);
          row.push(type === 1 ? BigInt(value) : type === 4 ? new Uint8Array(value) : value);
        }
        rows.push(row);
      }
      return { columns: columns, rows: rows };
    } finally { field(handle, 8); }
  }
  function Database() { throw new TypeError("SQLite databases come from sqlite.open"); }
  function Statement() { throw new TypeError("SQLite statements come from database.prepare"); }
  Database.prototype.execute = function (sql, values) {
    var state = own(databases, this, "database");
    try {
      var args = [state.handle, text(sql, "SQL")].concat(parameters(values));
      return queued(state, function () { return state.raw.execute.apply(undefined, args).then(result); }, this);
    } catch (error) { return Promise.reject(error); }
  };
  Database.prototype.query = function (sql, values) {
    var state = own(databases, this, "database");
    try {
      var args = [state.handle, text(sql, "SQL")].concat(parameters(values));
      return queued(state, function () { return state.raw.query.apply(undefined, args).then(result); }, this);
    } catch (error) { return Promise.reject(error); }
  };
  Database.prototype.prepare = function (sql) {
    var database = this, state = own(databases, this, "database");
    try { sql = text(sql, "SQL"); } catch (error) { return Promise.reject(error); }
    return queued(state, function () { return state.raw.prepare(state.handle, sql).then(function (handle) {
      var statement = Object.create(Statement.prototype);
      statements.set(statement, { handle: handle, database: database, closed: false });
      retain(handle, 1, statement);
      return statement;
    }); }, database);
  };
  Database.prototype.transaction = function (batch) {
    var state = own(databases, this, "database");
    try {
      if (!Array.isArray(batch)) throw new TypeError("SQLite transaction needs a statement batch");
      var args = [state.handle, batch.length];
      for (var i = 0; i < batch.length; i++) {
        var entry = batch[i];
        if (!entry || typeof entry !== "object") throw new TypeError("invalid SQLite batch entry");
        var params = parameters(entry.params);
        args.push(text(entry.sql, "SQL"), params.length / 2);
        args = args.concat(params);
      }
      return queued(state, function () { return state.raw.transaction.apply(undefined, args).then(result); }, this);
    } catch (error) { return Promise.reject(error); }
  };
  Database.prototype.close = function () {
    var state = own(databases, this, "database");
    if (state.closed) return state.closing;
    state.closing = queued(state, function () { return state.raw.close(state.handle); }, this);
    state.closed = true;
    return state.closing;
  };
  function statementCall(statement, values, method) {
    var statementState = own(statements, statement, "statement");
    if (statementState.closed) return Promise.reject(new Error("SQLite statement is closed"));
    var state = own(databases, statementState.database, "database");
    try {
      var args = [statementState.handle].concat(parameters(values));
      return queued(state, function () { return state.raw[method].apply(undefined, args).then(result); }, statement);
    } catch (error) { return Promise.reject(error); }
  }
  Statement.prototype.execute = function (values) { return statementCall(this, values, "statementExecute"); };
  Statement.prototype.query = function (values) { return statementCall(this, values, "statementQuery"); };
  Statement.prototype.close = function () {
    var statement = own(statements, this, "statement");
    if (statement.closed) return statement.closing;
    var state = own(databases, statement.database, "database");
    statement.closed = true;
    statement.closing = state.closed ? state.closing : queued(state, function () {
      return state.raw.statementClose(statement.handle);
    }, this);
    return statement.closing;
  };
  [Database, Statement].forEach(function (constructor) {
    Object.getOwnPropertyNames(constructor.prototype).forEach(function (name) {
      var value = constructor.prototype[name];
      if (typeof value === "function") Object.freeze(value);
    });
    Object.freeze(constructor.prototype); Object.freeze(constructor);
  });
  return function (raw) {
    return Object.freeze({ open: Object.freeze(function (path) {
      try { path = text(path, "database path"); } catch (error) { return Promise.reject(error); }
      return raw.open(path).then(function (handle) {
        var database = Object.create(Database.prototype);
        databases.set(database, { handle: handle, raw: raw, pending: Promise.resolve(), closed: false });
        retain(handle, 0, database);
        return database;
      });
    }) });
  };
});
