#if os(iOS)
// Issued picker bytes share the existing deletion mutex/candidates; no persisted picker UI or Undo.
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import Foundation
import CoreFoundation
import CryptoKit
import Darwin

typealias T3PickerObject = [String: Any]
struct T3PickerRequest {
    let object: T3PickerObject
    let action: String
    let operationId: String
    let generation: Int
    let identity: T3ComposerIdentity
    static func error(_ message: String) -> T3Failure { .init(kind: "PickerIntake", message: message) }
    static func integer(_ value: Any?) -> Int? { T3ComposerFileRequest.integer(value) }
    static func encoded(_ value: Any) throws -> Data { try T3ComposerFileRequest.encoded(value) }
    static func equal(_ a: Any?, _ b: Any?) -> Bool {
        guard let a, let b, let aa = try? encoded(a), let bb = try? encoded(b) else { return false }; return aa == bb
    }
    init(_ object: T3PickerObject) throws {
        guard let action = object["action"] as? String, ["pick", "status", "hold", "finish", "cancel"].contains(action),
              let id = object["operationId"] as? String, T3ComposerFileRequest.uuid(id),
              let generation = Self.integer(object["generation"]), let raw = object["identity"] as? T3PickerObject,
              let identity = try? JSONDecoder().decode(T3ComposerIdentity.self, from: Self.encoded(raw)), identity.admitted, !identity.mountId.isEmpty else {
            throw Self.error("The captured picker request is invalid.")
        }
        let extra: [String] = action == "pick" ? ["target", "document", "source", "remaining", "fileLimit"] : action == "hold" ? ["request"] : action == "finish" ? ["publication"] : []
        guard Set(object.keys) == Set(["op", "action", "operationId", "generation", "identity"] + extra), object["op"] as? String == "composerPickerIntake" else { throw Self.error("The picker request fields are invalid.") }
        if action == "pick" {
            guard let target = object["target"] as? [String: String], Set(target.keys) == Set(["origin", "environmentId", "threadId", "draftKey"]),
                  target.values.allSatisfy({ !$0.isEmpty }), let key = target["draftKey"], let env = target["environmentId"], let thread = target["threadId"],
                  !thread.hasPrefix("new:"), !key.contains("~queued-edit~"), key == env + ":" + thread,
                  let owner = try? JSONSerialization.jsonObject(with: Data(identity.owner.utf8)) as? [Any], owner.count == 5 || owner.count == 6,
                  owner[0] as? String == "ordinary", owner[1] as? String == target["origin"], owner[2] as? String == env, Self.integer(owner[3]) == generation, owner[4] as? String == key,
                  let document = object["document"] as? T3PickerObject, Set(document.keys) == Set(["incarnation", "revision"]),
                  let incarnation = document["incarnation"] as? String, !incarnation.isEmpty, incarnation.utf16.count <= 128, Self.integer(document["revision"]) != nil,
                  let source = object["source"] as? String, ["photos", "files"].contains(source), let count = Self.integer(object["remaining"]), (1...100).contains(count),
                  let limit = Self.integer(object["fileLimit"]), limit <= T3MobileAttachments.maxFileBytes, source != "files" || limit > 0 else { throw Self.error("Only a captured ordinary draft can open this picker.") }
        }
        self.object = object; self.action = action; operationId = id; self.generation = generation; self.identity = identity
    }
}

/// Mutated only under T3MobileQueuedEdit's mutex. Workers retain this native claim, never a port.
final class T3PickerOperation: @unchecked Sendable {
    let request: T3PickerRequest
    let claim: T3ComposerFileClaim
    var status = "picking"
    var presented = false
    var files: [T3PickerObject] = []
    var candidates: [T3PickerObject] = []
    var fingerprints: [String: String] = [:]
    var error = ""
    var terminal: Data?
    var terminalOrder = 0
    init(_ request: T3PickerRequest, claim: T3ComposerFileClaim) { self.request = request; self.claim = claim }
}
struct T3PickerStage: @unchecked Sendable {
    let coordinator: T3MobileQueuedEdit
    let operation: T3PickerOperation
    func write(_ bytes: Data, metadata: T3PickerObject) throws -> T3PickerObject { try coordinator.stagePicker(operation, bytes: bytes, metadata: metadata) }
}

