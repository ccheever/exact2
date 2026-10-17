#if os(iOS)
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
// Every entry point runs under T3MobileQueuedEdit's existing preference/removal mutex.
import Foundation
import CryptoKit

final class T3MobileComposerPasteStore {
    struct Intent: Codable, Equatable {
        let identity: T3ComposerIdentity
        let richEventId: String
        let target: T3ComposerPasteTarget
        let sources: [T3ComposerPasteRequest.Source]
        let remaining: Int
    }
    struct Original: Codable { let leaseId: String; let filename: String }
    struct Skipped: Codable { let leaseId: String; let reason: String }
    struct Publication: Codable, Equatable {
        let origin: String; let environmentId: String; let draftKey: String; let incarnation: String; let revision: Int
    }
    struct Marker: Codable, Equatable { let proof: T3ComposerPasteProof; let publication: Publication }
    struct Record: Codable {
        var state: String
        let intent: Intent
        let proof: T3ComposerPasteProof
        let originals: [Original]
        let skipped: [Skipped]
        var publication: Marker?
    }
    struct Journal: Codable { var version = 1; var records: [String: Record] = [:] }
    static let maxUnresolvedBytes = 2 * 100 * T3MobileAttachments.maxImageBytes
    static let maxRecords = 1024
    private let root: URL
    private let replace: (Data, URL) throws -> Void
    private let synchronize: (URL) throws -> Void
    private var file: URL { root.appendingPathComponent("mobile-composer-paste.json") }
    private var established: URL { root.appendingPathComponent(".mobile-composer-paste-established") }
    static var temporaryRoot: URL { FileManager.default.temporaryDirectory.appendingPathComponent("t3-composer-paste", isDirectory: true) }
    init(root: URL, replace: @escaping (Data, URL) throws -> Void = T3MobileQueuedEdit.durableReplace,
         synchronize: @escaping (URL) throws -> Void = T3MobileIncomingShares.sync) {
        self.root = root; self.replace = replace; self.synchronize = synchronize
    }
    static func ownedTemporary(_ url: URL) -> Bool {
        url.isFileURL && url.deletingLastPathComponent().standardizedFileURL == temporaryRoot.standardizedFileURL
            && url.pathExtension == "png" && UUID(uuidString: url.deletingPathExtension().lastPathComponent) != nil
    }
    private func temporary(_ original: Original) throws -> URL {
        let url = Self.temporaryRoot.appendingPathComponent(original.filename)
        guard Self.ownedTemporary(url), url.lastPathComponent == original.filename else { throw T3ComposerPasteError.recovery }
        return url
    }
    private func canonical(_ file: T3ComposerPasteFile) -> URL { root.appendingPathComponent("snapshots/drafts").appendingPathComponent(file.id) }
    static func encoded<T: Encodable>(_ value: T) throws -> Data {
        let encoder = JSONEncoder(); encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
        return try encoder.encode(value)
    }
    static func object<T: Encodable>(_ value: T) throws -> Any { try JSONSerialization.jsonObject(with: encoded(value)) }
    private func load() throws -> Journal {
        if !FileManager.default.fileExists(atPath: file.path) {
            guard !FileManager.default.fileExists(atPath: established.path) else { throw T3ComposerPasteError.recovery }
            return Journal()
        }
        do {
            let values = try file.resourceValues(forKeys: [.fileSizeKey, .isSymbolicLinkKey, .isRegularFileKey])
            guard values.isSymbolicLink != true, values.isRegularFile == true, (values.fileSize ?? Int.max) <= T3Wire.maximumBytes else { throw T3ComposerPasteError.recovery }
            let journal = try JSONDecoder().decode(Journal.self, from: Data(contentsOf: file))
            guard journal.version == 1, journal.records.count <= Self.maxRecords else { throw T3ComposerPasteError.recovery }
            var destinations = Set<String>()
            for (id, record) in journal.records {
                guard id == record.proof.operationId, UUID(uuidString: id) != nil, record.proof.valid,
                      ["reserved", "staged", "discarding", "adopted", "discarded"].contains(record.state),
                      record.proof.target == record.intent.target, record.proof.identity == record.intent.identity,
                      record.proof.richEventId == record.intent.richEventId,
                      record.intent.target.valid, record.intent.identity.admitted, !record.intent.identity.mountId.isEmpty,
                      record.proof.files.count <= 100, Set(record.proof.files.map(\.leaseId)).count == record.proof.files.count, record.intent.sources.count <= 1024,
                      (0...100).contains(record.intent.remaining),
                      Set(record.intent.sources.map(\.leaseId)).count == record.intent.sources.count,
                      record.originals.map(\.leaseId) == record.intent.sources.map(\.leaseId),
                      Set(record.originals.map(\.filename)).count == record.originals.count else { throw T3ComposerPasteError.recovery }
                for f in record.proof.files {
                    guard UUID(uuidString: f.id) != nil, f.id == f.id.lowercased(), destinations.insert(f.id).inserted,
                          record.intent.sources.contains(where: { $0.leaseId == f.leaseId && $0.kind == "editorImage" }),
                          f.kind == "image", f.mimeType == "image/png", f.name == "pasted-image.png", f.sizeBytes > 0,
                          f.sizeBytes <= T3MobileAttachments.maxImageBytes, f.sha256.count == 64,
                          f.sha256.allSatisfy({ "0123456789abcdef".contains($0) }) else { throw T3ComposerPasteError.recovery }
                }
                for original in record.originals { _ = try temporary(original) }
                if let marker = record.publication {
                    guard marker.proof == record.proof, intrinsic(marker), ["staged", "adopted"].contains(record.state) else { throw T3ComposerPasteError.recovery }
                }
                if record.state == "adopted", record.publication == nil { throw T3ComposerPasteError.recovery }
            }
            return journal
        } catch { throw T3ComposerPasteError.recovery }
    }
    private func save(_ journal: Journal) throws {
        let bytes = try Self.encoded(journal)
        guard bytes.count <= T3Wire.maximumBytes else { throw T3ComposerPasteError.capacity }
        try replace(bytes, file)
        if !FileManager.default.fileExists(atPath: established.path) { try replace(Data("1".utf8), established) }
    }
    private func digest(_ bytes: Data) -> String { SHA256.hash(data: bytes).map { String(format: "%02x", $0) }.joined() }
    private func data(_ url: URL) throws -> Data? { try T3MobileAttachments.pastedImageBytes(url) }
    private func matches(_ file: T3ComposerPasteFile, bytes: Data) -> Bool { bytes.count == file.sizeBytes && digest(bytes) == file.sha256 }
    private func verify(_ record: Record) throws {
        for f in record.proof.files { guard let bytes = try data(canonical(f)), matches(f, bytes: bytes) else { throw T3ComposerPasteError.recovery } }
    }
    private func synchronizeCanonical(_ record: Record) throws {
        guard !record.proof.files.isEmpty else { return }
        // A prior rename may be visible even though its directory fsync failed.
        // Establish file and ancestor durability on replay before staged success or temp cleanup.
        for file in record.proof.files { try synchronize(canonical(file)) }
        for directory in [root.appendingPathComponent("snapshots/drafts"), root.appendingPathComponent("snapshots"), root] {
            try synchronize(directory)
        }
    }
    private func reply(_ record: Record) throws -> [String: Any] {
        ["status": record.state, "proof": try Self.object(record.proof), "skipped": try Self.object(record.skipped)]
    }
    func stage(_ request: T3ComposerPasteRequest, inputs: [T3ComposerPasteInput]?) throws -> [String: Any] {
        guard let identity = request.identity, let event = request.richEventId, let target = request.target,
              let sources = request.sources, let remaining = request.remaining else { throw T3ComposerPasteError.arguments }
        let intent = Intent(identity: identity, richEventId: event, target: target, sources: sources, remaining: remaining)
        var journal = try load()
        var record: Record
        if let existing = journal.records[request.operationId] {
            guard existing.intent == intent else { throw T3ComposerPasteError.arguments }
            if ["adopted", "discarded"].contains(existing.state) { try save(journal); return try reply(existing) }
            guard existing.state != "discarding" else { throw T3ComposerPasteError.recovery }
            record = existing
            try save(journal) // Cold/uncertain reservation and its sentinel must be durable before replay copies.
        } else {
            guard let inputs, inputs.count == sources.count,
                  zip(inputs, sources).allSatisfy({ $0.0.leaseId == $0.1.leaseId && Self.ownedTemporary($0.0.file) }) else { throw T3ComposerPasteError.superseded }
            let unresolved = journal.records.values.filter { ["reserved", "staged", "discarding"].contains($0.state) }
            guard journal.records.count < Self.maxRecords, unresolved.count < 128 else { throw T3ComposerPasteError.capacity }
            var files: [T3ComposerPasteFile] = [], skipped: [Skipped] = []
            for (index, input) in inputs.enumerated() {
                if index >= remaining { skipped.append(.init(leaseId: input.leaseId, reason: "excess")); continue }
                let size = try? input.file.resourceValues(forKeys: [.fileSizeKey]).fileSize
                guard let bytes = try data(input.file) else {
                    skipped.append(.init(leaseId: input.leaseId, reason: (size ?? 0) > T3MobileAttachments.maxImageBytes ? "too-large" : "unreadable")); continue
                }
                files.append(.init(leaseId: input.leaseId, id: UUID().uuidString.lowercased(), kind: "image", name: "pasted-image.png",
                    mimeType: "image/png", sizeBytes: bytes.count, sha256: digest(bytes)))
            }
            let heldBytes = unresolved.flatMap { $0.proof.files }.reduce(0) { $0 + $1.sizeBytes }
            guard heldBytes + files.reduce(0, { $0 + $1.sizeBytes }) <= Self.maxUnresolvedBytes else { throw T3ComposerPasteError.capacity }
            record = Record(state: "reserved", intent: intent,
                proof: .init(version: 1, operationId: request.operationId, identity: identity, richEventId: event, target: target, files: files),
                originals: inputs.map { .init(leaseId: $0.leaseId, filename: $0.file.lastPathComponent) }, skipped: skipped, publication: nil)
            journal.records[request.operationId] = record
            try save(journal) // All canonical UUIDs are durable before any canonical write.
        }
        // One <=10MiB image lives in memory at a time, including the source-valid100-image case.
        for f in record.proof.files {
            let destination = canonical(f)
            if FileManager.default.fileExists(atPath: destination.path) {
                guard let bytes = try data(destination), matches(f, bytes: bytes) else { throw T3ComposerPasteError.recovery }
                continue
            }
            guard record.state == "reserved", let original = record.originals.first(where: { $0.leaseId == f.leaseId }),
                  let bytes = try data(temporary(original)), matches(f, bytes: bytes) else { throw T3ComposerPasteError.recovery }
            try T3MobileAttachments.writePastedImage(bytes, id: f.id, dataRoot: root, replace: replace)
        }
        try verify(record)
        try synchronizeCanonical(record)
        record.state = "staged"; journal.records[request.operationId] = record; try save(journal)
        cleanupOriginals(record)
        return try reply(record)
    }
    private func cleanupOriginals(_ record: Record) {
        for original in record.originals { if let file = try? temporary(original) { try? FileManager.default.removeItem(at: file) } }
    }
    func protects(_ id: String, excluding operation: String? = nil) throws -> Bool {
        let journal = try load()
        for (key, record) in journal.records where record.proof.files.contains(where: { $0.id == id }) {
            if record.state == "adopted" {
                // This new paste UUID may fall through only with a readable ordinary inventory.
                // Unknown/retired UUIDs retain the preexisting remover's behavior.
                let current = try preferences()
                if let marker = try markers(current)[key], marker != record.publication { throw T3ComposerPasteError.recovery }
            } else if ["reserved", "staged", "discarding"].contains(record.state) {
                if key == operation {
                    guard record.state == "discarding" else { throw T3ComposerPasteError.recovery }
                    continue
                }
                return true
            }
        }
        return false
    }
    private func markers(_ preferences: [String: Any]) throws -> [String: Marker] {
        guard let raw = preferences["composerPasteAdoptions"] else { return [:] }
        do {
            guard raw is [String: Any] else { throw T3ComposerPasteError.recovery }
            let result = try JSONDecoder().decode([String: Marker].self, from: JSONSerialization.data(withJSONObject: raw))
            guard result.allSatisfy({ $0.key == $0.value.proof.operationId && $0.value.proof.valid && intrinsic($0.value) }) else { throw T3ComposerPasteError.recovery }
            return result
        } catch { throw T3ComposerPasteError.recovery }
    }
    static func integer(_ value: Any?) -> Int? {
        struct Value: Decodable { let value: Int }
        guard let value, let data = try? JSONSerialization.data(withJSONObject: ["value": value]),
              let number = try? JSONDecoder().decode(Value.self, from: data).value,
              number >= 0, number <= T3ComposerProtocolState.maxCount else { return nil }
        return number
    }
    /// Do not turn malformed inventories into empty ownership sets.
    static func validateInventory(_ preferences: [String: Any]) throws {
        guard integer(preferences["version"]) == 1,
              let snapshots = preferences["snapshotDrafts"] as? [String: Any],
              let drafts = preferences["drafts"] as? [String: String], !drafts.keys.contains(""),
              preferences["pending"] == nil || preferences["pending"] is [String: Any],
              preferences["composerFiles"] == nil || preferences["composerFiles"] is [[String: Any]] else { throw T3ComposerPasteError.recovery }
        for raw in snapshots.values {
            guard let files = raw as? [[String: Any]], files.allSatisfy({ ($0["id"] as? String).flatMap(UUID.init(uuidString:)) != nil }) else { throw T3ComposerPasteError.recovery }
        }
        if let files = preferences["composerFiles"] as? [[String: Any]],
           !files.allSatisfy({ ($0["id"] as? String).flatMap(UUID.init(uuidString:)) != nil }) { throw T3ComposerPasteError.recovery }
        if let pending = preferences["pending"] as? [String: Any] {
            func attachments(_ raw: Any?) throws {
                guard let raw else { return }
                guard let values = raw as? [[String: Any]], values.allSatisfy({ ($0["id"] as? String)?.isEmpty == false }) else { throw T3ComposerPasteError.recovery }
            }
            for (environment, raw) in pending {
                guard !environment.isEmpty, let entry = raw as? [String: Any],
                      (entry["method"] as? String)?.isEmpty == false, let payload = entry["payload"] as? [String: Any] else { throw T3ComposerPasteError.recovery }
                try attachments(payload["attachments"])
                if let rawInitial = payload["initialMessage"] {
                    guard let initial = rawInitial as? [String: Any] else { throw T3ComposerPasteError.recovery }
                    try attachments(initial["attachments"])
                }
            }
        }
    }
    private func intrinsic(_ marker: Marker) -> Bool {
        let target = marker.proof.target, p = marker.publication
        return p.origin == target.origin && p.environmentId == target.environmentId && p.draftKey == target.draftKey
            && p.incarnation == target.incarnation && p.revision >= target.capturedRevision && p.revision <= T3ComposerProtocolState.maxCount
    }
    private func validateFirstPublication(_ marker: Marker, preferences: [String: Any]) throws {
        try Self.validateInventory(preferences)
        let target = marker.proof.target, publication = marker.publication
        guard publication.origin == target.origin, publication.environmentId == target.environmentId,
              publication.draftKey == target.draftKey, publication.incarnation == target.incarnation,
              publication.revision >= target.capturedRevision, publication.revision <= T3ComposerProtocolState.maxCount,
              let registry = preferences["mobileComposerEditor"] as? [String: Any], Self.integer(registry["version"]) == 1,
              let documents = registry["documents"] as? [String: [String: Any]] else { throw T3ComposerPasteError.recovery }
        // Read the JSON array key semantically; do not invent a different slash-escaping convention.
        let rows = documents.filter { key, _ in
            guard let fields = try? JSONSerialization.jsonObject(with: Data(key.utf8)) as? [String] else { return false }
            return fields == [target.origin, target.environmentId, target.draftKey]
        }.values
        guard rows.count == 1, let document = rows.first,
              document["origin"] as? String == target.origin, document["environmentId"] as? String == target.environmentId,
              document["draftKey"] as? String == target.draftKey, document["incarnation"] as? String == target.incarnation,
              Self.integer(document["revision"]) == publication.revision, document["blocked"] == nil,
              ((preferences["drafts"] as? [String: String])?[target.draftKey] ?? "").utf16.count <= 1_000_000,
              let files = (preferences["snapshotDrafts"] as? [String: [[String: Any]]])?[target.draftKey] else { throw T3ComposerPasteError.recovery }
        for f in marker.proof.files {
            let matching = files.filter { $0["id"] as? String == f.id }
            guard matching.count == 1, let actual = matching.first,
                  actual["name"] as? String == f.name, actual["mimeType"] as? String == f.mimeType,
                  Self.integer(actual["sizeBytes"]) == f.sizeBytes else { throw T3ComposerPasteError.recovery }
        }
    }
    /// Called before preference replacement, while the existing writer mutex stays held.
    func validatePreferences(previous: [String: Any], next: [String: Any]) throws {
        let prior = try markers(previous), incoming = try markers(next)
        var journal = try load(), changed = false
        for (id, marker) in incoming {
            guard var record = journal.records[id], record.proof == marker.proof,
                  ["staged", "adopted"].contains(record.state) else { throw T3ComposerPasteError.recovery }
            if let old = prior[id] {
                guard old == marker, record.publication == marker else { throw T3ComposerPasteError.recovery }
                // This is historical CAS evidence, not a requirement to rewind newer typing.
            } else {
                guard record.state == "staged", record.publication == nil || record.publication == marker else { throw T3ComposerPasteError.recovery }
                try validateFirstPublication(marker, preferences: next)
                try verify(record)
                record.publication = marker; journal.records[id] = record; changed = true
            }
        }
        for (id, old) in prior where incoming[id] == nil {
            guard let record = journal.records[id], record.state == "adopted", record.publication == old else { throw T3ComposerPasteError.recovery }
        }
        if changed { try save(journal) } // Authorized publication is journaled before its possible prefs rename.
    }
    private func preferences() throws -> [String: Any] {
        let url = root.appendingPathComponent("t3-code.json")
        do {
            let resource = try url.resourceValues(forKeys: [.fileSizeKey, .isSymbolicLinkKey, .isRegularFileKey])
            guard resource.isSymbolicLink != true, resource.isRegularFile == true, (resource.fileSize ?? Int.max) <= T3Wire.maximumBytes,
                  let value = try JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any] else { throw T3ComposerPasteError.recovery }
            try Self.validateInventory(value)
            _ = try markers(value)
            return value
        } catch { throw T3ComposerPasteError.recovery }
    }
    private func syncPreferences() throws -> [String: Any] {
        // Reuse the existing public file/directory sync helper without changing its owner algorithms.
        try T3MobileIncomingShares.sync(root.appendingPathComponent("t3-code.json"))
        try T3MobileIncomingShares.sync(root)
        return try preferences()
    }
    func transition(_ request: T3ComposerPasteRequest, remove: (String, String) throws -> Bool) throws -> [String: Any] {
        var journal = try load()
        guard var record = journal.records[request.operationId] else {
            guard request.action == "retire", try markers(syncPreferences())[request.operationId] == nil else { throw T3ComposerPasteError.superseded }
            return ["status": "retired", "proof": try Self.object(request.proof!)] // Absence grants no authority to stage or publish a marker.
        }
        guard record.proof == request.proof else { throw T3ComposerPasteError.arguments }
        if request.action == "retire" {
            guard ["adopted", "discarded"].contains(record.state), try markers(syncPreferences())[request.operationId] == nil else { throw T3ComposerPasteError.recovery }
            journal.records.removeValue(forKey: request.operationId); try save(journal)
            return ["status": "retired", "proof": try Self.object(request.proof!)] // Never delete adopted canonical bytes.
        }
        if ["adopted", "discarded"].contains(record.state) { try save(journal); return try reply(record) }
        let current = try preferences(), marker = try markers(current)[request.operationId]
        if let marker {
            guard record.state == "staged", record.publication == marker, marker.proof == record.proof else { throw T3ComposerPasteError.recovery }
            try verify(record)
            guard try markers(syncPreferences())[request.operationId] == marker else { throw T3ComposerPasteError.recovery }
            record.state = "adopted"; journal.records[request.operationId] = record; try save(journal)
            cleanupOriginals(record)
            return try reply(record)
        }
        guard request.action == "discard" else { throw T3ComposerPasteError.recovery }
        guard try markers(syncPreferences())[request.operationId] == nil else { throw T3ComposerPasteError.recovery }
        record.state = "discarding"; record.publication = nil; journal.records[request.operationId] = record; try save(journal)
        for f in record.proof.files {
            guard try remove(f.id, request.operationId) else { throw T3ComposerPasteError.recovery }
            let directory = canonical(f).deletingLastPathComponent()
            if FileManager.default.fileExists(atPath: directory.path) { try T3MobileIncomingShares.sync(directory) }
        }
        record.state = "discarded"; journal.records[request.operationId] = record; try save(journal)
        cleanupOriginals(record)
        return try reply(record)
    }
}
#endif
