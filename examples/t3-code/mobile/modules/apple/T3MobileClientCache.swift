// @ref llp/1109.009-mobile-settings.decision.md#information-sources
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
                if let value = value as? String {
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

    private func perform(_ value: Object) throws -> Object {
        switch try string(value, "action", limit: 32) {
        case "clearKind":
            let kind = try string(value, "kind", limit: 32)
            guard Self.kinds.contains(kind), value["environmentId"] == nil, value["key"] == nil else {
                throw Self.fail("Invalid client cache kind clear.")
            }
            try execute("INSERT INTO cache_epochs(scope,token) VALUES(?,?) ON CONFLICT(scope) DO UPDATE SET token=excluded.token", ["kind:" + kind, UUID().uuidString])
            try execute("DELETE FROM client_cache WHERE kind=?", [kind])
            return ["removed": Int(sqlite3_changes(database))]
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
            return ["removed": Int(sqlite3_changes(database))]
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
            return ["removed": Int(sqlite3_changes(database))]
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
