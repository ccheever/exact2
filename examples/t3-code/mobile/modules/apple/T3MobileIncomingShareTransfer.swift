// @ref llp/1109.005-composer-and-transcript.decision.md#incoming-share-inbox
// Called exclusively under T3MobileQueuedEdit's preference/attachment mutex.
import Foundation
import CryptoKit

final class T3MobileIncomingShareTransfer {
    typealias Object = [String: Any]
    private let root: URL
    private let inbox: T3MobileIncomingShares
    private let manager = FileManager.default
    init(root: URL) {
        self.root = root
        inbox = T3MobileIncomingShares(directory: root.appendingPathComponent("incoming-shares", isDirectory: true))
    }
    private static func fail(_ message: String) -> NSError {
        NSError(domain: "T3MobileIncomingShareTransfer", code: 1, userInfo: [NSLocalizedDescriptionKey: message])
    }
    private static func equal(_ lhs: Any?, _ rhs: Any?) -> Bool {
        guard let lhs, let rhs,
              let a = try? JSONSerialization.data(withJSONObject: lhs, options: [.sortedKeys, .fragmentsAllowed]),
              let b = try? JSONSerialization.data(withJSONObject: rhs, options: [.sortedKeys, .fragmentsAllowed]) else { return lhs == nil && rhs == nil }
        return a == b
    }
    private static func object<T: Encodable>(_ value: T) throws -> Object {
        try JSONSerialization.jsonObject(with: JSONEncoder().encode(value)) as! Object
    }
    private static func destination(_ raw: Any?) throws -> Object {
        guard let value = raw as? Object, Set(value.keys) == Set(["draftKey", "environmentId", "projectId", "origin"]),
              value.values.allSatisfy({ ($0 as? String).map { !$0.isEmpty && $0 == $0.trimmingCharacters(in: .whitespacesAndNewlines) } ?? false }),
              let key = value["draftKey"] as? String, key.range(of: "^new-task:[A-Za-z0-9_-]{1,128}$", options: .regularExpression) != nil,
              !key.hasPrefix("new-task:pending-"),
              let origin = value["origin"] as? String, let url = URL(string: origin),
              ["http", "https"].contains(url.scheme ?? ""), url.host != nil else {
            throw fail("Choose the exact project draft for this incoming share.")
        }
        return value
    }
    static func records(_ raw: Any?) throws -> [String: Object] {
        guard let raw else { return [:] }
        guard let records = raw as? [String: Object] else { throw fail("The incoming share ownership journal is invalid.") }
        var active = Set<String>(), bytes = Set<String>()
        for (id, record) in records {
            guard UUID(uuidString: id) != nil, record["adoptionId"] as? String == id,
                  let share = record["shareId"] as? String, share.count == 70, share.hasPrefix("share-"),
                  let phase = record["phase"] as? String, ["reserved", "staging", "staged", "consumed", "released"].contains(phase),
                  let entry = record["entry"] as? Object, entry["id"] as? String == share,
                  let attachments = entry["attachments"] as? [Object],
                  let selected = record["attachmentIds"] as? [String], Set(selected).count == selected.count,
                  selected.allSatisfy({ UUID(uuidString: $0) != nil && $0 == $0.lowercased() }) else {
                throw fail("The saved incoming share reservation is invalid.")
            }
            let encoded = try JSONSerialization.data(withJSONObject: entry)
            let decoded = try JSONDecoder().decode(T3MobileIncomingShares.Entry.self, from: encoded)
            guard decoded.schemaVersion == 1, UUID(uuidString: decoded.instanceId) != nil, ISO8601DateFormatter().date(from: decoded.createdAt) != nil else { throw fail("The saved incoming share entry is invalid.") }
            _ = try destination(record["destination"])
            if !["consumed", "released"].contains(phase) {
                guard active.insert(share).inserted, selected.allSatisfy({ bytes.insert($0).inserted }) else { throw fail("Incoming share ownership overlaps.") }
            }
            guard selected.allSatisfy({ selectedID in attachments.contains { $0["id"] as? String == selectedID } }) else { throw fail("The saved incoming share attachment is invalid.") }
        }
        return records
    }
    static func protects(_ id: String, records raw: Any?) throws -> Bool {
        try records(raw).values.contains { !["consumed", "released"].contains($0["phase"] as? String ?? "") && ($0["attachmentIds"] as? [String] ?? []).contains(id) }
    }
    private func descriptors(_ record: Object) -> [Object] {
        let entry = record["entry"] as! Object, all = entry["attachments"] as! [Object]
        return (record["attachmentIds"] as! [String]).compactMap { id in all.first { $0["id"] as? String == id } }
    }
    private func response(_ record: Object) -> Object {
        let consumed = record["phase"] as? String == "consumed"
        return ["adoptionId": record["adoptionId"]!, "entry": consumed ? NSNull() : record["entry"]!,
                "attachments": descriptors(record), "consumed": consumed]
    }
    private func path(_ attachment: Object) -> URL {
        root.appendingPathComponent(attachment["kind"] as? String == "image" ? "snapshots/drafts" : "composer-files", isDirectory: true)
            .appendingPathComponent(attachment["id"] as! String)
    }
    private static func marker(_ record: Object) -> Object {
        ["version": 1, "shareId": record["shareId"]!, "adoptionId": record["adoptionId"]!,
         "instanceId": (record["entry"] as! Object)["instanceId"]!, "createdAt": (record["entry"] as! Object)["createdAt"]!, "destination": record["destination"]!, "attachmentIds": record["attachmentIds"]!]
    }
    private static func imports(_ document: Object) throws -> [String: [String: Object]] {
        guard let raw = document["mobileIncomingShareImports"] else { return [:] }
        guard let imports = raw as? [String: [String: Object]] else { throw fail("Saved incoming share receipts are invalid.") }
        return imports
    }
    private static func evidence(_ document: Object, record: Object, records: [String: Object]) throws {
        let destination = record["destination"] as! Object, key = destination["draftKey"] as! String
        let entry = record["entry"] as! Object
        let selected = record["attachmentIds"] as! [String]
        let receipts = try imports(document)[key] ?? [:]
        // The pinned composer deduplicates a share's content for the lifetime of a
        // draft. An earlier consumed native receipt proves that no-op even after
        // the user edited its text or removed its chips. A new inbox incarnation
        // still receives its own immutable receipt and must select no new bytes.
        let duplicate = selected.isEmpty && records.values.contains { prior in
            guard prior["phase"] as? String == "consumed", prior["adoptionId"] as? String != record["adoptionId"] as? String,
                  prior["shareId"] as? String == record["shareId"] as? String,
                  equal(prior["destination"], destination), let priorEntry = prior["entry"] as? Object,
                  priorEntry["instanceId"] as? String != entry["instanceId"] as? String,
                  let priorID = prior["adoptionId"] as? String else { return false }
            return equal(receipts[priorID], marker(prior))
        }
        let metadata = ((document["mobileNewTaskDrafts"] as? Object)?["records"] as? [String: Object])?[key]
        guard let metadata, metadata["key"] as? String == key,
              ["environmentId", "projectId", "origin"].allSatisfy({ equal(metadata[$0], destination[$0]) }),
              let text = (document["drafts"] as? [String: String])?[key],
              duplicate || (entry["text"] as? String ?? "").isEmpty || text.contains(entry["text"] as! String) else {
            throw fail("Save the shared text in its exact project draft before consuming it.")
        }
        let images = (document["snapshotDrafts"] as? [String: [Object]])?[key] ?? []
        let files = (document["composerFiles"] as? [Object] ?? []).filter { $0["draftKey"] as? String == key }
        let order = (document["mobileAttachmentOrder"] as? [String: [String]])?[key] ?? []
        let all = entry["attachments"] as! [Object]
        for id in selected {
            let attachment = all.first { $0["id"] as? String == id }!
            let candidates = attachment["kind"] as? String == "image" ? images : files
            guard candidates.contains(where: { item in item["id"] as? String == id && item["sizeBytes"] as? Int == attachment["sizeBytes"] as? Int }), order.contains(id) else {
                throw fail("Save every selected shared attachment and its order before consuming it.")
            }
        }
        guard order.filter({ selected.contains($0) }) == selected else { throw fail("The shared attachment order changed before adoption.") }
    }
    /// Existing receipts cannot disappear in a stale whole-document save. Until consume,
    /// also preserve the adopted content. Consumed receipts remain provenance after edits.
    static func validatePreferences(previous: Object, next: Object, records raw: Any?) throws {
        let records = try records(raw), prior = try imports(previous), proposed = try imports(next)
        for (key, entries) in prior {
            for (id, marker) in entries where !equal(proposed[key]?[id], marker) { throw fail("Preserve the saved incoming share receipt when saving this draft.") }
        }
        var seen = Set<String>()
        for (key, entries) in proposed {
            for (id, marker) in entries {
                guard seen.insert(id).inserted, let record = records[id],
                      (record["destination"] as? Object)?["draftKey"] as? String == key,
                      ["staged", "consumed"].contains(record["phase"] as? String ?? ""), equal(marker, Self.marker(record)) else {
                    throw fail("The incoming share receipt does not match its reserved draft.")
                }
                if record["phase"] as? String != "consumed" { try evidence(next, record: record, records: records) }
            }
        }
    }
    func request(_ request: Object, records raw: Any?, preferences: Object, save: ([String: Object]) throws -> Void) throws -> Object {
        var records = try Self.records(raw)
        let action = request["action"] as? String ?? "read"
        if action == "read" {
            if records.values.contains(where: { $0["phase"] as? String == "consumed" }) { try save(records) }
            for record in records.values where record["phase"] as? String == "consumed" {
                try inbox.remove(record["shareId"] as! String, adoptionID: record["adoptionId"] as! String)
            }
            let active = records.values.filter { !["consumed", "released"].contains($0["phase"] as? String ?? "") }
            let reservations = active.map { record -> Object in
                ["shareId": record["shareId"]!, "adoptionId": record["adoptionId"]!, "destination": record["destination"]!,
                 "attachmentIds": record["attachmentIds"]!, "phase": record["phase"] as? String == "reserved" ? "reserved" : "staged"]
            }
            return ["available": false, "entries": try inbox.entries().map(Self.object), "reservations": reservations]
        }
        guard let share = request["shareId"] as? String else { throw Self.fail("Choose a saved incoming share.") }
        let destination = try Self.destination(request["destination"])
        if action == "reserve" {
            if let existing = records.values.first(where: { $0["shareId"] as? String == share && !["consumed", "released"].contains($0["phase"] as? String ?? "") }) {
                guard Self.equal(existing["destination"], destination) else { throw Self.fail("This share is reserved for another project draft.") }
                guard try inbox.read(share)?.instanceId == (existing["entry"] as? Object)?["instanceId"] as? String else {
                    throw Self.fail("The reserved incoming share instance changed.")
                }
                try inbox.bind(share, adoptionID: existing["adoptionId"] as! String)
                try save(records) // A previous publish may have lost its durability reply.
                return response(existing)
            }
            try save(records)
            for old in records.values where old["shareId"] as? String == share && old["phase"] as? String == "consumed" {
                try inbox.remove(share, adoptionID: old["adoptionId"] as! String)
            }
            guard let entry = try inbox.read(share) else { throw Self.fail("The incoming share is no longer available.") }
            let id = UUID().uuidString.lowercased()
            let record: Object = ["adoptionId": id, "shareId": share, "destination": destination, "entry": try Self.object(entry), "attachmentIds": [String](), "phase": "reserved"]
            records[id] = record
            try save(records); try inbox.bind(share, adoptionID: id)
            return response(record)
        }
        guard let id = request["adoptionId"] as? String, var record = records[id], record["shareId"] as? String == share,
              Self.equal(record["destination"], destination) else { throw Self.fail("The incoming share reservation changed.") }
        if record["phase"] as? String == "consumed" {
            guard action == "consume" || action == "stage" else { throw Self.fail("This incoming share was already adopted.") }
            try save(records); try inbox.remove(share, adoptionID: id)
            return response(record)
        }
        if record["phase"] as? String == "released", action == "release" {
            try save(records); try inbox.unbind(share, adoptionID: id)
            return response(record)
        }
        guard record["phase"] as? String != "released", try inbox.reservation(share) == id else { throw Self.fail("The incoming share reservation ended.") }
        if action == "stage" {
            guard let selected = request["attachmentIds"] as? [String], Set(selected).count == selected.count,
                  selected.allSatisfy({ selectedID in ((record["entry"] as! Object)["attachments"] as! [Object]).contains { $0["id"] as? String == selectedID } }) else { throw Self.fail("Choose attachments from this incoming share.") }
            if record["phase"] as? String != "reserved" {
                guard Self.equal(record["attachmentIds"], selected) else { throw Self.fail("The incoming share attachment selection is already frozen.") }
            }
            record["attachmentIds"] = selected; record["phase"] = "staging"; records[id] = record
            try save(records) // Persist byte ownership before any copy is published.
            for attachment in descriptors(record) {
                let source = try inbox.attachmentURL(shareID: share, attachmentID: attachment["id"] as! String), target = path(attachment)
                let directory = target.deletingLastPathComponent()
                try manager.createDirectory(at: directory, withIntermediateDirectories: true)
                if manager.fileExists(atPath: target.path) {
                    let properties = try target.resourceValues(forKeys: [.isRegularFileKey, .isSymbolicLinkKey, .fileSizeKey])
                    guard properties.isRegularFile == true, properties.isSymbolicLink != true,
                          properties.fileSize == attachment["sizeBytes"] as? Int,
                          SHA256.hash(data: try Data(contentsOf: target)) == SHA256.hash(data: try Data(contentsOf: source)) else {
                        throw Self.fail("An existing attachment has different bytes; keep both owners for recovery.")
                    }
                } else {
                    // Publish atomically so a crash during copying cannot leave a partial UUID file.
                    try T3MobileIncomingShares.durableWrite(Data(contentsOf: source), to: target)
                }
                try T3MobileIncomingShares.sync(target); try T3MobileIncomingShares.sync(directory)
                try T3MobileIncomingShares.sync(directory.deletingLastPathComponent())
            }
            record["phase"] = "staged"; records[id] = record; try save(records)
            return response(record)
        }
        let key = destination["draftKey"] as! String, imported = try Self.imports(preferences)[key]?[id]
        if action == "consume" {
            guard record["phase"] as? String == "staged", Self.equal(imported, Self.marker(record)) else { throw Self.fail("Save the matching incoming share receipt before consuming it.") }
            try Self.evidence(preferences, record: record, records: records)
            for attachment in descriptors(record) {
                let target = path(attachment), source = try inbox.attachmentURL(shareID: share, attachmentID: attachment["id"] as! String)
                let properties = try target.resourceValues(forKeys: [.isRegularFileKey, .isSymbolicLinkKey, .fileSizeKey])
                guard properties.isRegularFile == true, properties.isSymbolicLink != true,
                      properties.fileSize == attachment["sizeBytes"] as? Int,
                      SHA256.hash(data: try Data(contentsOf: target)) == SHA256.hash(data: try Data(contentsOf: source)) else {
                    throw Self.fail("The saved incoming attachment bytes changed before adoption.")
                }
                try T3MobileIncomingShares.sync(target)
            }
            try T3MobileIncomingShares.sync(root.appendingPathComponent("t3-code.json")); try T3MobileIncomingShares.sync(root)
            record["phase"] = "consumed"; records[id] = record; try save(records)
            try inbox.remove(share, adoptionID: id)
            return response(record)
        }
        if action == "release" {
            guard imported == nil else { throw Self.fail("This draft already saved its incoming share; finish adoption before releasing it.") }
            record["phase"] = "released"; records[id] = record; try save(records)
            try inbox.unbind(share, adoptionID: id)
            return response(record)
        }
        throw Self.fail("Unknown incoming share action.")
    }
}
