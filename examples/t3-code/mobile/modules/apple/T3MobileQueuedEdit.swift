#if os(iOS)
// @ref llp/1109.005-composer-and-transcript.decision.md#scratch-tasks-and-queue-boundaries
// App-local queued editor and durable immutable commands; never shared ordinary pending.
import Foundation
import CoreFoundation
import Darwin

final class T3MobileQueuedEdit: @unchecked Sendable {
    static let methods: Set<String> = ["orchestration.dispatchCommand", "orchestration.launchThread", "projects.mutate"]
    private final class Weak { weak var value: T3MobileQueuedEdit?; init(_ value: T3MobileQueuedEdit) { self.value = value } }
    private static let registryLock = NSLock()
    private static var registry: [String: Weak] = [:]
    static func shared(root: URL, replace: @escaping (Data, URL) throws -> Void = T3MobileQueuedEdit.durableReplace) -> T3MobileQueuedEdit {
        registryLock.lock(); defer { registryLock.unlock() }
        let key = root.standardizedFileURL.resolvingSymlinksInPath().path
        if let value = registry[key]?.value { return value }
        let value = T3MobileQueuedEdit(root: root, replace: replace); registry[key] = Weak(value)
        registry = registry.filter { $0.value.value != nil }
        return value
    }
    private let lock = NSLock()
    private let root: URL
    private let outboxStore: T3MobileOutbox
    private var outboxOwner: T3MobileOutboxOwner!
    private var file: URL { root.appendingPathComponent("mobile-queued-edit.json") }
    private var preferences: URL { root.appendingPathComponent("t3-code.json") }
    private var active: [UUID: String] = [:]
    private var sending = Set<String>()
    // Native-only admission while UUID image files are hashed outside this mutex.
    private var inlinePreparations: [UUID: [String: Any]] = [:]
    private var inlineSendPreparations: [UUID: [String: Any]] = [:]
    private var inlineSendAttempts: [String: [String: Any]] = [:]
    // A cold/ambiguous visible journal is not proof of fsync. Only an exact successful save confirms it.
    private var durableOutbox: [String: Int] = [:]
    private var durableDraftHandoffs = Set<String>()
    private var outboxAttempts: [String: Int] = [:]
    private var outboxAdmitted = Set<String>()
    private var outboxCleanupAnswers: [String: [T3MobileOutboxOwner.Answer]] = [:]
    // Failure injection belongs to isolated source tests; shipping initializer uses durableReplace.
    private let replace: (Data, URL) throws -> Void
    init(root: URL, replace: @escaping (Data, URL) throws -> Void = T3MobileQueuedEdit.durableReplace) {
        self.root = root; self.replace = replace
        outboxStore = T3MobileOutbox(root: root, replace: replace)
        outboxOwner = T3MobileOutboxOwner(lock: lock, disk: outboxStore, release: { [weak self] records in
            guard let self else { throw T3Failure(kind: "Persistence", message: "The attachment owner ended.") }
            try self.locked {
                var value = try self.store()
                self.enqueueReleases(records.flatMap { $0["attachments"] as? [[String: Any]] ?? [] }, in: &value)
                try self.save(value)
            }
        }, transferEvidence: { [weak self] claim in
            guard let self else { throw T3Failure(kind: "Persistence", message: "The draft preference owner ended.") }
            try self.locked {
                let preferences = try self.readJSON(self.preferences)
                guard let markers = preferences["mobileOutboxTransferCompletions"] as? [String: [String: Any]],
                      let marker = markers[claim["transferId"] as! String], let version = marker["version"] as? NSNumber,
                      CFGetTypeID(version) != CFBooleanGetTypeID(), version.intValue == 1, version.doubleValue == 1,
                      Set(marker.keys) == Set(["version", "draftKey", "fingerprint"]),
                      marker["draftKey"] as? String == claim["draftKey"] as? String,
                      marker["fingerprint"] as? String == claim["fingerprint"] as? String else {
                    throw self.refusal("Save the matching draft cleanup marker before completing this transfer.", kind: "Persistence")
                }
                // A visible preference rename may have lost its fsync reply. Establish durability here.
                try self.replace(Self.encoded(preferences), self.preferences)
            }
        }, transferAdmission: { [weak self] record in
            // submit invokes this while holding this coordinator's mutex.
            guard let self else { throw T3Failure(kind: "Persistence", message: "The attachment owner ended.") }
            for attachment in record["attachments"] as! [[String: Any]] {
                if let upload = attachment["uploadId"] as? String, !upload.isEmpty,
                   attachment["uploadEnvironmentId"] as? String == record["environmentId"] as? String { continue }
                let directory = attachment["kind"] as? String == "image" ? "snapshots/drafts" : "composer-files"
                let path = self.root.appendingPathComponent(directory).appendingPathComponent((attachment["id"] as! String).lowercased())
                let properties = try path.resourceValues(forKeys: [.isRegularFileKey, .isSymbolicLinkKey, .fileSizeKey])
                guard properties.isRegularFile == true, properties.isSymbolicLink != true,
                      properties.fileSize == attachment["sizeBytes"] as? Int else {
                    throw self.refusal("Save this draft's local attachment bytes before queuing it.", kind: "Persistence")
                }
            }
        }, deliveryCleanupEvidence: { [weak self] request in
            guard let self, let guardValue = request["deliveryCleanup"] as? [String: Any],
                  let id = guardValue["operationId"] as? String else { throw T3Failure(kind: "Persistence", message: "The delivery cleanup owner ended.") }
            try self.locked {
                let value = try self.store()
                guard let operation = self.operations(value)[id], let cleanup = operation["cleanup"] as? [String: Any],
                      cleanup["mutationId"] as? String == request["mutationId"] as? String,
                      cleanup["ackRevision"] as? Int == guardValue["ackRevision"] as? Int,
                      ["removed", "edited", "failed"].contains(cleanup["phase"] as? String ?? "") else {
                    throw self.refusal("Save the delivery cleanup outcome before releasing its ownership.", kind: "Persistence")
                }
                self.durableOutbox.removeValue(forKey: id)
                try self.saveOutboxJournal(value); self.durableOutbox[id] = operation["revision"] as? Int
            }
        })
    }
    /// Registration happens before background scheduling. The existing lock owns all byte inventories.
    func submitOutbox(_ request: [String: Any], answer: @escaping T3MobileOutboxOwner.Answer) {
        outboxOwner.submit(request) { [self] result in
            withExtendedLifetime(self) { answer(result) }
        }
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
        var replaced = false
        do {
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
        replaced = true
        // Directory sync persists the replacement name. A failed sync is reported, never called success.
        let directoryFD = Darwin.open(directory.path, O_RDONLY)
        guard directoryFD >= 0 else { throw CocoaError(.fileWriteUnknown) }
        defer { Darwin.close(directoryFD) }
        guard fsync(directoryFD) == 0 else { throw CocoaError(.fileWriteUnknown) }
        } catch { throw T3OutboxReplaceFailure(replaced: replaced, cause: error) }
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
            if operation["kind"] != nil {
                let valid = operation["kind"] as? String == "outbox-inline"
                    ? T3OutboxInlineReceipt.valid(operation, id: id) : T3OutboxDeliveryReceipt.valid(operation, id: id, inlineSource: operations(value)[(operation["inlineSource"] as? [String: Any])?["operationId"] as? String ?? ""])
                guard valid else {
                    throw refusal("The saved outbox delivery receipt is corrupt.", kind: "Persistence")
                }
                continue
            }
            guard operation["operationId"] as? String == id, operation["owner"] is String,
                  operation["environmentId"] is String, operation["origin"] is String,
                  operation["method"] as? String == "orchestration.dispatchCommand",
                  let payload = operation["payload"] as? [String: Any], payload["commandId"] as? String == id, payload["type"] as? String == "queued-run.edit",
                  (operation["revision"] as? Int ?? 0) > 0,
                  ["reserved", "issued", "uncertain", "acknowledged", "rejected"].contains(operation["state"] as? String ?? "") else {
                throw refusal("The saved queued operation is corrupt.", kind: "Persistence")
            }
        }
        // A non-retired inline owner reserves its original lifecycle identity. A final
        // tombstone is never deleted, so a present command must retain the exact backlink.
        let commands = operations(value)
        if let raw = value["draftHandoffs"] {
            guard let handoffs = raw as? [String: [String: Any]], handoffs.allSatisfy({ id, handoff in
                T3OutboxDraftHandoff.valid(handoff, id: id, receipt: commands[id])
            }) else { throw refusal("The saved draft handoff is corrupt.", kind: "Persistence") }
        }
        var inlineCommands = Set<String>(), inlineMessages = Set<Data>()
        for (id, source) in commands where source["kind"] as? String == "outbox-inline" && source["state"] as? String != "retired" {
            let commandId = (source["record"] as! [String: Any])["commandId"] as! String
            let identity = try JSONSerialization.data(withJSONObject: ["origin", "environmentId", "threadId", "messageId"].map { source[$0]! })
            guard inlineCommands.insert(commandId).inserted, inlineMessages.insert(identity).inserted else {
                throw refusal("Multiple inline receipts claim the same lifecycle identity.", kind: "Persistence")
            }
            if let final = commands[commandId] {
                guard final["kind"] as? String == "outbox", (final["inlineSource"] as? [String: Any])?["operationId"] as? String == id else {
                    throw refusal("The inline lifecycle identity has a conflicting receipt.", kind: "Persistence")
                }
            }
        }
        return value
    }
    private func save(_ value: [String: Any]) throws { try replace(Self.encoded(value), file) }
    private func records(_ value: [String: Any]) -> [String: [String: Any]] { value["records"] as? [String: [String: Any]] ?? [:] }
    private func operations(_ value: [String: Any]) -> [String: [String: Any]] { value["operations"] as? [String: [String: Any]] ?? [:] }
    func outboxDraftHandoffStatus(_ id: String) throws -> [String: Any] {
        try locked {
            let saved = (try store()["draftHandoffs"] as? [String: [String: Any]])?[id]
            return ["handoff": saved.map { $0 as Any } ?? NSNull(), "durable": saved != nil && durableDraftHandoffs.contains(id)]
        }
    }
    /// Original command receipts remain unchanged. This completion lives beside
    /// them in the existing journal and ends only their attachment ownership.
    func completeOutboxDraftHandoff(_ input: [String: Any]) throws -> [String: Any] {
        try locked {
            guard let id = input["operationId"] as? String else { throw refusal("Choose the original command receipt.") }
            var value = try store(), handoffs = value["draftHandoffs"] as? [String: [String: Any]] ?? [:]
            guard T3OutboxDraftHandoff.valid(input, id: id, receipt: operations(value)[id]) else { throw refusal("The draft handoff does not match its terminal command.") }
            if let saved = handoffs[id] {
                guard T3MobileOutbox.jsonEqual(saved, input) else { throw refusal("A completed draft handoff cannot change its captured content.") }
                // A consumed/edited destination must never be recreated on retry.
            } else {
                try outboxOwner.draftRemovalEvidenceLocked(input["request"] as! [String: Any], record: input["record"] as! [String: Any])
                let preferences = try readJSON(self.preferences)
                guard T3OutboxDraftHandoff.matchesPreferences(input, preferences: preferences) else {
                    throw refusal("Save the complete recovered draft and its exact handoff marker first.", kind: "Persistence")
                }
                // Sync the current file without rewriting another preference writer's document.
                let fd = Darwin.open(self.preferences.path, O_RDONLY)
                guard fd >= 0 else { throw CocoaError(.fileReadUnknown) }
                defer { Darwin.close(fd) }
                guard fsync(fd) == 0 else { throw CocoaError(.fileWriteUnknown) }
                let dir = Darwin.open(root.path, O_RDONLY)
                guard dir >= 0 else { throw CocoaError(.fileReadUnknown) }
                defer { Darwin.close(dir) }
                guard fsync(dir) == 0, T3OutboxDraftHandoff.matchesPreferences(input, preferences: try readJSON(self.preferences)) else {
                    throw refusal("The recovered draft changed while confirming its durability.", kind: "Persistence")
                }
                handoffs[id] = input; value["draftHandoffs"] = handoffs
            }
            durableDraftHandoffs.remove(id)
            try saveOutboxJournal(value); durableDraftHandoffs.insert(id)
            return ["handoff": input, "durable": true]
        }
    }
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
                    "operations": operations(value).values.filter { $0["kind"] == nil }, "releases": value["releases"] ?? []]
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
            guard commands[id] == nil, !inlineIdentityBlocked(id, commands: commands) else { throw refusal("This queued operation is already reserved.", kind: "stale") }
            guard !commands.values.contains(where: { $0["origin"] as? String == origin && $0["environmentId"] as? String == environment && unresolved($0) }),
                  !active.values.contains(origin + "\n" + environment), try !hasPending(environment) else { throw refusal("Resolve the previous environment operation first.", kind: "Busy") }
            let operation: [String: Any] = ["operationId": id, "owner": owner, "editorRevision": record["revision"]!, "revision": 1,
                "origin": origin, "environmentId": environment, "state": "reserved", "method": "orchestration.dispatchCommand", "payload": payload,
                "attachmentIDs": (record["attachments"] as? [[String: Any]] ?? []).compactMap { $0["id"] as? String }]
            commands[id] = operation; value["operations"] = commands; try save(value); return operation
        }
    }
    private func inlineIdentityBlocked(_ id: String, commands: [String: [String: Any]]) -> Bool {
        inlinePreparations.values.contains { entry in
            let prepared = entry["prepared"] as! [String: Any]
            return prepared["operationId"] as? String == id || (prepared["record"] as? [String: Any])?["commandId"] as? String == id
        } || commands.values.contains { operation in
            operation["kind"] as? String == "outbox-inline" && operation["state"] as? String != "retired"
                && (operation["record"] as? [String: Any])?["commandId"] as? String == id
        }
    }
    /// Called on the transport queue after endpoint checks. No hashing or retained reply under the mutex.
    func beginOutboxInlineReservation(_ request: [String: Any], origin: String, environment: String) throws -> T3OutboxInlineAdmission {
        try locked {
            let prepared = try T3OutboxInlineReceipt.prepare(request, origin: origin, environment: environment)
            let id = prepared["operationId"] as! String, command = (prepared["record"] as! [String: Any])["commandId"] as! String
            let value = try store(), commands = operations(value)
            if let existing = commands[id] {
                guard existing["kind"] as? String == "outbox-inline", T3OutboxInlineReceipt.sameInput(existing, prepared) else {
                    throw refusal("This inline identity already belongs to another reservation.", kind: "stale")
                }
                durableOutbox.removeValue(forKey: id)
                try saveOutboxJournal(value); durableOutbox[id] = existing["revision"] as? Int
                return .existing(["operation": existing, "durable": true])
            }
            guard !inlineIdentityBlocked(id, commands: commands), !inlineIdentityBlocked(command, commands: commands),
                  commands[command] == nil,
                  !commands.values.contains(where: { $0["kind"] as? String == "outbox-inline" && $0["state"] as? String != "retired"
                    && $0["origin"] as? String == origin && $0["environmentId"] as? String == environment
                    && $0["threadId"] as? String == prepared["threadId"] as? String && $0["messageId"] as? String == prepared["messageId"] as? String }),
                  !commands.values.contains(where: { $0["kind"] as? String == "outbox-inline" && ($0["record"] as? [String: Any])?["commandId"] as? String == id }) else {
                throw refusal("The inline identity collides with an existing command.", kind: "stale")
            }
            _ = try outboxOwner.deliveryRecordLocked(request)
            guard !commands.values.contains(where: { $0["origin"] as? String == origin && $0["environmentId"] as? String == environment && unresolved($0) }),
                  !active.values.contains(origin + "\n" + environment), try !hasPending(environment) else {
                throw refusal("Resolve the previous environment operation before preparing inline images.", kind: "Busy")
            }
            let token = UUID()
            inlinePreparations[token] = ["prepared": prepared, "ownerEpoch": request["ownerEpoch"]!, "phase": "preparing"]
            active[token] = origin + "\n" + environment
            return .preparing(token)
        }
    }
    /// Off the transport queue. The transient entry owns all original bytes until finish/cancel.
    func captureOutboxInlineReservation(_ token: UUID) throws {
        do {
            let prepared: [String: Any] = try locked {
                guard var entry = inlinePreparations[token], entry["phase"] as? String == "preparing" else {
                    throw refusal("This inline preparation ended or is already hashing.", kind: "stale")
                }
                entry["phase"] = "hashing"; inlinePreparations[token] = entry
                return entry["prepared"] as! [String: Any]
            }
            let captured = try T3OutboxInlineTemplate.capture(root: root, record: prepared["record"] as! [String: Any], template: prepared["template"] as! [String: Any])
            try locked {
                guard var entry = inlinePreparations[token], entry["phase"] as? String == "hashing" else {
                    throw refusal("This inline preparation was canceled while reading its files.", kind: "stale")
                }
                entry["captured"] = captured; entry["phase"] = "captured"; inlinePreparations[token] = entry
            }
        } catch { cancelOutboxInlineReservation(token); throw error }
    }
    /// Return to the transport queue and recheck its endpoint before calling this publication step.
    func finishOutboxInlineReservation(_ token: UUID) throws -> [String: Any] {
        try locked {
            guard let entry = inlinePreparations[token] else { throw refusal("This inline preparation ended.", kind: "stale") }
            defer { inlinePreparations.removeValue(forKey: token); active.removeValue(forKey: token) }
            guard entry["phase"] as? String == "captured", let captured = entry["captured"] as? [String: Any] else {
                throw refusal("Capture the original inline image bytes before reserving them.", kind: "stale")
            }
            let prepared = entry["prepared"] as! [String: Any], id = prepared["operationId"] as! String
            let origin = prepared["origin"] as! String, environment = prepared["environmentId"] as! String
            var value = try store(), commands = operations(value)
            guard commands[id] == nil, !commands.values.contains(where: { $0["origin"] as? String == origin && $0["environmentId"] as? String == environment && unresolved($0) }),
                  !active.contains(where: { $0.key != token && $0.value == origin + "\n" + environment }), try !hasPending(environment) else {
                throw refusal("The environment changed during inline image preparation.", kind: "Busy")
            }
            _ = try outboxOwner.deliveryRecordLocked(["ownerEpoch": entry["ownerEpoch"]!, "messageId": prepared["messageId"]!,
                "expectedToken": prepared["rowToken"]!, "expectedRevision": prepared["rowRevision"]!, "record": prepared["record"]!])
            let operation = try T3OutboxInlineReceipt.captured(prepared, template: captured)
            commands[id] = operation; value["operations"] = commands
            durableOutbox.removeValue(forKey: id)
            try saveOutboxJournal(value); durableOutbox[id] = 1
            return ["operation": operation, "durable": true]
        }
    }
    func cancelOutboxInlineReservation(_ token: UUID) {
        locked { if inlinePreparations.removeValue(forKey: token) != nil { active.removeValue(forKey: token) } }
    }
    /// Read-only discovery after a lost local operation ID. No row, transport or durability mutation.
    func outboxInlineLookup(_ owner: [String: Any]) throws -> [String: Any] {
        try locked {
            let commands = operations(try store())
            let fields = ["origin", "environmentId", "threadId", "messageId", "commandId"]
            guard Set(owner.keys) == Set(fields), T3MobileOutbox.canonicalOrigin(owner["origin"]),
                  fields.dropFirst().allSatisfy({ field in
                      guard let text = owner[field] as? String else { return false }
                      return !text.isEmpty && text == text.trimmingCharacters(in: T3MobileOutbox.trimCharacters)
                  }) else { throw refusal("Choose the complete captured image owner.", kind: "Arguments") }
            func exact(_ record: [String: Any]) -> Bool {
                fields.allSatisfy { T3MobileOutbox.jsonEqual(owner[$0], record[$0]) }
            }
            func collides(_ record: [String: Any]) -> Bool {
                owner["commandId"] as? String == record["commandId"] as? String
                    || fields.dropLast().allSatisfy { T3MobileOutbox.jsonEqual(owner[$0], record[$0]) }
            }
            guard !inlinePreparations.values.contains(where: { entry in
                let prepared = entry["prepared"] as! [String: Any]
                return collides(prepared["record"] as! [String: Any])
            }) else { throw refusal("The matching image reservation is still being prepared.", kind: "Busy") }
            var matches: [[String: Any]] = []
            for id in commands.keys.sorted() {
                let operation = commands[id]!
                guard operation["kind"] as? String == "outbox-inline" else { continue }
                let record = operation["record"] as! [String: Any]
                if exact(record) {
                    matches.append(["operation": operation, "durable": durableOutbox[id] == operation["revision"] as? Int])
                } else if operation["state"] as? String != "retired" && collides(record) {
                    throw refusal("Another captured owner holds this image reservation identity.", kind: "stale")
                }
            }
            return ["operations": matches]
        }
    }
    func outboxInlineStatus(_ id: String) throws -> [String: Any] {
        try locked {
            guard let operation = operations(try store())[id], operation["kind"] as? String == "outbox-inline" else {
                return ["operation": NSNull(), "durable": false]
            }
            return ["operation": operation, "durable": durableOutbox[id] == operation["revision"] as? Int]
        }
    }
    func recoverOutboxInline(_ id: String, revision: Int) throws -> [String: Any] {
        try locked {
            let value = try store()
            guard let operation = operations(value)[id], operation["kind"] as? String == "outbox-inline", operation["revision"] as? Int == revision,
                  !inlineSendBusyLocked(id) else {
                throw refusal("Choose the exact inline reservation revision.", kind: "stale")
            }
            durableOutbox.removeValue(forKey: id)
            try saveOutboxJournal(value); durableOutbox[id] = revision
            return ["operation": operation, "durable": true]
        }
    }
    /// This stage cannot issue yet. Retirement never forgets or reuses its immutable local identity.
    func retireOutboxInline(_ id: String, revision: Int) throws -> [String: Any] {
        try locked {
            var value = try store(), commands = operations(value)
            guard var operation = commands[id], operation["kind"] as? String == "outbox-inline", operation["revision"] as? Int == revision,
                  ["reserved", "retired"].contains(operation["state"] as? String ?? ""), !inlineSendBusyLocked(id),
                  revision < 9_007_199_254_740_991 else {
                throw refusal("Choose the exact never-issued inline reservation.", kind: "stale")
            }
            durableOutbox.removeValue(forKey: id)
            if operation["state"] as? String == "reserved" {
                operation["state"] = "retired"; operation["revision"] = revision + 1
                enqueueReleases((operation["record"] as! [String: Any])["attachments"] as! [[String: Any]], in: &value)
                commands[id] = operation; value["operations"] = commands
            }
            try saveOutboxJournal(value); durableOutbox[id] = operation["revision"] as? Int
            _ = drainReleases(&value)
            return ["operation": operation, "durable": true]
        }
    }
    private func inlineSendBusyLocked(_ id: String) -> Bool {
        inlineSendAttempts[id] != nil || inlineSendPreparations.values.contains { ($0["operation"] as? [String: Any])?["operationId"] as? String == id }
    }
    private func inlineSendEnvironmentLocked(_ origin: String, _ environment: String) -> Bool {
        inlineSendPreparations.values.contains { entry in
            let operation = entry["operation"] as! [String: Any]
            return operation["origin"] as? String == origin && operation["environmentId"] as? String == environment
        }
    }
    private func inlineSendRowLocked(_ operation: [String: Any], ownerEpoch: Any?) throws {
        if operation["state"] as? String == "reserved" {
            _ = try outboxOwner.deliveryRecordLocked(["ownerEpoch": ownerEpoch ?? NSNull(), "messageId": operation["messageId"]!,
                "expectedToken": operation["rowToken"]!, "expectedRevision": operation["rowRevision"]!, "record": operation["record"]!])
        } else if !outboxOwner.deliveryRetryUnheldLocked(operation["messageId"] as! String) {
            throw refusal("Finish editing this queued message before retrying its images.", kind: "Busy")
        }
    }
    /// Transport owns its memory allowance; native preparation owns only receipt identity and byte retention.
    func beginOutboxInlineSend(_ request: [String: Any], origin: String, environment: String) throws -> T3OutboxInlineSendAdmission {
        try locked {
            let value = try store(), commands = operations(value)
            guard let id = request["operationId"] as? String, let operation = commands[id], operation["kind"] as? String == "outbox-inline",
                  T3OutboxDeliveryReceipt.integer(request["revision"], positive: true), request["revision"] as? Int == operation["revision"] as? Int,
                  operation["origin"] as? String == origin, operation["environmentId"] as? String == environment,
                  !inlineSendBusyLocked(id) else { throw refusal("The inline send receipt changed or is already active.", kind: "stale") }
            let state = operation["state"] as! String, revision = operation["revision"] as! Int
            guard revision < 9_007_199_254_740_990 else { throw refusal("The inline revision is exhausted.", kind: "Persistence") }
            if state == "acknowledged" {
                durableOutbox.removeValue(forKey: id); try saveOutboxJournal(value); durableOutbox[id] = revision
                return .acknowledged(["operation": operation, "durable": true])
            }
            let retryRejected = (request["retryRejected"] as? NSNumber).map { CFGetTypeID($0) == CFBooleanGetTypeID() && $0.boolValue } == true
            guard ["reserved", "issued", "uncertain"].contains(state) || state == "rejected" && retryRejected else {
                throw refusal("The inline receipt needs an explicit supported retry.", kind: "stale")
            }
            try inlineSendRowLocked(operation, ownerEpoch: request["ownerEpoch"])
            guard !commands.values.contains(where: { $0["operationId"] as? String != id && $0["origin"] as? String == origin && $0["environmentId"] as? String == environment && unresolved($0) }),
                  !active.values.contains(origin + "\n" + environment), try !hasPending(environment) else {
                throw refusal("Resolve the previous environment operation before sending images.", kind: "Busy")
            }
            durableOutbox.removeValue(forKey: id); try saveOutboxJournal(value); durableOutbox[id] = revision
            let token = UUID()
            inlineSendPreparations[token] = ["operation": operation, "ownerEpoch": request["ownerEpoch"] ?? NSNull(), "phase": "preparing"]
            active[token] = origin + "\n" + environment
            return .preparing(token)
        }
    }
    func expandOutboxInlineSend(_ token: UUID) throws -> T3OutboxInlinePayload {
        do {
            let operation: [String: Any] = try locked {
                guard var entry = inlineSendPreparations[token], entry["phase"] as? String == "preparing" else {
                    throw refusal("The inline send preparation ended or is already expanding.", kind: "stale")
                }
                entry["phase"] = "expanding"; inlineSendPreparations[token] = entry
                return entry["operation"] as! [String: Any]
            }
            let prepared = try T3OutboxInlinePayload.expand(token: token, root: root, record: operation["record"] as! [String: Any], captured: operation["template"] as! [String: Any])
            if let digest = operation["payloadDigest"] as? String, digest != prepared.digest {
                throw refusal("The exact inline payload changed before retry.", kind: "stale")
            }
            try locked {
                guard var entry = inlineSendPreparations[token], entry["phase"] as? String == "expanding" else {
                    throw refusal("The inline send was canceled during expansion.", kind: "stale")
                }
                entry["phase"] = "expanded"; entry["digest"] = prepared.digest; inlineSendPreparations[token] = entry
            }
            return prepared
        } catch { cancelOutboxInlineSend(token); throw error }
    }
    /// Parent transport checks endpoint and reserves its exact encoded-frame capacity before this call.
    func issueOutboxInlineSend(_ prepared: T3OutboxInlinePayload) throws -> [String: Any] {
        try locked {
            let token = prepared.token
            guard let entry = inlineSendPreparations[token], entry["phase"] as? String == "expanded",
                  entry["digest"] as? String == prepared.digest else { throw refusal("The prepared inline payload ended or changed.", kind: "stale") }
            var keepLease = false
            defer { inlineSendPreparations.removeValue(forKey: token); if !keepLease { active.removeValue(forKey: token) } }
            let original = entry["operation"] as! [String: Any], id = original["operationId"] as! String
            var value = try store(), commands = operations(value)
            guard var operation = commands[id], T3MobileOutbox.jsonEqual(operation, original), inlineSendAttempts[id] == nil else {
                throw refusal("The inline receipt changed during expansion.", kind: "stale")
            }
            try inlineSendRowLocked(operation, ownerEpoch: entry["ownerEpoch"])
            let origin = operation["origin"] as! String, environment = operation["environmentId"] as! String
            guard !commands.values.contains(where: { $0["operationId"] as? String != id && $0["origin"] as? String == origin && $0["environmentId"] as? String == environment && unresolved($0) }),
                  !active.contains(where: { $0.key != token && $0.value == origin + "\n" + environment }), try !hasPending(environment) else {
                throw refusal("The environment changed during inline expansion.", kind: "Busy")
            }
            let state = operation["state"] as! String, revision = operation["revision"] as! Int
            operation["payloadDigest"] = prepared.digest; operation["attemptRevision"] = revision + 1
            operation["attemptPreviousState"] = state == "issued" ? "uncertain" : state
            operation["revision"] = revision + 1; operation["state"] = "issued"
            operation.removeValue(forKey: "result"); operation.removeValue(forKey: "error")
            guard T3OutboxInlineReceipt.valid(operation, id: id) else { throw refusal("The issued inline receipt is invalid.", kind: "Persistence") }
            commands[id] = operation; value["operations"] = commands; durableOutbox.removeValue(forKey: id)
            try saveOutboxJournal(value); durableOutbox[id] = revision + 1
            inlineSendAttempts[id] = ["token": token, "revision": revision + 1, "digest": prepared.digest, "admitted": false]
            keepLease = true
            return operation
        }
    }
    func cancelOutboxInlineSend(_ token: UUID) {
        locked { if inlineSendPreparations.removeValue(forKey: token) != nil { active.removeValue(forKey: token) } }
    }
    /// Transport callback settlement remains owned after the originating Exact answer is disposed.
    func settleOutboxInline(_ id: String, issuedRevision: Int, result: [String: Any], knownUnsent: Bool = false) throws -> [String: Any] {
        try locked {
            var value = try store(), commands = operations(value)
            guard var operation = commands[id], operation["kind"] as? String == "outbox-inline", operation["state"] as? String == "issued",
                  operation["revision"] as? Int == issuedRevision, let attempt = inlineSendAttempts[id], attempt["revision"] as? Int == issuedRevision,
                  let token = attempt["token"] as? UUID else { throw refusal("The inline callback no longer owns its attempt.", kind: "stale") }
            defer { inlineSendAttempts.removeValue(forKey: id); active.removeValue(forKey: token) }
            let flag = result["ok"] as? NSNumber
            let boolean = flag.map { CFGetTypeID($0) == CFBooleanGetTypeID() } == true
            let okay = boolean && flag!.boolValue, negative = boolean && !flag!.boolValue
            let prior = operation["attemptPreviousState"] as! String
            if okay && T3OutboxInlineReceipt.validAcknowledgment(result["value"], receipt: operation) {
                operation["state"] = "acknowledged"; operation["result"] = result["value"]!; operation.removeValue(forKey: "error")
            } else {
                let notAccepted = negative && (result["_outboxNotAccepted"] as? NSNumber).map { CFGetTypeID($0) == CFBooleanGetTypeID() && $0.boolValue } == true
                let safelyUnsent = negative && knownUnsent && attempt["admitted"] as? Bool != true
                operation["state"] = safelyUnsent ? prior : prior == "uncertain" ? "uncertain" : notAccepted ? "rejected" : "uncertain"
                let fallback: [String: Any] = ["kind": "Protocol", "message": "The inline attachment response did not match its captured descriptors."]
                let error = result["error"] as? [String: Any]
                operation["error"] = negative && error.map(JSONSerialization.isValidJSONObject) == true ? error! : fallback
                operation.removeValue(forKey: "result")
            }
            operation["revision"] = issuedRevision + 1
            guard T3OutboxInlineReceipt.valid(operation, id: id) else { throw refusal("The settled inline receipt is invalid.", kind: "Persistence") }
            commands[id] = operation; value["operations"] = commands; durableOutbox.removeValue(forKey: id)
            try saveOutboxJournal(value); durableOutbox[id] = issuedRevision + 1
            return ["operation": operation, "durable": true]
        }
    }
    /// Outbox receipts share the journal but cannot enter the queued-edit sender.
    func reserveOutboxDelivery(_ request: [String: Any], origin: String, environment: String) throws -> [String: Any] {
        try locked {
            let candidate = try T3OutboxDeliveryReceipt.make(request, origin: origin, environment: environment)
            let id = candidate["operationId"] as! String
            var value = try store(), commands = operations(value)
            guard !inlineIdentityBlocked(id, commands: commands) else { throw refusal("This command belongs to inline preparation.", kind: "Busy") }
            if let existing = commands[id] {
                guard existing["kind"] as? String == "outbox" else {
                    throw refusal("This command identity belongs to another journal operation.", kind: "stale")
                }
                let identical = T3OutboxDeliveryReceipt.sameIdentity(existing, candidate)
                guard identical || (existing["state"] as? String == "retired"
                    && candidate["retiredRevision"] as? Int == existing["revision"] as? Int) else {
                    throw refusal("This command identity already owns another payload.", kind: "stale")
                }
                // Re-sync retirement before admitting a new row. Exact lost-reply retries return their receipt.
                durableOutbox.removeValue(forKey: id)
                try save(value); durableOutbox[id] = existing["revision"] as? Int
                if identical { return ["operation": existing, "durable": true] }
            } else if candidate["retiredRevision"] != nil {
                throw refusal("The expected retired reservation is missing.", kind: "stale")
            }
            _ = try outboxOwner.deliveryRecordLocked(request)
            guard !commands.values.contains(where: { $0["origin"] as? String == origin && $0["environmentId"] as? String == environment && unresolved($0) }),
                  !active.values.contains(origin + "\n" + environment), try !hasPending(environment) else {
                throw refusal("Resolve the previous environment operation first.", kind: "Busy")
            }
            commands[id] = candidate; value["operations"] = commands
            durableOutbox.removeValue(forKey: id)
            try save(value); durableOutbox[id] = candidate["revision"] as? Int
            return ["operation": candidate, "durable": true]
        }
    }
    /// Bind the original command to a durable assets ACK. No caller payload or record is admitted.
    func reserveOutboxInlineDelivery(_ request: [String: Any], origin: String, environment: String) throws -> [String: Any] {
        try locked {
            var value = try store(), commands = operations(value)
            guard let sourceId = request["inlineOperationId"] as? String, let source = commands[sourceId],
                  source["kind"] as? String == "outbox-inline", source["state"] as? String == "acknowledged",
                  T3OutboxDeliveryReceipt.integer(request["inlineRevision"], positive: true),
                  request["inlineRevision"] as? Int == source["revision"] as? Int,
                  source["origin"] as? String == origin, source["environmentId"] as? String == environment,
                  let command = T3OutboxInlineFinal.command(source), let payload = command["payload"] as? [String: Any],
                  let id = payload["commandId"] as? String else {
                throw refusal("Choose the exact acknowledged inline receipt.", kind: "stale")
            }
            if let retired = request["expectedRetiredRevision"], !T3OutboxDeliveryReceipt.integer(retired, positive: true) {
                throw refusal("Choose an exact retired lifecycle revision.", kind: "stale")
            }
            durableOutbox.removeValue(forKey: sourceId)
            try saveOutboxJournal(value); durableOutbox[sourceId] = source["revision"] as? Int
            if let existing = commands[id] {
                guard T3OutboxInlineFinal.matches(existing, source: source) else {
                    throw refusal("The lifecycle identity belongs to another command.", kind: "stale")
                }
                durableOutbox.removeValue(forKey: id)
                try saveOutboxJournal(value); durableOutbox[id] = existing["revision"] as? Int
                if request["expectedRetiredRevision"] == nil { return ["operation": existing, "durable": true] }
                guard existing["state"] as? String == "retired",
                      T3OutboxDeliveryReceipt.integer(request["expectedRetiredRevision"], positive: true),
                      request["expectedRetiredRevision"] as? Int == existing["revision"] as? Int else {
                    // An exact lost-reply retry of rearming returns that original reservation.
                    guard request["expectedRetiredRevision"] as? Int == existing["retiredRevision"] as? Int else {
                        throw refusal("The retired lifecycle revision changed.", kind: "stale")
                    }
                    return ["operation": existing, "durable": true]
                }
            } else if request["expectedRetiredRevision"] != nil {
                throw refusal("The expected retired lifecycle is missing.", kind: "stale")
            }
            var input: [String: Any] = ["ownerEpoch": request["ownerEpoch"] ?? NSNull(),
                "record": source["record"]!, "messageId": source["messageId"]!,
                "expectedToken": source["rowToken"]!, "expectedRevision": source["rowRevision"]!,
                "stage": command["stage"]!, "method": command["method"]!, "payload": payload]
            if let retired = request["expectedRetiredRevision"] { input["expectedRetiredRevision"] = retired }
            _ = try outboxOwner.deliveryRecordLocked(input)
            guard !commands.values.contains(where: { $0["operationId"] as? String != id && $0["origin"] as? String == origin && $0["environmentId"] as? String == environment && unresolved($0) }),
                  !active.values.contains(origin + "\n" + environment), try !hasPending(environment) else {
                throw refusal("Resolve the previous environment operation before binding this command.", kind: "Busy")
            }
            let final = try T3OutboxDeliveryReceipt.make(input, origin: origin, environment: environment, inlineSource: source)
            commands[id] = final; value["operations"] = commands
            durableOutbox.removeValue(forKey: id)
            try saveOutboxJournal(value); durableOutbox[id] = final["revision"] as? Int
            return ["operation": final, "durable": true]
        }
    }
    /// Read-only: a visible receipt after restart is deliberately not called durable.
    func outboxDeliveryStatus(_ id: String) throws -> [String: Any] {
        try locked {
            guard let operation = operations(try store())[id], operation["kind"] as? String == "outbox" else {
                return ["operation": NSNull(), "durable": false]
            }
            return ["operation": operation, "durable": durableOutbox[id] == operation["revision"] as? Int]
        }
    }
    /// Only known-unsent reservations retire. Replacing one requires its exact retired revision and fresh row admission.
    func retireOutboxDelivery(_ id: String, revision: Int) throws -> [String: Any] {
        try locked {
            var value = try store(), commands = operations(value)
            guard var operation = commands[id], operation["kind"] as? String == "outbox",
                  operation["revision"] as? Int == revision, revision < 9_007_199_254_740_991,
                  ["reserved", "retired"].contains(operation["state"] as? String ?? ""), !sending.contains(id) else {
                throw refusal("The outbox reservation changed or needs delivery resolution.", kind: "stale")
            }
            durableOutbox.removeValue(forKey: id)
            if operation["state"] as? String == "reserved" {
                operation["state"] = "retired"; operation["revision"] = revision + 1
                enqueueReleases((operation["record"] as? [String: Any])?["attachments"] as? [[String: Any]] ?? [], in: &value)
                commands[id] = operation; value["operations"] = commands
            }
            try save(value); durableOutbox[id] = operation["revision"] as? Int
            return ["operation": operation, "durable": true, "releases": drainReleases(&value)]
        }
    }
    private func saveOutboxJournal(_ value: [String: Any]) throws {
        do { try save(value) }
        catch { throw T3Failure(kind: "Persistence", message: "The delivery receipt could not be durably confirmed. Read and recover its exact revision.", uncertain: true) }
    }
    /// One saved command, one active attempt. Caller is the selected transport's serial queue.
    func beginOutboxDelivery(_ request: [String: Any], origin: String, environment: String) throws -> [String: Any] {
        try locked {
            var value = try store(), commands = operations(value)
            guard let id = request["operationId"] as? String, var operation = commands[id],
                  operation["kind"] as? String == "outbox", T3OutboxDeliveryReceipt.integer(request["revision"], positive: true),
                  request["revision"] as? Int == operation["revision"] as? Int,
                  operation["origin"] as? String == origin, operation["environmentId"] as? String == environment,
                  !sending.contains(id), outboxAttempts[id] == nil else {
                throw refusal("The delivery receipt changed or is already sending.", kind: "stale")
            }
            let state = operation["state"] as! String, revision = operation["revision"] as! Int
            guard revision < 9_007_199_254_740_990 else { throw refusal("The delivery revision is exhausted.", kind: "Persistence") }
            if state == "acknowledged" {
                durableOutbox.removeValue(forKey: id)
                try saveOutboxJournal(value); durableOutbox[id] = revision
                return operation
            }
            guard ["reserved", "issued", "uncertain"].contains(state)
                || (state == "rejected" && operation["stage"] as? String == "settings-sync"
                    && (request["retryRejected"] as? NSNumber).map({ CFGetTypeID($0) == CFBooleanGetTypeID() && $0.boolValue }) == true) else {
                throw refusal("This delivery receipt needs an explicit supported resolution.", kind: "stale")
            }
            if state == "reserved" {
                _ = try outboxOwner.deliveryRecordLocked(["ownerEpoch": request["ownerEpoch"] ?? NSNull(),
                    "messageId": operation["messageId"]!, "expectedToken": operation["rowToken"]!,
                    "expectedRevision": operation["rowRevision"]!, "record": operation["record"]!])
            } else if !outboxOwner.deliveryRetryUnheldLocked(operation["messageId"] as! String) {
                throw refusal("Finish editing this queued message before retrying its delivery.", kind: "Busy")
            }
            guard !commands.values.contains(where: { $0["operationId"] as? String != id && $0["origin"] as? String == origin && $0["environmentId"] as? String == environment && unresolved($0) }),
                  !active.values.contains(origin + "\n" + environment), try !hasPending(environment) else {
                throw refusal("Resolve the previous environment operation first.", kind: "Busy")
            }
            // First re-sync exact visible state; then durably mark this attempt before any RPC admission.
            durableOutbox.removeValue(forKey: id)
            try saveOutboxJournal(value); durableOutbox[id] = revision
            operation["attemptPreviousState"] = state == "issued" ? "uncertain" : state
            operation["attemptRevision"] = revision + 1
            operation["revision"] = revision + 1; operation["state"] = "issued"
            operation.removeValue(forKey: "result"); operation.removeValue(forKey: "error")
            commands[id] = operation; value["operations"] = commands
            durableOutbox.removeValue(forKey: id)
            try saveOutboxJournal(value); durableOutbox[id] = revision + 1
            sending.insert(id); outboxAttempts[id] = revision + 1; outboxAdmitted.remove(id)
            return operation
        }
    }
    /// Native completion owns settlement even when the original Exact answer has gone away.
    func settleOutboxDelivery(_ id: String, issuedRevision: Int, result: [String: Any], knownUnsent: Bool = false) throws -> [String: Any] {
        try locked {
            var value = try store(), commands = operations(value)
            guard var operation = commands[id], operation["kind"] as? String == "outbox",
                  operation["state"] as? String == "issued", operation["revision"] as? Int == issuedRevision,
                  outboxAttempts[id] == issuedRevision, sending.contains(id),
                  let okay = result["ok"] as? NSNumber, CFGetTypeID(okay) == CFBooleanGetTypeID(),
                  JSONSerialization.isValidJSONObject(result) else {
                throw refusal("The delivery callback no longer owns this attempt.", kind: "stale")
            }
            defer { sending.remove(id); outboxAttempts.removeValue(forKey: id); outboxAdmitted.remove(id) }
            let prior = operation["attemptPreviousState"] as! String
            if okay.boolValue {
                operation["state"] = "acknowledged"; operation["result"] = result["value"] ?? NSNull()
                operation.removeValue(forKey: "error")
            } else {
                // A typed RPC Fail alone can follow server commit. The transport must prove non-acceptance.
                let notAccepted = (result["_outboxNotAccepted"] as? NSNumber).map { CFGetTypeID($0) == CFBooleanGetTypeID() && $0.boolValue } == true
                let safelyUnsent = knownUnsent && !outboxAdmitted.contains(id)
                operation["state"] = safelyUnsent ? prior : prior == "uncertain" ? "uncertain" : notAccepted ? "rejected" : "uncertain"
                operation["error"] = result["error"] as? [String: Any] ?? [:]
                operation.removeValue(forKey: "result")
            }
            operation["revision"] = issuedRevision + 1
            commands[id] = operation; value["operations"] = commands
            durableOutbox.removeValue(forKey: id)
            try saveOutboxJournal(value); durableOutbox[id] = issuedRevision + 1
            return operation
        }
    }
    /// Explicit recovery establishes durability only; issued/uncertain still require exact server resolution.
    func recoverOutboxDelivery(_ id: String, revision: Int) throws -> [String: Any] {
        try locked {
            let value = try store()
            guard let operation = operations(value)[id], operation["kind"] as? String == "outbox",
                  operation["revision"] as? Int == revision, !sending.contains(id), outboxAttempts[id] == nil else {
                throw refusal("The delivery receipt changed or is still sending.", kind: "stale")
            }
            durableOutbox.removeValue(forKey: id)
            try saveOutboxJournal(value); durableOutbox[id] = revision
            return ["operation": operation, "durable": true]
        }
    }
    /// ACK remains immutable. This only removes the original queue row through its existing FIFO owner.
    func completeOutboxDelivery(_ request: [String: Any], answer: @escaping T3MobileOutboxOwner.Answer) {
        typealias Object = [String: Any]
        var pending: Object?
        var inspect: (String, String)?
        var immediate: Object?
        var registered = false
        let id = request["operationId"] as? String ?? ""
        do {
            try locked {
                var value = try store(), commands = operations(value)
                guard var operation = commands[id], operation["kind"] as? String == "outbox",
                      operation["state"] as? String == "acknowledged",
                      T3OutboxDeliveryReceipt.integer(request["revision"], positive: true), let revision = request["revision"] as? Int,
                      let current = operation["revision"] as? Int,
                      let attempt = operation["attemptRevision"] as? Int,
                      revision >= attempt + 1, revision <= current else {
                    throw refusal("Choose the acknowledged delivery receipt.", kind: "stale")
                }
                if outboxCleanupAnswers[id] != nil { outboxCleanupAnswers[id]!.append(answer); return }
                durableOutbox.removeValue(forKey: id)
                try saveOutboxJournal(value); durableOutbox[id] = current
                let existing = operation["cleanup"] as? Object
                if operation["stage"] as? String == "settings-sync" {
                    if existing == nil {
                        operation["revision"] = current + 1
                        operation["cleanup"] = ["ackRevision": attempt + 1, "intentRevision": current + 1, "phase": "settings", "outcome": NSNull()]
                        commands[id] = operation; value["operations"] = commands
                        durableOutbox.removeValue(forKey: id)
                        try saveOutboxJournal(value); durableOutbox[id] = current + 1
                    }
                    immediate = cleanupReply(operation); return
                }
                var rebind = false
                if let existing {
                    let phase = existing["phase"] as! String
                    if request["retryCleanupRevision"] != nil {
                        guard request["retryCleanupRevision"] as? Int == current, revision == current,
                              ["edited", "failed", "not-started"].contains(phase) else {
                            throw refusal("Resolve the exact cleanup before choosing a fresh attempt.", kind: "stale")
                        }
                        try outboxOwner.deliveryCleanupRetiredLocked(operation["messageId"] as! String,
                            mutation: existing["mutationId"] as! String)
                        _ = try outboxOwner.deliveryRecordLocked(["ownerEpoch": request["ownerEpoch"] ?? NSNull(),
                            "messageId": operation["messageId"]!, "record": operation["record"]!,
                            "expectedToken": operation["rowToken"]!, "expectedRevision": operation["rowRevision"]!])
                        rebind = true
                    } else if ["removed", "edited", "failed", "not-started"].contains(phase) {
                        immediate = cleanupReply(operation); return
                    } else {
                        inspect = (operation["messageId"] as! String, existing["mutationId"] as! String)
                    }
                }
                if existing == nil || rebind {
                    guard let mutation = request["mutationId"] as? String, !mutation.isEmpty,
                          let epoch = request["ownerEpoch"] as? String, !epoch.isEmpty,
                          existing?["mutationId"] as? String != mutation else {
                        throw refusal("Choose a fresh cleanup mutation identity.", kind: "Outbox")
                    }
                    let cleanup: Object = ["mutationId": mutation, "ownerEpoch": epoch, "ackRevision": attempt + 1,
                        "intentRevision": current + 1, "phase": "pending", "outcome": NSNull()]
                    pending = ["action": "mutate", "operation": "remove", "ownerEpoch": epoch, "mutationId": mutation,
                        "messageId": operation["messageId"]!, "expectedToken": operation["rowToken"]!,
                        "expectedRevision": operation["rowRevision"]!, "requireUnheld": true,
                        "deliveryCleanup": ["operationId": id, "ackRevision": attempt + 1, "record": operation["record"]!]]
                    guard T3MobileOutbox.validateMutation(pending!, id: operation["messageId"] as! String) else { throw refusal("The cleanup mutation identity is invalid.", kind: "Outbox") }
                    operation["cleanup"] = cleanup; operation["revision"] = current + 1
                    commands[id] = operation; value["operations"] = commands
                    durableOutbox.removeValue(forKey: id)
                    try saveOutboxJournal(value); durableOutbox[id] = current + 1
                }
                outboxCleanupAnswers[id] = [answer]; registered = true
            }
            if let immediate { answer(.success(immediate)); return }
            guard registered else { return }
            if let pending {
                outboxOwner.submitDeliveryCleanup(pending) { [self] result in
                    switch result {
                    case .success(let outcome): finishOutboxCleanup(id, evidence: ["status": "outcome", "outcome": outcome])
                    case .failure:
                        // Admission may have refused after the durable intent. FIFO inspection proves whether it started.
                        outboxOwner.inspectDeliveryCleanup(pending["messageId"] as! String, mutation: pending["mutationId"] as! String) { [self] result in
                            finishOutboxCleanup(id, inspection: result)
                        }
                    }
                }
            } else if let inspect {
                outboxOwner.inspectDeliveryCleanup(inspect.0, mutation: inspect.1) { [self] result in finishOutboxCleanup(id, inspection: result) }
            }
        } catch { answer(.failure(error)) }
    }
    private func cleanupReply(_ operation: [String: Any]) -> [String: Any] {
        let cleanup = operation["cleanup"] as! [String: Any]
        return ["operation": operation, "durable": true, "cleanup": cleanup["phase"]!, "outcome": cleanup["outcome"]!]
    }
    private func finishOutboxCleanup(_ id: String, inspection: Result<[String: Any], Error>) {
        switch inspection {
        case .success(let evidence): finishOutboxCleanup(id, evidence: evidence)
        case .failure(let error):
            let answers = locked { outboxCleanupAnswers.removeValue(forKey: id) ?? [] }
            for answer in answers { answer(.failure(error)) }
        }
    }
    private func finishOutboxCleanup(_ id: String, evidence: [String: Any]) {
        let result: Result<[String: Any], Error>
        do {
            result = .success(try locked {
                var value = try store(), commands = operations(value)
                guard var operation = commands[id], var cleanup = operation["cleanup"] as? [String: Any],
                      operation["state"] as? String == "acknowledged" else { throw refusal("The cleanup receipt is missing.", kind: "Persistence") }
                let outcome = evidence["outcome"] as? [String: Any]
                let status = outcome?["status"] as? String
                let phase: String
                if status == "committed", T3MobileOutbox.jsonEqual(outcome?["removed"], operation["record"]) { phase = "removed" }
                else if status == "stale" { phase = "edited" }
                else if status == "failed" { phase = "failed" }
                else if evidence["status"] as? String == "not-started" { phase = "not-started" }
                else { phase = "uncertain" }
                cleanup["phase"] = phase; cleanup["outcome"] = outcome.map { $0 as Any } ?? NSNull()
                operation["cleanup"] = cleanup; operation["revision"] = (operation["revision"] as! Int) + 1
                guard T3OutboxDeliveryReceipt.valid(operation, id: id, inlineSource: operations(value)[(operation["inlineSource"] as? [String: Any])?["operationId"] as? String ?? ""]) else { throw refusal("The cleanup outcome does not match its acknowledged receipt.", kind: "Persistence") }
                commands[id] = operation; value["operations"] = commands
                if phase == "removed" { enqueueReleases((operation["record"] as! [String: Any])["attachments"] as! [[String: Any]], in: &value) }
                durableOutbox.removeValue(forKey: id)
                try saveOutboxJournal(value); durableOutbox[id] = operation["revision"] as? Int
                return cleanupReply(operation)
            })
        } catch { result = .failure(error) }
        let answers = locked { outboxCleanupAnswers.removeValue(forKey: id) ?? [] }
        for answer in answers { answer(result) }
    }
    func beginSend(_ id: String, revision: Int, origin: String, environment: String) throws -> [String: Any] {
        try locked {
            var value = try store(), commands = operations(value)
            guard var operation = commands[id], operation["kind"] == nil, operation["revision"] as? Int == revision,
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
            var value = try store(), commands = operations(value)
            guard var operation = commands[id], operation["kind"] == nil else { throw refusal("The queued operation is missing.", kind: "Persistence") }
            defer { sending.remove(id) }
            let success = result["ok"] as? Bool == true
            operation["state"] = success ? "acknowledged" : knownUnsent ? "reserved" : result["_definitiveFailure"] as? Bool == true ? "rejected" : "uncertain"
            operation["revision"] = (operation["revision"] as? Int ?? 0) + 1
            if success { operation["result"] = result["value"] ?? NSNull(); operation.removeValue(forKey: "error") }
            else { operation["error"] = result["error"] ?? [:] }
            commands[id] = operation; value["operations"] = commands; try save(value); return operation
        }
    }
    /// The final durability stamp covers this whole validated journal, including its inline ACK.
    private func inlineReleased(_ source: [String: Any], commands: [String: [String: Any]]) -> Bool {
        guard let commandId = (source["record"] as? [String: Any])?["commandId"] as? String,
              let final = commands[commandId], T3OutboxInlineFinal.matches(final, source: source) else { return false }
        return durableDraftHandoffs.contains(commandId) || final["state"] as? String == "acknowledged"
            && (final["cleanup"] as? [String: Any])?["phase"] as? String == "removed"
            && durableOutbox[commandId] == final["revision"] as? Int
    }
    private func inlinePayloadBlocked(_ method: String, payload: Any?, origin: String, environment: String, commands: [String: [String: Any]]) -> Bool {
        guard let payload = payload as? [String: Any] else { return false }
        let message = method == "orchestration.launchThread" ? payload["initialMessage"] as? [String: Any] : payload
        return commands.values.contains { source in
            guard source["kind"] as? String == "outbox-inline", source["state"] as? String != "retired",
                  source["origin"] as? String == origin, source["environmentId"] as? String == environment,
                  let record = source["record"] as? [String: Any] else { return false }
            let sameMessage = payload["threadId"] as? String == record["threadId"] as? String
                && message?["messageId"] as? String == record["messageId"] as? String
            return (sameMessage && !inlineReleased(source, commands: commands))
                || (method != "assets.persistChatAttachments" && payload["commandId"] as? String == record["commandId"] as? String)
        }
    }
    func admit(method: String, origin: String, environment: String, journal: String? = nil, payload: Any? = nil, inlineAttempt: UUID? = nil) throws -> UUID? {
        if let inlineAttempt {
            return try locked {
                guard method == "assets.persistChatAttachments", let journal, let payload,
                      let operation = operations(try store())[journal], operation["kind"] as? String == "outbox-inline",
                      operation["state"] as? String == "issued", operation["origin"] as? String == origin, operation["environmentId"] as? String == environment,
                      var attempt = inlineSendAttempts[journal], attempt["token"] as? UUID == inlineAttempt,
                      attempt["revision"] as? Int == operation["revision"] as? Int, attempt["admitted"] as? Bool != true,
                      try T3OutboxInlinePayload.digest(method: method, payload: payload) == attempt["digest"] as? String else {
                    throw refusal("The inline RPC does not match its one prepared attempt.", kind: "stale")
                }
                attempt["admitted"] = true; inlineSendAttempts[journal] = attempt
                let token = UUID(); active[token] = origin + "\n" + environment; return token
            }
        }
        if !Self.methods.contains(method) {
            if let journal {
                try locked {
                    if ["outbox", "outbox-inline"].contains(operations(try store())[journal]?["kind"] as? String ?? "") {
                        throw refusal("The delivery RPC method does not match its receipt.", kind: "stale")
                    }
                }
            }
            if method == "assets.persistChatAttachments" {
                return try locked {
                    let commands = operations(try store())
                    if inlinePayloadBlocked(method, payload: payload, origin: origin, environment: environment, commands: commands)
                        || inlineSendEnvironmentLocked(origin, environment) || inlinePreparations.values.contains(where: { ($0["prepared"] as? [String: Any])?["origin"] as? String == origin && ($0["prepared"] as? [String: Any])?["environmentId"] as? String == environment })
                        || commands.values.contains(where: { $0["kind"] as? String == "outbox-inline" && $0["origin"] as? String == origin && $0["environmentId"] as? String == environment && unresolved($0) }) {
                        throw refusal("The inline asset stage already owns this environment.", kind: "Busy")
                    }
                    // Existing ordinary asset RPCs also own an active lease until their transport callback.
                    let token = UUID(); active[token] = origin + "\n" + environment; return token
                }
            }
            return nil
        }
        return try locked {
            let commands = operations(try store())
            guard !inlineSendEnvironmentLocked(origin, environment), !inlinePreparations.values.contains(where: { ($0["prepared"] as? [String: Any])?["origin"] as? String == origin && ($0["prepared"] as? [String: Any])?["environmentId"] as? String == environment }) else {
                throw refusal("Inline image preparation owns this environment.", kind: "Busy")
            }
            if let journal {
                guard sending.contains(journal), let operation = commands[journal], operation["origin"] as? String == origin, operation["environmentId"] as? String == environment,
                      operation["state"] as? String == "issued" else { throw refusal("The queued send is no longer admitted.", kind: "stale") }
                if operation["kind"] as? String == "outbox" {
                    guard outboxAttempts[journal] == operation["revision"] as? Int, !outboxAdmitted.contains(journal),
                          operation["method"] as? String == method,
                          T3MobileOutbox.jsonEqual(operation["payload"], payload) else {
                        throw refusal("The delivery RPC does not match its immutable receipt.", kind: "stale")
                    }
                    outboxAdmitted.insert(journal)
                }
            } else if inlinePayloadBlocked(method, payload: payload, origin: origin, environment: environment, commands: commands)
                || commands.values.contains(where: { $0["origin"] as? String == origin && $0["environmentId"] as? String == environment && unresolved($0) }) {
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
            for entry in inlineSendPreparations.values {
                let operation = entry["operation"] as! [String: Any]
                if pending[operation["environmentId"] as! String] != nil { throw refusal("Finish inline expansion before saving another pending operation.", kind: "Busy") }
            }
            for entry in inlinePreparations.values {
                let prepared = entry["prepared"] as! [String: Any]
                if pending[prepared["environmentId"] as! String] != nil { throw refusal("Finish inline image preparation before saving another pending operation.", kind: "Busy") }
            }
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
                  let id = request["operationId"] as? String, let operation = commands[id], operation["kind"] == nil, operation["owner"] as? String == owner,
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
            let owned = commands.values.filter { $0["kind"] == nil && $0["owner"] as? String == owner }
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
    // Captured independent launches retain bytes even after the current editor removes a chip.
    // Quarantined extension data is unknown ownership, never an empty attachment inventory.
    private func launchReceiptsHold(_ identifier: String, preferences: [String: Any]) throws -> Bool {
        guard let raw = preferences["mobileNewTaskDrafts"] else { return false }
        let unknown = refusal("Independent draft attachment ownership is invalid.", kind: "Persistence")
        guard let registry = raw as? [String: Any], let version = registry["version"] as? NSNumber,
              CFGetTypeID(version) != CFBooleanGetTypeID(), version.doubleValue == 1,
              registry["records"] is [String: [String: Any]], let receipts = registry["receipts"] as? [String: [String: Any]],
              let claims = registry["claims"] as? [String: String], registry["fileReleases"] is [String],
              claims.keys.allSatisfy({ receipts[$0] != nil }) else { throw unknown }
        var found = false
        for receipt in receipts.values {
            guard let images = receipt["images"] as? [[String: Any]], let files = receipt["files"] as? [[String: Any]] else { throw unknown }
            for attachment in images + files {
                guard let id = attachment["id"] as? String, id.utf16.count == 36, UUID(uuidString: id) != nil else { throw unknown }
                if id.lowercased() == identifier { found = true }
            }
        }
        return found
    }
    // A saved update owns its captured bytes before native mutation admission,
    // even when newer draft edits have removed them. Unknown metadata is not empty.
    private func pendingEditorsHold(_ identifier: String, preferences: [String: Any]) throws -> Bool {
        guard let raw = preferences["mobilePendingTaskEditors"] else { return false }
        let unknown = refusal("Pending editor attachment ownership is invalid.", kind: "Persistence")
        func number(_ value: Any?, minimum: Int = 0) -> Int? {
            guard let n = value as? NSNumber, CFGetTypeID(n) != CFBooleanGetTypeID(),
                  n.doubleValue.isFinite, n.doubleValue.rounded() == n.doubleValue,
                  n.doubleValue >= Double(minimum), n.doubleValue <= 9_007_199_254_740_991 else { return nil }
            return n.intValue
        }
        func text(_ value: Any?) -> String? {
            guard let value = value as? String, !value.isEmpty,
                  value.trimmingCharacters(in: T3MobileOutbox.trimCharacters) == value else { return nil }
            return value
        }
        let ownerFields = ["origin", "environmentId", "threadId", "messageId", "commandId"]
        func ownerKey(_ owner: [String: String]) throws -> String {
            let fields = try ownerFields.map { field in
                let data = try JSONSerialization.data(withJSONObject: [owner[field]!], options: [.withoutEscapingSlashes])
                let quoted = String(decoding: data, as: UTF8.self).dropFirst().dropLast()
                return "\"\(field)\":\(quoted)"
            }
            return "{" + fields.joined(separator: ",") + "}"
        }
        guard let registry = raw as? [String: Any], Set(registry.keys) == Set(["version", "markers"]),
              number(registry["version"]) == 1, let markers = registry["markers"] as? [String: [String: Any]] else { throw unknown }
        var found = false, messages = Set<String>()
        for (key, marker) in markers {
            guard Set(marker.keys) == Set(["version", "owner", "session", "revision", "draftKey", "contentRevision", "baseline", "pending"]),
                  number(marker["version"]) == 1, let owner = marker["owner"] as? [String: String],
                  Set(owner.keys) == Set(ownerFields), ownerFields.allSatisfy({ text(owner[$0]) != nil }),
                  key == (try ownerKey(owner)), text(marker["session"]) != nil, number(marker["revision"], minimum: 1) != nil,
                  let revision = number(marker["contentRevision"]), let draft = marker["draftKey"] as? String,
                  draft == "new-task:pending-\(owner["messageId"]!)", draft.range(of: "^new-task:[A-Za-z0-9_-]{1,128}$", options: .regularExpression) != nil,
                  messages.insert(owner["messageId"]!).inserted,
                  let baseline = marker["baseline"] as? [String: Any], Set(baseline.keys) == Set(["record", "token", "revision"]),
                  let token = text(baseline["token"]), let baselineRevision = number(baseline["revision"]),
                  let record = baseline["record"] as? [String: Any], T3MobileOutbox.validateRecord(record), record["creation"] is [String: Any],
                  ownerFields.allSatisfy({ record[$0] as? String == owner[$0] }) else { throw unknown }
            var records = [record]
            if !(marker["pending"] is NSNull) {
                guard let pending = marker["pending"] as? [String: Any], Set(pending.keys) == Set(["mutationId", "contentRevision", "request"]),
                      let contentRevision = number(pending["contentRevision"]), contentRevision <= revision,
                      let mutation = text(pending["mutationId"]), let request = pending["request"] as? [String: Any],
                      Set(request.keys) == Set(["ownerEpoch", "mutationId", "messageId", "operation", "record", "expectedToken", "expectedRevision", "requireUnheld"]),
                      let epoch = text(request["ownerEpoch"]), mutation.hasPrefix(epoch + ":"),
                      request["mutationId"] as? String == mutation, request["messageId"] as? String == owner["messageId"],
                      request["operation"] as? String == "update", let unheld = request["requireUnheld"] as? NSNumber,
                      CFGetTypeID(unheld) == CFBooleanGetTypeID(), !unheld.boolValue,
                      request["expectedToken"] as? String == token, number(request["expectedRevision"]) == baselineRevision,
                      let proposed = request["record"] as? [String: Any], T3MobileOutbox.validateRecord(proposed), proposed["creation"] is [String: Any],
                      ownerFields.allSatisfy({ proposed[$0] as? String == owner[$0] }), proposed["createdAt"] as? String == record["createdAt"] as? String else { throw unknown }
                let suffix = String(mutation.dropFirst(epoch.count + 1))
                guard let sequence = Int(suffix), sequence > 0, sequence <= 9_007_199_254_740_991,
                      String(sequence) == suffix else { throw unknown }
                records.append(proposed)
            }
            if records.contains(where: { ($0["attachments"] as! [[String: Any]]).contains { ($0["id"] as! String).lowercased() == identifier } }) { found = true }
        }
        return found
    }
    private func held(_ identifier: String, value: [String: Any]) throws -> Bool {
        let preparing = inlinePreparations.values.contains { entry in
            let prepared = entry["prepared"] as! [String: Any]
            return (prepared["attachmentIDs"] as! [String]).contains { $0.lowercased() == identifier }
        }
        if preparing { return true }
        let owned = records(value).values.contains { record in
            (record["attachments"] as? [[String: Any]] ?? []).contains { ($0["id"] as? String)?.lowercased() == identifier }
        } || operations(value).values.contains { operation in
            if operation["kind"] as? String == "outbox-inline", inlineReleased(operation, commands: operations(value)) { return false }
            if operation["kind"] as? String == "outbox", let id = operation["operationId"] as? String,
               durableDraftHandoffs.contains(id) { return false }
            if ["outbox", "outbox-inline"].contains(operation["kind"] as? String ?? ""),
               (operation["state"] as? String == "retired" || ["removed", "settings"].contains((operation["cleanup"] as? [String: Any])?["phase"] as? String ?? "")),
               let id = operation["operationId"] as? String, durableOutbox[id] == operation["revision"] as? Int { return false }
            return (operation["attachmentIDs"] as? [String] ?? []).map { $0.lowercased() }.contains(identifier)
        }
        if owned { return true }
        if outboxOwner.protects(identifier) { return true }
        if try outboxStore.inventoryHolds(identifier) { return true }
        let preferencesValue = try readJSON(preferences)
        if try launchReceiptsHold(identifier, preferences: preferencesValue) { return true }
        if try pendingEditorsHold(identifier, preferences: preferencesValue) { return true }
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
