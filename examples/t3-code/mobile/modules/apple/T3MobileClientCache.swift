// @ref examples/t3-code/mobile/docs/1130.009-mobile-settings.decision.md#information-sources
// Disposable app read cache. Preferences, credentials and delivery journals never enter this owner.
import Foundation
import CoreFoundation
import CryptoKit
import Darwin
import SQLite3

final class T3MobileClientCache {
    typealias Object = [String: Any]
    private static let kinds = Set(["shell", "thread", "server-config", "vcs-refs", "project-favicon"])
    private let directory: URL
    private let queue = DispatchQueue(label: "com.exact.t3code.client-cache")
    private var database: OpaquePointer?
    private let transient = unsafeBitCast(-1, to: sqlite3_destructor_type.self)

    init(directory: URL) { self.directory = directory }
    deinit { if let database { sqlite3_close(database) } }

    func request(_ value: Object) throws -> Object {
        try queue.sync {
            try open()
            return try transaction { try perform(value) }
        }
    }

    private static func fail(_ message: String) -> NSError {
        NSError(domain: "T3MobileClientCache", code: 1, userInfo: [NSLocalizedDescriptionKey: message])
    }

    private func sqlFailure() -> NSError {
        // SQL values and paths do not appear in errors returned to the app.
        Self.fail("Client cache storage failed (SQLite \(sqlite3_errcode(database))).")
    }

    private func safePath(_ url: URL, directory expectedDirectory: Bool) throws {
        var isDirectory: ObjCBool = false
        let manager = FileManager.default
        // attributesOfItem detects dangling links, unlike fileExists.
        if let attributes = try? manager.attributesOfItem(atPath: url.path) {
            guard attributes[.type] as? FileAttributeType != .typeSymbolicLink,
                  manager.fileExists(atPath: url.path, isDirectory: &isDirectory),
                  isDirectory.boolValue == expectedDirectory else {
                throw Self.fail("Client cache storage has an unsafe path.")
            }
        }
    }