/// No independent storage owner: queue read/save is supplied only by the concrete locked coordinator.
final class T3MobilePickerIntakeStore {
    private let root: URL
    private let replace: (Data, URL) throws -> Void
    private var terminalSerial = 0
    private var operations: [ObjectIdentifier: [String: T3PickerOperation]] = [:]
    init(root: URL, replace: @escaping (Data, URL) throws -> Void) { self.root = root; self.replace = replace }
    private func known(_ request: T3PickerRequest, claim: T3ComposerFileClaim?) throws -> T3PickerOperation {
        let found = operations.values.compactMap { $0[request.operationId] }.filter {
            $0.request.identity == request.identity && $0.request.generation == request.generation
        }
        guard found.count == 1, let op = found.first else { throw T3PickerRequest.error("That native picker operation is no longer available.") }
        if request.action == "hold" {
            guard let claim, claim.live, op.claim === claim else { throw T3PickerRequest.error("The picker file owner ended.") }
        }
        return op
    }
    func begin(_ request: T3PickerRequest, claim: T3ComposerFileClaim) throws -> (T3PickerOperation, Bool) {
        guard claim.live, claim.identity == request.identity, request.action == "pick" else { throw T3PickerRequest.error("The captured picker owner ended.") }
        let key = ObjectIdentifier(claim)
        if let prior = operations[key]?[request.operationId] {
            guard T3PickerRequest.equal(prior.request.object, request.object) else { throw T3PickerRequest.error("A picker operation cannot change its captured input.") }; return (prior, !prior.presented)
        }
        guard (operations[key]?.count ?? 0) < 4096 else { throw T3PickerRequest.error("This editor's picker request limit was reached.") }
        let op = T3PickerOperation(request, claim: claim); operations[key, default: [:]][request.operationId] = op; return (op, true)
    }
    func operation(_ request: T3PickerRequest, claim: T3ComposerFileClaim?) throws -> T3PickerOperation { try known(request, claim: claim) }
    func protects(_ id: String) -> Bool { operations.values.contains { $0.values.contains { op in
        !["finished", "cancelled"].contains(op.status) && op.candidates.contains { $0["id"] as? String == id }
    } } }
    static func candidates(_ additions: [T3PickerObject], in queue: inout T3PickerObject) throws {
        guard var rows = queue["releases"] as? [T3PickerObject], rows.allSatisfy({ row in
            guard let id = row["id"] as? String, T3ComposerFileRequest.uuid(id), ["image", "file"].contains(row["kind"] as? String ?? "") else { return false }
            return row["pickerIntake"] == nil || (row["pickerIntake"] as? NSNumber).map { CFGetTypeID($0) == CFBooleanGetTypeID() && $0.boolValue } == true
        }) else { throw T3PickerRequest.error("Attachment cleanup ownership is invalid.") }
        for item in additions {
            if let index = rows.firstIndex(where: { $0["id"] as? String == item["id"] as? String && $0["kind"] as? String == item["kind"] as? String }) { rows[index]["pickerIntake"] = true }
            else { var value = item; value["pickerIntake"] = true; rows.append(value) }
        }
        queue["releases"] = rows
    }
    func saveCandidates(_ op: T3PickerOperation, queue: inout T3PickerObject, save: (T3PickerObject) throws -> Void) throws {
        try Self.candidates(op.candidates, in: &queue); try save(queue)
    }
    private func canonical(_ item: T3PickerObject) -> URL { root.appendingPathComponent(item["kind"] as? String == "image" ? "snapshots/drafts" : "composer-files").appendingPathComponent(item["id"] as! String) }
    private func synchronize(_ file: T3PickerObject) throws {
        let path = canonical(file), parent = path.deletingLastPathComponent()
        for url in [path, parent] + (file["kind"] as? String == "image" ? [parent.deletingLastPathComponent(), root] : [root]) {
            let attributes = try FileManager.default.attributesOfItem(atPath: url.path)
            guard attributes[.type] as? FileAttributeType != .typeSymbolicLink else { throw T3PickerRequest.error("The attachment storage path changed.") }
            try T3MobileIncomingShares.sync(url)
        }
    }
    private func fingerprint(_ file: T3PickerObject) throws -> String {
        let parent = canonical(file).deletingLastPathComponent()
        for directory in [root, parent] + (file["kind"] as? String == "image" ? [root.appendingPathComponent("snapshots")] : []) {
            let attr = try FileManager.default.attributesOfItem(atPath: directory.path)
            guard attr[.type] as? FileAttributeType == .typeDirectory else { throw T3PickerRequest.error("The canonical attachment ancestor changed.") }
        }
        return try T3ComposerFileRequest.fingerprint(root: root, file: file, image: file["kind"] as? String == "image")
    }
    func stage(_ op: T3PickerOperation, bytes: Data, metadata: T3PickerObject, queue: inout T3PickerObject, save: (T3PickerObject) throws -> Void) throws -> T3PickerObject {
        guard op.claim.live, op.status == "picking", op.files.count < (op.request.object["remaining"] as! Int),
              let kind = metadata["kind"] as? String, ["image", "file"].contains(kind), let name = metadata["name"] as? String, !name.isEmpty,
              let mime = metadata["mimeType"] as? String, !mime.isEmpty, !bytes.isEmpty,
              bytes.count <= (kind == "image" ? T3MobileAttachments.maxImageBytes : op.request.object["fileLimit"] as! Int) else { throw T3PickerRequest.error("The picked bytes are unavailable or their owner ended.") }
        let id = UUID().uuidString.lowercased()
        var file = metadata; file["id"] = id; file["sizeBytes"] = bytes.count
        op.candidates.append(["id": id, "kind": kind])
        // The live operation protects the UUID before the first durable candidate write.
        try saveCandidates(op, queue: &queue, save: save)
        let destination = canonical(file), directories = kind == "image" ? [root.appendingPathComponent("snapshots"), destination.deletingLastPathComponent()] : [destination.deletingLastPathComponent()]
        for directory in directories {
            if let attr = try? FileManager.default.attributesOfItem(atPath: directory.path), attr[.type] as? FileAttributeType == .typeSymbolicLink { throw T3PickerRequest.error("The attachment directory changed.") }
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        }
        guard !FileManager.default.fileExists(atPath: destination.path) else { throw T3PickerRequest.error("The new attachment ID already exists.") }
        try replace(bytes, destination); try synchronize(file)
        let stamp = try fingerprint(file)
        guard op.claim.live, op.status == "picking" else { throw T3PickerRequest.error("The picker owner ended while copying.") }
        op.files.append(file); op.fingerprints[id] = stamp; return file
    }
    private func verify(_ op: T3PickerOperation) throws {
        for file in op.files {
            guard try fingerprint(file) == op.fingerprints[file["id"] as! String] else { throw T3PickerRequest.error("The issued picker bytes changed.") }
            try synchronize(file)
        }
    }
    func complete(_ op: T3PickerOperation, error: String, queue: inout T3PickerObject, save: (T3PickerObject) throws -> Void) throws -> T3PickerObject {
        guard op.claim.live else { throw T3PickerRequest.error("The picker owner ended.") }
        if op.status == "picking" { try saveCandidates(op, queue: &queue, save: save); try verify(op); op.error = error; op.status = "staged" }
        return try response(op, queue: &queue, save: save)
    }
    func response(_ op: T3PickerOperation, queue: inout T3PickerObject, save: (T3PickerObject) throws -> Void) throws -> T3PickerObject {
        // Exact issued lookup remains cleanup authority after the native port ends.
        try saveCandidates(op, queue: &queue, save: save)
        if op.status == "staged" { try verify(op) }
        return ["operationId": op.request.operationId, "identity": op.request.identity.json, "status": op.status, "files": op.files, "error": op.error]
    }
    func issuedFile(_ request: T3ComposerFileRequest, operation op: T3PickerOperation) throws -> T3PickerObject {
        guard op.status == "staged", op.claim.live, request.identity == op.request.identity, request.generation == op.request.generation,
              T3PickerRequest.equal(request.object["target"], op.request.object["target"]), request.action == "acquire" else { throw T3PickerRequest.error("This issued picker cannot acquire a file hold.") }
        let file = try request.file()
        guard file["status"] as? String == "staged", file["attachmentId"] as? String == "", let source = op.files.first(where: { $0["id"] as? String == file["id"] as? String }), source["kind"] as? String == "file",
              ["name", "mimeType", "sizeBytes", "videoWidth", "videoHeight"].allSatisfy({ T3PickerRequest.equal(source[$0] ?? NSNull(), file[$0] ?? NSNull()) }),
              try fingerprint(source) == op.fingerprints[file["id"] as! String] else { throw T3PickerRequest.error("The hold does not name an unchanged issued file.") }
        return file
    }
    /// Cold cleanup must not convert malformed ordinary ownership into empty inventories.
    static func validateInventory(_ preferences: T3PickerObject) throws {
        guard T3PickerRequest.integer(preferences["version"]) == 1, let drafts = preferences["drafts"] as? [String: String], !drafts.keys.contains(""),
              preferences["pending"] == nil || preferences["pending"] is T3PickerObject else { throw T3PickerRequest.error("The saved ordinary inventory is invalid.") }
        guard let snapshots = preferences["snapshotDrafts"] as? [String: [T3PickerObject]],
              let files = (preferences["composerFiles"] ?? [T3PickerObject]()) as? [T3PickerObject],
              let orders = (preferences["mobileAttachmentOrder"] ?? T3PickerObject()) as? [String: [String]] else { throw T3PickerRequest.error("The saved attachment inventory is invalid.") }
        var ids = [String: Set<String>]()
        for (key, rows) in snapshots {
            guard !key.isEmpty else { throw T3PickerRequest.error("The saved image owner is invalid.") }
            for row in rows {
                try descriptor(row, owner: key, ids: &ids)
                guard row["uploadId"] == nil || row["uploadId"] is String, row["source"] == nil || row["source"] is T3PickerObject else { throw T3PickerRequest.error("The saved image metadata is invalid.") }
            }
        }
        for file in files {
            try descriptor(file, owner: file["draftKey"] as? String ?? "", ids: &ids)
            guard ["draftKey", "environmentId", "contextId"].allSatisfy({ (file[$0] as? String)?.isEmpty == false }),
                  ["attached", "pasted-text"].contains(file["source"] as? String ?? ""), ["staged", "ready"].contains(file["status"] as? String ?? ""), file["attachmentId"] is String else { throw T3PickerRequest.error("The saved file owner is invalid.") }
            for name in ["videoWidth", "videoHeight"] where file[name] != nil {
                guard let number = file[name] as? NSNumber, CFGetTypeID(number) != CFBooleanGetTypeID(), number.doubleValue.isFinite, number.doubleValue > 0 else { throw T3PickerRequest.error("The saved video metadata is invalid.") }
            }
        }
        if let pending = preferences["pending"] as? T3PickerObject {
            for (environment, raw) in pending {
                guard !environment.isEmpty, let entry = raw as? T3PickerObject, (entry["method"] as? String)?.isEmpty == false, let payload = entry["payload"] as? T3PickerObject else { throw T3PickerRequest.error("The saved pending owner is invalid.") }
                var payloads = [payload]
                if let raw = payload["initialMessage"] { guard let initial = raw as? T3PickerObject else { throw T3PickerRequest.error("The saved pending message is invalid.") }; payloads.append(initial) }
                for body in payloads where body["attachments"] != nil {
                    guard let rows = body["attachments"] as? [T3PickerObject], rows.allSatisfy({ ($0["id"] as? String)?.isEmpty == false }) else { throw T3PickerRequest.error("The saved pending attachments are invalid.") }
                }
            }
        }
        if let raw = preferences["mobileComposerContexts"] {
            guard let registry = raw as? T3PickerObject, T3PickerRequest.integer(registry["version"]) == 1, let entries = registry["entries"] as? [String: T3PickerObject] else { throw T3PickerRequest.error("The saved context registry is invalid.") }
            for (key, row) in entries {
                guard let scope = try? JSONSerialization.jsonObject(with: Data(key.utf8)) as? [String], scope.count == 3,
                      scope == [row["origin"] as? String ?? "", row["environmentId"] as? String ?? "", row["key"] as? String ?? ""], ["origin", "environmentId", "key"].allSatisfy({ (row[$0] as? String)?.isEmpty == false }), T3PickerRequest.integer(row["revision"]) != nil, row["text"] is String,
                      let context = row["context"] as? T3PickerObject, T3PickerRequest.integer(context["version"]) == 1, let records = context["records"] as? [T3PickerObject],
                      records.allSatisfy({ ($0["contextId"] as? String)?.isEmpty == false && ($0["kind"] as? String)?.isEmpty == false }) else { throw T3PickerRequest.error("The saved context entry is invalid.") }
            }
        }
        guard orders.allSatisfy({ !$0.key.isEmpty && $0.value.allSatisfy { !$0.isEmpty } && Set($0.value).count == $0.value.count }) else { throw T3PickerRequest.error("The saved attachment order is invalid.") }
    }
    private static func descriptor(_ row: T3PickerObject, owner: String, ids: inout [String: Set<String>]) throws {
        guard let id = row["id"] as? String, !id.isEmpty, !owner.isEmpty, ids[owner, default: []].insert(id.lowercased()).inserted,
              ["name", "mimeType"].allSatisfy({ (row[$0] as? String)?.isEmpty == false }), T3PickerRequest.integer(row["sizeBytes"]) != nil else { throw T3PickerRequest.error("The saved attachment metadata is invalid.") }
    }
    private func publication(_ raw: Any?, op: T3PickerOperation, preferences: T3PickerObject) throws {
        guard let p = raw as? T3PickerObject, Set(p.keys) == Set(["after", "acceptedIds", "discardedIds"]), let after = p["after"] as? T3PickerObject,
              Set(after.keys) == Set(["document", "text", "context", "images", "files", "attachmentIds", "attachmentOrder"]),
              let accepted = p["acceptedIds"] as? [String], let discarded = p["discardedIds"] as? [String],
              Set(accepted + discarded).count == accepted.count + discarded.count,
              Set(accepted + discarded) == Set(op.files.map { $0["id"] as! String }) else { throw T3PickerRequest.error("The picker publication partition is invalid.") }
        try Self.validateInventory(preferences)
        let target = op.request.object["target"] as! [String: String], key = target["draftKey"]!, origin = target["origin"]!, env = target["environmentId"]!
        guard let registry = preferences["mobileComposerEditor"] as? T3PickerObject, T3PickerRequest.integer(registry["version"]) == 1,
              let documents = registry["documents"] as? [String: T3PickerObject], let document = after["document"] as? T3PickerObject,
              let text = after["text"] as? String, text.utf16.count <= 1_000_000,
              Set(document.keys) == Set(["origin", "environmentId", "threadId", "draftKey", "incarnation", "revision", "selection"]),
              document["origin"] as? String == origin, document["environmentId"] as? String == env, document["threadId"] as? String == target["threadId"], document["draftKey"] as? String == key,
              document["blocked"] == nil, document["value"] == nil,
              let captured = op.request.object["document"] as? T3PickerObject, document["incarnation"] as? String == captured["incarnation"] as? String,
              let revision = T3PickerRequest.integer(document["revision"]), revision >= (captured["revision"] as! Int),
              let drafts = preferences["drafts"] as? [String: String], (drafts[key] ?? "") == text else { throw T3PickerRequest.error("The captured saved document changed.") }
        if !(document["selection"] is NSNull) {
            guard let selection = document["selection"] as? T3PickerObject, Set(selection.keys) == Set(["start", "end"]),
                  let start = T3PickerRequest.integer(selection["start"]), let end = T3PickerRequest.integer(selection["end"]), start <= end, end <= text.utf16.count else { throw T3PickerRequest.error("The saved selection is invalid.") }
        }
        let matches = documents.filter { entry in
            guard let fields = try? JSONSerialization.jsonObject(with: Data(entry.key.utf8)) as? [String] else { return false }; return fields == [origin, env, key]
        }
        guard matches.count == 1, T3PickerRequest.equal(matches.first!.value, document),
              let images = preferences["snapshotDrafts"] as? [String: [T3PickerObject]], let files = (preferences["composerFiles"] ?? [T3PickerObject]()) as? [T3PickerObject],
              let orders = (preferences["mobileAttachmentOrder"] ?? T3PickerObject()) as? [String: [String]],
              T3PickerRequest.equal(images[key] ?? [], after["images"]), T3PickerRequest.equal(files.filter { $0["draftKey"] as? String == key }, after["files"]),
              T3PickerRequest.equal(orders[key].map { $0 as Any } ?? NSNull(), after["attachmentOrder"]) else { throw T3PickerRequest.error("The saved picker inventory or document changed.") }
        let contexts: T3PickerObject
        if let raw = preferences["mobileComposerContexts"] {
            guard let registry = raw as? T3PickerObject, T3PickerRequest.integer(registry["version"]) == 1, let entries = registry["entries"] as? T3PickerObject else { throw T3PickerRequest.error("The saved context registry is invalid.") }; contexts = entries
        } else { contexts = [:] }
        if !(after["context"] is NSNull) {
            guard let context = after["context"] as? T3PickerObject, Set(context.keys) == Set(["origin", "environmentId", "key", "revision", "text", "context"]),
                  context["origin"] as? String == origin, context["environmentId"] as? String == env, context["key"] as? String == key,
                  context["text"] as? String == text, T3PickerRequest.integer(context["revision"]) != nil else { throw T3PickerRequest.error("The saved context does not match this document.") }
        }
        let contextMatches = contexts.filter { entry in
            guard let fields = try? JSONSerialization.jsonObject(with: Data(entry.key.utf8)) as? [String] else { return false }; return fields == [origin, env, key]
        }
        guard contextMatches.count <= 1, T3PickerRequest.equal(contextMatches.first?.value ?? NSNull(), after["context"]), let ids = after["attachmentIds"] as? [String],
              Set(ids).count == ids.count, Set(ids) == Set((images[key] ?? []).compactMap { $0["id"] as? String } + files.filter { $0["draftKey"] as? String == key }.compactMap { $0["id"] as? String }),
              T3PickerRequest.equal(after["attachmentOrder"], ids.isEmpty ? NSNull() : ids as Any) else { throw T3PickerRequest.error("The saved picker context or mixed order changed.") }
        for id in accepted {
            let issued = op.files.first { $0["id"] as? String == id }!, image = issued["kind"] as? String == "image"
            let rows = image ? images[key] ?? [] : files.filter { $0["draftKey"] as? String == key }
            guard let row = rows.first(where: { $0["id"] as? String == id }), ["name", "mimeType", "sizeBytes", "videoWidth", "videoHeight"].allSatisfy({ T3PickerRequest.equal(row[$0] ?? NSNull(), issued[$0] ?? NSNull()) }) else { throw T3PickerRequest.error("An accepted picker member is not saved in its original byte lane.") }
        }
    }
    func transition(_ request: T3PickerRequest, op: T3PickerOperation, preferences: T3PickerObject, queue: inout T3PickerObject, save: (T3PickerObject) throws -> Void) throws -> T3PickerObject {
        if request.action == "finish" {
            let signature = Data(SHA256.hash(data: try T3PickerRequest.encoded(request.object["publication"] ?? NSNull())))
            if let prior = op.terminal { guard prior == signature, op.status == "finished" else { throw T3PickerRequest.error("A finished picker partition cannot change.") } }
            else {
                guard op.status == "staged" else { throw T3PickerRequest.error("Only a staged picker can finish publication.") }
                try publication(request.object["publication"], op: op, preferences: preferences); try verify(op)
                try replace(T3PickerRequest.encoded(preferences), root.appendingPathComponent("t3-code.json"))
                try saveCandidates(op, queue: &queue, save: save)
                op.terminal = signature; op.status = "finished"; terminalSerial += 1; op.terminalOrder = terminalSerial
            }
        } else if request.action == "cancel" {
            guard op.status != "finished" else { throw T3PickerRequest.error("An adopted picker cannot be cancelled.") }
            try saveCandidates(op, queue: &queue, save: save)
            if op.status != "cancelled" { terminalSerial += 1; op.terminalOrder = terminalSerial }; op.status = "cancelled"
        }
        let result = try response(op, queue: &queue, save: save); compactTerminals(); return result
    }
    private func compactTerminals() {
        let ended = operations.values.flatMap { $0.values }.filter { !$0.claim.live && ["finished", "cancelled"].contains($0.status) }.sorted { $0.terminalOrder < $1.terminalOrder }
        for op in ended.prefix(max(0, ended.count - 4096)) {
            let key = ObjectIdentifier(op.claim); operations[key]?.removeValue(forKey: op.request.operationId)
            if operations[key]?.isEmpty == true { operations.removeValue(forKey: key) }
        }
    }
    func retireEnded(queue: inout T3PickerObject, save: (T3PickerObject) throws -> Void) {
        for values in operations.values where values.values.first?.claim.live == false {
            for op in values.values where op.status == "picking" {
                do {
                    try saveCandidates(op, queue: &queue, save: save)
                    // No staged result ever escaped. Staged/terminal operations remain issued so
                    // an accepted CAS can finish its original named save after route retirement.
                    op.status = "cancelled"; terminalSerial += 1; op.terminalOrder = terminalSerial
                } catch { /* Keep native protection while candidate durability is unknown. */ }
            }
        }
        compactTerminals()
    }
}
#endif
