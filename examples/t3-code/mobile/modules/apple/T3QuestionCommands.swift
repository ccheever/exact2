// Question receipt transitions entered only under T3MobileQueuedEdit's preference mutex.
// @ref llp/1109.005-composer-and-transcript.decision.md#scratch-tasks-and-queue-boundaries
import Foundation
import CoreFoundation

struct T3QuestionCommandFailure: Error, LocalizedError {
    let message: String
    var kind = "QuestionJournal"
    var errorDescription: String? { message }
}
struct T3QuestionConnection {
    let home: String; let endpoint: String; let environment: String; let generation: Int; let catalog: Set<String>
    func verify(_ request: [String: Any]) throws {
        guard request["expectedOrigin"] as? String == endpoint,
              request["expectedEnvironmentId"] as? String == environment,
              T3QuestionCommands.integer(request["generation"]) == generation else {
            throw T3QuestionCommandFailure(message: "The question connection changed.", kind: "stale")
        }
        _ = try T3QuestionScope(origin: home, environment: environment, thread: "verification", request: "verification")
    }
}

final class T3QuestionCommands {
    static let process = T3QuestionProcess.id
    static let recordsKey = "mobileQuestionCommands", metadataKey = "mobileQuestionMetadata"
    static let limit = 16 * 1024 * 1024
    private let read: () throws -> [String: Any]
    private let save: ([String: Any]) throws -> Void
    private let inventory: () -> T3QuestionAttachments
    private let rootKey: String
    private var observers: [UUID: () -> Void] = [:]
    private var notificationsPending = false
    var wake: Int { T3QuestionProcess.wake(rootKey) }
    func observe(_ callback: @escaping () -> Void) -> UUID { let id = UUID(); observers[id] = callback; return id }
    func unobserve(_ id: UUID) { observers.removeValue(forKey: id) }
    func committed() { notificationsPending = true; T3QuestionProcess.changed(rootKey) }
    func notifications() -> [() -> Void] { guard notificationsPending else { return [] }; notificationsPending = false; return Array(observers.values) }
    private var attempts: [String: [String: Any]] = [:]
    private var outcomes: [String: [String: Any]] = [:]
    var hasWork: Bool { !attempts.isEmpty || !outcomes.isEmpty }
    init(root: URL, read: @escaping () throws -> [String: Any], save: @escaping ([String: Any]) throws -> Void,
         inventory: @escaping () -> T3QuestionAttachments) { self.rootKey = T3QuestionProcess.key(root); self.read = read; self.save = save; self.inventory = inventory }
    static func integer(_ raw: Any?) -> Int? {
        guard let n = raw as? NSNumber, CFGetTypeID(n) != CFBooleanGetTypeID(), n.doubleValue.isFinite,
              n.doubleValue >= 0, n.doubleValue <= 9_007_199_254_740_991, n.doubleValue.rounded(.towardZero) == n.doubleValue else { return nil }
        return n.intValue
    }
    static func bool(_ raw: Any?) -> Bool? {
        guard let n = raw as? NSNumber, CFGetTypeID(n) == CFBooleanGetTypeID() else { return nil }; return n.boolValue
    }
    private static func nextRevision(_ raw: Any?) throws -> Int {
        guard let value = integer(raw), value < 9_007_199_254_740_991 else { throw bad() }; return value + 1
    }
    static func string(_ raw: Any?, limit: Int = 65_536) throws -> String {
        guard let s = raw as? String, !s.isEmpty, s.utf8.count <= limit, !s.contains("\0") else { throw bad() }; return s
    }
    static func fields(_ raw: [String: Any], required: Set<String>, optional: Set<String> = []) throws {
        let keys = Set(raw.keys); guard required.isSubset(of: keys), keys.isSubset(of: required.union(optional)) else { throw bad() }
    }
    static func bad(_ text: String = "The saved question receipt is invalid.") -> T3QuestionCommandFailure {
        .init(message: text, kind: "Persistence")
    }
    static func encoded(_ value: Any) throws -> Data {
        let data = try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys, .withoutEscapingSlashes, .fragmentsAllowed])
        guard data.count <= limit else { throw bad("The question receipt is too large.") }; return data
    }
    static func equal(_ a: Any?, _ b: Any?) -> Bool {
        guard let a, let b else { return a == nil && b == nil }
        if let n = a as? NSNumber, let m = b as? NSNumber {
            return CFGetTypeID(n) == CFGetTypeID(m) && n == m
        }
        if let a = a as? [String: Any], let b = b as? [String: Any] {
            return Set(a.keys) == Set(b.keys) && a.allSatisfy { equal($0.value, b[$0.key]) }
        }
        if let a = a as? [Any], let b = b as? [Any] { return a.count == b.count && zip(a, b).allSatisfy { equal($0, $1) } }
        return (a as? NSObject)?.isEqual(b) == true
    }
    static func digest(_ value: Any) throws -> String { t3QuestionDigest(try encoded(value)) }
    private static func hash(_ raw: Any?) -> Bool { (raw as? String).map { $0.count == 64 && $0.allSatisfy { $0.isHexDigit && !$0.isUppercase } } == true }
    static func payload(_ record: [String: Any]) throws -> [String: Any] {
        try fields(record, required: ["origin", "environmentId", "threadId", "requestId", "signature", "pending", "drafts", "phase"], optional: ["retiring"])
        for key in ["origin", "environmentId", "threadId", "requestId", "signature"] { _ = try string(record[key]) }
        _ = try T3QuestionScope(origin: record["origin"] as! String, environment: record["environmentId"] as! String,
                                thread: record["threadId"] as! String, request: record["requestId"] as! String)
        guard ["preflight", "issued", "uncertain", "unsent", "settled"].contains(record["phase"] as? String ?? ""),
              record["retiring"] == nil || bool(record["retiring"]) != nil,
              let pending = record["pending"] as? [String: Any], let command = pending["payload"] as? [String: Any],
              let drafts = record["drafts"] as? [String: Any] else { throw bad() }
        try fields(pending, required: ["method", "payload", "description", "threadId", "text", "uncertain"])
        guard pending["method"] as? String == "orchestration.dispatchCommand", bool(pending["uncertain"]) != nil,
              pending["description"] is String, pending["text"] as? String == "",
              pending["threadId"] as? String == record["threadId"] as? String,
              command["threadId"] as? String == record["threadId"] as? String else { throw bad() }
        _ = try string(command["commandId"])
        switch command["type"] as? String {
        case "runtime-request.respond":
            try fields(command, required: ["type", "threadId", "requestId", "commandId", "answers"], optional: ["attachmentsByQuestionId"])
            guard command["requestId"] as? String == record["requestId"] as? String,
                  let answers = command["answers"] as? [String: Any], !answers.isEmpty, answers.count <= 100 else { throw bad() }
            for (id, answer) in answers {
                _ = try string(id, limit: 2048)
                guard answer is String || (answer as? [String]).map({ !$0.isEmpty && $0.count <= 100 }) == true else { throw bad() }
            }
            // The upload owner is not implemented. No caller can supply a ready server attachment.
            if let mapping = command["attachmentsByQuestionId"] {
                guard let mapping = mapping as? [String: Any], mapping.isEmpty else { throw bad("Native question upload provenance is required before sending attachments.") }
            }
        case "thread.user-input.dismiss":
            try fields(command, required: ["type", "threadId", "requestId", "commandId"])
            guard command["requestId"] as? String == record["requestId"] as? String else { throw bad() }
        case "run.interrupt":
            try fields(command, required: ["type", "threadId", "runId", "holdQueue", "commandId"])
            _ = try string(command["runId"]); guard bool(command["holdQueue"]) == true else { throw bad() }
        default: throw bad()
        }
        for (key, draft) in drafts {
            _ = try string(key, limit: 2048); guard let draft = draft as? [String: Any] else { throw bad() }
            try fields(draft, required: ["selected", "custom"])
            guard draft["selected"] is [String], draft["custom"] is String else { throw bad() }
        }
        return command
    }
    static func recordDigest(_ record: [String: Any]) throws -> String {
        var immutable = record; immutable.removeValue(forKey: "phase"); immutable.removeValue(forKey: "retiring")
        var pending = immutable["pending"] as! [String: Any]; pending.removeValue(forKey: "uncertain"); immutable["pending"] = pending
        return try digest(immutable)
    }
    private static func metadata(_ record: [String: Any], home: String?) throws -> [String: Any] {
        let bytes = try encoded(payload(record)); var meta: [String: Any] = ["recordRevision": 0, "recordDigest": try recordDigest(record),
            "payloadUTF8": String(decoding: bytes, as: UTF8.self), "payloadDigest": t3QuestionDigest(bytes),
            "acceptanceUncertain": ["issued", "uncertain"].contains(record["phase"] as? String ?? "")]
        if let home { meta["homeOrigin"] = home }; return meta
    }
    private static func validateMeta(_ meta: [String: Any], record: [String: Any], prepared: Bool = false) throws {
        try fields(meta, required: ["recordRevision", "recordDigest", "payloadUTF8", "payloadDigest"],
                   optional: ["homeOrigin", "pinId", "pinDigest", "requestIncarnation", "inventoryRevision", "currentAttempt", "releasePending", "releaseToken", "releaseAck", "questionCatalog", "acceptanceUncertain", "requestRetired"])
        guard integer(meta["recordRevision"]) != nil, hash(meta["recordDigest"]), hash(meta["payloadDigest"]), let opaque = meta["payloadUTF8"] as? String,
              t3QuestionDigest(Data(opaque.utf8)) == meta["payloadDigest"] as? String,
              equal(try JSONSerialization.jsonObject(with: Data(opaque.utf8)), try payload(record)),
              try recordDigest(record) == meta["recordDigest"] as? String else { throw bad() }
        if let home = meta["homeOrigin"] { _ = try T3QuestionScope(origin: string(home), environment: record["environmentId"] as! String, thread: record["threadId"] as! String, request: record["requestId"] as! String) }
        if meta["pinId"] != nil {
            for key in ["pinId", "requestIncarnation"] { guard UUID(uuidString: try string(meta[key])) != nil else { throw bad() } }
            guard let digest = meta["pinDigest"] as? String, digest.count == 64, digest.allSatisfy({ $0.isHexDigit && !$0.isUppercase }),
                  integer(meta["inventoryRevision"]) != nil, meta["homeOrigin"] is String,
                  let catalog = meta["questionCatalog"] as? [String: String], !catalog.isEmpty, catalog.count <= 100,
                  catalog.allSatisfy({ !$0.key.isEmpty && $0.key.utf8.count <= 2048 && !$0.value.isEmpty && $0.value.utf8.count <= 65_536 }) else { throw bad() }
        } else if ["pinDigest", "requestIncarnation", "inventoryRevision", "questionCatalog"].contains(where: { meta[$0] != nil }) { throw bad() }
        guard !prepared || meta["pinId"] != nil else { throw bad("Prepared question metadata requires its native pin binding.") }
        if let attempt = meta["currentAttempt"] as? [String: Any] {
            try fields(attempt, required: ["id", "rpcId", "generation", "endpoint", "state"])
            for key in ["id", "rpcId", "endpoint"] { _ = try string(attempt[key]) }
            guard integer(attempt["generation"]) != nil, ["live", "settled", "unsent", "uncertain"].contains(attempt["state"] as? String ?? "") else { throw bad() }
            guard UUID(uuidString: attempt["id"] as! String) != nil,
                  attempt["state"] as? String != "live" || record["phase"] as? String == "issued" else { throw bad() }
        } else if meta["currentAttempt"] != nil { throw bad() }
        for key in ["releasePending", "releaseAck", "acceptanceUncertain", "requestRetired"] where meta[key] != nil { guard bool(meta[key]) != nil else { throw bad() } }
        if let token = meta["releaseToken"] { guard UUID(uuidString: try string(token)) != nil, bool(meta["releasePending"]) == true else { throw bad() } }
        guard bool(meta["releaseAck"]) != true || meta["releaseToken"] != nil else { throw bad() }
        guard bool(meta["releasePending"]) != true || ["settled", "unsent"].contains(record["phase"] as? String ?? ""),
              bool(meta["requestRetired"]) != true || record["phase"] as? String == "settled" else { throw bad() }
        if prepared { guard record["phase"] as? String == "preflight", meta["currentAttempt"] == nil,
            meta["releaseToken"] == nil, bool(meta["releasePending"]) != true, bool(meta["requestRetired"]) != true else { throw bad() } }
    }
    private static func decode(_ document: [String: Any]) throws -> ([String: [String: Any]], [String: Any]?) {
        guard document[recordsKey] == nil || document[recordsKey] is [String: [String: Any]] else { throw bad() }
        let records = document[recordsKey] as? [String: [String: Any]] ?? [:]
        for (id, record) in records { guard try payload(record)["commandId"] as? String == id else { throw bad() } }
        guard let raw = document[metadataKey] else { return (records, nil) }
        guard let meta = raw as? [String: Any] else { throw bad() }
        try fields(meta, required: ["epoch", "revision", "process", "commands", "prepared", "tombstones", "seals", "mutations"])
        for key in ["epoch", "process"] { guard UUID(uuidString: try string(meta[key])) != nil else { throw bad() } }
        guard integer(meta["revision"]) != nil, let commands = meta["commands"] as? [String: [String: Any]],
              Set(commands.keys) == Set(records.keys), let prepared = meta["prepared"] as? [String: [String: Any]],
              let tombstones = meta["tombstones"] as? [String: [String: Any]], let seals = meta["seals"] as? [String: [String: Any]], let mutations = meta["mutations"] as? [String: [String: Any]] else { throw bad() }
        for (id, command) in commands { try validateMeta(command, record: records[id]!) }
        for (id, item) in prepared {
            try fields(item, required: ["record", "metadata", "epoch", "process"])
            for key in ["epoch", "process"] { guard UUID(uuidString: try string(item[key])) != nil else { throw bad() } }
            guard let record = item["record"] as? [String: Any], let metadata = item["metadata"] as? [String: Any],
                  try payload(record)["commandId"] as? String == id else { throw bad() }; try validateMeta(metadata, record: record, prepared: true)
        }
        guard Set(tombstones.keys).isDisjoint(with: records.keys), Set(prepared.keys).isDisjoint(with: records.keys) else { throw bad() }
        for (_, item) in tombstones {
            try fields(item, required: ["recordDigest", "payloadDigest", "releaseToken", "epoch", "process"], optional: ["pinId", "pinDigest"])
            for raw in item.values { _ = try string(raw) }
            guard hash(item["recordDigest"]), hash(item["payloadDigest"]), UUID(uuidString: item["releaseToken"] as! String) != nil,
                  UUID(uuidString: item["epoch"] as! String) != nil, UUID(uuidString: item["process"] as! String) != nil,
                  (item["pinId"] == nil && item["pinDigest"] == nil) || (UUID(uuidString: item["pinId"] as? String ?? "") != nil && hash(item["pinDigest"])) else { throw bad() }
        }
        for (id, mutation) in mutations {
            _ = try string(id)
            try fields(mutation, required: ["digest", "epoch", "revision", "commandId", "action"])
            for key in ["digest", "epoch", "commandId", "action"] { _ = try string(mutation[key]) }
            guard let revision = integer(mutation["revision"]), revision <= integer(meta["revision"])!, hash(mutation["digest"]),
                  UUID(uuidString: mutation["epoch"] as! String) != nil,
                  ["prepare", "create", "bind", "phase", "terminal", "release", "ack", "remove", "seal"].contains(mutation["action"] as! String) else { throw bad() }
        }
        for (epoch, seal) in seals {
            guard UUID(uuidString: epoch) != nil else { throw bad() }
            try fields(seal, required: ["process", "revision"])
            guard UUID(uuidString: try string(seal["process"])) != nil, integer(seal["revision"]) != nil else { throw bad() }
        }
        return (records, meta)
    }
    static func merge(previous: [String: Any], incoming: [String: Any], owned: Set<String> = []) throws -> [String: Any] {
        let (records, metadata) = try decode(previous); var next = incoming
        next.removeValue(forKey: "mobileQuestionRecovery")
        next[recordsKey] = records; next[metadataKey] = metadata
        let ids = Set(records.keys).union((metadata?["tombstones"] as? [String: Any] ?? [:]).keys).union((metadata?["prepared"] as? [String: Any] ?? [:]).keys).union(owned)
        guard incoming["pending"] == nil || incoming["pending"] is [String: Any] else { throw bad() }
        var pending = incoming["pending"] as? [String: Any] ?? [:]
        let oldPending = previous["pending"] as? [String: Any] ?? [:]
        for (environment, raw) in pending where ids.contains((((raw as? [String: Any])?["payload"] as? [String: Any])?["commandId"] as? String) ?? "") {
            pending.removeValue(forKey: environment)
            if let successor = oldPending[environment], !ids.contains((((successor as? [String: Any])?["payload"] as? [String: Any])?["commandId"] as? String) ?? "") {
                pending[environment] = successor
            }
        }
        next["pending"] = pending; return next
    }
    private func state(connection: T3QuestionConnection? = nil) throws -> [String: Any] {
        let original: [String: Any]
        do { original = try read() } catch { try inventory().quarantineUnverifiedPins(); throw error }
        let records: [String: [String: Any]], decoded: [String: Any]?
        do { (records, decoded) = try Self.decode(original) } catch { try inventory().quarantineUnverifiedPins(); throw error }
        var document = original
        let ownedBefore = Set(records.keys).union((decoded?["prepared"] as? [String: Any] ?? [:]).keys)
            .union((decoded?["tombstones"] as? [String: Any] ?? [:]).keys).union(try inventory().ownedCommandIDs())
        if document.isEmpty, try inventory().hasBoundPins() { throw Self.bad("Saved question preferences are missing beside retained pins.") }
        var meta = decoded ?? ["epoch": UUID().uuidString.lowercased(), "revision": 0, "process": Self.process,
            "commands": [String: Any](), "prepared": [String: Any](), "tombstones": [String: Any](), "seals": [String: Any](), "mutations": [String: Any]()]
        let seals = meta["seals"] as! [String: [String: Any]]
        let facts = seals.mapValues { T3QuestionProcess.Seal(process: $0["process"] as! String, revision: Self.integer($0["revision"])!) }
        T3QuestionProcess.capture(rootKey, seals: facts)
        try save(original)
        T3QuestionProcess.confirm(rootKey, seals: facts)
        let eligible = T3QuestionProcess.eligible(rootKey).filter { facts[$0.key] == $0.value }
        var commands = meta["commands"] as! [String: [String: Any]]
        for (id, record) in records {
            var entry = try commands[id] ?? Self.metadata(record, home: nil)
            if entry["homeOrigin"] == nil, let connection,
               record["environmentId"] as? String == connection.environment, connection.catalog.contains(record["origin"] as! String) {
                entry["homeOrigin"] = connection.home
            }
            commands[id] = entry
        }
        meta["commands"] = commands
        var currentRecords = records
        if meta["process"] as? String != Self.process {
            let oldEpoch = meta["epoch"] as! String, oldProcess = meta["process"] as! String
            var newSeals = seals
            if newSeals[oldEpoch] == nil { newSeals[oldEpoch] = ["process": oldProcess, "revision": meta["revision"]!] }
            meta["seals"] = newSeals; meta["epoch"] = UUID().uuidString.lowercased(); meta["process"] = Self.process
            for (id, var record) in currentRecords {
                if var command = commands[id], var attempt = command["currentAttempt"] as? [String: Any], attempt["state"] as? String == "live" {
                    attempt["state"] = "uncertain"; command["currentAttempt"] = attempt; command["acceptanceUncertain"] = true; record["phase"] = "uncertain"
                    commands[id] = command; currentRecords[id] = record
                }
            }
            meta["commands"] = commands
        }
        // Tombstones remain through this process and through a seal first written at this startup.
        var tombstones = meta["tombstones"] as! [String: [String: Any]]
        for (id, tombstone) in tombstones where tombstone["process"] as? String != Self.process && eligible[tombstone["epoch"] as! String]?.process == tombstone["process"] as? String {
            if let pin = tombstone["pinId"] as? String {
                let status = try inventory().pinStatus(pin)
                guard status["releaseToken"] as? String == tombstone["releaseToken"] as? String, Self.bool(status["acknowledged"]) == true else { continue }
            }
            tombstones.removeValue(forKey: id)
        }
        meta["tombstones"] = tombstones
        var prepared = meta["prepared"] as! [String: [String: Any]]
        for (id, entry) in prepared where entry["process"] as? String != Self.process && eligible[entry["epoch"] as? String ?? ""]?.process == entry["process"] as? String {
            let pin = (entry["metadata"] as! [String: Any])["pinId"] as! String
            let status = try inventory().pinStatus(pin)
            if status["phase"] as? String == "prepared" { prepared.removeValue(forKey: id) }
        }
        meta["prepared"] = prepared
        var mutations = meta["mutations"] as! [String: [String: Any]]
        for (id, entry) in mutations where eligible[entry["epoch"] as? String ?? ""] != nil {
            let command = entry["commandId"] as! String
            if currentRecords[command] == nil, prepared[command] == nil, tombstones[command] == nil { mutations.removeValue(forKey: id) }
        }
        meta["mutations"] = mutations
        let dependencies = Set(prepared.values.compactMap { $0["epoch"] as? String })
            .union(tombstones.values.compactMap { $0["epoch"] as? String }).union(mutations.values.compactMap { $0["epoch"] as? String })
            .union(try inventory().retainedEpochs()).union(attempts.values.compactMap { $0["writerEpoch"] as? String })
            .union(outcomes.values.compactMap { $0["writerEpoch"] as? String })
        var retainedSeals = meta["seals"] as! [String: [String: Any]]
        for epoch in eligible.keys where !dependencies.contains(epoch) && epoch != meta["epoch"] as? String { retainedSeals.removeValue(forKey: epoch) }
        meta["seals"] = retainedSeals
        document[Self.recordsKey] = currentRecords; document[Self.metadataKey] = meta
        document = try Self.merge(previous: original, incoming: document)
        var pending = document["pending"] as? [String: Any] ?? [:]
        for (environment, raw) in pending where ownedBefore.contains((((raw as? [String: Any])?["payload"] as? [String: Any])?["commandId"] as? String) ?? "") { pending.removeValue(forKey: environment) }
        document["pending"] = pending
        document[Self.recordsKey] = currentRecords; document[Self.metadataKey] = meta
        try inventory().reconcilePins(receipts: try receiptBindings(records: currentRecords, metadata: meta, includePrepared: true),
            pinless: Set(currentRecords.keys).subtracting((meta["commands"] as! [String: [String: Any]]).filter { $0.value["pinId"] != nil }.keys))
        // Every observation that can authorize a later mutation has positive durability evidence.
        try save(document)
        let confirmedSeals = (meta["seals"] as! [String: [String: Any]]).mapValues { T3QuestionProcess.Seal(process: $0["process"] as! String, revision: Self.integer($0["revision"])!) }
        T3QuestionProcess.confirm(rootKey, seals: confirmedSeals)
        try recoverPins(document)
        return document
    }
    private func receiptBindings(records: [String: [String: Any]], metadata: [String: Any], includePrepared: Bool = false) throws -> [String: T3QuestionPinReceipt] {
        guard var commands = metadata["commands"] as? [String: [String: Any]] else { throw Self.bad() }
        var records = records
        if includePrepared {
            for (id, entry) in metadata["prepared"] as? [String: [String: Any]] ?? [:] {
                guard let record = entry["record"] as? [String: Any], let meta = entry["metadata"] as? [String: Any] else { throw Self.bad() }
                records[id] = record; commands[id] = meta
            }
        }
        var bindings: [String: T3QuestionPinReceipt] = [:]
        for (id, record) in records {
            guard let meta = commands[id] else { throw Self.bad() }
            try Self.validateMeta(meta, record: record)
            if meta["pinId"] == nil { continue }
            guard let pin = meta["pinId"] as? String, let digest = meta["pinDigest"] as? String,
                  let payload = meta["payloadDigest"] as? String, let request = meta["requestIncarnation"] as? String,
                  let revision = Self.integer(meta["inventoryRevision"]), let home = meta["homeOrigin"] as? String,
                  let catalog = meta["questionCatalog"] as? [String: String] else { throw Self.bad() }
            let scope = try T3QuestionScope(origin: home, environment: record["environmentId"] as! String,
                thread: record["threadId"] as! String, request: record["requestId"] as! String)
            bindings[id] = T3QuestionPinReceipt(command: id, id: pin, digest: digest, payload: payload,
                record: meta["recordDigest"] as! String, request: request, revision: revision, scope: scope, questions: catalog)
        }
        return bindings
    }
    private func recoverPins(_ document: [String: Any]) throws {
        let meta = document[Self.metadataKey] as! [String: Any]
        let seals = meta["seals"] as! [String: [String: Any]]
        try inventory().recoverPins(receipts: Set((document[Self.recordsKey] as! [String: Any]).keys),
            priorSeals: Set(T3QuestionProcess.eligible(rootKey).filter { epoch, fact in
                seals[epoch]?["process"] as? String == fact.process && Self.integer(seals[epoch]?["revision"]) == fact.revision
            }.keys), process: Self.process)
    }
    private func replace(_ document: inout [String: Any], records: [String: [String: Any]], metadata: [String: Any], mutation: [String: Any]? = nil) throws {
        var meta = metadata; let revision = Self.integer(meta["revision"])!
        guard revision < 9_007_199_254_740_991 else { throw Self.bad() }; meta["revision"] = revision + 1
        if let mutation {
            var entries = meta["mutations"] as! [String: Any], content = mutation; content.removeValue(forKey: "trace")
            let id = try Self.string(mutation["mutationId"])
            let record = mutation["record"] as? [String: Any], pending = record?["pending"] as? [String: Any], payload = pending?["payload"] as? [String: Any]
            let command = mutation["commandId"] as? String ?? payload?["commandId"] as? String ?? "seal"
            entries[id] = ["digest": try Self.digest(content), "epoch": meta["epoch"]!, "revision": meta["revision"]!,
                "commandId": command, "action": mutation["action"]!]
            meta["mutations"] = entries
        }
        document[Self.recordsKey] = records; document[Self.metadataKey] = meta; try save(document)
    }
    private func snapshot(_ document: [String: Any]) -> [String: Any] {
        [Self.recordsKey: document[Self.recordsKey]!, Self.metadataKey: document[Self.metadataKey]!]
    }
    private func checked(_ request: [String: Any], document: [String: Any]) throws {
        let meta = document[Self.metadataKey] as! [String: Any]
        guard request["epoch"] as? String == meta["epoch"] as? String,
              Self.integer(request["revision"]) == Self.integer(meta["revision"]) else {
            throw T3QuestionCommandFailure(message: "Refresh the saved question receipt before continuing.", kind: "stale")
        }
    }
    private func exact(_ request: [String: Any], records: [String: [String: Any]], commands: [String: [String: Any]]) throws -> String {
        let id = try Self.string(request["commandId"])
        guard records[id] != nil, let meta = commands[id], request["recordDigest"] as? String == meta["recordDigest"] as? String,
              request["payloadDigest"] as? String == meta["payloadDigest"] as? String,
              Self.integer(request["recordRevision"]) == Self.integer(meta["recordRevision"]) else {
            throw T3QuestionCommandFailure(message: "This saved answer changed.", kind: "stale")
        }
        return id
    }
    func request(_ request: [String: Any], connection: T3QuestionConnection? = nil, ordinaryIDs: Set<String> = []) throws -> [String: Any] {
        try flushOutcomes()
        let action = try Self.string(request["action"])
        let common: Set<String> = ["op", "action", "generation", "fleet", "trace", "mutationId"]
        let base: Set<String> = ["epoch", "revision", "commandId", "recordRevision", "recordDigest", "payloadDigest"]
        let extra: Set<String>
        switch action {
        case "read": extra = []
        case "prepare": extra = ["record", "catalog", "expectedOrigin", "expectedEnvironmentId"]
        case "create": extra = ["epoch", "revision", "commandId"]
        case "bind", "release", "remove": extra = base
        case "phase": extra = base.union(["phase"])
        case "terminal": extra = base.union(["phase", "consumption"])
        case "ack": extra = base.union(["releaseToken"])
        case "seal": extra = ["epoch", "revision"]
        default: throw Self.bad("Unknown question journal operation.")
        }
        try Self.fields(request, required: ["op", "action"], optional: common.union(extra))
        var document = try state(connection: connection), records = document[Self.recordsKey] as! [String: [String: Any]]
        var metadata = document[Self.metadataKey] as! [String: Any], commands = metadata["commands"] as! [String: [String: Any]]
        var prepared = metadata["prepared"] as! [String: [String: Any]], tombstones = metadata["tombstones"] as! [String: [String: Any]]
        if action == "read" { return snapshot(document) }
        let mutationID = try Self.string(request["mutationId"])
        if let prior = (metadata["mutations"] as! [String: [String: Any]])[mutationID] {
            var content = request; content.removeValue(forKey: "trace")
            guard try Self.digest(content) == prior["digest"] as? String else { throw Self.bad("This question mutation ID already owns different inputs.") }
            return snapshot(document)
        }
        if action == "prepare" {
            guard let connection, let record = request["record"] as? [String: Any], let catalog = request["catalog"] as? [String: String], !catalog.isEmpty else { throw Self.bad("A complete synchronized question catalog is required.") }
            try connection.verify(request); let payload = try Self.payload(record), id = payload["commandId"] as! String
            guard record["origin"] as? String == connection.endpoint, record["environmentId"] as? String == connection.environment,
                  record["phase"] as? String == "preflight", !ordinaryIDs.contains(id), tombstones[id] == nil else { throw Self.bad() }
            if let saved = prepared[id] {
                guard Self.equal(saved["record"], record) else { throw Self.bad("This command ID already owns different answers.") }
                try inventory().pinStatus((saved["metadata"] as! [String: Any])["pinId"] as! String)
                return snapshot(document)
            }
            guard records[id] == nil, !records.values.contains(where: { $0["environmentId"] as? String == connection.environment }) else { throw Self.bad("Resolve the previous question operation first.") }
            if payload["type"] as? String == "runtime-request.respond", Set((payload["answers"] as! [String: Any]).keys) != Set(catalog.keys) { throw Self.bad() }
            let scope = try T3QuestionScope(origin: connection.home, environment: connection.environment, thread: record["threadId"] as! String, request: record["requestId"] as! String)
            let incarnation = try inventory().register(scope, questions: catalog)
            var meta = try Self.metadata(record, home: connection.home)
            let pin = try inventory().preparePin(request: incarnation, questions: catalog, command: id,
                epoch: metadata["epoch"] as! String, process: Self.process, payloadDigest: meta["payloadDigest"] as! String,
                requireEmpty: payload["type"] as? String == "runtime-request.respond", selection: [], recordDigest: meta["recordDigest"] as! String)
            meta.merge(pin, uniquingKeysWith: { _, new in new }); meta["questionCatalog"] = catalog; prepared[id] = ["record": record, "metadata": meta, "epoch": metadata["epoch"]!, "process": Self.process]
            metadata["prepared"] = prepared; try replace(&document, records: records, metadata: metadata, mutation: request); return snapshot(document)
        }
        if action == "create" {
            let id = try Self.string(request["commandId"])
            if records[id] != nil { return snapshot(document) }
            if request["epoch"] as? String != metadata["epoch"] as? String || Self.integer(request["revision"]) != Self.integer(metadata["revision"]) {
                return snapshot(document).merging(["stale": true], uniquingKeysWith: { _, new in new })
            }
            guard let item = prepared[id], !ordinaryIDs.contains(id), tombstones[id] == nil else { throw Self.bad() }
            let meta = item["metadata"] as! [String: Any]; let pin = try inventory().pinStatus(meta["pinId"] as! String)
            guard pin["phase"] as? String == "prepared" else { throw Self.bad() }
            records[id] = item["record"] as? [String: Any]; commands[id] = meta; prepared.removeValue(forKey: id)
            metadata["commands"] = commands; metadata["prepared"] = prepared
            try replace(&document, records: records, metadata: metadata, mutation: request); return snapshot(document)
        }
        if request["epoch"] as? String != metadata["epoch"] as? String || Self.integer(request["revision"]) != Self.integer(metadata["revision"]) {
                return snapshot(document).merging(["stale": true], uniquingKeysWith: { _, new in new })
            }
        if action == "seal" {
            var seals = metadata["seals"] as! [String: Any]
            let epoch = metadata["epoch"] as! String; seals[epoch] = ["process": metadata["process"]!, "revision": metadata["revision"]!]
            metadata["seals"] = seals; metadata["epoch"] = UUID().uuidString.lowercased()
            try replace(&document, records: records, metadata: metadata, mutation: request); return snapshot(document)
        }
        let id = try exact(request, records: records, commands: commands); var record = records[id]!, meta = commands[id]!
        let phase = record["phase"] as! String
        switch action {
        case "bind":
            if let pin = meta["pinId"] as? String {
                try inventory().bindPin(pin, command: id, payloadDigest: meta["payloadDigest"] as! String, pinDigest: meta["pinDigest"] as! String)
            }
        case "phase":
            guard request["phase"] as? String == "issued", ["preflight", "issued", "uncertain"].contains(phase), attempts[id] == nil else { throw Self.bad() }
            record["phase"] = "issued"
        case "terminal":
            guard let terminal = request["phase"] as? String, ["unsent", "settled"].contains(terminal), attempts[id] == nil,
                  phase == terminal || terminal == "settled" || phase == "preflight" && terminal == "unsent" else { throw Self.bad() }
            // Uncertainty cannot be turned into nonacceptance by a generic app failure.
            guard phase != "settled" || terminal == "settled" else { throw Self.bad() }
            if terminal == "settled", phase != "settled", request["consumption"] == nil {
                throw Self.bad("A complete authoritative projection must confirm answer consumption.")
            }
            if let proof = request["consumption"] as? [String: Any] {
                guard terminal == "settled" else { throw Self.bad() }
                if (try Self.payload(record))["type"] as? String == "run.interrupt" {
                    guard try T3QuestionConsumption.stopped(proof, record: record) else { throw Self.bad("The captured run is still active.") }
                } else {
                    guard try T3QuestionConsumption.confirmed(proof, record: record) else { throw Self.bad("The pending request is not authoritatively consumed.") }
                    if let binding = try receiptBindings(records: records, metadata: metadata)[id] { try inventory().retireBoundRequest(binding) }
                    meta["requestRetired"] = true
                }
            } else if request["consumption"] != nil { throw Self.bad() }
            record["phase"] = terminal; meta["releasePending"] = true; meta["acceptanceUncertain"] = false
        case "release":
            guard ["unsent", "settled"].contains(phase), Self.bool(meta["releasePending"]) == true else { throw Self.bad() }
            if let pin = meta["pinId"] as? String {
                meta["releaseToken"] = try inventory().releasePin(pin, command: id, pinDigest: meta["pinDigest"] as! String)
            } else if meta["releaseToken"] == nil { meta["releaseToken"] = UUID().uuidString.lowercased() }
        case "ack":
            guard ["unsent", "settled"].contains(phase), let token = request["releaseToken"] as? String,
                  token == meta["releaseToken"] as? String else { throw Self.bad() }
            if let pin = meta["pinId"] as? String { try inventory().ackPin(pin, command: id, token: token) }
            meta["releaseAck"] = true
        case "remove":
            guard ["unsent", "settled"].contains(phase), Self.bool(meta["releaseAck"]) == true, let token = meta["releaseToken"] as? String else { throw Self.bad() }
            if phase == "settled", meta["pinId"] != nil, (try Self.payload(record))["type"] as? String != "run.interrupt", Self.bool(meta["requestRetired"]) != true {
                throw Self.bad("Wait for authoritative question consumption before removing this saved answer.")
            }
            var tombstone: [String: Any] = ["recordDigest": meta["recordDigest"]!, "payloadDigest": meta["payloadDigest"]!, "releaseToken": token,
                "epoch": metadata["epoch"]!, "process": Self.process]
            tombstone["pinId"] = meta["pinId"]; tombstone["pinDigest"] = meta["pinDigest"]
            tombstones[id] = tombstone; records.removeValue(forKey: id); commands.removeValue(forKey: id)
            metadata["commands"] = commands; metadata["tombstones"] = tombstones
            try replace(&document, records: records, metadata: metadata, mutation: request); return snapshot(document)
        default: throw Self.bad()
        }
        meta["recordRevision"] = try Self.nextRevision(meta["recordRevision"])
        records[id] = record; commands[id] = meta; metadata["commands"] = commands
        try replace(&document, records: records, metadata: metadata, mutation: request); return snapshot(document)
    }
    func preferencesRead() throws -> String {
        do { try flushOutcomes(); return String(decoding: try Self.encoded(state()), as: UTF8.self) }
        catch {
            // Ordinary load stays usable, while native question corruption remains visible and blocked.
            var value = try read(), pending = value["pending"] as? [String: Any] ?? [:]
            let metadata = value[Self.metadataKey] as? [String: Any]
            let ids = Set((value[Self.recordsKey] as? [String: Any] ?? [:]).keys)
                .union((metadata?["commands"] as? [String: Any] ?? [:]).keys).union((metadata?["prepared"] as? [String: Any] ?? [:]).keys)
                .union((metadata?["tombstones"] as? [String: Any] ?? [:]).keys).union(inventory().blockedCommandIDs())
            for (environment, raw) in pending where ids.contains((((raw as? [String: Any])?["payload"] as? [String: Any])?["commandId"] as? String) ?? "") { pending.removeValue(forKey: environment) }
            value["pending"] = pending; value["mobileQuestionRecovery"] = ["blocked": true, "message": error.localizedDescription]
            return String(decoding: try Self.encoded(value), as: UTF8.self)
        }
    }
    func ownedIDs() throws -> Set<String> {
        let value = try read(), records = value[Self.recordsKey] as? [String: Any] ?? [:], metadata = value[Self.metadataKey] as? [String: Any]
        return Set(records.keys).union((metadata?["tombstones"] as? [String: Any] ?? [:]).keys)
            .union((metadata?["prepared"] as? [String: Any] ?? [:]).keys).union(try inventory().ownedCommandIDs())
    }
    func frame(_ marker: [String: Any], rpcID: String) throws -> String {
        try flushOutcomes()
        let document = try state(); try checked(marker, document: document)
        let records = document[Self.recordsKey] as! [String: [String: Any]], metadata = document[Self.metadataKey] as! [String: Any]
        let commands = metadata["commands"] as! [String: [String: Any]], id = try exact(marker, records: records, commands: commands)
        guard let opaque = commands[id]?["payloadUTF8"] as? String else { throw Self.bad() }
        return try T3QuestionCommandWire.request(id: rpcID, payloadUTF8: opaque)
    }
    func issue(_ marker: [String: Any], connection: T3QuestionConnection, rpcID: String) throws -> (String, String, String) {
        try Self.fields(marker, required: ["epoch", "revision", "commandId", "recordRevision", "recordDigest", "payloadDigest"])
        try flushOutcomes(); var document = try state(connection: connection)
        try checked(marker, document: document)
        var records = document[Self.recordsKey] as! [String: [String: Any]], metadata = document[Self.metadataKey] as! [String: Any]
        var commands = metadata["commands"] as! [String: [String: Any]]
        let id = try exact(marker, records: records, commands: commands); var record = records[id]!, meta = commands[id]!
        guard attempts[id] == nil, outcomes[id] == nil, record["environmentId"] as? String == connection.environment,
              meta["homeOrigin"] as? String == connection.home, ["issued", "uncertain"].contains(record["phase"] as? String ?? "") else { throw Self.bad("This answer is retained by another attempt or connection.") }
        guard (meta["currentAttempt"] as? [String: Any])?["state"] as? String != "live" else { throw Self.bad("The native question attempt is still running.") }
        let opaque = meta["payloadUTF8"] as! String
        let text = try T3QuestionCommandWire.request(id: rpcID, payloadUTF8: opaque)
        if let pin = meta["pinId"] as? String { try inventory().issuePin(pin, command: id, payloadDigest: meta["payloadDigest"] as! String, pinDigest: meta["pinDigest"] as! String) }
        let attempt = UUID().uuidString.lowercased(), baseMetadata = meta, baseRecord = record
        let owned: [String: Any] = ["id": attempt, "rpcId": rpcID, "generation": connection.generation, "endpoint": connection.endpoint, "state": "live"]
        meta["currentAttempt"] = owned; meta["recordRevision"] = try Self.nextRevision(meta["recordRevision"]); record["phase"] = "issued"
        commands[id] = meta; records[id] = record; metadata["commands"] = commands
        // Retain before replacement. An ambiguous failure still owns this attempt as uncertain.
        var captured = owned
        captured["payloadDigest"] = meta["payloadDigest"]!; captured["recordDigest"] = meta["recordDigest"]!
        captured["baseMetadata"] = baseMetadata; captured["baseRecord"] = baseRecord; captured["attemptData"] = owned
        captured["writerEpoch"] = metadata["epoch"]!
        captured["priorUncertain"] = Self.bool(baseMetadata["acceptanceUncertain"]) ?? ["issued", "uncertain"].contains(baseRecord["phase"] as? String ?? "")
        attempts[id] = captured
        do { try replace(&document, records: records, metadata: metadata) }
        catch {
            var outcome = captured; outcome["attempt"] = attempt; outcome["phase"] = "uncertain"; outcomes[id] = outcome
            attempts.removeValue(forKey: id); throw error
        }
        return (text, id, attempt)
    }
    func complete(command: String, attempt: String, result: [String: Any]) throws -> [String: Any] {
        guard let owned = attempts[command], owned["id"] as? String == attempt else { throw Self.bad("The native question attempt changed.") }
        let phase = Self.bool(result["ok"]) == true ? "settled"
            : Self.bool(result["_outboxNotAccepted"]) == true && Self.bool(owned["priorUncertain"]) != true ? "unsent" : "uncertain"
        var outcome = owned; outcome["attempt"] = attempt; outcome["phase"] = phase; outcomes[command] = outcome
        attempts.removeValue(forKey: command)
        try flushOutcomes(); return snapshot(try state())
    }
    private func flushOutcomes() throws {
        for (id, outcome) in outcomes {
            var document = try read(); let (decoded, decodedMetadata) = try Self.decode(document)
            guard var metadata = decodedMetadata, var meta = (metadata["commands"] as? [String: [String: Any]])?[id],
                  var record = decoded[id], let capture = outcome["attemptData"] as? [String: Any],
                  let base = outcome["baseMetadata"] as? [String: Any], let baseRecord = outcome["baseRecord"] as? [String: Any],
                  meta["payloadDigest"] as? String == outcome["payloadDigest"] as? String,
                  meta["recordDigest"] as? String == outcome["recordDigest"] as? String,
                  Self.equal(meta["pinId"], base["pinId"]), Self.equal(meta["pinDigest"], base["pinDigest"]),
                  Self.equal(meta["requestIncarnation"], base["requestIncarnation"]) else {
                throw Self.bad("The exact native question outcome needs recovery before another attempt.")
            }
            try inventory().reconcilePins(receipts: try receiptBindings(records: decoded, metadata: metadata, includePrepared: true),
                pinless: Set(decoded.keys).subtracting((metadata["commands"] as! [String: [String: Any]]).filter { $0.value["pinId"] != nil }.keys))
            let current = meta["currentAttempt"] as? [String: Any]
            let sameAttempt = current?["id"] as? String == outcome["attempt"] as? String
            let unpublishedIssue = Self.equal(meta, base) && Self.equal(record, baseRecord)
            guard sameAttempt || unpublishedIssue else { throw Self.bad("Another native attempt or receipt owns this question outcome.") }
            var records = decoded, commands = metadata["commands"] as! [String: [String: Any]]
            let phase = outcome["phase"] as! String
            // Exact already-visible terminal outcomes are confirmed without applying the transition twice.
            if sameAttempt && current?["state"] as? String == phase && record["phase"] as? String == phase {
                try save(document); if !notificationsPending { committed() }; outcomes.removeValue(forKey: id); continue
            }
            var ended = capture; ended["state"] = phase; meta["currentAttempt"] = ended
            if record["phase"] as? String != "settled" { record["phase"] = phase }
            meta["acceptanceUncertain"] = phase == "uncertain"
            if ["settled", "unsent"].contains(phase) { meta["releasePending"] = true }
            meta["recordRevision"] = try Self.nextRevision(meta["recordRevision"])
            records[id] = record; commands[id] = meta; metadata["commands"] = commands
            try replace(&document, records: records, metadata: metadata)
            if !notificationsPending { committed() }
            outcomes.removeValue(forKey: id)
        }
    }
}
