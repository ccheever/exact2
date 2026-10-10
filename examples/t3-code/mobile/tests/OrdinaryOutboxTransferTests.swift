// @ref llp/1109.005-composer-and-transcript.decision.md#local-outbox-storage
// Actual outbox/store bodies and actual durableReplace body, isolated Foundation disk fixture.
import Foundation
import Darwin

typealias OrdinaryObject = [String: Any]
private final class OrdinaryAnswer {
    let semaphore = DispatchSemaphore(value: 0)
    var result: Result<OrdinaryObject, Error>?
    func receive(_ value: Result<OrdinaryObject, Error>) { result = value; semaphore.signal() }
    func get() throws -> OrdinaryObject {
        guard semaphore.wait(timeout: .now() + 10) == .success else { throw T3Failure(kind: "Test", message: "Owner callback timed out") }
        return try result!.get()
    }
}
final class OrdinaryDisk {
    var before: ((Data, URL) -> Bool)?
    var after: ((Data, URL) -> Bool)?
    var writes: [String: Int] = [:]
    func replace(_ data: Data, _ url: URL) throws {
        writes[url.lastPathComponent, default: 0] += 1
        if before?(data, url) == true { throw T3OutboxReplaceFailure(replaced: false, cause: CocoaError(.fileWriteUnknown)) }
        try OrdinaryFixtureDurability.durableReplace(data, url)
        // This simulates a lost successful write reply, not an injected failed directory fsync.
        if after?(data, url) == true { throw T3OutboxReplaceFailure(replaced: true, cause: CocoaError(.fileWriteUnknown)) }
    }
}
@main struct OrdinaryOutboxTransferTests {
    typealias O = OrdinaryObject
    static var checks = 0, failures: [String] = []
    static let imageID = "aaaaaaaa-0000-4000-8000-000000000001", fileID = "bbbbbbbb-0000-4000-8000-000000000002"
    static let target: O = ["origin": "https://ordinary.test", "environmentId": "env", "threadId": "thread", "draftKey": "env:thread"]
    static let identity = "[\"https://ordinary.test\",\"env\",\"env:thread\"]"
    static func check(_ value: Bool, _ name: String) { checks += 1; if !value { failures.append(name) } }
    static func run(_ name: String, _ body: () throws -> Void) { do { try body() } catch { failures.append("\(name): \(error)") } }
    static func refuses(_ name: String, _ body: () throws -> Void) { do { try body(); check(false, name) } catch { check(true, name) } }
    static func object(_ value: Any?) -> O { value as? O ?? [:] }
    static func encoded(_ value: O) throws -> Data { try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys, .withoutEscapingSlashes]) }
    static func fileJSON(_ url: URL) throws -> O { try JSONSerialization.jsonObject(with: Data(contentsOf: url)) as! O }
    static func rawCapture(_ text: String = "  payload  ") -> O {
        let image: O = ["id": imageID, "name": "image.png", "mimeType": "image/png", "sizeBytes": 4, "future": ["kept": true]]
        let file: O = ["id": fileID, "name": "document.txt", "mimeType": "text/plain", "sizeBytes": 4,
            "draftKey": "env:thread", "environmentId": "env", "contextId": "old-unused-id", "source": "attached", "attachmentId": "", "status": "staged", "future": [1, 2]]
        let records: [O] = [
            ["version": 1, "contextId": "canonical", "kind": "file", "label": "File", "attachmentId": fileID, "name": "document.txt", "mimeType": "text/plain", "sizeBytes": 4],
            ["version": 1, "contextId": "unreferenced", "kind": "skill", "label": "Skill", "name": "unreferenced"],
            ["version": 1, "contextId": "future", "kind": "future-kind", "label": "Future", "payload": ["kept": true]]]
        return ["version": 2, "kind": "ordinary", "draft": ["key": "env:thread", "origin": target["origin"]!, "environmentId": "env", "threadId": "thread",
            "document": ["incarnation": "doc-a", "revision": 7, "selection": ["start": 1, "end": 2]], "text": text,
            "context": ["version": 1, "records": records], "contextRevision": 9, "images": [image], "files": [file],
            "attachmentIds": [fileID, imageID], "attachmentOrder": ["stale-id", fileID]]]
    }
    static func record(_ capture: O, id: String = "message") -> O {
        let draft = object(capture["draft"]), file = (draft["files"] as! [O])[0], image = (draft["images"] as! [O])[0]
        return ["schemaVersion": 1, "origin": draft["origin"]!, "environmentId": draft["environmentId"]!, "threadId": draft["threadId"]!,
            "messageId": id, "commandId": "command-\(id)", "text": (draft["text"] as! String).trimmingCharacters(in: T3MobileOutbox.trimCharacters),
            "context": draft["context"]!, "attachments": [
                ["kind": "file", "id": file["id"]!, "name": file["name"]!, "mimeType": file["mimeType"]!, "sizeBytes": file["sizeBytes"]!, "source": "attached", "contextId": file["contextId"]!, "uploadId": "", "status": "staged"],
                ["kind": "image", "id": image["id"]!, "name": image["name"]!, "mimeType": image["mimeType"]!, "sizeBytes": image["sizeBytes"]!, "uploadId": "", "status": "staged"]],
            "createdAt": "2026-10-09T12:00:00.000Z", "modelSelection": ["instanceId": "provider", "model": "model"], "runtimeMode": "full-access", "interactionMode": "default"]
    }
    static func completeMarker(_ claim: O, preserve: Bool = false, newIncarnation: Bool = false) -> O {
        let draft = object(object(claim["capture"])["draft"])
        let text = preserve ? "newer" : ""
        let document: O = target.merging(["incarnation": newIncarnation ? "doc-b" : "doc-a", "revision": newIncarnation ? 0 : 8, "selection": ["start": 0, "end": 0]]) { _, b in b }
        let context: Any = preserve ? ["origin": target["origin"]!, "environmentId": "env", "key": "env:thread", "revision": 10, "text": text, "context": draft["context"]!] : NSNull()
        return ["version": 2, "kind": "ordinary", "draftKey": "env:thread", "fingerprint": claim["fingerprint"]!, "disposition": preserve ? "preserved" : "cleared",
            "before": ["incarnation": "doc-a", "revision": 7], "after": ["document": document, "text": text, "context": context,
                "images": preserve ? draft["images"]! : [O](), "files": preserve ? draft["files"]! : [O](),
                "attachmentIds": preserve ? draft["attachmentIds"]! : [String](), "attachmentOrder": preserve ? draft["attachmentIds"]! : NSNull()]]
    }
    static func preferences(_ marker: O, transfer: String = "message") -> O {
        let after = object(marker["after"])
        var orders: O = ["foreign": ["foreign-file"]]
        if !(after["attachmentOrder"] is NSNull) { orders["env:thread"] = after["attachmentOrder"] }
        var contexts: O = [:]
        if !(after["context"] is NSNull) { contexts[identity] = after["context"] }
        return ["mobileOutboxTransferCompletions": [transfer: marker], "mobileComposerEditor": ["version": 1, "documents": [identity: after["document"]!]],
            "drafts": ["env:thread": after["text"]!, "foreign": "keep"], "snapshotDrafts": ["env:thread": after["images"]!],
            "composerFiles": after["files"]!, "mobileAttachmentOrder": orders, "mobileComposerContexts": ["version": 1, "entries": contexts], "foreign": ["keep": true]]
    }
    final class Fixture {
        let root: URL, disk = OrdinaryDisk(), lock = NSLock()
        var owner: T3MobileOutboxOwner!, store: T3MobileOutbox!
        var releases = 0, v1Evidence = 0, epoch = ""
        init(_ root: URL) throws {
            self.root = root
            try FileManager.default.createDirectory(at: root.appendingPathComponent("snapshots/drafts"), withIntermediateDirectories: true)
            try FileManager.default.createDirectory(at: root.appendingPathComponent("composer-files"), withIntermediateDirectories: true)
            try Data("data".utf8).write(to: root.appendingPathComponent("snapshots/drafts/\(imageID)"))
            try Data("data".utf8).write(to: root.appendingPathComponent("composer-files/\(fileID)"))
            restart()
            epoch = try call("read")["ownerEpoch"] as! String
        }
        func restart() {
            store = T3MobileOutbox(root: root, replace: disk.replace)
            owner = T3MobileOutboxOwner(lock: lock, disk: store, release: { [unowned self] _ in self.releases += 1 }, transferEvidence: { [unowned self] _ in self.v1Evidence += 1 }, transferAdmission: { [unowned self] record in
                // The production coordinator's exact callback body is separately extracted and invoked here.
                try OrdinaryFixtureAdmission.validate(root: self.root, record: record)
            })
        }
        func call(_ action: String, _ fields: O = [:]) throws -> O {
            let answer = OrdinaryAnswer(); owner.submit(fields.merging(["action": action]) { _, b in b }) { answer.receive($0) }; return try answer.get()
        }
        func ordinary(_ action: String, _ fields: O = [:], target: O = OrdinaryOutboxTransferTests.target) throws -> O {
            try call(action, fields.merging(["kind": "ordinary", "target": target]) { _, b in b })
        }
        func enqueue(_ capture: O = rawCapture(), id: String = "message", sequence: Int = 1) throws -> O {
            try ordinary("enqueueTransfer", ["ownerEpoch": epoch, "mutationId": "\(epoch):\(sequence)", "capture": capture, "record": record(capture, id: id)], target: T3MobileOutbox.ordinaryScope(object(capture["draft"]), draft: true))
        }
        func savePreferences(_ marker: O) throws { try disk.replace(encoded(preferences(marker)), root.appendingPathComponent("t3-code.json")) }
        func finish(_ claim: O) throws -> O { try ordinary("completeTransfer", ["transferId": claim["transferId"]!, "fingerprint": claim["fingerprint"]!]) }
        func gates(_ expected: Bool, _ name: String) throws {
            let row = ((try call("read"))["records"] as! [O]).first { object($0["record"])["messageId"] as? String == "message" }!
            let request: O = ["ownerEpoch": epoch, "messageId": "message", "token": row["token"]!, "expectedToken": row["token"]!, "expectedRevision": row["revision"]!, "record": row["record"]!]
            check(try call("confirmQueued", request)["current"] as? Bool == expected, name + " confirm")
            lock.lock()
            let admitted = (try? owner.deliveryRecordLocked(request)) != nil
            let retry = owner.deliveryRetryUnheldLocked("message")
            lock.unlock()
            check(admitted == expected, name + " reservation"); check(retry == expected, name + " retry")
        }
    }
    static func main() {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("ordinary-transfer-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: root) }
        run("capture/parser") {
            let capture = rawCapture(), payload = record(capture)
            check(T3MobileOutbox.validateCapture(capture, record: payload), "full ordinary capture with future context")
            for field in ["modelSelection", "runtimeMode", "interactionMode"] { var bad = payload; bad.removeValue(forKey: field); check(!T3MobileOutbox.validateCapture(capture, record: bad), "missing captured \(field)") }
            var bad = payload; bad["text"] = "untrimmed"; check(!T3MobileOutbox.validateCapture(capture, record: bad), "trim-only record binding")
            bad = payload; bad["creation"] = ["projectId": "p"]; check(!T3MobileOutbox.validateCapture(capture, record: bad), "ordinary cannot become creation")
            for number in [true, -1, 1.5, Double.infinity, Double.nan, 9_007_199_254_740_992.0] as [Any] {
                var draft = object(capture["draft"]), doc = object(draft["document"]); doc["revision"] = number; draft["document"] = doc
                check(!T3MobileOutbox.validateCapture(["version": 2, "kind": "ordinary", "draft": draft]), "strict captured revision \(number)")
            }
            var draft = object(capture["draft"]); draft["attachmentIds"] = [imageID, fileID]
            check(!T3MobileOutbox.validateCapture(["version": 2, "kind": "ordinary", "draft": draft]), "raw order binds effective mixed order")
        }
        run("Files image semantic kind preserves file-byte capture") {
            let cases: [(String, String, Bool)] = [
                ("photo.png", "image/png", true), ("photo.JPG", "APPLICATION/OCTET-STREAM\u{FEFF}; charset=x", true),
                ("photo.bin", "IMAGE/WEBP\u{00A0}; charset=x", true), ("photo.png", "image/avif", false),
                ("photo.png", "image/svg+xml", false), ("photo.png", "application/pdf", false), ("photo.png", "text/plain", false)]
            for (name, mime, accepted) in cases {
                var capture = rawCapture(), draft = object(capture["draft"]), context = object(draft["context"])
                var files = draft["files"] as! [O], records = context["records"] as! [O]
                files[0]["name"] = name; files[0]["mimeType"] = mime
                records[0]["kind"] = "image"; records[0]["name"] = name; records[0]["mimeType"] = mime
                context["records"] = records; draft["context"] = context; draft["files"] = files; capture["draft"] = draft
                let payload = record(capture)
                check(T3MobileOutbox.validateOrdinaryCapture(capture) == accepted, "semantic capture \(mime)")
                check(T3MobileOutbox.validateOrdinaryCapture(capture, record: payload) == accepted, "semantic record \(mime)")
                check((payload["attachments"] as! [O])[0]["kind"] as? String == "file", "file storage retained \(mime)")
                if accepted {
                    for (key, value) in [("attachmentId", imageID), ("name", "other.png"), ("mimeType", "image/jpeg"), ("sizeBytes", 22)] as [(String, Any)] {
                        var badCapture = capture, badDraft = draft, badContext = context, badRecords = records
                        badRecords[0][key] = value; badContext["records"] = badRecords; badDraft["context"] = badContext; badCapture["draft"] = badDraft
                        check(!T3MobileOutbox.validateOrdinaryCapture(badCapture), "semantic raw binding \(mime) \(key)")
                    }
                    var spoof = payload, attachments = payload["attachments"] as! [O]
                    attachments[0]["kind"] = "image"; spoof["attachments"] = attachments
                    check(!T3MobileOutbox.validateOrdinaryCapture(capture, record: spoof), "no byte-slot relabel \(mime)")
                }
                records[0]["kind"] = "file"; context["records"] = records; draft["context"] = context; capture["draft"] = draft
                check(T3MobileOutbox.validateOrdinaryCapture(capture, record: record(capture)), "existing file context retained \(mime)")
            }
        }
        run("durable admission, lost reply, clear, restart") {
            let f = try Fixture(root.appendingPathComponent("clear")), capture = rawCapture()
            let admitted = try f.enqueue(capture), claim = object(admitted["claim"])
            check(admitted["disposition"] as? String == "created", "first admission created")
            check(object(claim["capture"])["kind"] as? String == "ordinary", "ordinary kind captured")
            f.lock.lock(); let held = f.owner.protects(fileID); f.lock.unlock(); check(held, "accepted queued canonical bytes owned")
            try f.gates(false, "queued before saved retirement")
            let replay = try f.enqueue(capture, id: "unused", sequence: 2)
            check(replay["disposition"] as? String == "existing" && object(replay["claim"])["messageId"] as? String == "message", "lost reply repeats immutable original IDs")
            f.restart(); f.epoch = try f.call("read")["ownerEpoch"] as! String
            try f.gates(false, "cold queued original still blocks delivery")
            check(try f.enqueue(capture, id: "unused-cold", sequence: 1)["disposition"] as? String == "existing", "cold retry preserves original identity")
            let marker = completeMarker(claim); try f.savePreferences(marker)
            let before = try Data(contentsOf: f.root.appendingPathComponent("t3-code.json")), writes = f.disk.writes["t3-code.json"]!
            let complete = try f.finish(claim)
            check(complete["completed"] as? Bool == true, "completed only after saved evidence")
            check(f.disk.writes["t3-code.json"] == writes + 1, "completion reestablishes preference durability")
            check(try Data(contentsOf: f.root.appendingPathComponent("t3-code.json")) == before, "evidence rewrite preserves exact foreign bytes")
            check(f.v1Evidence == 0 && f.releases == 1, "ordinary uses own evidence then existing release callback")
            try f.gates(true, "completed ordinary")
            f.restart(); f.epoch = try f.call("read")["ownerEpoch"] as! String
            try f.gates(true, "cold completed re-synced")
            let compact = object(try f.ordinary("transferStatus", ["transferId": "message"])["claim"])
            check(compact["kind"] as? String == "ordinary" && compact["origin"] as? String == target["origin"] as? String && compact["capture"] is NSNull, "terminal scope survives compaction")
            refuses("legacy status cannot read ordinary claim") { _ = try f.call("transferStatus", ["transferId": "message"]) }
        }
        run("strict saved evidence and newer incarnation preserve") {
            let f = try Fixture(root.appendingPathComponent("preserve")), claim = object(try f.enqueue()["claim"]), marker = completeMarker(claim, preserve: true, newIncarnation: true)
            let valid = preferences(marker)
            var cases: [O] = []
            var bad = valid; bad["mobileComposerEditor"] = ["version": 1, "documents": O()]; cases.append(bad)
            bad = valid; var documents = object(bad["mobileComposerEditor"]), entries = object(documents["documents"]), doc = object(entries[identity]); doc["blocked"] = true; entries[identity] = doc; documents["documents"] = entries; bad["mobileComposerEditor"] = documents; cases.append(bad)
            bad = valid; bad["drafts"] = ["env:thread": "later"]; cases.append(bad)
            bad = valid; bad["mobileAttachmentOrder"] = ["env:thread": ["stale-id", fileID]]; cases.append(bad)
            bad = valid; bad["snapshotDrafts"] = ["bad-sibling": [1]]; cases.append(bad)
            bad = valid; bad["composerFiles"] = [1]; cases.append(bad)
            bad = valid; bad["mobileComposerContexts"] = ["version": 1, "entries": O()]; cases.append(bad)
            let namedFile = (object(marker["after"])["files"] as! [O])[0]
            var sibling = namedFile; sibling["draftKey"] = "env:foreign"; sibling["id"] = "foreign-file"
            bad = valid; bad["composerFiles"] = [namedFile, sibling, sibling]; cases.append(bad)
            var sameContext = sibling; sameContext["id"] = "other-file"
            bad = valid; bad["composerFiles"] = [namedFile, sibling, sameContext]; cases.append(bad)
            bad = valid; bad["composerFiles"] = [namedFile, sibling]
            var snapshots = object(bad["snapshotDrafts"]); snapshots["env:foreign"] = [["id": "foreign-file", "name": "foreign.png", "mimeType": "image/png", "sizeBytes": 4]]
            bad["snapshotDrafts"] = snapshots; cases.append(bad)
            for (index, value) in cases.enumerated() {
                try f.disk.replace(encoded(value), f.root.appendingPathComponent("t3-code.json"))
                refuses("malformed/mismatching durable proof \(index)") { _ = try f.finish(claim) }
                try f.gates(false, "refused saved evidence \(index)")
            }
            try f.savePreferences(marker); _ = try f.finish(claim); try f.gates(true, "different current incarnation preserved")
            check(object(try fileJSON(f.root.appendingPathComponent("t3-code.json")))["foreign"] is O, "foreign preferences retained")
        }
        run("preference failed write and uncertain terminal replay") {
            let f = try Fixture(root.appendingPathComponent("fault")), claim = object(try f.enqueue()["claim"])
            try f.savePreferences(completeMarker(claim))
            f.disk.before = { _, url in url.lastPathComponent == "t3-code.json" }
            refuses("failed preference durability blocks completion") { _ = try f.finish(claim) }; try f.gates(false, "failed preference sync")
            f.disk.before = nil
            var used = false
            f.disk.after = { bytes, url in
                guard url.deletingLastPathComponent().lastPathComponent == "mobile-outbox", !used,
                      let envelope = try? JSONSerialization.jsonObject(with: bytes) as? O, object(envelope["transfer"])["state"] as? String == "completed" else { return false }
                used = true; return true
            }
            refuses("terminal successful rename with lost reply remains uncertain") { _ = try f.finish(claim) }
            f.lock.lock(); let retry = f.owner.deliveryRetryUnheldLocked("message"); f.lock.unlock(); check(!retry, "uncertain completion cannot retry wire")
            // Force a cold durability retry to fail despite visible completed JSON.
            f.restart(); f.disk.before = { _, url in url.deletingLastPathComponent().lastPathComponent == "mobile-outbox" }
            let failedRead = try f.call("read"); f.epoch = failedRead["ownerEpoch"] as! String
            check(failedRead["complete"] as? Bool == false, "visible completed file with failed re-sync is incomplete")
            check((failedRead["transfers"] as! [O]).isEmpty, "uncertain terminal claim is withheld from inventory")
            let lookup = try f.ordinary("transferLookup")
            check(lookup["complete"] as? Bool == false && (lookup["claims"] as! [O]).isEmpty, "lookup cannot publish uncertain terminal authority")
            refuses("status withholds uncertain compacted terminal") { _ = try f.ordinary("transferStatus", ["transferId": "message"]) }
            refuses("admission replay withholds uncertain compacted terminal") { _ = try f.enqueue(rawCapture(), id: "ignored", sequence: 1) }
            f.lock.lock(); let coldRetry = f.owner.deliveryRetryUnheldLocked("message"); f.lock.unlock(); check(!coldRetry, "cold visible completion is not authority before fsync")
            f.disk.before = nil; f.disk.after = nil; _ = try f.call("read"); try f.gates(true, "completed re-sync recovered")
            check(try f.finish(claim)["completed"] as? Bool == true, "terminal retry idempotent")
        }
        run("full scope, conflict, missing canonical byte") {
            let f = try Fixture(root.appendingPathComponent("scope")), capture = rawCapture()
            _ = try f.enqueue(capture)
            var changed = capture, draft = object(capture["draft"]), doc = object(draft["document"]); doc["revision"] = 8; draft["document"] = doc; changed["draft"] = draft
            check(try f.enqueue(changed, id: "different", sequence: 2)["disposition"] as? String == "conflict", "new revision conflicts with unretired original")
            var foreign = target; foreign["origin"] = "https://foreign.test"
            check((try f.ordinary("transferLookup", target: foreign)["claims"] as! [O]).isEmpty, "same key foreign home lookup isolated")
            refuses("foreign status refused") { _ = try f.ordinary("transferStatus", ["transferId": "message"], target: foreign) }
            refuses("foreign completion refused") { _ = try f.ordinary("completeTransfer", ["transferId": "message", "fingerprint": "a"], target: foreign) }
            draft = object(capture["draft"]); draft["origin"] = foreign["origin"]; changed["draft"] = draft
            check(try f.enqueue(changed, id: "foreign", sequence: 2)["disposition"] as? String == "created", "same key distinct home admitted separately")
            try FileManager.default.removeItem(at: f.root.appendingPathComponent("composer-files/\(fileID)"))
            draft["origin"] = "https://third.test"; changed["draft"] = draft
            refuses("missing canonical byte prevents new admission") { _ = try f.enqueue(changed, id: "third", sequence: 3) }
        }
        run("preserved recovery inventory is not new upload authority") {
            let f = try Fixture(root.appendingPathComponent("overflow")), claim = object(try f.enqueue()["claim"])
            var marker = completeMarker(claim, preserve: true), after = object(marker["after"])
            let images: [O] = (0..<101).map { ["id": "recovered-\($0)", "name": "image.png", "mimeType": "image/png", "sizeBytes": 4] }
            var file = (after["files"] as! [O])[0]; file["id"] = "private-text"; file["source"] = "pasted-text"
            let ids = images.map { $0["id"] as! String } + ["private-text"]
            after["images"] = images; after["files"] = [file]; after["attachmentIds"] = ids; after["attachmentOrder"] = ids; after["context"] = NSNull(); marker["after"] = after
            try f.savePreferences(marker); check(try f.finish(claim)["completed"] as? Bool == true, "complete preserved oversized/private current inventory without touching those bytes")
        }
        run("v1 independent draft behavior unchanged") {
            let f = try Fixture(root.appendingPathComponent("v1"))
            let draft: O = ["key": "new-task:one", "revision": 0, "createdAt": "2026-10-09T12:00:00.000Z", "environmentId": "env", "origin": target["origin"]!,
                "projectId": "project", "text": "v1", "images": [O](), "files": [O](), "attachmentIds": [String](), "choices": NSNull(), "workspace": NSNull()]
            let capture: O = ["version": 1, "draft": draft]
            var payload = record(rawCapture()); payload["text"] = "v1"; payload["attachments"] = [O](); payload.removeValue(forKey: "context")
            payload["creation"] = ["projectId": "project", "workspaceMode": "local", "branch": NSNull(), "worktreePath": NSNull()]
            let result = try f.call("enqueueTransfer", ["ownerEpoch": f.epoch, "mutationId": "\(f.epoch):1", "record": payload, "capture": capture])
            let claim = object(result["claim"]); check(claim["kind"] == nil, "v1 public kind unchanged")
            try f.gates(true, "v1 delivery remains available before transfer retirement")
            check((try f.call("transferLookup", ["draftKey": "new-task:one"])["claims"] as! [O]).count == 1, "v1 lookup unchanged")
            check(try f.call("completeTransfer", ["transferId": "message", "fingerprint": claim["fingerprint"]!])["completed"] as? Bool == true, "v1 callback completion unchanged")
            check(f.v1Evidence == 1, "v1 uses unchanged evidence callback")
        }
        print("Ordinary transfer: \(checks) checks, \(failures.count) failures")
        for failure in failures { print("FAIL: \(failure)") }
        if !failures.isEmpty { exit(1) }
    }
}
