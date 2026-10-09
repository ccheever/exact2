// Compile with the two Foundation incoming-share owners; no UI or real preferences.
import Foundation

@main struct IncomingShareTransferTests {
    typealias Object = [String: Any]
    static func main() throws {
        let fm = FileManager.default, root = fm.temporaryDirectory.appendingPathComponent("t3-share-transfer-" + UUID().uuidString)
        try fm.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? fm.removeItem(at: root) }
        let inbox = T3MobileIncomingShares(directory: root.appendingPathComponent("incoming-shares"))
        let owner = T3MobileIncomingShareTransfer(root: root)
        var journal: [String: Object] = [:], preferences: Object = [:], failures = [String](), checks = 0
        var failSave = false, publishThenFail = false, saveCalls = 0, failAtSave = -1
        let destination: Object = ["draftKey": "new-task:one", "environmentId": "env", "projectId": "project", "origin": "https://example.test"]
        func check(_ condition: Bool, _ message: String) { checks += 1; if !condition { failures.append(message) } }
        func refuse(_ message: String, _ run: () throws -> Void) { do { try run(); check(false, message) } catch { check(true, message) } }
        func call(_ action: String, share: String = "", id: String? = nil, selected: [String]? = nil, target: Object? = nil) throws -> Object {
            var request: Object = ["action": action, "shareId": share, "destination": target ?? destination]
            if let id { request["adoptionId"] = id }; if let selected { request["attachmentIds"] = selected }
            return try owner.request(request, records: journal, preferences: preferences) { next in
                saveCalls += 1
                if failSave || saveCalls == failAtSave { throw CocoaError(.fileWriteOutOfSpace) }
                journal = next
                if publishThenFail { throw CocoaError(.fileWriteUnknown) }
            }
        }
        func ingest(_ text: String, attachments: Int = 0) throws -> T3MobileIncomingShares.Entry {
            var payloads = [T3MobileIncomingShares.Payload(shareType: "text", mimeType: nil, value: text, originalName: nil)]
            for index in 0..<attachments {
                let file = root.appendingPathComponent("sender-\(index)")
                try Data("bytes-\(index)".utf8).write(to: file)
                payloads.append(.init(shareType: index == 0 ? "image" : "file", mimeType: index == 0 ? "image/png" : "text/plain", value: file.absoluteString, originalName: file.lastPathComponent))
            }
            return try inbox.ingest(payloads) { _ in }!
        }
        func document(_ entry: T3MobileIncomingShares.Entry, id: String, selected: [String]) -> Object {
            let images = entry.attachments.filter { selected.contains($0.id) && $0.kind == "image" }.map { ["id": $0.id, "sizeBytes": $0.sizeBytes] as Object }
            let files = entry.attachments.filter { selected.contains($0.id) && $0.kind == "file" }.map { ["id": $0.id, "sizeBytes": $0.sizeBytes, "draftKey": "new-task:one"] as Object }
            return ["drafts": ["new-task:one": "existing\n" + entry.text], "snapshotDrafts": ["new-task:one": images], "composerFiles": files,
                    "mobileAttachmentOrder": ["new-task:one": selected], "mobileNewTaskDrafts": ["records": ["new-task:one": destination.merging(["key": "new-task:one"]) { _, new in new }]],
                    "mobileIncomingShareImports": ["new-task:one": [id: ["version": 1, "shareId": entry.id, "adoptionId": id, "instanceId": entry.instanceId, "createdAt": entry.createdAt, "destination": destination, "attachmentIds": selected]]]]
        }
        func savePreferences(_ document: Object) throws {
            try T3MobileIncomingShareTransfer.validatePreferences(previous: preferences, next: document, records: journal)
            try T3MobileIncomingShares.durableWrite(JSONSerialization.data(withJSONObject: document), to: root.appendingPathComponent("t3-code.json"))
            preferences = document
        }
        let entry = try ingest("first", attachments: 2)
        failSave = true
        refuse("failed reservation retains inbox") { _ = try call("reserve", share: entry.id) }; failSave = false
        check(try inbox.read(entry.id) != nil, "reservation failure keeps original")
        let reserved = try call("reserve", share: entry.id), id = reserved["adoptionId"] as! String
        check(try call("reserve", share: entry.id)["adoptionId"] as? String == id, "reserve reply loss is idempotent")
        var other = destination; other["draftKey"] = "new-task:other"
        refuse("same share cannot cross drafts") { _ = try call("reserve", share: entry.id, target: other) }
        let selected = [entry.attachments[1].id]
        publishThenFail = true
        refuse("stage journal publish ambiguity stops before copying") { _ = try call("stage", share: entry.id, id: id, selected: selected) }; publishThenFail = false
        check(try T3MobileIncomingShareTransfer.protects(selected[0], records: journal), "staging journal protects bytes across cold owner")
        _ = try call("stage", share: entry.id, id: id, selected: selected)
        check(fm.fileExists(atPath: root.appendingPathComponent("composer-files/" + selected[0]).path), "selected original file is staged")
        check(!fm.fileExists(atPath: root.appendingPathComponent("snapshots/drafts/" + entry.attachments[0].id).path), "unselected image is not staged")
        refuse("selection cannot change after staging") { _ = try call("stage", share: entry.id, id: id, selected: entry.attachments.map(\.id)) }
        refuse("consume needs exact receipt") { _ = try call("consume", share: entry.id, id: id) }
        var wrong = document(entry, id: id, selected: selected); wrong["drafts"] = ["new-task:other": entry.text]
        refuse("wrong draft cannot publish receipt") { try savePreferences(wrong) }
        try savePreferences(document(entry, id: id, selected: selected))
        refuse("stale writer cannot drop receipt") { try savePreferences([:]) }
        refuse("release cannot delete committed draft bytes") { _ = try call("release", share: entry.id, id: id) }
        publishThenFail = true
        refuse("consumed publish lost reply retains inbox") { _ = try call("consume", share: entry.id, id: id) }; publishThenFail = false
        check(try inbox.read(entry.id) != nil, "ambiguous consumed publish retains inbox")
        check(try call("consume", share: entry.id, id: id)["consumed"] as? Bool == true, "consume recovery reestablishes durable tombstone")
        check(try inbox.read(entry.id) == nil, "consume removes inbox only after durable receipt")
        var edited = preferences; edited["drafts"] = [:] as Object; edited["snapshotDrafts"] = [:] as Object; edited["composerFiles"] = [Object](); edited["mobileAttachmentOrder"] = [:] as Object
        try savePreferences(edited)
        check(try call("consume", share: entry.id, id: id)["consumed"] as? Bool == true, "consume replay does not recreate removed draft contents")
        var pending = destination; pending["draftKey"] = "new-task:pending-owner"
        refuse("pending editor is not an incoming-share destination") { _ = try call("reserve", share: entry.id, target: pending) }
        let textEntry = try ingest("same text")
        let first = try call("reserve", share: textEntry.id)["adoptionId"] as! String
        _ = try call("stage", share: textEntry.id, id: first, selected: [])
        try savePreferences(document(textEntry, id: first, selected: []).merging(["mobileIncomingShareImports": ["new-task:one": (preferences["mobileIncomingShareImports"] as! [String: Object])["new-task:one"]!.merging((document(textEntry, id: first, selected: [])["mobileIncomingShareImports"] as! [String: Object])["new-task:one"]!) { _, new in new }]]) { _, new in new })
        _ = try call("consume", share: textEntry.id, id: first)
        let again = try ingest("same text"), second = try call("reserve", share: again.id)["adoptionId"] as! String
        check(again.instanceId != textEntry.instanceId, "identical payloads retain distinct native inbox incarnations")
        check(first != second && again.id == textEntry.id, "later identical handoff gets new adoption identity")
        _ = try call("consume", share: textEntry.id, id: first)
        check(try inbox.read(again.id) != nil, "old consume cannot delete later identical handoff")
        _ = try call("release", share: again.id, id: second)
        _ = try call("release", share: again.id, id: second)
        check(try inbox.read(again.id) != nil, "release/reply loss leaves inbox reusable")
        let duplicateId = try call("reserve", share: again.id)["adoptionId"] as! String
        _ = try call("stage", share: again.id, id: duplicateId, selected: [])
        var duplicateDocument = document(again, id: duplicateId, selected: [])
        let priorReceipts = preferences["mobileIncomingShareImports"] as! [String: [String: Object]]
        var duplicateReceipts = duplicateDocument["mobileIncomingShareImports"] as! [String: [String: Object]]
        duplicateReceipts["new-task:one"] = priorReceipts["new-task:one"]!.merging(duplicateReceipts["new-task:one"]!) { _, new in new }
        duplicateDocument["mobileIncomingShareImports"] = duplicateReceipts
        duplicateDocument["drafts"] = ["new-task:one": "User replaced every word"]
        var unconsumed = journal; unconsumed[first]!["phase"] = "released"
        var onlyCurrent = duplicateDocument
        onlyCurrent["mobileIncomingShareImports"] = ["new-task:one": [duplicateId: duplicateReceipts["new-task:one"]![duplicateId]!]]
        let duplicateRequest: Object = ["action": "consume", "shareId": again.id, "adoptionId": duplicateId, "destination": destination]
        refuse("unconsumed prior reservation cannot prove a duplicate no-op") {
            _ = try owner.request(duplicateRequest, records: unconsumed, preferences: duplicateDocument) { _ in }
        }
        refuse("consumed journal without its saved preference receipt cannot prove a duplicate no-op") {
            _ = try owner.request(duplicateRequest, records: journal, preferences: onlyCurrent) { _ in }
        }
        var wrongDraft = journal, otherDestination = destination; otherDestination["draftKey"] = "new-task:other"
        wrongDraft[first]!["destination"] = otherDestination
        var misplacedReceipt = duplicateReceipts["new-task:one"]![first]!
        misplacedReceipt["destination"] = otherDestination
        var misplaced = onlyCurrent
        misplaced["mobileIncomingShareImports"] = ["new-task:one": [duplicateId: duplicateReceipts["new-task:one"]![duplicateId]!], "new-task:other": [first: misplacedReceipt]]
        refuse("another draft's consumed receipt cannot prove a duplicate no-op") {
            _ = try owner.request(duplicateRequest, records: wrongDraft, preferences: misplaced) { _ in }
        }
        var forged = duplicateDocument, forgedReceipts = duplicateReceipts
        forgedReceipts["new-task:one"]![first]!["instanceId"] = UUID().uuidString.lowercased()
        forged["mobileIncomingShareImports"] = forgedReceipts
        refuse("forged earlier receipt cannot prove a duplicate no-op") {
            _ = try owner.request(duplicateRequest, records: journal, preferences: forged) { _ in }
        }
        try savePreferences(duplicateDocument)
        _ = try call("consume", share: again.id, id: duplicateId)
        check(try inbox.read(again.id) == nil, "proven duplicate consumes its new inbox incarnation")
        check((preferences["drafts"] as? [String: String])?["new-task:one"] == "User replaced every word", "duplicate adoption preserves edited-away original content")
        let interrupted = try ingest("interrupted-copy", attachments: 1)
        let interruptedId = try call("reserve", share: interrupted.id)["adoptionId"] as! String
        let interruptedFiles = interrupted.attachments.map(\.id)
        failAtSave = saveCalls + 2
        refuse("crash after byte publish before staged receipt remains recoverable") { _ = try call("stage", share: interrupted.id, id: interruptedId, selected: interruptedFiles) }
        failAtSave = -1
        check(fm.fileExists(atPath: root.appendingPathComponent("snapshots/drafts/" + interruptedFiles[0]).path), "interrupted stage retains published bytes")
        _ = try call("stage", share: interrupted.id, id: interruptedId, selected: interruptedFiles)
        let stagedPath = root.appendingPathComponent("snapshots/drafts/" + interruptedFiles[0])
        try Data("changed".utf8).write(to: stagedPath)
        refuse("mismatched existing destination bytes are never overwritten") { _ = try call("stage", share: interrupted.id, id: interruptedId, selected: interruptedFiles) }
        check(try Data(contentsOf: stagedPath) == Data("changed".utf8), "collision retains existing destination bytes")
        let read = try call("read")
        check(read["available"] as? Bool == false, "GAP006 remains unavailable")
        for failure in failures { print("FAIL: \(failure)") }
        print("Incoming share transfer: \(checks - failures.count)/\(checks) checks passed")
        if !failures.isEmpty { exit(1) }
    }
}
