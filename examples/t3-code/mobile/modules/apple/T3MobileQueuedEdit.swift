#if os(iOS)
// @ref llp/1106.005-composer-and-transcript.decision.md#scratch-tasks-and-queue-boundaries
// App-local queued editor and durable immutable commands; never shared ordinary pending.
import Foundation
import Darwin

final class T3MobileQueuedEdit: @unchecked Sendable {
    static let methods: Set<String> = ["orchestration.dispatchCommand", "orchestration.launchThread", "projects.mutate"]
    private final class Weak { weak var value: T3MobileQueuedEdit?; init(_ value: T3MobileQueuedEdit) { self.value = value } }
    private static let registryLock = NSLock()
    private static var registry: [String: Weak] = [:]
    static func shared(root: URL) -> T3MobileQueuedEdit {
        registryLock.lock(); defer { registryLock.unlock() }
        let key = root.standardizedFileURL.resolvingSymlinksInPath().path
        if let value = registry[key]?.value { return value }
        let value = T3MobileQueuedEdit(root: root); registry[key] = Weak(value)
        registry = registry.filter { $0.value.value != nil }
        return value
    }
    private let lock = NSLock()
    private let root: URL
    private var file: URL { root.appendingPathComponent("mobile-queued-edit.json") }
    private var preferences: URL { root.appendingPathComponent("t3-code.json") }
    private var active: [UUID: String] = [:]
    private var sending = Set<String>()
    // Failure injection belongs to isolated source tests; shipping initializer uses durableReplace.
    private let replace: (Data, URL) throws -> Void
    init(root: URL, replace: @escaping (Data, URL) throws -> Void = T3MobileQueuedEdit.durableReplace) {
        self.root = root; self.replace = replace
    }
    private func locked<T>(_ action: () throws -> T) rethrows -> T { lock.lock(); defer { lock.unlock() }; return try action() }
    private func refusal(_ message: String, kind: String = "QueuedEdit") -> T3Failure { T3Failure(kind: kind, message: message) }
    private static func encoded(_ value: Any) throws -> Data {
        guard JSONSerialization.isValidJSONObject(value) else { throw T3Failure(kind: "Persistence", message: "The queued edit is not valid JSON.") }
        let data = try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys])
        guard data.count <= T3Wire.maximumBytes else { throw T3Failure(kind: "Limit", message: "The queued edit store is too large.") }
        return data
    }
    /// Synchronized file contents followed by same-directory atomic replacement. No cross-process lock.
    static func durableReplace(_ data: Data, _ destination: URL) throws {
        let manager = FileManager.default, directory = destination.deletingLastPathComponent()
        try manager.createDirectory(at: directory, withIntermediateDirectories: true)
        let temporary = directory.appendingPathComponent(".queued-edit-\(UUID().uuidString)")
        let fd = Darwin.open(temporary.path, O_WRONLY | O_CREAT | O_EXCL, mode_t(0o600))
        guard fd >= 0 else { throw CocoaError(.fileWriteUnknown) }
        var closed = false
        defer { if !closed { Darwin.close(fd) }; try? manager.removeItem(at: temporary) }
        try data.withUnsafeBytes { buffer in
            var offset = 0
            while offset < buffer.count {
                let count = Darwin.write(fd, buffer.baseAddress!.advanced(by: offset), buffer.count - offset)
                if count < 0 && errno == EINTR { continue }
                guard count > 0 else { throw CocoaError(.fileWriteUnknown) }; offset += count
            }
        }
        guard fsync(fd) == 0 else { throw CocoaError(.fileWriteUnknown) }
        guard Darwin.close(fd) == 0 else { closed = true; throw CocoaError(.fileWriteUnknown) }; closed = true
        guard Darwin.rename(temporary.path, destination.path) == 0 else { throw CocoaError(.fileWriteUnknown) }
        // Directory sync persists the replacement name. A failed sync is reported, never called success.
        let directoryFD = Darwin.open(directory.path, O_RDONLY)
        guard directoryFD >= 0 else { throw CocoaError(.fileWriteUnknown) }
        defer { Darwin.close(directoryFD) }
        guard fsync(directoryFD) == 0 else { throw CocoaError(.fileWriteUnknown) }
    }
    private func readJSON(_ url: URL) throws -> [String: Any] {
        do {
            let attributes = try FileManager.default.attributesOfItem(atPath: url.path)
            guard (attributes[.size] as? NSNumber)?.intValue ?? 0 <= T3Wire.maximumBytes else { throw refusal("The saved file is too large.", kind: "Persistence") }
            guard let value = try JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any] else { throw refusal("The saved file is invalid.", kind: "Persistence") }
            return value
        } catch let error as CocoaError where error.code == .fileReadNoSuchFile { return [:] }
    }
    private func store() throws -> [String: Any] {
        let value = try readJSON(file)
        if value.isEmpty && !FileManager.default.fileExists(atPath: file.path) { return ["version": 1, "records": [String: Any](), "operations": [String: Any](), "ended": [String: Any](), "releases": [[String: Any]]()] }
        guard value["version"] as? Int == 1, value["records"] is [String: [String: Any]], value["operations"] is [String: [String: Any]] else {
            throw refusal("The queued edit store is invalid.", kind: "Persistence")
        }
        for (owner, record) in records(value) {
            guard record["owner"] as? String == owner, (record["revision"] as? Int ?? 0) > 0,
                  record["attachments"] is [[String: Any]] else { throw refusal("The queued editor record is corrupt.", kind: "Persistence") }
        }
        for (id, operation) in operations(value) {
            guard operation["operationId"] as? String == id, operation["owner"] is String,
                  operation["environmentId"] is String, operation["origin"] is String,
                  operation["method"] as? String == "orchestration.dispatchCommand",
                  let payload = operation["payload"] as? [String: Any], payload["commandId"] as? String == id, payload["type"] as? String == "queued-run.edit",
                  (operation["revision"] as? Int ?? 0) > 0,
                  ["reserved", "issued", "uncertain", "acknowledged", "rejected"].contains(operation["state"] as? String ?? "") else {
                throw refusal("The saved queued operation is corrupt.", kind: "Persistence")
            }
        }
        return value
    }
    private func save(_ value: [String: Any]) throws { try replace(Self.encoded(value), file) }
    private func records(_ value: [String: Any]) -> [String: [String: Any]] { value["records"] as? [String: [String: Any]] ?? [:] }
    private func operations(_ value: [String: Any]) -> [String: [String: Any]] { value["operations"] as? [String: [String: Any]] ?? [:] }
    private func unresolved(_ value: [String: Any]) -> Bool { ["reserved", "issued", "uncertain"].contains(value["state"] as? String ?? "") }
    private func hasPending(_ environment: String) throws -> Bool {
        let value = try readJSON(preferences)
        if let pending = value["pending"], !(pending is [String: Any]) { throw refusal("Saved pending operations are invalid.", kind: "Persistence") }
        return (value["pending"] as? [String: Any])?[environment] != nil
    }
    func read() throws -> [String: Any] {
        try locked {
            let value = try store()
            return ["records": records(value).values.map { ["owner": $0["owner"] ?? "", "revision": $0["revision"] ?? 0, "record": $0] },
                    "operations": Array(operations(value).values), "releases": value["releases"] ?? []]
        }
    }
    func cas(_ request: [String: Any]) throws -> [String: Any] {
        try locked {
            guard let owner = request["owner"] as? String, !owner.isEmpty, let next = request["record"] as? [String: Any],
                  let revision = request["revision"] as? Int, revision > 0, next["revision"] as? Int == revision, next["owner"] as? String == owner,
                  next["attachments"] is [[String: Any]], next["existingAttachments"] is [[String: Any]] else { throw refusal("The queued editor record is invalid.") }
            let fields = ["owner", "session", "draftKey", "origin", "environmentId", "threadId", "projectId", "generation", "runId", "messageId"]
            guard fields.allSatisfy({ next[$0] != nil }), UUID(uuidString: next["session"] as? String ?? "") != nil,
                  !(next["environmentId"] as? String ?? "").isEmpty, !(next["threadId"] as? String ?? "").isEmpty else { throw refusal("The queued editor identity is invalid.") }
            var value = try store(), entries = records(value)
            guard (value["ended"] as? [String: Any])?[owner] == nil else { throw refusal("The queued editor has ended.", kind: "stale") }
            if let old = entries[owner] {
                guard fields.allSatisfy({ old[$0] != nil && NSDictionary(dictionary: ["v": old[$0] ?? NSNull()]).isEqual(to: ["v": next[$0] ?? NSNull()]) }) else { throw refusal("The queued editor identity changed.", kind: "stale") }
                let oldRevision = old["revision"] as? Int ?? 0
                if revision < oldRevision { return ["applied": false, "revision": oldRevision, "record": old] }
                if revision == oldRevision {
                    guard try Self.encoded(old) == Self.encoded(next) else { throw refusal("That editor revision already contains another value.", kind: "stale") }
                    return ["applied": true, "revision": revision, "record": old]
                }
            }
            guard !operations(value).values.contains(where: { $0["owner"] as? String == owner && unresolved($0) }) else { throw refusal("Resolve the queued update before changing its editor.", kind: "Busy") }
            let nextIDs = Set((next["attachments"] as? [[String: Any]] ?? []).compactMap { $0["id"] as? String })
            let removed = (entries[owner]?["attachments"] as? [[String: Any]] ?? []).filter { !nextIDs.contains($0["id"] as? String ?? "") }
            enqueueReleases(removed, in: &value)
            entries[owner] = next; value["records"] = entries; try save(value)
            return ["applied": true, "revision": revision, "record": next]
        }
    }
    func reserve(_ request: [String: Any], origin: String, environment: String, activeOrigin: String) throws -> [String: Any] {
        try locked {
            var value = try store(), commands = operations(value)
            guard let owner = request["owner"] as? String, let record = records(value)[owner],
                  record["revision"] as? Int == request["editorRevision"] as? Int,
                  record["environmentId"] as? String == environment,
                  [origin, activeOrigin].contains(record["origin"] as? String ?? ""),
                  let payload = request["payload"] as? [String: Any], let id = payload["commandId"] as? String, !id.isEmpty,
                  request["operationId"] as? String == id, request["method"] as? String == "orchestration.dispatchCommand",
                  payload["type"] as? String == "queued-run.edit", payload["threadId"] as? String == record["threadId"] as? String,
                  payload["runId"] as? String == record["runId"] as? String,
                  !(payload["text"] as? String ?? "").trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { throw refusal("The queued edit changed before reservation.", kind: "stale") }
            guard commands[id] == nil else { throw refusal("This queued operation is already reserved.", kind: "stale") }
            guard !commands.values.contains(where: { $0["origin"] as? String == origin && $0["environmentId"] as? String == environment && unresolved($0) }),
                  !active.values.contains(origin + "\n" + environment), try !hasPending(environment) else { throw refusal("Resolve the previous environment operation first.", kind: "Busy") }
            let operation: [String: Any] = ["operationId": id, "owner": owner, "editorRevision": record["revision"]!, "revision": 1,
                "origin": origin, "environmentId": environment, "state": "reserved", "method": "orchestration.dispatchCommand", "payload": payload,
                "attachmentIDs": (record["attachments"] as? [[String: Any]] ?? []).compactMap { $0["id"] as? String }]
            commands[id] = operation; value["operations"] = commands; try save(value); return operation
        }
    }
    func beginSend(_ id: String, revision: Int, origin: String, environment: String) throws -> [String: Any] {
        try locked {
            var value = try store(), commands = operations(value)
            guard var operation = commands[id], operation["revision"] as? Int == revision,
                  operation["origin"] as? String == origin, operation["environmentId"] as? String == environment else { throw refusal("The queued operation or environment changed.", kind: "stale") }
            if operation["state"] as? String == "acknowledged" { return operation }
            guard unresolved(operation), !sending.contains(id), !active.values.contains(origin + "\n" + environment), try !hasPending(environment) else { throw refusal("The queued operation is already sending or blocked.", kind: "Busy") }
            operation["state"] = "issued"; operation["revision"] = revision + 1
            commands[id] = operation; value["operations"] = commands; try save(value); sending.insert(id)
            return operation
        }
    }
    func settle(_ id: String, result: [String: Any], knownUnsent: Bool = false) throws -> [String: Any] {
        try locked {
            defer { sending.remove(id) }
            var value = try store(), commands = operations(value)
            guard var operation = commands[id] else { throw refusal("The queued operation is missing.", kind: "Persistence") }
            let success = result["ok"] as? Bool == true
            operation["state"] = success ? "acknowledged" : knownUnsent ? "reserved" : result["_definitiveFailure"] as? Bool == true ? "rejected" : "uncertain"
            operation["revision"] = (operation["revision"] as? Int ?? 0) + 1
            if success { operation["result"] = result["value"] ?? NSNull(); operation.removeValue(forKey: "error") }
            else { operation["error"] = result["error"] ?? [:] }
            commands[id] = operation; value["operations"] = commands; try save(value); return operation
        }
    }
    func admit(method: String, origin: String, environment: String, journal: String? = nil) throws -> UUID? {
        guard Self.methods.contains(method) else { return nil }
        return try locked {
            let commands = operations(try store())
            if let journal {
                guard sending.contains(journal), let operation = commands[journal], operation["origin"] as? String == origin, operation["environmentId"] as? String == environment,
                      operation["state"] as? String == "issued" else { throw refusal("The queued send is no longer admitted.", kind: "stale") }
            } else if commands.values.contains(where: { $0["origin"] as? String == origin && $0["environmentId"] as? String == environment && unresolved($0) }) {
                throw refusal("Resolve the queued update before making another change in this environment.", kind: "Busy")
            }
            let token = UUID(); active[token] = origin + "\n" + environment; return token
        }
    }
    func release(_ token: UUID?) { guard let token else { return }; locked { _ = active.removeValue(forKey: token) } }
    func writePreferences(_ text: String) throws {
        try locked {
            guard let value = try JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Any],
                  (value["pending"] == nil || value["pending"] is [String: Any]) else { throw refusal("Saved preferences are invalid.", kind: "Persistence") }
            let pending = value["pending"] as? [String: Any] ?? [:]
            for operation in operations(try store()).values where unresolved(operation) {
                if pending[operation["environmentId"] as? String ?? ""] != nil { throw refusal("Resolve the queued update before saving another pending operation.", kind: "Busy") }
            }
            try replace(Self.encoded(value), preferences)
        }
    }
    func retire(_ request: [String: Any]) throws -> [String: Any] {
        try locked {
            var value = try store(), commands = operations(value)
            guard let owner = request["owner"] as? String, let record = records(value)[owner],
                  record["revision"] as? Int == request["editorRevision"] as? Int,
                  let id = request["operationId"] as? String, let operation = commands[id], operation["owner"] as? String == owner,
                  operation["revision"] as? Int == request["revision"] as? Int,
                  ["reserved", "acknowledged", "rejected"].contains(operation["state"] as? String ?? ""), !sending.contains(id) else {
                throw refusal("The queued operation changed or still needs resolution.", kind: "stale")
            }
            commands.removeValue(forKey: id); value["operations"] = commands; try save(value)
            return ["removed": true, "record": record, "releases": drainReleases(&value)]
        }
    }
    func cleanup(_ request: [String: Any]) throws -> [String: Any] {
        try locked {
            var value = try store(), entries = records(value), commands = operations(value)
            guard let owner = request["owner"] as? String, let record = entries[owner],
                  record["revision"] as? Int == request["editorRevision"] as? Int else { throw refusal("The editor changed before cleanup.", kind: "stale") }
            let owned = commands.values.filter { $0["owner"] as? String == owner }
            for operation in owned {
                guard operation["operationId"] as? String == request["operationId"] as? String,
                      operation["revision"] as? Int == request["operationRevision"] as? Int,
                      ["reserved", "acknowledged", "rejected"].contains(operation["state"] as? String ?? ""),
                      !sending.contains(operation["operationId"] as? String ?? "") else { throw refusal("Resolve the sent queued update before cleanup.", kind: "Busy") }
            }
            entries.removeValue(forKey: owner)
            for operation in owned { commands.removeValue(forKey: operation["operationId"] as? String ?? "") }
            var ended = value["ended"] as? [String: Any] ?? [:]; ended[owner] = record["revision"]
            enqueueReleases(record["attachments"] as? [[String: Any]] ?? [], in: &value)
            value["ended"] = ended; value["records"] = entries; value["operations"] = commands; try save(value)
            let releases = drainReleases(&value)
            return ["removed": true, "releases": releases]
        }
    }
    private func enqueueReleases(_ attachments: [[String: Any]], in value: inout [String: Any]) {
        var releases = value["releases"] as? [[String: Any]] ?? []
        for attachment in attachments {
            guard let id = attachment["id"] as? String, UUID(uuidString: id) != nil,
                  let kind = attachment["kind"] as? String, ["image", "file"].contains(kind),
                  !releases.contains(where: { $0["id"] as? String == id && $0["kind"] as? String == kind }) else { continue }
            releases.append(["id": id.lowercased(), "kind": kind])
        }
        value["releases"] = releases
    }
    private func held(_ identifier: String, value: [String: Any]) throws -> Bool {
        let owned = records(value).values.contains { record in
            (record["attachments"] as? [[String: Any]] ?? []).contains { ($0["id"] as? String)?.lowercased() == identifier }
        } || operations(value).values.contains { ($0["attachmentIDs"] as? [String] ?? []).map { $0.lowercased() }.contains(identifier) }
        if owned { return true }
        let preferencesValue = try readJSON(preferences)
        let ordinary = preferencesValue["snapshotDrafts"] as? [String: [[String: Any]]] ?? [:]
        return ordinary.values.joined().contains { ($0["id"] as? String)?.lowercased() == identifier }
            || (preferencesValue["composerFiles"] as? [[String: Any]] ?? []).contains { ($0["id"] as? String)?.lowercased() == identifier }
    }
    private func unlink(_ id: String, image: Bool) throws {
        let directory = image ? "snapshots/drafts" : "composer-files"
        let url = root.appendingPathComponent(directory).appendingPathComponent(id)
        if FileManager.default.fileExists(atPath: url.path) { try FileManager.default.removeItem(at: url) }
    }
    private func drainReleases(_ value: inout [String: Any]) -> [[String: Any]] {
        let releases = value["releases"] as? [[String: Any]] ?? []
        var remaining: [[String: Any]] = []
        for (index, release) in releases.enumerated() {
            // One answer does bounded cleanup; retained work stays visible for the next explicit drain.
            guard index < 100, let id = release["id"] as? String, UUID(uuidString: id) != nil else { remaining.append(release); continue }
            do {
                if try held(id, value: value) { remaining.append(release); continue }
                try unlink(id, image: release["kind"] as? String == "image")
            } catch { remaining.append(release) }
        }
        value["releases"] = remaining
        do { try save(value); return remaining }
        catch { return releases } // Already-removed files are harmless on the next idempotent attempt.
    }
    func releaseAttachments() throws -> [String: Any] {
        try locked { var value = try store(); return ["releases": drainReleases(&value)] }
    }
    /// Called instead of the old unconditional attachment remover. Check and unlink share the CAS lock.
    func removeAttachment(_ request: [String: Any]) throws -> [String: Any] {
        try locked {
            guard let id = request["id"] as? String, UUID(uuidString: id) != nil else { throw refusal("That attachment is unavailable.") }
            let value = try store(), identifier = id.lowercased()
            guard try !held(identifier, value: value) else { return ["removed": false, "retained": true] }
            try unlink(identifier, image: request["op"] as? String == "snapshotDraftRemove")
            return ["removed": true]
        }
    }
}
#endif