    private func open() throws {
        guard database == nil else { return }
        guard directory.isFileURL else { throw Self.fail("Client cache needs a local directory.") }
        try safePath(directory, directory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        // The app data root is supplied by the module, and macOS /var is a system
        // symlink. Canonicalize that trusted parent, never the owned cache leaf.
        guard let resolved = realpath(directory.deletingLastPathComponent().path, nil) else {
            throw Self.fail("Client cache parent directory could not be resolved.")
        }
        let parent = String(cString: resolved)
        free(resolved)
        let canonicalDirectory = URL(fileURLWithPath: parent, isDirectory: true)
            .appendingPathComponent(directory.lastPathComponent, isDirectory: true)
        let file = canonicalDirectory.appendingPathComponent("cache.sqlite3")
        for suffix in ["", "-wal", "-shm", "-journal"] {
            try safePath(URL(fileURLWithPath: file.path + suffix), directory: false)
        }
        var handle: OpaquePointer?
        let flags = SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE | SQLITE_OPEN_FULLMUTEX | SQLITE_OPEN_NOFOLLOW
        guard sqlite3_open_v2(file.path, &handle, flags, nil) == SQLITE_OK else {
            if let handle { sqlite3_close(handle) }
            throw Self.fail("Client cache storage could not be opened.")
        }
        database = handle
        do {
            guard sqlite3_busy_timeout(database, 2_000) == SQLITE_OK else { throw sqlFailure() }
            try execute("PRAGMA journal_mode=WAL")
            try execute("PRAGMA synchronous=FULL")
            try execute("CREATE TABLE IF NOT EXISTS client_cache (environment_id TEXT NOT NULL, kind TEXT NOT NULL, cache_key TEXT NOT NULL, schema_version INTEGER NOT NULL, payload TEXT NOT NULL, updated_at INTEGER NOT NULL, PRIMARY KEY(environment_id, kind, cache_key))")
            try execute("CREATE TABLE IF NOT EXISTS cache_epochs (scope TEXT PRIMARY KEY NOT NULL, token TEXT NOT NULL)")
            try execute("INSERT OR IGNORE INTO cache_epochs(scope,token) VALUES(?,?)", [scope([]), UUID().uuidString])
            try execute("CREATE TABLE IF NOT EXISTS thread_history (environment_id TEXT NOT NULL, thread_id TEXT NOT NULL, item_key TEXT NOT NULL, ordinal INTEGER NOT NULL, payload TEXT NOT NULL, PRIMARY KEY(environment_id, thread_id, item_key))")
            try execute("CREATE INDEX IF NOT EXISTS thread_history_order ON thread_history(environment_id, thread_id, ordinal, item_key)")
            try execute("CREATE TABLE IF NOT EXISTS thread_history_meta (environment_id TEXT NOT NULL, thread_id TEXT NOT NULL, revision INTEGER NOT NULL, before_cursor TEXT, after_cursor TEXT, has_more INTEGER NOT NULL, PRIMARY KEY(environment_id, thread_id))")
        } catch {
            sqlite3_close(database)
            database = nil
            throw error
        }
    }

    private func transaction<T>(_ run: () throws -> T) throws -> T {
        try execute("BEGIN IMMEDIATE")
        do {
            let result = try run()
            try execute("COMMIT")
            return result
        } catch {
            try? execute("ROLLBACK")
            throw error
        }
    }

    private func statement(_ sql: String, _ values: [Any] = []) throws -> OpaquePointer {
        var pointer: OpaquePointer?
        guard sqlite3_prepare_v2(database, sql, -1, &pointer, nil) == SQLITE_OK, let pointer else { throw sqlFailure() }
        do {
            for (index, value) in values.enumerated() {
                let status: Int32
                if value is NSNull {
                    status = sqlite3_bind_null(pointer, Int32(index + 1))
                } else if let value = value as? String {
                    status = value.withCString { sqlite3_bind_text(pointer, Int32(index + 1), $0, Int32(value.utf8.count), transient) }
                } else if let value = value as? Int64 {
                    status = sqlite3_bind_int64(pointer, Int32(index + 1), value)
                } else { throw Self.fail("Invalid client cache SQL parameter.") }
                guard status == SQLITE_OK else { throw sqlFailure() }
            }
            return pointer
        } catch {
            sqlite3_finalize(pointer)
            throw error
        }
    }

    private func execute(_ sql: String, _ values: [Any] = []) throws {
        let pointer = try statement(sql, values)
        defer { sqlite3_finalize(pointer) }
        while true {
            switch sqlite3_step(pointer) {
            case SQLITE_DONE: return
            case SQLITE_ROW: continue
            default: throw sqlFailure()
            }
        }
    }

    private func text(_ pointer: OpaquePointer, _ column: Int32) -> String {
        guard let bytes = sqlite3_column_text(pointer, column) else { return "" }
        return String(decoding: UnsafeBufferPointer(start: bytes, count: Int(sqlite3_column_bytes(pointer, column))), as: UTF8.self)
    }

    private func string(_ value: Object, _ name: String, limit: Int = 8_192) throws -> String {
        guard let result = value[name] as? String, !result.isEmpty, result.utf8.count <= limit,
              !result.contains("\0") else { throw Self.fail("Invalid client cache \(name).") }
        return result
    }

    private func identity(_ value: Object) throws -> [String] {
        let environment = try string(value, "environmentId", limit: 512)
        let kind = try string(value, "kind", limit: 32)
        guard Self.kinds.contains(kind) else { throw Self.fail("Unknown client cache kind.") }
        let key = try string(value, "key")
        guard kind != "shell" || key == "snapshot", kind != "server-config" || key == "config" else {
            throw Self.fail("Invalid client cache key.")
        }
        return [environment, kind, key]
    }

    private func scope(_ parts: [String]) -> String {
        // JSON arrays prevent environment/kind/key separator collisions.
        String(data: try! JSONSerialization.data(withJSONObject: parts, options: [.fragmentsAllowed]), encoding: .utf8)!
    }

    private func ticket(_ identity: [String]) throws -> String {
        var tokens = [String]()
        // Separate namespace from JSON-array scopes. A kind-wide clear must also
        // reject writers whose environment has no persisted rows yet.
        let kindPointer = try statement("SELECT token FROM cache_epochs WHERE scope=?", ["kind:" + identity[1]])
        defer { sqlite3_finalize(kindPointer) }
        let kindStatus = sqlite3_step(kindPointer)
        guard kindStatus == SQLITE_ROW || kindStatus == SQLITE_DONE else { throw sqlFailure() }
        tokens.append(kindStatus == SQLITE_ROW ? text(kindPointer, 0) : "")
        for count in 0...identity.count {
            let pointer = try statement("SELECT token FROM cache_epochs WHERE scope=?", [scope(Array(identity.prefix(count)))])
            defer { sqlite3_finalize(pointer) }
            let status = sqlite3_step(pointer)
            guard status == SQLITE_ROW || status == SQLITE_DONE else { throw sqlFailure() }
            tokens.append(status == SQLITE_ROW ? text(pointer, 0) : "")
        }
        let identityDigest = SHA256.hash(data: Data(scope(identity).utf8)).map { String(format: "%02x", $0) }.joined()
        return ([identityDigest] + tokens).joined(separator: "|")
    }

    private func invalidate(_ parts: [String]) throws {
        try execute("INSERT INTO cache_epochs(scope,token) VALUES(?,?) ON CONFLICT(scope) DO UPDATE SET token=excluded.token", [scope(parts), UUID().uuidString])
    }

    private func validPayload(_ payload: String, identity: [String], version: Int64) -> Bool {
        let maximum = identity[1] == "thread" ? 16 * 1_024 * 1_024 : 4 * 1_024 * 1_024
        guard version == 1, payload.utf8.count <= maximum,
              let data = payload.data(using: .utf8),
              let object = try? JSONSerialization.jsonObject(with: data) as? Object,
              object["environmentId"] as? String == identity[0] else { return false }
        // These are explicitly app codecs, not upstream schema3 snapshots.
        return true
    }

    private func historyIdentity(_ value: Object) throws -> [String] {
        let environment = try string(value, "environmentId", limit: 512)
        let thread = try string(value, "threadId", limit: 8_192)
        return [environment, thread]
    }

    private func historyScope(_ identity: [String]) -> String { scope(["history", identity[0], identity[1]]) }

    private func historyTicket(_ identity: [String]) throws -> String {
        var tokens = [String](), scopes = ["kind:thread", scope([]), scope([identity[0]]), scope([identity[0], "thread"]), historyScope(identity)]
        for value in scopes {
            let pointer = try statement("SELECT token FROM cache_epochs WHERE scope=?", [value])
            defer { sqlite3_finalize(pointer) }
            let status = sqlite3_step(pointer)
            guard status == SQLITE_ROW || status == SQLITE_DONE else { throw sqlFailure() }
            tokens.append(status == SQLITE_ROW ? text(pointer, 0) : "")
        }
        let digest = SHA256.hash(data: Data(historyScope(identity).utf8)).map { String(format: "%02x", $0) }.joined()
        return ([digest] + tokens).joined(separator: "|")
    }

    private func historyCursor(_ value: Object, _ name: String) throws -> String? {
        guard let raw = value[name] else { throw Self.fail("Invalid client history cursor.") }
        if raw is NSNull { return nil }
        guard let cursor = raw as? String, !cursor.isEmpty, cursor.utf8.count <= 8_192, !cursor.contains("\0") else {
            throw Self.fail("Invalid client history cursor.")
        }
        return cursor
    }

    private func historyItem(_ value: Any) throws -> (String, Int64, String) {
        guard let item = value as? Object,
              let key = item["key"] as? String, !key.isEmpty, key.utf8.count <= 512, !key.contains("\0"),
              let ordinal = item["ordinal"] as? NSNumber, CFGetTypeID(ordinal) != CFBooleanGetTypeID(),
              ordinal.doubleValue.isFinite, ordinal.doubleValue.rounded() == ordinal.doubleValue, ordinal.doubleValue >= 0,
              ordinal.doubleValue <= Double(Int64.max),
              let payload = item["payload"] as? String, !payload.isEmpty, payload.utf8.count <= 256 * 1_024 else {
            throw Self.fail("Invalid client history item.")
        }
        guard let data = payload.data(using: .utf8), let object = try? JSONSerialization.jsonObject(with: data), object is Object else {
            throw Self.fail("Invalid client history item payload.")
        }
        return (key, ordinal.int64Value, payload)
    }

    private func historyAnchor(_ value: Any) throws -> (String, Int64) {
        guard let item = value as? Object,
              let key = item["key"] as? String, !key.isEmpty, key.utf8.count <= 512, !key.contains("\0"),
              let ordinal = item["ordinal"] as? NSNumber, CFGetTypeID(ordinal) != CFBooleanGetTypeID(),
              ordinal.doubleValue.isFinite, ordinal.doubleValue.rounded() == ordinal.doubleValue, ordinal.doubleValue >= 0,
              ordinal.doubleValue <= Double(Int64.max) else {
            throw Self.fail("Invalid client history anchor.")
        }
        return (key, ordinal.int64Value)
    }

    private func historyRead(_ value: Object) throws -> Object {
        let identity = try historyIdentity(value)
        let direction = try string(value, "direction", limit: 8)
        guard direction == "older" || direction == "newer" else { throw Self.fail("Invalid client history direction.") }
        guard let limit = value["limit"] as? NSNumber, CFGetTypeID(limit) != CFBooleanGetTypeID(),
              limit.doubleValue.rounded() == limit.doubleValue, (1...128).contains(limit.doubleValue) else {
            throw Self.fail("Invalid client history page.")
        }
        var anchor: (String, Int64)?
        if let raw = value["anchor"], !(raw is NSNull) {
            anchor = try historyAnchor(raw)
        }
        var sql = "SELECT item_key,ordinal,payload FROM thread_history WHERE environment_id=? AND thread_id=?"
        var params: [Any] = identity
        if let anchor {
            sql += direction == "older" ? " AND (ordinal,item_key COLLATE BINARY)<(?,?)" : " AND (ordinal,item_key COLLATE BINARY)>(?,?)"
            params += [anchor.1, anchor.0]
        }
        sql += direction == "older" ? " ORDER BY ordinal DESC,item_key COLLATE BINARY DESC LIMIT ?" : " ORDER BY ordinal,item_key COLLATE BINARY LIMIT ?"
        params.append(limit.int64Value)
        let pointer = try statement(sql, params)
        defer { sqlite3_finalize(pointer) }
        var rows = [Object](), status: Int32
        while true {
            status = sqlite3_step(pointer)
            if status == SQLITE_DONE { break }
            guard status == SQLITE_ROW else { throw sqlFailure() }
            rows.append(["key": text(pointer, 0), "ordinal": sqlite3_column_int64(pointer, 1), "payload": text(pointer, 2)])
        }
        if direction == "older" { rows.reverse() }
        let meta = try statement("SELECT revision,before_cursor,after_cursor,has_more FROM thread_history_meta WHERE environment_id=? AND thread_id=?", identity)
        defer { sqlite3_finalize(meta) }
        let metaStatus = sqlite3_step(meta)
        guard metaStatus == SQLITE_ROW || metaStatus == SQLITE_DONE else { throw sqlFailure() }
        guard metaStatus == SQLITE_ROW else { return ["revision": 0, "beforeCursor": NSNull(), "afterCursor": NSNull(), "hasMore": false, "items": rows] }
        return ["revision": sqlite3_column_int64(meta, 0), "beforeCursor": sqlite3_column_type(meta, 1) == SQLITE_NULL ? NSNull() : text(meta, 1),
                "afterCursor": sqlite3_column_type(meta, 2) == SQLITE_NULL ? NSNull() : text(meta, 2),
                "hasMore": sqlite3_column_int(meta, 3) != 0, "items": rows]
    }

    private func perform(_ value: Object) throws -> Object {
        switch try string(value, "action", limit: 32) {
        case "historyTicket": return ["token": try historyTicket(try historyIdentity(value))]
        case "historyCommit":
            let identity = try historyIdentity(value), supplied = try string(value, "token", limit: 256)
            guard supplied == (try historyTicket(identity)) else { return ["committed": false, "stale": true] }
            guard let revision = value["revision"] as? NSNumber, CFGetTypeID(revision) != CFBooleanGetTypeID(),
                  revision.doubleValue.isFinite, revision.doubleValue.rounded() == revision.doubleValue, revision.doubleValue >= 0,
                  revision.doubleValue <= Double(Int64.max), let hasMore = value["hasMore"] as? Bool,
                  let rawItems = value["items"] as? [Any], rawItems.count <= 128 else { throw Self.fail("Invalid client history commit.") }
            let before = try historyCursor(value, "beforeCursor"), after = try historyCursor(value, "afterCursor")
            guard JSONSerialization.isValidJSONObject(value), try JSONSerialization.data(withJSONObject: value).count <= 256 * 1_024 else {
                throw Self.fail("Client history commit is too large.")
            }
            var seen = Set<String>(), parsed = [(String, Int64, String)](), previous: (Int64, String)?
            for raw in rawItems {
                let row = try historyItem(raw)
                guard seen.insert(row.0).inserted, previous == nil || row.1 > previous!.0 || row.1 == previous!.0 && row.0 > previous!.1 else {
                    throw Self.fail("Duplicate or unordered client history item.")
                }
                parsed.append(row)
                previous = (row.1, row.0)
            }
            let current = try statement("SELECT revision FROM thread_history_meta WHERE environment_id=? AND thread_id=?", identity)
            defer { sqlite3_finalize(current) }
            let currentStatus = sqlite3_step(current)
            guard currentStatus == SQLITE_ROW || currentStatus == SQLITE_DONE else { throw sqlFailure() }
            if currentStatus == SQLITE_ROW && revision.int64Value < sqlite3_column_int64(current, 0) {
                return ["committed": false, "stale": true]
            }
            for row in parsed {
                try execute("INSERT INTO thread_history(environment_id,thread_id,item_key,ordinal,payload) VALUES(?,?,?,?,?) ON CONFLICT(environment_id,thread_id,item_key) DO UPDATE SET ordinal=excluded.ordinal,payload=excluded.payload", identity + [row.0, row.1, row.2])
            }
            try execute("INSERT INTO thread_history_meta(environment_id,thread_id,revision,before_cursor,after_cursor,has_more) VALUES(?,?,?,?,?,?) ON CONFLICT(environment_id,thread_id) DO UPDATE SET revision=excluded.revision,before_cursor=excluded.before_cursor,after_cursor=excluded.after_cursor,has_more=excluded.has_more", identity + [revision.int64Value, before ?? NSNull(), after ?? NSNull(), Int64(hasMore ? 1 : 0)])
            return ["committed": true, "stale": false]
        case "historyRead": return try historyRead(value)
        case "historyClear":
            let identity = try historyIdentity(value)
            try invalidate(["history", identity[0], identity[1]])
            try execute("DELETE FROM thread_history WHERE environment_id=? AND thread_id=?", identity)
            let removed = Int(sqlite3_changes(database))
            try execute("DELETE FROM thread_history_meta WHERE environment_id=? AND thread_id=?", identity)
            return ["removed": removed + Int(sqlite3_changes(database))]
        case "clearKind":
            let kind = try string(value, "kind", limit: 32)
            guard Self.kinds.contains(kind), value["environmentId"] == nil, value["key"] == nil else {
                throw Self.fail("Invalid client cache kind clear.")
            }
            try execute("INSERT INTO cache_epochs(scope,token) VALUES(?,?) ON CONFLICT(scope) DO UPDATE SET token=excluded.token", ["kind:" + kind, UUID().uuidString])
            try execute("DELETE FROM client_cache WHERE kind=?", [kind])
            let removed = Int(sqlite3_changes(database))
            if kind == "thread" {
                try execute("DELETE FROM thread_history")
                try execute("DELETE FROM thread_history_meta")
            }
            return ["removed": removed]
        case "list":
            let kind = try string(value, "kind", limit: 32)
            guard Self.kinds.contains(kind), let limit = value["limit"] as? NSNumber,
                  CFGetTypeID(limit) != CFBooleanGetTypeID(), limit.doubleValue.rounded() == limit.doubleValue,
                  (1...128).contains(limit.doubleValue) else { throw Self.fail("Invalid client cache page.") }
            var predicate = "kind=?", values: [Any] = [kind]
            if let afterValue = value["after"], !(afterValue is NSNull) {
                guard let after = afterValue as? Object, try string(after, "kind", limit: 32) == kind,
                      let timestamp = after["updatedAt"] as? NSNumber,
                      CFGetTypeID(timestamp) != CFBooleanGetTypeID(), timestamp.doubleValue.rounded() == timestamp.doubleValue,
                      (0...9_007_199_254_740_991).contains(timestamp.doubleValue) else {
                    throw Self.fail("Invalid client cache cursor.")
                }
                let environment = try string(after, "environmentId", limit: 512), key = try string(after, "key")
                // BINARY uses UTF-8 scalar order. The TypeScript page validator
                // uses the same order, including BMP versus astral characters.
                predicate += " AND (updated_at,environment_id,cache_key) > (?,?,?)"
                values += [timestamp.int64Value, environment, key]
            }
            values.append(limit.int64Value)
            let pointer = try statement("SELECT environment_id,kind,cache_key,schema_version,updated_at FROM client_cache WHERE " + predicate + " ORDER BY updated_at,environment_id COLLATE BINARY,cache_key COLLATE BINARY LIMIT ?", values)
            defer { sqlite3_finalize(pointer) }
            var rows = [Object]()
            while true {
                let status = sqlite3_step(pointer)
                if status == SQLITE_DONE { break }
                guard status == SQLITE_ROW else { throw sqlFailure() }
                rows.append(["environmentId": text(pointer, 0), "kind": text(pointer, 1), "key": text(pointer, 2),
                             "schemaVersion": sqlite3_column_int64(pointer, 3), "updatedAt": sqlite3_column_int64(pointer, 4)])
            }
            return ["rows": rows]
        case "ticket": return ["ticket": try ticket(identity(value))]
        case "read":
            let id = try identity(value)
            let pointer = try statement("SELECT schema_version,payload,updated_at FROM client_cache WHERE environment_id=? AND kind=? AND cache_key=?", id)
            defer { sqlite3_finalize(pointer) }
            let status = sqlite3_step(pointer)
            if status == SQLITE_DONE { return ["record": NSNull()] }
            guard status == SQLITE_ROW else { throw sqlFailure() }
            let version = sqlite3_column_int64(pointer, 0), payload = text(pointer, 1)
            guard validPayload(payload, identity: id, version: version) else {
                try invalidate(id)
                try execute("DELETE FROM client_cache WHERE environment_id=? AND kind=? AND cache_key=?", id)
                return ["record": NSNull(), "discarded": true]
            }
            return ["record": ["environmentId": id[0], "kind": id[1], "key": id[2], "schemaVersion": version, "payload": payload, "updatedAt": sqlite3_column_int64(pointer, 2)]]
        case "write":
            let id = try identity(value), supplied = try string(value, "ticket", limit: 256)
            guard let number = value["schemaVersion"] as? NSNumber, CFGetTypeID(number) != CFBooleanGetTypeID(), number.doubleValue == 1,
                  let payload = value["payload"] as? String, validPayload(payload, identity: id, version: number.int64Value) else {
                throw Self.fail("Invalid client cache payload or schema version.")
            }
            guard supplied == (try ticket(id)) else { return ["written": false, "stale": true] }
            let timestamp = Int64(Date().timeIntervalSince1970 * 1_000)
            try execute("INSERT INTO client_cache(environment_id,kind,cache_key,schema_version,payload,updated_at) VALUES(?,?,?,?,?,?) ON CONFLICT(environment_id,kind,cache_key) DO UPDATE SET schema_version=excluded.schema_version,payload=excluded.payload,updated_at=excluded.updated_at WHERE client_cache.schema_version!=excluded.schema_version OR client_cache.payload!=excluded.payload", id + [number.int64Value, payload, timestamp])
            return ["written": true, "stale": false, "changed": sqlite3_changes(database) > 0]
        case "remove":
            let id = try identity(value)
            if value["expectedPayload"] != nil {
                guard let expected = value["expectedPayload"] as? String, expected.utf8.count <= 16 * 1_024 * 1_024 else {
                    throw Self.fail("Invalid expected client cache payload.")
                }
                let pointer = try statement("SELECT payload FROM client_cache WHERE environment_id=? AND kind=? AND cache_key=?", id)
                defer { sqlite3_finalize(pointer) }
                let status = sqlite3_step(pointer)
                guard status == SQLITE_ROW || status == SQLITE_DONE else { throw sqlFailure() }
                guard status == SQLITE_ROW, text(pointer, 0) == expected else { return ["removed": 0] }
            }
            try invalidate(id)
            try execute("DELETE FROM client_cache WHERE environment_id=? AND kind=? AND cache_key=?", id)
            let removed = Int(sqlite3_changes(database))
            if id[1] == "thread" {
                try invalidate(["history", id[0], id[2]])
                try execute("DELETE FROM thread_history WHERE environment_id=? AND thread_id=?", [id[0], id[2]])
                try execute("DELETE FROM thread_history_meta WHERE environment_id=? AND thread_id=?", [id[0], id[2]])
            }
            return ["removed": removed]
        case "clear":
            var parts = [String]()
            for field in ["environmentId", "kind", "key"] {
                if value[field] != nil {
                    guard parts.count == ["environmentId", "kind", "key"].firstIndex(of: field) else { throw Self.fail("Invalid client cache clear scope.") }
                    parts.append(try string(value, field, limit: field == "environmentId" ? 512 : 8_192))
                }
            }
            if parts.count > 1, !Self.kinds.contains(parts[1]) { throw Self.fail("Unknown client cache kind.") }
            try invalidate(parts)
            let clauses = ["environment_id=?", "kind=?", "cache_key=?"].prefix(parts.count)
            try execute("DELETE FROM client_cache" + (parts.isEmpty ? "" : " WHERE " + clauses.joined(separator: " AND ")), parts)
            let removed = Int(sqlite3_changes(database))
            if parts.count == 0 {
                try execute("DELETE FROM thread_history")
                try execute("DELETE FROM thread_history_meta")
            } else if parts.count == 1 {
                try execute("DELETE FROM thread_history WHERE environment_id=?", parts)
                try execute("DELETE FROM thread_history_meta WHERE environment_id=?", parts)
            } else if parts[1] == "thread" && parts.count == 2 {
                try execute("DELETE FROM thread_history WHERE environment_id=?", [parts[0]])
                try execute("DELETE FROM thread_history_meta WHERE environment_id=?", [parts[0]])
            } else if parts[1] == "thread" && parts.count == 3 {
                try invalidate(["history", parts[0], parts[2]])
                try execute("DELETE FROM thread_history WHERE environment_id=? AND thread_id=?", [parts[0], parts[2]])
                try execute("DELETE FROM thread_history_meta WHERE environment_id=? AND thread_id=?", [parts[0], parts[2]])
            }
            return ["removed": removed]
        case "inspect":
            let environment = value["environmentId"] == nil ? nil : try string(value, "environmentId", limit: 512)
            let pointer = try statement("SELECT environment_id,kind,COUNT(*),SUM(LENGTH(CAST(payload AS BLOB))) FROM client_cache" + (environment == nil ? "" : " WHERE environment_id=?") + " GROUP BY environment_id,kind ORDER BY environment_id,kind", environment.map { [$0] } ?? [])
            defer { sqlite3_finalize(pointer) }
            var rows = [Object](), count: Int64 = 0, bytes: Int64 = 0
            while true {
                let status = sqlite3_step(pointer)
                if status == SQLITE_DONE { break }
                guard status == SQLITE_ROW else { throw sqlFailure() }
                let rowCount = sqlite3_column_int64(pointer, 2), rowBytes = sqlite3_column_int64(pointer, 3)
                rows.append(["environmentId": text(pointer, 0), "kind": text(pointer, 1), "recordCount": rowCount, "payloadBytes": rowBytes])
                count += rowCount; bytes += rowBytes
            }
            return ["rows": rows, "recordCount": count, "payloadBytes": bytes]
        default: throw Self.fail("Unknown client cache operation.")
        }
    }
}
