// @ref llp/1109.005-composer-and-transcript.decision.md#local-outbox-storage
// Standalone Foundation fixture; compile actual store sources with their iOS guards removed.
import Foundation

struct T3Failure: Error { let kind: String; let message: String; var uncertain = false }
enum T3Wire { static let maximumBytes = 16 * 1024 * 1024 }

@main struct OutboxStorageTests {
    static var checks = 0
    static func check(_ condition: Bool, _ name: String) {
        guard condition else { fatalError(name) }; checks += 1
    }
    static func refuses(_ name: String, _ body: () throws -> Void) {
        do { try body(); fatalError("Expected refusal: \(name)") } catch { checks += 1 }
    }
    static let fileID = "00000000-0000-4000-a000-000000000001"
    static func record(_ id: String = "message", environment: String = "env") -> [String: Any] {
        ["schemaVersion": 1, "origin": "https://one.test", "environmentId": environment,
         "threadId": "thread-\(id)", "messageId": id, "commandId": "command-\(id)", "text": "original",
         "createdAt": "2026-10-08T12:00:00.000Z", "attachments": [["id": fileID, "kind": "file", "name": "a.txt",
         "mimeType": "text/plain", "sizeBytes": 5, "uploadId": "", "status": "staged", "contextId": "file1", "source": "attached"]],
         "creation": ["projectId": "project", "workspaceMode": "local", "branch": NSNull(), "worktreePath": NSNull()]]
    }
    static func call(_ store: T3MobileQueuedEdit, _ action: String, _ fields: [String: Any] = [:]) throws -> [String: Any] {
        try store.outbox(fields.merging(["action": action]) { _, new in new })
    }
    static func confirm(_ store: T3MobileQueuedEdit, _ id: String, _ revision: Int, _ decision: String = "commit") throws -> [String: Any] {
        try call(store, "confirm", ["messageId": id, "expectedRevision": revision, "decision": decision])
    }
    static func receiptRetention(_ root: URL) throws {
        let imageID = "00000000-0000-4000-a000-000000000002"
        let store = T3MobileQueuedEdit(root: root)
        let image = root.appendingPathComponent("snapshots/drafts/\(imageID)")
        let file = root.appendingPathComponent("composer-files/\(fileID)")
        for url in [image, file] {
            try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            try Data("owned".utf8).write(to: url)
        }
        let slot = "[\"env\",\"command-receipt\"]"
        let receipt: [String: Any] = ["version": 1, "key": "new-task:draft", "environmentId": "env", "origin": "https://one.test",
            "projectId": "project", "threadId": "thread-receipt", "commandId": "command-receipt", "revision": 1,
            "text": "sent", "payload": "{}", "images": [["id": imageID]], "files": [["id": fileID]]]
        var registry: [String: Any] = ["version": 1, "records": [String: Any](), "receipts": [slot: receipt],
            "claims": [slot: "new-task:draft"], "fileReleases": [String]()]
        func preferences(_ value: [String: Any]) throws -> String {
            String(data: try JSONSerialization.data(withJSONObject: ["mobileNewTaskDrafts": value]), encoding: .utf8)!
        }
        try store.writePreferences(preferences(registry))
        var queued = record("receipt")
        queued["attachments"] = (queued["attachments"] as! [[String: Any]]) + [["id": imageID, "kind": "image", "name": "a.png",
            "mimeType": "image/png", "sizeBytes": 5, "uploadId": "", "status": "staged"]]
        _ = try call(store, "enqueue", ["record": queued]); _ = try confirm(store, "receipt", 1)
        _ = try call(store, "remove", ["messageId": "receipt", "expectedRevision": 1]); _ = try confirm(store, "receipt", 2)
        _ = try call(store, "completeRemoval", ["messageId": "receipt", "expectedRevision": 2])
        _ = try store.releaseAttachments()
        check(FileManager.default.fileExists(atPath: image.path) && FileManager.default.fileExists(atPath: file.path), "receipt-only image and file survive outbox release")
        check(try store.removeAttachment(["op": "snapshotDraftRemove", "id": imageID])["retained"] as? Bool == true, "direct image removal respects receipt")
        check(try store.removeAttachment(["op": "composerAttachRemove", "id": fileID])["retained"] as? Bool == true, "direct file removal respects receipt")
        registry["receipts"] = [String: Any](); registry["claims"] = [String: Any]()
        let failing = T3MobileQueuedEdit(root: root, replace: { _, _ in throw CocoaError(.fileWriteUnknown) })
        refuses("failed preferences save cannot release receipts") { try failing.writePreferences(preferences(registry)) }
        _ = try store.releaseAttachments()
        check(FileManager.default.fileExists(atPath: file.path), "old durable receipt survives failed replacement")
        for invalid in [NSNull(), ["slot": NSNull()]] as [Any] {
            var malformed = registry; malformed["receipts"] = invalid
            try store.writePreferences(preferences(malformed))
            refuses("malformed receipt inventory retains bytes") { _ = try store.removeAttachment(["op": "composerAttachRemove", "id": fileID]) }
        }
        try store.writePreferences(preferences(registry)); _ = try store.releaseAttachments()
        check(!FileManager.default.fileExists(atPath: image.path) && !FileManager.default.fileExists(atPath: file.path), "durable receipt removal permits both releases")
    }
    static func main() throws {
        if CommandLine.arguments.count == 3, CommandLine.arguments[1] == "--decode" {
            let values = try JSONSerialization.jsonObject(with: Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[2]))) as! [[String: Any]]
            print(String(data: try JSONSerialization.data(withJSONObject: values.map(T3MobileOutbox.validateRecord)), encoding: .utf8)!)
            return
        }
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("t3-outbox-test-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: root) }
        let store = T3MobileQueuedEdit(root: root)
        check(try call(store, "read")["complete"] as? Bool == true, "fresh inventory complete")
        let file = root.appendingPathComponent("composer-files/\(fileID)")
        try FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
        try Data("hello".utf8).write(to: file)
        let first = try call(store, "enqueue", ["record": record()])
        check(first["revision"] as? Int == 1 && first["pending"] as? Bool == true, "enqueue pending durable marker")
        check(try store.removeAttachment(["op": "composerAttachRemove", "id": fileID])["retained"] as? Bool == true, "pending record protects bytes")
        check(try confirm(store, "message", 99)["current"] as? Bool == false, "wrong revision cannot confirm")
        check((try call(store, "clearEnvironment", ["environmentId": "env", "origin": "https://one.test"])["errors"] as! [[String: Any]]).isEmpty == false, "clear reports unresolved marker")
        _ = try confirm(store, "message", 1)
        let reopened = T3MobileQueuedEdit(root: root)
        check(try confirm(reopened, "message", 1)["current"] as? Bool == true, "confirmed record survives restart")
        _ = try call(store, "hold", ["messageId": "message", "expectedRevision": 1, "owner": "a"])
        _ = try call(store, "hold", ["messageId": "message", "expectedRevision": 1, "owner": "b"])
        _ = try call(store, "releaseHold", ["messageId": "message", "owner": "a"])
        check(try confirm(store, "message", 1)["current"] as? Bool == false, "other editor hold remains")
        check(try call(store, "remove", ["messageId": "message", "expectedRevision": 1, "requireUnheld": true])["removed"] as? Bool == false, "held removal refused")
        _ = try call(store, "releaseHold", ["messageId": "message", "owner": "b"])
        var edited = record(); edited["text"] = "edited"
        check(try call(store, "update", ["record": edited, "expectedRevision": 2])["applied"] as? Bool == false, "stale CAS never writes")
        _ = try call(store, "update", ["record": edited, "expectedRevision": 1])
        _ = try confirm(store, "message", 2, "rollback")
        let restored = try call(store, "read")["records"] as! [[String: Any]]
        check((restored[0]["record"] as! [String: Any])["text"] as? String == "original", "rollback keeps prior and consumes revision")
        check(try call(store, "update", ["record": edited, "expectedRevision": 1])["applied"] as? Bool == false, "rollback cannot cause ABA")
        _ = try call(store, "remove", ["messageId": "message", "expectedRevision": 2])
        check(try store.removeAttachment(["op": "composerAttachRemove", "id": fileID])["retained"] as? Bool == true, "pending deletion protects previous bytes")
        _ = try confirm(store, "message", 3)
        check(try store.removeAttachment(["op": "composerAttachRemove", "id": fileID])["retained"] as? Bool == true, "deletion receipt owns bytes through editor cleanup")
        _ = try call(store, "completeRemoval", ["messageId": "message", "expectedRevision": 3])
        _ = try store.releaseAttachments()
        check(!FileManager.default.fileExists(atPath: file.path), "completed removal releases unowned bytes")
        _ = try call(store, "enqueue", ["record": record()])
        _ = try confirm(store, "message", 4)
        check(try call(store, "update", ["record": edited, "expectedRevision": 1])["applied"] as? Bool == false, "durable tombstone preserves revision across new enqueue")

        for side in ["before", "after"] {
            let dir = root.appendingPathComponent(side)
            var fail = true
            let failing = T3MobileQueuedEdit(root: dir, replace: { data, url in
                if side == "before" && fail { throw CocoaError(.fileWriteUnknown) }
                try T3MobileQueuedEdit.durableReplace(data, url)
                if fail { throw CocoaError(.fileWriteUnknown) }
            })
            do { _ = try call(failing, "enqueue", ["record": record(side)]); fatalError("expected injected failure") }
            catch let error as T3Failure { check(error.uncertain, "failure is explicitly ambiguous") }
            fail = false
            let restarted = T3MobileQueuedEdit(root: dir)
            let snapshot = try call(restarted, "read")
            if side == "before" { check((snapshot["records"] as! [[String: Any]]).isEmpty, "failure before write has no candidate") }
            else {
                check((snapshot["mutations"] as! [[String: Any]]).count == 1, "after-rename marker survives recreated owner")
                check(try confirm(restarted, side, 1)["current"] as? Bool == true, "exact saved candidate can be confirmed")
                _ = try call(restarted, "remove", ["messageId": side, "expectedRevision": 1])
                _ = try confirm(restarted, side, 2)
                check((try call(restarted, "read")["removals"] as! [[String: Any]]).count == 1, "recreated confirmed delete leaves receipt")
            }
        }
        let directory = root.appendingPathComponent("mobile-outbox")
        let corrupt = directory.appendingPathComponent("corrupt.json")
        try Data("{broken".utf8).write(to: corrupt)
        let partial = try call(store, "read")
        check(partial["complete"] as? Bool == false && !(partial["records"] as! [[String: Any]]).isEmpty, "partial load keeps readable rows")
        refuses("unknown inventory blocks unrelated attachment delete") { _ = try store.removeAttachment(["op": "composerAttachRemove", "id": "00000000-0000-4000-a000-000000000999"]) }
        refuses("unknown inventory blocks environment clear") { _ = try call(store, "clearEnvironment", ["environmentId": "env", "origin": "https://one.test"]) }
        _ = try call(store, "enqueue", ["record": record("readable-sibling")])
        _ = try confirm(store, "readable-sibling", 1)
        check(try confirm(store, "readable-sibling", 1)["current"] as? Bool == true, "corrupt sibling does not freeze healthy record")
        try FileManager.default.removeItem(at: corrupt)

        DispatchQueue.concurrentPerform(iterations: 24) { index in
            let id = "parallel-\(index)"
            _ = try! call(store, "enqueue", ["record": record(id, environment: "other")])
            _ = try! confirm(store, id, 1)
        }
        check((try call(store, "read")["records"] as! [[String: Any]]).count == 26, "coordinator serializes independent concurrent writers")
        _ = try call(store, "hold", ["messageId": "message", "expectedRevision": 4, "owner": "one"])
        check(try confirm(store, "parallel-0", 1)["current"] as? Bool == true, "editor hold is per message")
        let clear = try call(store, "clearEnvironment", ["environmentId": "other", "origin": "https://one.test"])
        let removals = clear["removals"] as! [[String: Any]]
        check(removals.count == 24, "environment clear captures only matching records")
        for removal in removals { _ = try confirm(store, removal["messageId"] as! String, removal["revision"] as! Int) }
        check((try call(store, "read")["records"] as! [[String: Any]]).count == 2, "confirmed environment deletion preserves unrelated records")

        var malformed = record(); malformed["schemaVersion"] = true
        check(!T3MobileOutbox.validateRecord(malformed), "Boolean is not schema integer")
        malformed = record(); var attachments = malformed["attachments"] as! [[String: Any]]; attachments[0]["sizeBytes"] = true; malformed["attachments"] = attachments
        check(!T3MobileOutbox.validateRecord(malformed), "Boolean is not attachment size")
        malformed = record(); attachments = malformed["attachments"] as! [[String: Any]]; var alias = attachments[0]; alias["id"] = fileID.uppercased(); malformed["attachments"] = attachments + [alias]
        check(!T3MobileOutbox.validateRecord(malformed), "UUID aliases cannot duplicate ownership")
        malformed = record(); malformed["createdAt"] = "2026-02-30T12:00:00.000Z"
        check(!T3MobileOutbox.validateRecord(malformed), "calendar-invalid ISO rejected")
        malformed = record(); malformed["origin"] = "http://127.1"
        check(!T3MobileOutbox.validateRecord(malformed), "noncanonical numeric host rejected")
        malformed = record(); malformed["context"] = ["version": 1, "records": [["version": 1, "contextId": "abc\n", "label": "a", "kind": "file"]]]
        check(!T3MobileOutbox.validateRecord(malformed), "context ownership ID newline rejected")
        for (field, value, expected) in [("origin", "https://xn--bcher-kva.example", true), ("origin", "https://example.test:99999", false),
            ("createdAt", "0000-02-29T12:00:00.000Z", true), ("environmentId", "env\u{FEFF}", false), ("environmentId", "env\u{0085}", true)] {
            malformed = record(); malformed[field] = value
            check(T3MobileOutbox.validateRecord(malformed) == expected, "TypeScript schema agreement: \(field) \(value)")
        }
        malformed = record(); attachments = malformed["attachments"] as! [[String: Any]]; attachments[0]["id"] = fileID + "\n"; malformed["attachments"] = attachments
        check(!T3MobileOutbox.validateRecord(malformed), "UUID newline refused")
        try receiptRetention(root.appendingPathComponent("receipt-ownership"))
        print("PASS \(checks) native outbox storage assertions")
    }
}
