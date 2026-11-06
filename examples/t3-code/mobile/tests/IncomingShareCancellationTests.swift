// Foundation-only cancellation owner tests; every file is under a unique temporary root.
import Foundation

@main struct IncomingShareCancellationTests {
    typealias Object = [String: Any]
    static var checks = 0, failures = [String]()
    static func check(_ condition: Bool, _ message: String) { checks += 1; if !condition { failures.append(message) } }
    static func refuse(_ message: String, _ body: () throws -> Void) { do { try body(); check(false, message) } catch { check(true, message) } }
    final class Fixture {
        let root: URL, inbox: T3MobileIncomingShares, owner: T3MobileIncomingShareTransfer
        let destination: Object = ["draftKey": "new-task:cancel", "environmentId": "env", "projectId": "project", "origin": "https://example.test"]
        var journal = [String: Object](), preferences = Object(), failJournal = "", failPreferences = ""
        var entry: T3MobileIncomingShares.Entry!, adoption = ""
        init() throws {
            root = FileManager.default.temporaryDirectory.appendingPathComponent("t3-share-cancel-" + UUID().uuidString)
            try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
            inbox = T3MobileIncomingShares(directory: root.appendingPathComponent("incoming-shares"))
            owner = T3MobileIncomingShareTransfer(root: root)
            preferences = ["version": 1, "drafts": ["new-task:cancel": "Before", "new-task:other": "Other text"],
                "mobileNewTaskDrafts": ["version": 1, "records": ["new-task:cancel": destination.merging(["key": "new-task:cancel", "revision": 7, "createdAt": "2026-10-08T00:00:00Z"]) { _, new in new }]],
                "composerControls": ["staged": ["new-task:cancel": ["modelId": "model"]], "contexts": ["new-task:cancel": ["branch": "main"]]],
                "mobileRecoveredDrafts": ["old": ["key": "new-task:cancel", "text": "context"]]]
            try write(preferences)
            let source = root.appendingPathComponent("sender.txt"); try Data("owned bytes".utf8).write(to: source)
            entry = try inbox.ingest([.init(shareType: "text", mimeType: nil, value: "Shared content", originalName: nil),
                .init(shareType: "file", mimeType: "text/plain", value: source.absoluteString, originalName: "sender.txt")]) { _ in }!
            adoption = try call("reserve")["adoptionId"] as! String
            _ = try call("stage")
        }
        deinit { try? FileManager.default.removeItem(at: root) }
        func write(_ next: Object) throws {
            if failPreferences == "before" { throw CocoaError(.fileWriteOutOfSpace) }
            try T3MobileIncomingShares.durableWrite(JSONSerialization.data(withJSONObject: next), to: root.appendingPathComponent("t3-code.json"))
            preferences = next
            if failPreferences == "after" { throw CocoaError(.fileWriteUnknown) }
        }
        func saveJournal(_ next: [String: Object]) throws {
            if failJournal == "before" { throw CocoaError(.fileWriteOutOfSpace) }
            journal = next
            if failJournal == "after" { throw CocoaError(.fileWriteUnknown) }
        }
        func call(_ action: String, expected: Object? = nil) throws -> Object {
            var request: Object = ["action": action, "shareId": entry.id, "adoptionId": adoption, "destination": destination,
                "attachmentIds": entry.attachments.map(\.id)]
            if let expected { request["expectedDraft"] = expected }
            return try owner.request(request, records: journal, preferences: preferences, save: saveJournal, writePreferences: write)
        }
        func snapshot() -> Object { T3MobileIncomingShareTransfer.projection(preferences, key: "new-task:cancel") }
        func publishAdoption() throws {
            let file = entry.attachments[0]
            var document = preferences, drafts = preferences["drafts"] as! [String: String]
            drafts["new-task:cancel"] = "Before\n\nShared content"; document["drafts"] = drafts
            document["composerFiles"] = [["id": file.id, "sizeBytes": file.sizeBytes, "draftKey": "new-task:cancel"]]
            document["mobileAttachmentOrder"] = ["new-task:cancel": [file.id]]
            var store = document["mobileNewTaskDrafts"] as! Object, metadata = store["records"] as! [String: Object]
            metadata["new-task:cancel"]!["revision"] = 8; store["records"] = metadata; document["mobileNewTaskDrafts"] = store
            document["mobileIncomingShareImports"] = ["new-task:cancel": [adoption: ["version": 1, "shareId": entry.id, "adoptionId": adoption,
                "instanceId": entry.instanceId, "createdAt": entry.createdAt, "destination": destination, "attachmentIds": [file.id]]]]
            try normalWrite(document)
        }
        func normalWrite(_ document: Object) throws {
            try T3MobileIncomingShareTransfer.validatePreferences(previous: preferences, next: document, records: journal)
            let captured = try T3MobileIncomingShareTransfer.captureAdoptions(document, previous: preferences, records: journal)
            try saveJournal(captured); try write(document)
        }
        var baseline: Object { journal[adoption]!["baselineDraft"] as! Object }
        var adopted: Object { journal[adoption]!["adoptionDraft"] as! Object }
    }
    static func main() throws {
        do {
            let f = try Fixture(), baseline = f.baseline
            let reply = try f.call("cancel", expected: baseline)
            check(reply["cancelled"] as? Bool == true, "cancel before first preference write completes")
            check((f.snapshot()["metadata"] as! Object)["revision"] as? Int == 8, "baseline rollback advances revision")
            check(try f.inbox.read(f.entry.id) != nil, "cancel retains the inbox originals")
            refuse("late consume cannot revive a cancelled adoption") { _ = try f.call("consume") }
        }
        do {
            let f = try Fixture(); try f.publishAdoption(); let adopted = f.adopted
            let reply = try f.call("cancel", expected: adopted)
            check(f.snapshot()["text"] as? String == "Before", "cancel restores pre-import text")
            check((f.snapshot()["files"] as! [Object]).isEmpty, "cancel restores pre-import attachments")
            check((f.snapshot()["metadata"] as! Object)["revision"] as? Int == 9, "post-import rollback uses a fresh revision")
            check((f.preferences["drafts"] as! [String: String])["new-task:other"] == "Other text", "cancel preserves unrelated drafts")
            check((f.preferences["mobileIncomingShareImports"] as! [String: Object])["new-task:cancel"]?.isEmpty == true, "cancel removes only its own receipt")
            check(try f.call("cancel", expected: adopted)["cancelled"] as? Bool == true, "lost cancellation reply retries without another restore")
            check((reply["restoredDraft"] as? Object)?["text"] as? String == "Before", "reply carries exact restored projection")
            var changed = f.preferences, drafts = changed["drafts"] as! [String: String]; drafts["new-task:cancel"] = "New typing"; changed["drafts"] = drafts
            try f.normalWrite(changed)
            refuse("terminal replay cannot return an old snapshot over newer disk edits") { _ = try f.call("cancel", expected: adopted) }
            check(f.snapshot()["text"] as? String == "New typing", "newer text survives cancelled replay")
        }
        for failure in ["before", "after"] {
            let f = try Fixture(); try f.publishAdoption(); let adopted = f.adopted
            f.failPreferences = failure
            refuse("rollback preference \(failure) failure is retained") { _ = try f.call("cancel", expected: adopted) }
            check(f.journal[f.adoption]?["phase"] as? String == "cancelling", "failed rollback retains durable cancellation intent")
            check(try T3MobileIncomingShareTransfer.protects(f.entry.attachments[0].id, records: f.journal), "cancellation intent protects staged bytes")
            refuse("consume cannot beat an already recorded cancellation intent") { _ = try f.call("consume") }
            f.failPreferences = ""
            check(try f.call("cancel", expected: adopted)["cancelled"] as? Bool == true, "rollback retry resolves \(failure) publication ambiguity")
        }
        for failure in ["before", "after"] {
            let f = try Fixture(); try f.publishAdoption(); let adopted = f.adopted
            f.failJournal = failure
            refuse("cancellation journal \(failure) failure does not restore early") { _ = try f.call("cancel", expected: adopted) }
            check(f.snapshot()["text"] as? String == "Before\n\nShared content", "journal failure leaves adopted content")
            f.failJournal = ""
            check(try f.call("cancel", expected: adopted)["cancelled"] as? Bool == true, "journal failure retry completes")
        }
        do {
            let f = try Fixture(); f.failPreferences = "before"
            refuse("initial adoption write may fail after native capture") { try f.publishAdoption() }
            let adopted = f.adopted; f.failPreferences = ""
            check(try f.call("cancel", expected: adopted)["cancelled"] as? Bool == true, "unsaved adoption cancels against durable baseline")
        }
        do {
            let f = try Fixture(); try f.publishAdoption(); let adopted = f.adopted
            var newer = f.preferences, drafts = newer["drafts"] as! [String: String]
            drafts["new-task:cancel"] = "Before\n\nShared content\nNew typing"; newer["drafts"] = drafts; try f.normalWrite(newer)
            refuse("newer persisted text blocks rollback") { _ = try f.call("cancel", expected: adopted) }
            refuse("newer unsaved local snapshot also blocks rollback") { _ = try f.call("cancel", expected: f.snapshot()) }
            check(f.snapshot()["text"] as? String == drafts["new-task:cancel"], "rollback rejection preserves newer text")
        }
        do {
            let f = try Fixture(); try f.publishAdoption(); let adopted = f.adopted; _ = try f.call("consume")
            refuse("consumption that won before cancellation keeps the imported draft") { _ = try f.call("cancel", expected: adopted) }
            check(f.snapshot()["text"] as? String == "Before\n\nShared content", "consumed-won refusal does not remove imported content")
        }
        do {
            let f = try Fixture()
            var newer = f.preferences, drafts = newer["drafts"] as! [String: String]
            drafts["new-task:cancel"] = "Before, with newer edits"; newer["drafts"] = drafts; try f.normalWrite(newer)
            refuse("reserve retry cannot pair a newer draft with the old rollback baseline") { _ = try f.call("reserve") }
            refuse("first import write refuses a changed durable baseline") { try f.publishAdoption() }
            check(f.snapshot()["text"] as? String == "Before, with newer edits", "failed admission preserves newer pre-import text")
        }
        for failure in failures { print("FAIL: \(failure)") }
        print("Incoming share cancellation: \(checks - failures.count)/\(checks) checks passed")
        if !failures.isEmpty { exit(1) }
    }
}
