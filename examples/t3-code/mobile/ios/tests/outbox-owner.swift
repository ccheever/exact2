// @ref llp/1109.005-composer-and-transcript.decision.md#local-outbox-storage
// Compile actual Foundation owner/coordinator/store sources with only iOS guards removed.
import Foundation

struct T3Failure: Error { let kind: String; let message: String; var uncertain = false }
enum T3Wire { static let maximumBytes = 16 * 1024 * 1024 }
typealias Obj = [String: Any]
final class AnswerBox {
    let ready = DispatchSemaphore(value: 0)
    var result: Result<Obj, Error>?
    func accept(_ value: Result<Obj, Error>) { result = value; ready.signal() }
    func get() throws -> Obj {
        guard ready.wait(timeout: .now() + 5) == .success else { throw T3Failure(kind: "Test", message: "Timed out") }
        return try result!.get()
    }
}
final class WriteGate {
    let entered = DispatchSemaphore(value: 0), release = DispatchSemaphore(value: 0)
    var match: (Obj) -> Bool
    private var used = false
    init(_ match: @escaping (Obj) -> Bool) { self.match = match }
    func write(_ data: Data, _ url: URL) throws {
        let value = (try? JSONSerialization.jsonObject(with: data)) as? Obj ?? [:]
        if !used && match(value) { used = true; entered.signal(); _ = release.wait(timeout: .now() + 5) }
        try T3MobileQueuedEdit.durableReplace(data, url)
    }
}
@main struct OutboxOwnerTests {
    static var checks = 0, failures: [String] = []
    static let imageID = "00000000-0000-4000-a000-000000000001"
    static let fileID = "00000000-0000-4000-a000-000000000002"
    static func check(_ value: Bool, _ name: String) { checks += 1; if !value { failures.append(name) } }
    static func run(_ name: String, _ body: () throws -> Void) { do { try body() } catch { failures.append("\(name): \(error)") } }
    static func record(_ text: String, _ id: String = "message", files: Bool = false) -> Obj {
        ["schemaVersion": 1, "origin": "https://one.test", "environmentId": "env", "threadId": "thread", "messageId": id,
         "commandId": "command", "text": text, "createdAt": "2026-10-08T12:00:00.000Z", "attachments": files ? [
            ["id": imageID, "kind": "image", "name": "a.png", "mimeType": "image/png", "sizeBytes": 4, "uploadId": "", "status": "staged"],
            ["id": fileID, "kind": "file", "name": "a.txt", "mimeType": "text/plain", "sizeBytes": 4, "uploadId": "", "status": "staged", "contextId": "f", "source": "attached"]] : []]
    }
    static func submit(_ store: T3MobileQueuedEdit, _ request: Obj) -> AnswerBox {
        let result = AnswerBox(); store.submitOutbox(request) { result.accept($0) }; return result
    }
    static func call(_ store: T3MobileQueuedEdit, _ action: String, _ fields: Obj = [:]) throws -> Obj {
        try submit(store, fields.merging(["action": action]) { _, b in b }).get()
    }
    static func request(_ epoch: String, _ seq: Int, _ operation: String, _ text: String = "", _ extra: Obj = [:]) -> Obj {
        var value: Obj = ["action": "mutate", "ownerEpoch": epoch, "mutationId": "\(epoch):\(seq)", "messageId": "message", "operation": operation]
        if operation != "remove" { value["record"] = record(text) }
        return value.merging(extra) { _, b in b }
    }
    static func text(_ result: Obj) -> String? { (result["record"] as? Obj)?["text"] as? String }
    static func currentText(_ result: Obj) -> String? { text(result["current"] as? Obj ?? [:]) }
    static func main() {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("t3-outbox-owner-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: root) }
        run("FIFO and dropped observer") {
            for phase in ["pending", "active"] {
                let gate = WriteGate { $0["state"] as? String == phase }
                let store = T3MobileQueuedEdit(root: root.appendingPathComponent(phase), replace: gate.write)
                let epoch = try call(store, "read")["ownerEpoch"] as! String
                let a = submit(store, request(epoch, 1, "enqueue", "A"))
                check(gate.entered.wait(timeout: .now() + 5) == .success, "pause A \(phase)")
                let b = submit(store, request(epoch, 2, "enqueue", "B"))
                gate.release.signal()
                let answerA = try a.get(), answerB = try b.get()
                check(answerA["status"] as? String == "committed" && answerB["status"] as? String == "committed", "both same-ID invocations commit at \(phase)")
                check(currentText(answerA) == "B" && currentText(answerB) == "B", "A cannot overwrite optimistic B at \(phase)")
                check(try call(store, "confirmQueued", ["ownerEpoch": epoch, "messageId": "message", "token": "\(epoch):2"])["current"] as? Bool == true, "B admitted after own durability")
                let reopened = T3MobileQueuedEdit(root: root.appendingPathComponent(phase))
                let read = try call(reopened, "read")
                check(text((read["records"] as! [Obj])[0]) == "B", "B survives restart")
                check((try call(reopened, "status", ["messageId": "message", "mutationId": "\(epoch):1"])["status"] as? String) == "committed", "A exact outcome survives dropped reply")
            }
        }
        run("source post-write update races") {
            for guarded in [true, false] {
                let gate = WriteGate { $0["state"] as? String == "active" && text($0) == "U" }
                let store = T3MobileQueuedEdit(root: root.appendingPathComponent("update-\(guarded)"), replace: gate.write)
                let epoch = try call(store, "read")["ownerEpoch"] as! String
                _ = try submit(store, request(epoch, 1, "enqueue", "O")).get()
                let update = submit(store, request(epoch, 2, "update", "U", guarded ? ["expectedToken": "\(epoch):1"] : [:]))
                check(gate.entered.wait(timeout: .now() + 5) == .success, "pause update")
                let enqueue = submit(store, request(epoch, 3, "enqueue", "B")); gate.release.signal()
                let u = try update.get(), b = try enqueue.get()
                check(u["status"] as? String == (guarded ? "stale" : "committed"), "optional CAS source status")
                check(currentText(b) == (guarded ? "B" : "U"), "optional CAS source projection")
                check((b["current"] as! Obj)["revision"] as? Int == (guarded ? 2 : 3), "post-publication revision correct")
                let recovered = try call(T3MobileQueuedEdit(root: root.appendingPathComponent("update-\(guarded)")), "read")
                check(text((recovered["records"] as! [Obj])[0]) == "B", "source later enqueue persists B")
            }
        }
        run("remove loses to editor hold") {
            let gate = WriteGate { $0["state"] as? String == "deleted" && ($0["outcomes"] as? [String: Obj])?.count == 2 }
            let store = T3MobileQueuedEdit(root: root.appendingPathComponent("hold"), replace: gate.write)
            let epoch = try call(store, "read")["ownerEpoch"] as! String
            _ = try submit(store, request(epoch, 1, "enqueue", "O")).get()
            let removal = submit(store, request(epoch, 2, "remove", "", ["requireUnheld": true, "expectedToken": "\(epoch):1"]))
            check(gate.entered.wait(timeout: .now() + 5) == .success, "pause remove")
            let hold = submit(store, ["action": "hold", "ownerEpoch": epoch, "messageId": "message", "owner": "editor", "expectedToken": "\(epoch):1"])
            gate.release.signal()
            check(try removal.get()["status"] as? String == "stale", "late hold cancels removal")
            check(try hold.get()["held"] as? Bool == true, "hold admitted while fsync paused")
            check(text((try call(store, "read")["records"] as! [Obj])[0]) == "O", "canceled removal restores payload")
        }
        run("operation-specific failure and replay") {
            for operation in ["enqueue", "update", "remove"] {
                var armed = false
                let store = T3MobileQueuedEdit(root: root.appendingPathComponent("failure-\(operation)"), replace: { data, url in
                    let value = try JSONSerialization.jsonObject(with: data) as! Obj
                    if armed && value["state"] as? String == "pending" { armed = false; throw T3OutboxReplaceFailure(replaced: false, cause: CocoaError(.fileWriteUnknown)) }
                    try T3MobileQueuedEdit.durableReplace(data, url)
                })
                let epoch = try call(store, "read")["ownerEpoch"] as! String
                _ = try submit(store, request(epoch, 1, "enqueue", "O")).get(); armed = true
                let req = request(epoch, 2, operation, "N")
                let answer = try submit(store, req).get()
                check(answer["status"] as? String == "failed", "known failure classified \(operation)")
                check(currentText(answer) == (operation == "enqueue" ? nil : "O"), "failure semantics \(operation)")
                check(try submit(store, req).get()["status"] as? String == "failed", "exact replay is idempotent")
                _ = try call(store, "acknowledge", ["messageId": "message", "mutationId": "\(epoch):2"])
                do { _ = try submit(store, req).get(); check(false, "ack replay rejected") } catch { check(true, "ack replay rejected") }
            }
        }
        run("final rename uncertainty and recreated owner sync") {
            let directory = root.appendingPathComponent("uncertain")
            var fail = true
            let store = T3MobileQueuedEdit(root: directory, replace: { data, url in
                try T3MobileQueuedEdit.durableReplace(data, url)
                let value = try JSONSerialization.jsonObject(with: data) as! Obj
                if fail && value["state"] as? String == "active" { fail = false; throw T3OutboxReplaceFailure(replaced: true, cause: CocoaError(.fileWriteUnknown)) }
            })
            let epoch = try call(store, "read")["ownerEpoch"] as! String
            check(try submit(store, request(epoch, 1, "enqueue", "A")).get()["status"] as? String == "uncertain", "after final rename uncertain")
            check(try call(store, "status", ["messageId": "message", "mutationId": "\(epoch):1"])["status"] as? String == "uncertain", "visible committed outcome does not authorize cleanup")
            check(try call(store, "confirmQueued", ["ownerEpoch": epoch, "messageId": "message", "token": "\(epoch):1"])["current"] as? Bool == false, "uncertain cannot dispatch")
            var failSync = true
            let restarted = T3MobileQueuedEdit(root: directory, replace: { data, url in
                if failSync { throw T3OutboxReplaceFailure(replaced: false, cause: CocoaError(.fileWriteUnknown)) }
                try T3MobileQueuedEdit.durableReplace(data, url)
            })
            check(try call(restarted, "read")["complete"] as? Bool == false, "recreated owner failed sync explicit")
            check(try call(restarted, "status", ["messageId": "message", "mutationId": "\(epoch):1"])["status"] as? String == "uncertain", "recreated failed sync remains uncertain")
            failSync = false
            check(try call(restarted, "recover", ["messageId": "message", "mutationId": "\(epoch):1", "decision": "retry"])["status"] as? String == "committed", "exact terminal sync succeeds")
        }
        run("accepted proposal bytes and receipt release") {
            let directory = root.appendingPathComponent("bytes")
            let gate = WriteGate { $0["state"] as? String == "pending" }
            let store = T3MobileQueuedEdit(root: directory, replace: gate.write)
            let epoch = try call(store, "read")["ownerEpoch"] as! String
            let file = directory.appendingPathComponent("composer-files/\(fileID)")
            try FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
            try Data("file".utf8).write(to: file)
            let a = submit(store, request(epoch, 1, "enqueue", "A"))
            check(gate.entered.wait(timeout: .now() + 5) == .success, "A paused before pending installed")
            let b = submit(store, request(epoch, 2, "enqueue", "B", ["record": record("B", files: true)]))
            check(try store.removeAttachment(["op": "composerAttachRemove", "id": fileID])["retained"] as? Bool == true, "unpersisted B owns bytes")
            gate.release.signal(); _ = try a.get(); _ = try b.get()
            _ = try submit(store, request(epoch, 3, "remove")).get()
            for n in 1...3 { _ = try call(store, "acknowledge", ["messageId": "message", "mutationId": "\(epoch):\(n)"]) }
            check(try store.removeAttachment(["op": "composerAttachRemove", "id": fileID])["retained"] as? Bool == true, "deletion receipt owns after ack")
            _ = try call(store, "completeRemoval", ["messageId": "message", "mutationId": "\(epoch):3"])
            _ = try store.releaseAttachments()
            check(!FileManager.default.fileExists(atPath: file.path), "completed cleanup releases bytes")
        }
        run("B known-no-write receipt preserves uncertain A") {
            let directory = root.appendingPathComponent("blocked")
            let gate = WriteGate { $0["state"] as? String == "pending" }
            var failA = true
            let store = T3MobileQueuedEdit(root: directory, replace: { data, url in
                let value = try JSONSerialization.jsonObject(with: data) as! Obj
                try gate.write(data, url)
                if failA && value["state"] as? String == "pending" { failA = false; throw T3OutboxReplaceFailure(replaced: true, cause: CocoaError(.fileWriteUnknown)) }
            })
            let epoch = try call(store, "read")["ownerEpoch"] as! String
            let a = submit(store, request(epoch, 1, "enqueue", "A"))
            check(gate.entered.wait(timeout: .now() + 5) == .success, "A pending pause for uncertainty")
            let b = submit(store, request(epoch, 2, "enqueue", "B", ["record": record("B", files: true)])); gate.release.signal()
            check(try a.get()["status"] as? String == "uncertain", "A pending uncertainty retained")
            let failedB = try b.get()
            check(failedB["status"] as? String == "failed" && currentText(failedB) == nil, "B known no write removes only its enqueue")
            let reopened = T3MobileQueuedEdit(root: directory)
            let read = try call(reopened, "read")
            check((read["mutations"] as! [Obj]).count == 1, "A marker survives B classification")
            check(try call(reopened, "status", ["messageId": "message", "mutationId": "\(epoch):2"])["status"] as? String == "failed", "B exact failure survives restart")
            check(try reopened.removeAttachment(["op": "composerAttachRemove", "id": fileID])["retained"] as? Bool == true, "B failure receipt retains bytes")
            let recovered = try call(reopened, "recover", ["messageId": "message", "mutationId": "\(epoch):1", "decision": "commit"])
            check(currentText(recovered) == "A" && (recovered["current"] as! Obj)["pending"] as? Bool == false, "pending recovery replaces old-token row")
        }
        run("acknowledged terminal sync retry") {
            let directory = root.appendingPathComponent("ack-sync")
            let store = T3MobileQueuedEdit(root: directory)
            let epoch = try call(store, "read")["ownerEpoch"] as! String
            _ = try submit(store, request(epoch, 1, "enqueue", "A")).get()
            _ = try call(store, "acknowledge", ["messageId": "message", "mutationId": "\(epoch):1"])
            var failSync = true
            let reopened = T3MobileQueuedEdit(root: directory, replace: { data, url in
                if failSync { throw T3OutboxReplaceFailure(replaced: false, cause: CocoaError(.fileWriteUnknown)) }
                try T3MobileQueuedEdit.durableReplace(data, url)
            })
            check(try call(reopened, "read")["complete"] as? Bool == false, "acked row sync failure visible")
            failSync = false
            let reread = try call(reopened, "read")
            check(reread["complete"] as? Bool == true && (reread["records"] as! [Obj])[0]["pending"] as? Bool == false, "read retries acknowledged terminal sync")
        }
        run("partial load retry and malformed transaction") {
            let directory = root.appendingPathComponent("partial")
            let original = T3MobileQueuedEdit(root: directory)
            let epoch = try call(original, "read")["ownerEpoch"] as! String
            _ = try submit(original, request(epoch, 1, "enqueue", "A")).get()
            let file = try FileManager.default.contentsOfDirectory(at: directory.appendingPathComponent("mobile-outbox"), includingPropertiesForKeys: nil).first!
            let healthy = try Data(contentsOf: file)
            var malformed = try JSONSerialization.jsonObject(with: healthy) as! Obj
            malformed["state"] = "pending"; malformed["mutation"] = Obj(); malformed["previous"] = malformed["record"]; malformed["proposed"] = malformed["record"]; malformed.removeValue(forKey: "record")
            try JSONSerialization.data(withJSONObject: malformed).write(to: file)
            let reopened = T3MobileQueuedEdit(root: directory)
            check(try call(reopened, "read")["complete"] as? Bool == false, "malformed transaction is partial load, not crash")
            try healthy.write(to: file)
            check((try call(reopened, "read")["records"] as! [Obj]).count == 1, "repaired unknown ID merges on retry")
        }
        run("coordinator survives dropped module reference") {
            let directory = root.appendingPathComponent("lifetime")
            let gate = WriteGate { $0["state"] as? String == "pending" }
            var owner: T3MobileQueuedEdit? = T3MobileQueuedEdit.shared(root: directory, replace: gate.write)
            weak var weakOwner = owner
            let epoch = try call(owner!, "read")["ownerEpoch"] as! String
            let a = submit(owner!, request(epoch, 1, "enqueue", "A"))
            check(gate.entered.wait(timeout: .now() + 5) == .success, "lifetime pause")
            owner = nil
            let next = T3MobileQueuedEdit.shared(root: directory)
            check(weakOwner != nil && weakOwner === next, "accepted callback retains same registered coordinator")
            let b = submit(next, request(epoch, 2, "enqueue", "B")); gate.release.signal()
            _ = try a.get(); check(try b.get()["status"] as? String == "committed", "new module uses original FIFO")
        }
        run("B failed receipt save remains owned and uncertain") {
            let directory = root.appendingPathComponent("blocked-receipt-failure")
            let gate = WriteGate { $0["state"] as? String == "pending" }
            var attempts = 0
            let store = T3MobileQueuedEdit(root: directory, replace: { data, url in
                let value = try JSONSerialization.jsonObject(with: data) as! Obj
                if value["state"] as? String == "pending" {
                    attempts += 1
                    if attempts == 2 { throw T3OutboxReplaceFailure(replaced: false, cause: CocoaError(.fileWriteUnknown)) }
                    try gate.write(data, url)
                    if attempts == 1 { throw T3OutboxReplaceFailure(replaced: true, cause: CocoaError(.fileWriteUnknown)) }
                } else { try T3MobileQueuedEdit.durableReplace(data, url) }
            })
            let epoch = try call(store, "read")["ownerEpoch"] as! String
            let a = submit(store, request(epoch, 1, "enqueue", "A"))
            check(gate.entered.wait(timeout: .now() + 5) == .success, "pause before B failed receipt")
            let b = submit(store, request(epoch, 2, "enqueue", "B", ["record": record("B", files: true)])); gate.release.signal()
            _ = try a.get(); check(try b.get()["status"] as? String == "uncertain", "B cannot claim durable known failure")
            check(try call(store, "status", ["messageId": "message", "mutationId": "\(epoch):2"])["status"] as? String == "uncertain", "B uncertainty survives answer")
            check(try store.removeAttachment(["op": "composerAttachRemove", "id": fileID])["retained"] as? Bool == true, "B unpersisted failure receipt retains bytes in memory")
        }
        run("pending update/remove rollback restores confirmed prior row") {
            for operation in ["update", "remove"] {
            let directory = root.appendingPathComponent("rollback-\(operation)")
            var armed = false
            let store = T3MobileQueuedEdit(root: directory, replace: { data, url in
                try T3MobileQueuedEdit.durableReplace(data, url)
                let value = try JSONSerialization.jsonObject(with: data) as! Obj
                if armed && value["state"] as? String == "pending" { armed = false; throw T3OutboxReplaceFailure(replaced: true, cause: CocoaError(.fileWriteUnknown)) }
            })
            let epoch = try call(store, "read")["ownerEpoch"] as! String
            _ = try submit(store, request(epoch, 1, "enqueue", "O")).get(); armed = true
            _ = try submit(store, request(epoch, 2, operation, "U")).get()
            let reopened = T3MobileQueuedEdit(root: directory)
            let pendingRow = (try call(reopened, "read")["records"] as! [Obj])[0]
            check(pendingRow["token"] as? String == "\(epoch):\(operation == "remove" ? 1 : 2)", "pending \(operation) exposes correct payload owner")
            let answer = try call(reopened, "recover", ["messageId": "message", "mutationId": "\(epoch):2", "decision": "rollback"])
            check(currentText(answer) == "O" && (answer["current"] as! Obj)["pending"] as? Bool == false, "rollback updates old-token in-memory projection")
            check((answer["current"] as! Obj)["token"] as? String == "\(epoch):1", "rollback restores previous token")
            }
        }
        run("receipt-only bytes retain existing independent launch guarantees") {
            let directory = root.appendingPathComponent("launch-receipt")
            let store = T3MobileQueuedEdit(root: directory)
            let paths = [directory.appendingPathComponent("snapshots/drafts/\(imageID)"), directory.appendingPathComponent("composer-files/\(fileID)")]
            for path in paths { try FileManager.default.createDirectory(at: path.deletingLastPathComponent(), withIntermediateDirectories: true); try Data("data".utf8).write(to: path) }
            var registry: Obj = ["version": 1, "records": Obj(), "receipts": ["slot": ["images": [["id": imageID]], "files": [["id": fileID]]]],
                                 "claims": ["slot": "new-task:a"], "fileReleases": [String]()]
            func write(_ value: Obj) throws { try store.writePreferences(String(data: JSONSerialization.data(withJSONObject: ["mobileNewTaskDrafts": value]), encoding: .utf8)!) }
            try write(registry)
            check(try store.removeAttachment(["op": "snapshotDraftRemove", "id": imageID])["retained"] as? Bool == true, "receipt-only image retained")
            check(try store.removeAttachment(["op": "composerAttachRemove", "id": fileID])["retained"] as? Bool == true, "receipt-only file retained")
            registry["receipts"] = NSNull(); try write(registry)
            do { _ = try store.removeAttachment(["op": "composerAttachRemove", "id": fileID]); check(false, "malformed receipt blocks removal") } catch { check(true, "malformed receipt blocks removal") }
            registry["receipts"] = Obj(); registry["claims"] = Obj(); try write(registry)
            _ = try store.removeAttachment(["op": "snapshotDraftRemove", "id": imageID]); _ = try store.removeAttachment(["op": "composerAttachRemove", "id": fileID])
            check(paths.allSatisfy { !FileManager.default.fileExists(atPath: $0.path) }, "durable receipt removal releases both")
        }
        run("pending envelope consistency and interrupted API shape") {
            for operation in ["enqueue", "update", "remove"] {
                let directory = root.appendingPathComponent("inconsistent-\(operation)")
                let store = T3MobileQueuedEdit(root: directory)
                let epoch = try call(store, "read")["ownerEpoch"] as! String
                _ = try submit(store, request(epoch, 1, "enqueue", "O")).get()
                let file = try FileManager.default.contentsOfDirectory(at: directory.appendingPathComponent("mobile-outbox"), includingPropertiesForKeys: nil).first!
                var envelope = try JSONSerialization.jsonObject(with: Data(contentsOf: file)) as! Obj
                envelope["state"] = "pending"; envelope["previous"] = envelope["record"]; envelope.removeValue(forKey: "record")
                envelope["mutation"] = request(epoch, 2, operation, "intended"); envelope["proposed"] = record("wrong")
                try JSONSerialization.data(withJSONObject: envelope).write(to: file)
                let reopened = T3MobileQueuedEdit(root: directory)
                check(try call(reopened, "read")["complete"] as? Bool == false, "inconsistent \(operation) proposal quarantined")
                do { _ = try call(reopened, "recover", ["messageId": "message", "mutationId": "\(epoch):2", "decision": "commit"]); check(false, "inconsistent recovery refused") }
                catch { check(true, "inconsistent recovery refused") }
            }
            let directory = root.appendingPathComponent("pending-shape")
            var armed = true
            let store = T3MobileQueuedEdit(root: directory, replace: { data, url in
                try T3MobileQueuedEdit.durableReplace(data, url)
                if armed { armed = false; throw T3OutboxReplaceFailure(replaced: true, cause: CocoaError(.fileWriteUnknown)) }
            })
            let epoch = try call(store, "read")["ownerEpoch"] as! String
            _ = try submit(store, request(epoch, 1, "enqueue", "A")).get()
            let status = try call(store, "status", ["messageId": "message", "mutationId": "\(epoch):1"])
            check(status["status"] as? String == "uncertain" && status["revision"] is Int && status["record"] is NSNull && status["removed"] is NSNull,
                  "interrupted pending status is a complete Outcome")
            let reopened = T3MobileQueuedEdit(root: directory)
            let read = try call(reopened, "read"), row = (read["records"] as! [Obj])[0]
            check(row["token"] as? String == "\(epoch):1" && row["pending"] as? Bool == true, "cold pending proposal owns mutation token")
            let rollback = try call(reopened, "recover", ["messageId": "message", "mutationId": "\(epoch):1", "decision": "rollback"])
            check((rollback["current"] as! Obj)["record"] is NSNull, "pending enqueue rollback preserves previous empty owner semantics")
        }
        print("Outbox owner: \(checks) assertions, \(failures.count) failures")
        for failure in failures { print("FAIL: \(failure)") }
        if !failures.isEmpty { exit(1) }
    }
}
