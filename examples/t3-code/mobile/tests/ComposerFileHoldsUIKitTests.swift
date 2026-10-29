#if os(iOS)
// Actual editor registration, coordinator lock, canonical bytes and durable release queue.
// @ref examples/t3-code/mobile/docs/1130.005-composer-and-transcript.decision.md#composer-command-foundation
import UIKit
import CryptoKit

@MainActor final class ComposerFileHoldsUIKitTests {
    private let host: UIView
    private let voice = T3MobileVoice(agent: true, audioSession: T3MobileAudioSession(), changed: { _ in })
    private var checks: [[String: Any]] = []
    init(host: UIView) { self.host = host }
    private func check(_ pass: Bool, _ name: String) { checks.append(["name": name, "pass": pass]) }
    private func json(_ object: Any) throws -> Data { try JSONSerialization.data(withJSONObject: object, options: [.sortedKeys]) }
    private func attempt(_ name: String, _ block: () async throws -> Void) async {
        do { try await block() } catch { check(false, name + ": " + String(describing: error)) }
    }
    private final class Disk: @unchecked Sendable {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("file-holds-" + UUID().uuidString)
        var failBefore = false, failAfter = false, failPreferences = false, saves = 0
        var failPreferencesAfter = false, preferenceWrites = 0, failQueueAt: Int?
        var pickerAnnouncements = 0, pickerAnnouncementsOnMain = true
        var sourceAnnouncements = 0
        let entered = DispatchSemaphore(value: 0), resume = DispatchSemaphore(value: 0), leftPause = DispatchSemaphore(value: 0)
        var pause = false
        func replace(_ bytes: Data, _ url: URL) throws {
            if url.lastPathComponent == "t3-code.json" {
                preferenceWrites += 1
                if failPreferences { throw CocoaError(.fileWriteUnknown) }
                try T3MobileQueuedEdit.durableReplace(bytes, url)
                if failPreferencesAfter { throw CocoaError(.fileWriteUnknown) }
                return
            }
            if url.lastPathComponent == "mobile-queued-edit.json" {
                saves += 1
                if pause { pause = false; entered.signal(); _ = resume.wait(timeout: .now() + 5); leftPause.signal() }
                if failBefore || failQueueAt == saves { throw CocoaError(.fileWriteUnknown) }
                try T3MobileQueuedEdit.durableReplace(bytes, url)
                if failAfter { throw CocoaError(.fileWriteUnknown) }
            } else { try T3MobileQueuedEdit.durableReplace(bytes, url) }
        }
    }
    private struct Fixture {
        let disk: Disk
        let coordinator: T3MobileQueuedEdit
        let registry: T3MobileComposerFileHolds
        let file: [String: Any]
        let url: URL
        let port: T3MobileComposerEditor
        let request: [String: Any]
        let operations: T3MobileComposerOperations
    }
    private func port(_ registry: T3MobileComposerFileHolds, owner: String, operations: T3MobileComposerOperations? = nil) throws -> T3MobileComposerEditor {
        let port = T3MobileComposerEditor(voice: voice.editor, operations: operations, fileHolds: registry,
            events: ExactNativeEvents(fn: { _, _, _, _, _ in }, ctx: nil, nonce: 1))
        port.view.frame = CGRect(x: 20, y: 100, width: 400, height: 120); host.addSubview(port.view)
        let control: [String: Any] = ["owner": owner, "editorId": "file-composer", "routeVisit": UUID().uuidString,
            "renderEpoch": UUID().uuidString, "mountId": "", "acknowledgedEventCount": 0, "ackCommandId": "",
            "active": true, "editable": false, "readOnly": false,
            "focusIntent": ["serial": "", "attempt": 0, "operation": "none"], "command": NSNull(),
            "document": ["value": "", "selection": ["start": 0, "end": 0], "tokensJson": "[]", "isNativeEcho": false]]
        try port.setProps(["configuration": String(decoding: json(control), as: UTF8.self)])
        return port
    }
    private func fixture(newTask: Bool = false) throws -> Fixture {
        let disk = Disk(), coordinator = T3MobileQueuedEdit(root: disk.root, replace: disk.replace)
        let registry = T3MobileComposerFileHolds(coordinator: coordinator, pickerChanged: {
            disk.pickerAnnouncements += 1; disk.pickerAnnouncementsOnMain = disk.pickerAnnouncementsOnMain && Thread.isMainThread
        }), id = UUID().uuidString.lowercased()
        let key = newTask ? "new-task:picker" : "env:thread", created = "2026-10-29T02:00:00.000Z"
        let file: [String: Any] = ["id": id, "contextId": "file-context", "draftKey": key, "environmentId": "env",
            "name": "note.txt", "mimeType": "text/plain", "sizeBytes": 5, "source": "attached", "status": "staged", "attachmentId": ""]
        try FileManager.default.createDirectory(at: disk.root.appendingPathComponent("composer-files"), withIntermediateDirectories: true)
        let url = disk.root.appendingPathComponent("composer-files").appendingPathComponent(id)
        try Data("hello".utf8).write(to: url)
        var preferences: [String: Any] = ["version": 1, "composerFiles": [file], "snapshotDrafts": [:], "drafts": [key: ""], "pending": [:]]
        if newTask { preferences["mobileNewTaskDrafts"] = ["version": 1, "records": [key: ["key": key, "origin": "https://file.test", "environmentId": "env", "projectId": "project", "createdAt": created, "revision": 2, "choices": NSNull()]], "receipts": [:], "claims": [:], "fileReleases": []] }
        try json(preferences).write(to: disk.root.appendingPathComponent("t3-code.json"))
        let fields: [Any] = ["ordinary", "https://file.test", "env", 1, key] + (newTask ? [created] : [])
        let owner = String(decoding: try json(fields), as: UTF8.self)
        let operations = T3MobileComposerOperations(changed: { disk.sourceAnnouncements += 1 })
        let port = try self.port(registry, owner: owner, operations: operations)
        let target = newTask ? ["kind": "new-task", "origin": "https://file.test", "environmentId": "env", "projectId": "project", "draftKey": key, "createdAt": created]
            : ["origin": "https://file.test", "environmentId": "env", "threadId": "thread", "draftKey": key]
        let request: [String: Any] = ["op": "composerFileHold", "action": "acquire", "generation": 1,
            "identity": port.composerIdentity!.json, "requestId": UUID().uuidString.lowercased(),
            "target": target, "file": file]
        return Fixture(disk: disk, coordinator: coordinator, registry: registry, file: file, url: url, port: port, request: request, operations: operations)
    }
    private func perform(_ f: Fixture, _ request: [String: Any]? = nil) async -> [String: Any] {
        await withCheckedContinuation { continuation in f.registry.perform(request ?? f.request) { continuation.resume(returning: $0) } }
    }
    private func release(_ f: Fixture, _ request: [String: Any]? = nil) async -> [String: Any] {
        var request = request ?? f.request; request["action"] = "release"; return await perform(f, request)
    }
    private func remove(_ f: Fixture) throws -> [String: Any] { try f.coordinator.removeAttachment(["op": "composerAttachRemove", "id": f.file["id"]!]) }
    private func saved(_ f: Fixture, _ files: Any) throws {
        var preferences = try JSONSerialization.jsonObject(with: Data(contentsOf: f.disk.root.appendingPathComponent("t3-code.json"))) as! [String: Any]
        preferences["composerFiles"] = files
        try f.coordinator.writePreferences(String(decoding: json(preferences), as: UTF8.self))
    }
    private func receipt(_ reply: [String: Any]) -> [String: Any] { (reply["value"] as? [String: Any])?["receipt"] as? [String: Any] ?? [:] }
    private func cleanup(_ f: Fixture) { f.registry.destroy(); f.operations.destroy(); f.port.destroy(); f.port.view.removeFromSuperview() }
    private func idle(_ f: Fixture) async {
        await withCheckedContinuation { continuation in f.registry.whenIdle { continuation.resume() } }
    }
    private func refuses(_ name: String, _ operation: () throws -> Void) {
        do { try operation(); check(false, name) } catch { check(true, name) }
    }
    private func preferences(_ f: Fixture) throws -> [String: Any] {
        try JSONSerialization.jsonObject(with: Data(contentsOf: f.disk.root.appendingPathComponent("t3-code.json"))) as! [String: Any]
    }
    private func save(_ f: Fixture, _ preferences: [String: Any]) throws {
        try f.coordinator.writePreferences(String(decoding: json(preferences), as: UTF8.self))
    }
    private func ready(_ f: Fixture) throws {
        var control = f.port.composerIdentity!.json
        control.merge(["acknowledgedEventCount": f.port.composerEventCount, "ackCommandId": "", "active": true, "editable": true, "readOnly": false,
            "focusIntent": ["serial": "", "attempt": 0, "operation": "none"], "command": NSNull(),
            "document": ["value": "", "selection": ["start": 0, "end": 0], "tokensJson": "[]", "isNativeEcho": true]]) { _, next in next }
        try f.port.setProps(["configuration": String(decoding: json(control), as: UTF8.self)])
    }
    private func finals(_ f: Fixture) -> [[String: Any]] {
        let reply = f.operations.performFinalizations(["op": "composerEditorFinalizations", "generation": 1, "action": "read"])
        return (reply["value"] as? [String: Any])?["receipts"] as? [[String: Any]] ?? []
    }
    private func acknowledge(_ f: Fixture, _ receiptId: String) -> Bool {
        let reply = f.operations.performFinalizations(["op": "composerEditorFinalizations", "generation": 1, "action": "acknowledge", "receiptId": receiptId])
        return (reply["value"] as? [String: Any])?["acknowledged"] as? Bool == true
    }
    private func newTaskHolds() async throws {
        let f = try fixture(newTask: true); defer { cleanup(f) }
        let identity = f.port.composerIdentity!, original = try preferences(f), key = f.file["draftKey"] as! String
        var control = identity.json
        control.merge(["acknowledgedEventCount": f.port.composerEventCount, "ackCommandId": "", "active": true, "editable": true, "readOnly": false,
            "focusIntent": ["serial": "", "attempt": 0, "operation": "none"], "command": NSNull(),
            "document": ["value": "", "selection": ["start": 0, "end": 0], "tokensJson": "[]", "isNativeEcho": true]]) { _, next in next }
        try f.port.setProps(["configuration": String(decoding: json(control), as: UTF8.self)])
        _ = try f.port.handoff(T3ComposerHandoffRequest(["op": "composerEditorHandoff", "generation": 1, "identity": identity.json,
            "action": "freeze", "operationId": UUID().uuidString, "operationRevision": 1, "retireUndo": false]))
        check(f.port.composerIdentity == identity && f.port.composerFileHoldEligible && !f.port.composerMounted, "frozen NewTask retains its exact eligible native claim")
        check(await perform(f)["ok"] as? Bool == true, "registered frozen NewTask acquires exact saved file")
        for field in ["origin", "environmentId", "projectId", "createdAt", "key", "revision"] {
            var bad = original, store = bad["mobileNewTaskDrafts"] as! [String: Any], rows = store["records"] as! [String: [String: Any]], record = rows[key]!
            record[field] = field == "revision" ? true : "changed" as Any; rows[key] = record; store["records"] = rows; bad["mobileNewTaskDrafts"] = store
            try save(f, bad)
            var request = f.request; request["requestId"] = UUID().uuidString.lowercased()
            check(await perform(f, request)["ok"] as? Bool == false, "NewTask initial hold refuses changed saved \(field)")
        }
        try save(f, original)
        var wrongGeneration = f.request; wrongGeneration["generation"] = 2
        check(await perform(f, wrongGeneration)["ok"] as? Bool == false, "NewTask hold compares owner generation")
        try saved(f, [])
        check(await perform(f)["ok"] as? Bool == true, "NewTask acquired replay survives removal from live inventory")
        check(try remove(f)["retained"] as? Bool == true, "NewTask native Undo claim protects removed metadata")
        _ = await release(f); _ = try f.coordinator.releaseAttachments()
        check(!FileManager.default.fileExists(atPath: f.url.path), "NewTask release allows guarded cleanup")
    }
    private func pickerPublication(newTask: Bool) throws {
        let f = try fixture(newTask: newTask); defer { cleanup(f) }
        let identity = f.port.composerIdentity!, claim = T3ComposerFileClaim(identity: identity), target = f.request["target"] as! [String: String]
        let incarnation = target["createdAt"] ?? "thread-incarnation", key = target["draftKey"]!, operationId = UUID().uuidString.lowercased()
        let input: [String: Any] = ["op": "composerPickerIntake", "action": "pick", "operationId": operationId, "generation": 1,
            "identity": identity.json, "target": target, "document": ["incarnation": incarnation, "revision": 2], "source": "files", "remaining": 3, "fileLimit": 100]
        let statusInput: [String: Any] = ["op": "composerPickerIntake", "action": "status", "operationId": operationId,
                                         "generation": 1, "identity": identity.json]
        let before = f.disk.saves
        let absent = try f.coordinator.performPicker(T3PickerRequest(statusInput), claim: nil)
        check(absent["status"] as? String == "absent" && absent["operationId"] as? String == operationId
              && absent["identity"] as? NSDictionary == identity.json as NSDictionary
              && (absent["files"] as? [T3PickerObject])?.isEmpty == true && absent["error"] as? String == "",
              "\(newTask) exact unissued status reports absence without claiming cancellation")
        check(f.disk.saves == before && FileManager.default.fileExists(atPath: f.url.path), "\(newTask) absence observation writes no release queue and removes no existing file")
        var missingCancel = statusInput; missingCancel["action"] = "cancel"
        refuses("\(newTask) unknown cancellation still requires an issued operation") {
            _ = try f.coordinator.performPicker(T3PickerRequest(missingCancel), claim: nil)
        }
        if newTask {
            for field in ["generation", "incarnation", "createdAt", "kind", "pending", "projectId", "extra"] {
                var bad = input, badTarget = target
                switch field {
                case "generation": bad["generation"] = 2
                case "incarnation": bad["document"] = ["incarnation": "other", "revision": 2]
                case "pending": badTarget["draftKey"] = "new-task:pending-one"; bad["target"] = badTarget
                case "extra": badTarget["threadId"] = "thread"; bad["target"] = badTarget
                default: badTarget[field] = ""; bad["target"] = badTarget
                }
                refuses("NewTask picker refuses invalid \(field)") { _ = try T3PickerRequest(bad) }
            }
        }
        let request = try T3PickerRequest(input), (op, opened, _) = try f.coordinator.beginPicker(request, claim: claim)
        check(opened, "\(newTask) picker opens once")
        check(try f.coordinator.performPicker(T3PickerRequest(statusInput), claim: nil)["status"] as? String == "picking",
              "\(newTask) the same status becomes issued after registration")
        for field in ["operationId", "generation", "identity"] {
            var foreign = statusInput
            if field == "identity" { var other = identity.json; other["mountId"] = "other"; foreign[field] = other }
            else { foreign[field] = field == "generation" ? 2 : UUID().uuidString.lowercased() }
            check(try f.coordinator.performPicker(T3PickerRequest(foreign), claim: nil)["status"] as? String == "absent"
                  && op.status == "picking", "\(newTask) absence for another \(field) does not alter the issued operation")
        }
        check(try f.coordinator.beginPicker(request, claim: claim).1 == false, "\(newTask) lost pick reply does not reopen UI")
        var changed = input; changed["remaining"] = 2
        refuses("\(newTask) picker capture is immutable") { _ = try f.coordinator.beginPicker(T3PickerRequest(changed), claim: claim) }
        let file = try f.coordinator.stagePicker(op, bytes: Data("picked".utf8), metadata: ["kind": "file", "name": "picked.txt", "mimeType": "text/plain"])
        let image = try f.coordinator.stagePicker(op, bytes: Data([1, 2, 3]), metadata: ["kind": "image", "name": "image.png", "mimeType": "image/png"])
        let discard = try f.coordinator.stagePicker(op, bytes: Data("discard".utf8), metadata: ["kind": "file", "name": "discard.txt", "mimeType": "text/plain"])
        check(try f.coordinator.completePicker(op, error: "")["status"] as? String == "staged", "\(newTask) copied bytes reach recoverable staged status")
        var adopted = file; adopted.removeValue(forKey: "kind")
        adopted.merge(["contextId": "picked-file", "draftKey": key, "environmentId": "env", "source": "attached", "status": "staged", "attachmentId": ""]) { _, next in next }
        func follow(_ action: String, _ extra: [String: Any] = [:]) throws -> T3PickerRequest {
            try T3PickerRequest(["op": "composerPickerIntake", "action": action, "operationId": operationId, "generation": 1, "identity": identity.json].merging(extra) { _, next in next })
        }
        var hold = f.request; hold["file"] = adopted; hold["requestId"] = UUID().uuidString.lowercased()
        let held = try f.coordinator.performPicker(follow("hold", ["request": hold]), claim: claim)
        check(held["status"] as? String == "held", "\(newTask) issued file hold precedes saved metadata")
        var saved = try preferences(f)
        let text = "hello 🙂", images = [image], files = [f.file, adopted]
        let ids = [image["id"] as! String, f.file["id"] as! String, file["id"] as! String]
        saved["drafts"] = [key: text]; saved["snapshotDrafts"] = [key: images]; saved["composerFiles"] = files; saved["mobileAttachmentOrder"] = [key: ids]
        var after: [String: Any] = ["text": text, "images": images, "files": files, "attachmentIds": ids, "attachmentOrder": ids]
        if newTask {
            var store = saved["mobileNewTaskDrafts"] as! [String: Any], records = store["records"] as! [String: [String: Any]], record = records[key]!
            record["revision"] = 3
            record["context"] = ["version": 1, "records": [["version": 1, "kind": "mention", "contextId": "mention-one", "label": "path", "path": "README.md"]]]
            records[key] = record; store["records"] = records; saved["mobileNewTaskDrafts"] = store; after["record"] = record
        } else {
            let document: [String: Any] = ["origin": target["origin"]!, "environmentId": "env", "threadId": "thread", "draftKey": key, "incarnation": incarnation, "revision": 3, "selection": NSNull()]
            let scope = String(decoding: try json([target["origin"]!, "env", key]), as: UTF8.self)
            saved["mobileComposerEditor"] = ["version": 1, "documents": [scope: document]]; after["document"] = document; after["context"] = NSNull()
        }
        let accepted = [file["id"] as! String, image["id"] as! String], discarded = [discard["id"] as! String]
        func finish(_ after: [String: Any], partition: ([String], [String])? = nil) throws -> T3PickerRequest {
            try follow("finish", ["publication": ["after": after, "acceptedIds": partition?.0 ?? accepted, "discardedIds": partition?.1 ?? discarded]])
        }
        let modes = ["order", "inventory", "partition", "metadata"] + (newTask ? ["project", "birth", "revision", "context", "context-null", "record", "registry", "discarded"] : [])
        for mode in modes {
            var badSaved = saved, badAfter = after, badAccepted = accepted, badDiscarded = discarded
            switch mode {
            case "order": badSaved["mobileAttachmentOrder"] = [key: ids.reversed().map { $0 }]
            case "inventory": badSaved["composerFiles"] = [f.file]
            case "partition": badAccepted = []; badDiscarded = []
            case "metadata": var changedFile = adopted; changedFile["name"] = "changed"; badAfter["files"] = [f.file, changedFile]; badSaved["composerFiles"] = [f.file, changedFile]
            case "record": var record = after["record"] as! [String: Any]; record["choices"] = ["modelId": "other"]; badAfter["record"] = record
            case "registry": badSaved["mobileNewTaskDrafts"] = ["version": true, "records": [:]]
            case "discarded": badAccepted = [image["id"] as! String]; badDiscarded = [file["id"] as! String, discard["id"] as! String]
            default:
                var record = after["record"] as! [String: Any]
                switch mode {
                case "project": record["projectId"] = "other"
                case "birth": record["createdAt"] = "other"
                case "revision": record["revision"] = 1
                case "context-null": record["context"] = NSNull()
                default: record["context"] = ["version": 1, "records": [["contextId": "duplicate", "kind": "mention"], ["contextId": "duplicate", "kind": "mention"]]]
                }
                var store = saved["mobileNewTaskDrafts"] as! [String: Any]; store["records"] = [key: record]; badSaved["mobileNewTaskDrafts"] = store; badAfter["record"] = record
            }
            try save(f, badSaved)
            refuses("\(newTask) publication refuses \(mode)") { _ = try f.coordinator.performPicker(finish(badAfter, partition: (badAccepted, badDiscarded)), claim: claim) }
            check(op.status == "staged", "\(newTask) failed \(mode) proof retains staged custody")
        }
        try save(f, saved)
        if newTask { claim.detach(); _ = try f.coordinator.releaseAttachments() }
        else { f.coordinator.endFileHoldOwner(claim) }
        check(try f.coordinator.performPicker(follow("status"), claim: nil)["status"] as? String == "staged", "\(newTask) exact status survives native owner retirement")
        refuses("\(newTask) retired owner cannot acquire another issued hold") { _ = try f.coordinator.performPicker(follow("hold", ["request": hold]), claim: claim) }
        let completion = try finish(after)
        f.disk.failPreferences = true
        refuses("\(newTask) failed durable preference replace cannot finish") { _ = try f.coordinator.performPicker(completion, claim: nil) }
        check(op.status == "staged", "\(newTask) failed save keeps exact issued operation")
        f.disk.failPreferences = false
        var announcements = 0
        check(try f.coordinator.performPicker(completion, claim: nil, changed: { announcements += 1 })["status"] as? String == "finished", "\(newTask) retired operation adopts exact saved publication")
        check(announcements == 1, "\(newTask) finishing announces its actual issued-state transition")
        var newer = saved; newer["drafts"] = [key: "newer text"]; try save(f, newer)
        check(try f.coordinator.performPicker(completion, claim: nil, changed: { announcements += 1 })["status"] as? String == "finished", "\(newTask) lost finish reply replays after newer edits")
        _ = try f.coordinator.performPicker(follow("status"), claim: nil, changed: { announcements += 1 })
        check(announcements == 1, "\(newTask) terminal replay and status do not cause a refresh loop")
        refuses("\(newTask) finished partition cannot change") { _ = try f.coordinator.performPicker(finish(after, partition: ([], accepted + discarded)), claim: nil) }
        refuses("\(newTask) finished adoption cannot cancel") { _ = try f.coordinator.performPicker(follow("cancel"), claim: nil) }
        let fresh = T3MobileQueuedEdit(root: f.disk.root); _ = try fresh.releaseAttachments()
        check(FileManager.default.fileExists(atPath: f.disk.root.appendingPathComponent("composer-files").appendingPathComponent(file["id"] as! String).path), "\(newTask) cold cleanup retains adopted file")
        check(FileManager.default.fileExists(atPath: f.disk.root.appendingPathComponent("snapshots/drafts").appendingPathComponent(image["id"] as! String).path), "\(newTask) cold cleanup retains adopted image")
        check(!FileManager.default.fileExists(atPath: f.disk.root.appendingPathComponent("composer-files").appendingPathComponent(discard["id"] as! String).path), "\(newTask) cold cleanup removes discarded file")
        f.coordinator.endFileHoldOwner(claim)
    }
    private struct NewTaskPick {
        let f: Fixture
        let op: T3PickerOperation
        let claim: T3ComposerFileClaim
        let saved: T3PickerObject
        let after: T3PickerObject
        let accepted: [String]
        let discarded: [String]
        var key: String { op.destination.draftKey }
        func request(_ action: String, after: T3PickerObject? = nil) throws -> T3PickerRequest {
            var raw: T3PickerObject = ["op": "composerPickerIntake", "action": action, "operationId": op.request.operationId,
                "generation": 1, "identity": op.request.identity.json]
            if action == "finish" { raw["publication"] = ["after": after ?? self.after, "acceptedIds": accepted, "discardedIds": discarded] }
            return try T3PickerRequest(raw)
        }
        func url(_ row: T3PickerObject) -> URL {
            f.disk.root.appendingPathComponent(row["kind"] as? String == "image" ? "snapshots/drafts" : "composer-files").appendingPathComponent(row["id"] as! String)
        }
    }
    private func record(_ saved: T3PickerObject, key: String) -> T3PickerObject {
        ((saved["mobileNewTaskDrafts"] as! T3PickerObject)["records"] as! [String: T3PickerObject])[key]!
    }
    private func settingRecord(_ raw: T3PickerObject, in saved: T3PickerObject, key: String) -> T3PickerObject {
        var saved = saved, store = saved["mobileNewTaskDrafts"] as! T3PickerObject
        var records = store["records"] as! [String: T3PickerObject]
        records[key] = raw; store["records"] = records; saved["mobileNewTaskDrafts"] = store
        return saved
    }
    private func choices(_ name: String) -> T3PickerObject {
        ["providerId": name, "modelId": name, "modelOptions": [["id": "effort", "value": name], ["id": "fast", "value": true]],
         "runtimeMode": name, "interactionMode": name]
    }
    private func newTaskPick() throws -> NewTaskPick {
        let f = try fixture(newTask: true), identity = f.port.composerIdentity!, claim = T3ComposerFileClaim(identity: identity)
        let target = f.request["target"] as! [String: String], key = target["draftKey"]!
        let input = try T3PickerRequest(["op": "composerPickerIntake", "action": "pick", "operationId": UUID().uuidString.lowercased(),
            "generation": 1, "identity": identity.json, "target": target, "document": ["incarnation": target["createdAt"]!, "revision": 2],
            "source": "files", "remaining": 3, "fileLimit": 100_000])
        let (op, _, _) = try f.coordinator.beginPicker(input, claim: claim)
        let file = try f.coordinator.stagePicker(op, bytes: Data("picked".utf8), metadata: ["kind": "file", "name": "picked.txt", "mimeType": "text/plain"])
        let png = UIGraphicsImageRenderer(size: CGSize(width: 2, height: 2)).pngData { context in
            UIColor.red.setFill(); context.fill(CGRect(x: 0, y: 0, width: 2, height: 2))
        }
        check(UIImage(data: png) != nil, "NewTask finish fixture stages decodable PNG bytes")
        let image = try f.coordinator.stagePicker(op, bytes: png, metadata: ["kind": "image", "name": "image.png", "mimeType": "image/png"])
        let discard = try f.coordinator.stagePicker(op, bytes: Data("discard".utf8), metadata: ["kind": "file", "name": "discard.txt", "mimeType": "text/plain"])
        _ = try f.coordinator.completePicker(op, error: "")
        var adopted = file; adopted.removeValue(forKey: "kind")
        adopted.merge(["contextId": "picked-file", "draftKey": key, "environmentId": "env", "source": "attached", "status": "staged", "attachmentId": ""]) { _, next in next }
        var saved = try preferences(f), record = record(saved, key: key)
        record["revision"] = 3; record["choices"] = choices("original")
        record["branchChoice"] = ["kind": "current", "branch": "main"]
        record["context"] = ["version": 1, "records": [["kind": "mention", "contextId": "mention", "label": "path", "path": "README.md"]]]
        record["future"] = ["counter": 7, "choices": ["runtimeMode": "protected"], "null": NSNull()]
        saved = settingRecord(record, in: saved, key: key)
        let text = "hello 🙂", images = [image], files = [f.file, adopted]
        let ids = [image["id"] as! String, f.file["id"] as! String, file["id"] as! String]
        saved["drafts"] = [key: text]; saved["snapshotDrafts"] = [key: images]; saved["composerFiles"] = files; saved["mobileAttachmentOrder"] = [key: ids]
        let after: T3PickerObject = ["record": record, "text": text, "images": images, "files": files, "attachmentIds": ids, "attachmentOrder": ids]
        try save(f, saved)
        return NewTaskPick(f: f, op: op, claim: claim, saved: try preferences(f), after: after,
            accepted: [file["id"] as! String, image["id"] as! String], discarded: [discard["id"] as! String])
    }
    private func delayedNewTaskFinish(retryFirst: Bool) async throws {
        let p = try newTaskPick(), f = p.f; defer { f.coordinator.endFileHoldOwner(p.claim); cleanup(f) }
        let original = try p.request("finish"), encoded = try T3PickerRequest.encoded(original.object)
        let entered = DispatchSemaphore(value: 0), arrive = DispatchSemaphore(value: 0)
        let delayed = Task.detached {
            entered.signal()
            guard arrive.wait(timeout: .now() + 10) == .success else { return Result<T3PickerObject, Error>.failure(CocoaError(.userCancelled)) }
            return Result { try f.coordinator.performPicker(original, claim: nil) }
        }
        let waiting = await Task.detached { entered.wait(timeout: .now() + 5) == .success }.value
        check(waiting, "Original finish is dispatched but held before native arrival")
        var current = record(p.saved, key: p.key); current["choices"] = choices("latest")
        let latest = settingRecord(current, in: p.saved, key: p.key); try save(f, latest)
        check(try f.coordinator.performPicker(p.request("status"), claim: nil)["status"] as? String == "staged",
              "Status between dispatch and arrival leaves original finish unresolved")
        let firstRetry: Result<T3PickerObject, Error>? = retryFirst ? Result { try f.coordinator.performPicker(original, claim: nil) } : nil
        arrive.signal()
        let arrived = await delayed.value
        let result = try arrived.get()
        check(result["status"] as? String == "finished", "Delayed original first finish accepts latest choices, retryFirst=\(retryFirst)")
        if let firstRetry { check(try firstRetry.get()["status"] as? String == "finished", "Retry may arrive before original with the same immutable proof") }
        else { check(try f.coordinator.performPicker(original, claim: nil)["status"] as? String == "finished", "Retry after original returns the same terminal") }
        check(try json(preferences(f)) == json(latest), "First finish preserves complete latest saved preferences")
        check(try p.op.terminal == Data(SHA256.hash(data: T3PickerRequest.encoded(original.object["publication"]!)))
              && (try T3PickerRequest.encoded(original.object)) == encoded, "Terminal binds the unchanged original full publication")
    }
    private func assertRefused(_ p: NewTaskPick, after: T3PickerObject, name: String) throws {
        let before = try Data(contentsOf: p.f.disk.root.appendingPathComponent("t3-code.json")), writes = p.f.disk.preferenceWrites
        refuses(name) { _ = try p.f.coordinator.performPicker(p.request("finish", after: after), claim: nil) }
        check(p.op.status == "staged" && p.op.terminal == nil && p.f.disk.preferenceWrites == writes,
              name + " retains staged custody without adoption write")
        check(try Data(contentsOf: p.f.disk.root.appendingPathComponent("t3-code.json")) == before,
              name + " preserves actual saved bytes")
        check(p.op.files.allSatisfy { FileManager.default.fileExists(atPath: p.url($0).path) }, name + " retains all issued bytes")
    }
    private func choicePair(_ original: Any?, _ current: Any?, name: String, accepts: Bool) throws {
        let p = try newTaskPick(), f = p.f; defer { f.coordinator.endFileHoldOwner(p.claim); cleanup(f) }
        var offered = p.after["record"] as! T3PickerObject, now = offered, after = p.after
        offered["choices"] = original; now["choices"] = current; after["record"] = offered
        let latest = settingRecord(now, in: p.saved, key: p.key); try save(f, latest)
        if accepts {
            let request = try p.request("finish", after: after)
            check(try f.coordinator.performPicker(request, claim: nil)["status"] as? String == "finished", name)
            check(try json(preferences(f)) == json(latest), name + " preserves latest choices and protected record")
            check(p.op.terminal == Data(SHA256.hash(data: try T3PickerRequest.encoded(request.object["publication"]!))), name + " keeps original signature")
        } else { try assertRefused(p, after: after, name: name) }
    }
    private func newTaskChoiceBindings() throws {
        func model(_ rows: [Any]) -> T3PickerObject { ["providerId": "", "modelId": "future-model", "modelOptions": rows] }
        let clean: [Any] = [NSNull(), T3PickerObject(), choices(""), choices("future-value")]
        for (i, original) in clean.enumerated() {
            for (j, current) in clean.enumerated() { try choicePair(original, current, name: "Clean choice transition \(i) to \(j)", accepts: true) }
        }
        try choicePair(model([["id": "one", "value": "a"], ["id": "two", "value": false]]),
                       model([["id": "two", "value": true], ["id": "added", "value": ""], ["id": "one", "value": "b"]]),
                       name: "Known-only options add and reorder", accepts: true)
        try choicePair(model([["id": "one", "value": true]]), model([]), name: "Known-only options remove", accepts: true)
        let opaque: [T3PickerObject] = [[:], ["id": "missing-value"], ["value": "missing-id"], ["id": 1, "value": true],
            ["id": "numeric", "value": 1], ["id": "zero", "value": 0], ["id": "null", "value": NSNull()],
            ["id": "object", "value": ["runtimeMode": "bound", "choices": ["x": true]]], ["id": "array", "value": ["a", "b"]]]
        for (index, row) in opaque.enumerated() {
            var changed = model([row]); changed["providerId"] = "new"; changed["interactionMode"] = "future"
            try choicePair(model([row]), changed, name: "Opaque object row \(index) remains exact through choices drift", accepts: true)
            var modified = row; modified["future"] = NSNull()
            try choicePair(model([row]), model([modified]), name: "Opaque row \(index) mutation", accepts: false)
        }
        for number in [0, 1] {
            try choicePair(model([["id": "flag", "value": number]]), model([["id": "flag", "value": number == 1]]),
                           name: "Numeric \(number) does not become Boolean", accepts: false)
            try choicePair(model([["id": "flag", "value": number == 1]]), model([["id": "flag", "value": number]]),
                           name: "Boolean does not become numeric \(number)", accepts: false)
        }
        let extended: T3PickerObject = ["id": "bound-id", "value": "old", "metadata": ["providerId": "bound", "null": NSNull(), "array": [true, 2] as [Any]]]
        try choicePair(NSNull(), model([extended]), name: "Null cannot acquire protected option metadata", accepts: false)
        try choicePair(model([extended]), NSNull(), name: "Null cannot erase protected option metadata", accepts: false)
        try choicePair(model([T3PickerObject()]), model([]), name: "Empty opaque row cannot disappear", accepts: false)
        try choicePair(model([T3PickerObject()]), model([["id": "plain", "value": ""], T3PickerObject()]), name: "Opaque row keeps original index", accepts: false)
        var next = extended; next["value"] = true
        try choicePair(model([extended]), model([next]), name: "Metadata-bearing option permits only selected value drift", accepts: true)
        var missingValue = extended; missingValue.removeValue(forKey: "value")
        try choicePair(model([extended]), model([missingValue]), name: "Recognized option cannot become an opaque row with the same residual", accepts: false)
        try choicePair(model([missingValue]), model([extended]), name: "Opaque row cannot become a recognized option with the same residual", accepts: false)
        let nullMetadata: T3PickerObject = ["id": "bound", "value": true, "extra": NSNull()]
        try choicePair(model([nullMetadata]), model([["id": "bound", "value": false]]), name: "Null option metadata differs from absent", accepts: false)
        try choicePair(model([nullMetadata]), model([["id": "bound", "value": false, "extra": T3PickerObject()]]), name: "Null option metadata differs from empty object", accepts: false)
        try choicePair(model([["id": "x", "value": true, "extra": [true, 1] as [Any]]]),
                       model([["id": "x", "value": false, "extra": [1, true] as [Any]]]), name: "Protected option array order and Boolean type remain exact", accepts: false)
        for field in ["id", "metadata", "removed", "index", "delete"] {
            var next = extended; next["value"] = false
            switch field {
            case "id": next["id"] = "other"
            case "metadata": next["metadata"] = ["providerId": "other"]
            case "removed": next.removeValue(forKey: "metadata")
            default: break
            }
            let rows: [Any] = field == "delete" ? [] : field == "index" ? [["id": "plain", "value": ""], next] : [next]
            try choicePair(model([extended]), model(rows), name: "Protected option \(field) binds", accepts: false)
        }
        let unknown: T3PickerObject = ["future": ["runtimeMode": "bound", "modelOptions": [["id": "x", "value": true]], "array": [1, 2]]]
        var withModes = unknown; withModes["runtimeMode"] = ""; withModes["interactionMode"] = "future"
        try choicePair(unknown, withModes, name: "Unknown choice metadata remains exact beside validated mode drift", accepts: true)
        for value in [NSNull(), T3PickerObject(), ["future": NSNull()], ["future": ["runtimeMode": "other"]],
                      ["future": ["runtimeMode": "bound", "modelOptions": [["id": "x", "value": 1]], "array": [1, 2]]]] as [Any] {
            try choicePair(unknown, value, name: "Unknown choices cannot be removed, masked, nulled or coerced", accepts: false)
            try choicePair(value, unknown, name: "Unknown choices cannot be added through clean transitions", accepts: false)
        }
        let keyedA: T3PickerObject = ["z": NSNull(), "a": ["n": true, "s": "same"]]
        let keyedB: T3PickerObject = ["a": ["s": "same", "n": true], "z": NSNull()]
        try choicePair(["future": keyedA, "runtimeMode": "before"], ["runtimeMode": "after", "future": keyedB],
                       name: "Dictionary key order does not bind", accepts: true)
    }
    private func invalidNewTaskChoices() throws {
        var invalid: [(String, Any?)] = [("absent", nil), ("array", []), ("string", ""), ("number", 0), ("bool", true)]
        let group = ["providerId", "modelId", "modelOptions"]
        for mask in 1..<7 {
            var raw: T3PickerObject = [:]
            for (index, field) in group.enumerated() where mask & (1 << index) != 0 { raw[field] = field == "modelOptions" ? [] : "" as Any }
            invalid.append(("partial group \(mask)", raw))
        }
        for field in group + ["runtimeMode", "interactionMode"] {
            for (index, value) in [NSNull(), false, 1, [], [:], ""].enumerated() {
                if field == "modelOptions" && index == 3 { continue }
                if field != "modelOptions" && index == 5 { continue }
                var raw = choices("valid"); raw[field] = value
                invalid.append(("\(field) type \(index)", raw))
            }
        }
        for (index, row) in [NSNull(), "row", 1, true, []].enumerated() {
            var raw = choices("valid"); raw["modelOptions"] = [row]; invalid.append(("non-object option \(index)", raw))
        }
        let p = try newTaskPick(), f = p.f; defer { f.coordinator.endFileHoldOwner(p.claim); cleanup(f) }
        for (name, raw) in invalid {
            for side in ["current", "offered", "both"] {
                var offered = p.after["record"] as! T3PickerObject, now = offered, after = p.after
                if side != "current" { offered["choices"] = raw }
                if side != "offered" { now["choices"] = raw }
                after["record"] = offered; try save(f, settingRecord(now, in: p.saved, key: p.key))
                try assertRefused(p, after: after, name: "Invalid \(name) on \(side), including equal-invalid pairs")
            }
        }
    }
    private func protectedNewTaskBindings() throws {
        let p = try newTaskPick(), f = p.f; defer { f.coordinator.endFileHoldOwner(p.claim); cleanup(f) }
        let fields = ["key", "origin", "environmentId", "projectId", "createdAt", "revision", "branchChoice", "context",
                      "context-absent", "context-null", "future", "counter", "boolean", "array-order", "nested-choices"]
        for field in fields {
            for side in ["current", "offered"] {
                var offered = p.after["record"] as! T3PickerObject, now = offered, changed = offered, after = p.after
                switch field {
                case "revision": changed[field] = 4
                case "context-absent": changed.removeValue(forKey: "context")
                case "context-null": changed["context"] = NSNull()
                case "context": changed[field] = ["version": 1, "records": [["kind": "mention", "contextId": "other"]]]
                case "branchChoice": changed[field] = ["kind": "current", "branch": "other"]
                case "future": changed.removeValue(forKey: field)
                case "counter": changed[field] = 1
                case "boolean": offered["flag"] = true; now["flag"] = true; changed["flag"] = 1
                case "array-order": offered["order"] = [1, 2]; now["order"] = [1, 2]; changed["order"] = [2, 1]
                case "nested-choices": changed["future"] = ["counter": 7, "choices": ["runtimeMode": "changed"], "null": NSNull()]
                default: changed[field] = "changed"
                }
                if side == "current" { now = changed } else { offered = changed }
                now["choices"] = choices("latest"); after["record"] = offered
                try save(f, settingRecord(now, in: p.saved, key: p.key))
                try assertRefused(p, after: after, name: "Protected \(field) on \(side)")
            }
        }
        for field in ["text", "image", "file", "order", "ids", "file-metadata", "image-metadata", "extra-after", "invalid-sibling"] {
            var current = p.saved, after = p.after, now = record(current, key: p.key)
            now["choices"] = choices("latest"); current = settingRecord(now, in: current, key: p.key)
            switch field {
            case "text": current["drafts"] = [p.key: "changed"]
            case "image": current["snapshotDrafts"] = [p.key: [T3PickerObject]()]
            case "file": current["composerFiles"] = [f.file]
            case "order": current["mobileAttachmentOrder"] = [p.key: (p.after["attachmentIds"] as! [String]).reversed().map { $0 }]
            case "ids": after["attachmentIds"] = [String]()
            case "file-metadata":
                var files = p.after["files"] as! [T3PickerObject]; files[1]["name"] = "changed"; after["files"] = files; current["composerFiles"] = files
            case "image-metadata":
                var images = p.after["images"] as! [T3PickerObject]; images[0]["sizeBytes"] = 1; after["images"] = images; current["snapshotDrafts"] = [p.key: images]
            case "extra-after": after["counter"] = 1
            default: current["composerFiles"] = (p.after["files"] as! [T3PickerObject]) + [["id": "malformed"]]
            }
            try save(f, current); try assertRefused(p, after: after, name: "Full current publication binds \(field)")
        }
        var latest = record(p.saved, key: p.key); latest["choices"] = choices("latest")
        try save(f, settingRecord(latest, in: p.saved, key: p.key))
        for field in ["generation", "operationId", "owner", "editorId", "routeVisit", "renderEpoch", "mountId", "eventCount", "operationRevision"] {
            var raw = try p.request("finish").object
            if ["owner", "editorId", "routeVisit", "renderEpoch", "mountId"].contains(field) {
                var identity = raw["identity"] as! T3PickerObject; identity[field] = "other"; raw["identity"] = identity
            } else { raw[field] = field == "operationId" ? UUID().uuidString.lowercased() : 2 as Any }
            refuses("Finish cannot change request \(field)") { _ = try f.coordinator.performPicker(T3PickerRequest(raw), claim: nil) }
            check(p.op.status == "staged" && p.op.terminal == nil, "Foreign \(field) leaves original operation staged")
        }
        for partition in [([String](), [String]()), (p.accepted, p.discarded + p.accepted), ([p.accepted[0]], p.discarded + [p.accepted[1]])] {
            var raw = try p.request("finish").object, publication = raw["publication"] as! T3PickerObject
            publication["acceptedIds"] = partition.0; publication["discardedIds"] = partition.1; raw["publication"] = publication
            refuses("Partition remains complete, disjoint and consistent with saved inventory") { _ = try f.coordinator.performPicker(T3PickerRequest(raw), claim: nil) }
            check(p.op.status == "staged" && p.op.terminal == nil, "Invalid partition retains custody")
        }
        func reordered(_ raw: Any) -> Any {
            if let object = raw as? T3PickerObject { return object.keys.sorted(by: >).reduce(into: T3PickerObject()) { $0[$1] = reordered(object[$1]!) } }
            if let array = raw as? [Any] { return array.map(reordered) }
            return raw
        }
        let after = reordered(p.after) as! T3PickerObject
        check(try f.coordinator.performPicker(p.request("finish", after: after), claim: nil)["status"] as? String == "finished",
              "Complete record and publication dictionary key reorder accepts after protected refusals")
    }
    private func newTaskFinishFailures() throws {
        for mode in ["preferences-before", "preferences-after", "queue-before", "queue-after", "response"] {
            let p = try newTaskPick(), f = p.f; defer { f.coordinator.endFileHoldOwner(p.claim); cleanup(f) }
            let original = try p.request("finish"), signature = Data(SHA256.hash(data: try T3PickerRequest.encoded(original.object["publication"]!)))
            var now = record(p.saved, key: p.key); now["choices"] = choices("latest")
            let latest = settingRecord(now, in: p.saved, key: p.key); try save(f, latest)
            switch mode {
            case "preferences-before": f.disk.failPreferences = true
            case "preferences-after": f.disk.failPreferencesAfter = true
            case "queue-before": f.disk.failBefore = true
            case "queue-after": f.disk.failAfter = true
            default: f.disk.failQueueAt = f.disk.saves + 2
            }
            refuses("Finish reports \(mode) durability failure") { _ = try f.coordinator.performPicker(original, claim: nil) }
            let terminal = mode == "response"
            check(p.op.status == (terminal ? "finished" : "staged") && p.op.terminal == (terminal ? signature : nil),
                  "\(mode) respects terminal assignment boundary")
            check(try json(preferences(f)) == json(latest), "\(mode) never restores offered old settings")
            check(p.op.files.allSatisfy { FileManager.default.fileExists(atPath: p.url($0).path) }, "\(mode) leaves issued bytes before cleanup")
            f.disk.failPreferences = false; f.disk.failPreferencesAfter = false; f.disk.failBefore = false; f.disk.failAfter = false; f.disk.failQueueAt = nil
            now["choices"] = choices("newer")
            var newer = settingRecord(now, in: p.saved, key: p.key)
            if terminal { newer["drafts"] = [p.key: "changed after historical success"] }
            try save(f, newer)
            let writes = f.disk.preferenceWrites
            check(try f.coordinator.performPicker(original, claim: nil)["status"] as? String == "finished", "Original request repairs \(mode)")
            check(try json(preferences(f)) == json(newer), "Retry after \(mode) preserves newest complete preferences")
            check(p.op.terminal == signature && f.disk.preferenceWrites == writes + (terminal ? 0 : 1),
                  "Retry after \(mode) adopts once or replays original terminal")
        }
    }
    private func newTaskLostReply() throws {
        let p = try newTaskPick(), f = p.f; defer { f.coordinator.endFileHoldOwner(p.claim); cleanup(f) }
        let original = try p.request("finish"), signature = Data(SHA256.hash(data: try T3PickerRequest.encoded(original.object["publication"]!)))
        var now = record(p.saved, key: p.key); now["choices"] = choices("latest")
        try save(f, settingRecord(now, in: p.saved, key: p.key))
        _ = try f.coordinator.performPicker(original, claim: nil)
        now["choices"] = choices("after-lost-reply"); now["revision"] = 4
        var newer = settingRecord(now, in: p.saved, key: p.key); newer["drafts"] = [p.key: "newer text"]
        try save(f, newer)
        let before = try Data(contentsOf: f.disk.root.appendingPathComponent("t3-code.json")), writes = f.disk.preferenceWrites
        check(try f.coordinator.performPicker(p.request("status"), claim: nil)["status"] as? String == "finished", "Status observes historical terminal after lost reply")
        f.disk.failBefore = true
        refuses("Terminal response can still fail cleanup persistence") { _ = try f.coordinator.performPicker(original, claim: nil) }
        check(p.op.terminal == signature && p.op.status == "finished", "Cleanup failure cannot revoke historical terminal")
        f.disk.failBefore = false
        check(try f.coordinator.performPicker(original, claim: nil)["status"] as? String == "finished", "Lost successful reply replays after more choice and text changes")
        var changed = p.after, changedRecord = changed["record"] as! T3PickerObject
        changedRecord["choices"] = choices("after-lost-reply"); changed["record"] = changedRecord
        refuses("Even choices-only proof replacement cannot replay original success") { _ = try f.coordinator.performPicker(p.request("finish", after: changed), claim: nil) }
        check(try Data(contentsOf: f.disk.root.appendingPathComponent("t3-code.json")) == before && f.disk.preferenceWrites == writes,
              "Terminal replay and changed-proof refusal never rewrite preferences")
    }
    private func newTaskByteBinding() throws {
        for member in 0..<3 {
            for mutation in ["bytes", "inode", "leaf-link", "parent-link"] {
                let p = try newTaskPick(), f = p.f; defer { f.coordinator.endFileHoldOwner(p.claim); cleanup(f) }
                var current = record(p.saved, key: p.key); current["choices"] = choices("latest")
                try save(f, settingRecord(current, in: p.saved, key: p.key))
                let row = p.op.files[member], url = p.url(row), original = try Data(contentsOf: url)
                let backup = f.disk.root.appendingPathComponent("backup-" + UUID().uuidString)
                switch mutation {
                case "bytes":
                    var changed = original; changed[changed.count - 1] ^= 1
                    let handle = try FileHandle(forWritingTo: url); try handle.write(contentsOf: changed); try handle.synchronize(); try handle.close()
                case "inode":
                    try FileManager.default.moveItem(at: url, to: backup)
                    try original.write(to: url)
                case "leaf-link":
                    try FileManager.default.moveItem(at: url, to: backup)
                    try FileManager.default.createSymbolicLink(at: url, withDestinationURL: backup)
                default:
                    let parent = url.deletingLastPathComponent()
                    try FileManager.default.moveItem(at: parent, to: backup)
                    try FileManager.default.createSymbolicLink(at: parent, withDestinationURL: backup)
                }
                check((try Data(contentsOf: url)).count == original.count, "Byte probe preserves size for member \(member), \(mutation)")
                try assertRefused(p, after: p.after, name: "Issued member \(member), \(mutation), including discarded bytes")
            }
        }
        let p = try newTaskPick(), f = p.f; defer { f.coordinator.endFileHoldOwner(p.claim); cleanup(f) }
        var current = record(p.saved, key: p.key); current["choices"] = choices("latest")
        try save(f, settingRecord(current, in: p.saved, key: p.key))
        let snapshots = f.disk.root.appendingPathComponent("snapshots"), backup = f.disk.root.appendingPathComponent("snapshots-backup")
        try FileManager.default.moveItem(at: snapshots, to: backup)
        try FileManager.default.createSymbolicLink(at: snapshots, withDestinationURL: backup)
        try assertRefused(p, after: p.after, name: "Image grandparent symlink cannot inherit issued byte custody")
    }
    private func newTaskRetirement() throws {
        for detach in [false, true] {
            let p = try newTaskPick(), f = p.f; defer { f.coordinator.endFileHoldOwner(p.claim); cleanup(f) }
            let rejected = p.op.files[2]
            var file = rejected; file.removeValue(forKey: "kind")
            file.merge(["contextId": "discarded", "draftKey": p.key, "environmentId": "env", "source": "attached", "status": "staged", "attachmentId": ""]) { _, next in next }
            var hold = f.request; hold["file"] = file; hold["requestId"] = UUID().uuidString.lowercased()
            var raw = try p.request("status").object; raw["action"] = "hold"; raw["request"] = hold
            let request = try T3PickerRequest(raw)
            _ = try f.coordinator.performPicker(request, claim: p.claim)
            var current = record(p.saved, key: p.key); current["choices"] = choices("latest")
            try save(f, settingRecord(current, in: p.saved, key: p.key))
            if detach { p.claim.detach() } else { f.coordinator.endFileHoldOwner(p.claim) }
            refuses("Retired NewTask owner cannot acquire, detach=\(detach)") { _ = try f.coordinator.performPicker(request, claim: p.claim) }
            check(try f.coordinator.performPicker(p.request("status"), claim: nil)["status"] as? String == "staged", "Retired operation remains issued before original finish")
            _ = try f.coordinator.performPicker(p.request("finish"), claim: nil)
            if detach {
                check(FileManager.default.fileExists(atPath: p.url(rejected).path), "Detached retained history still protects discarded issued bytes")
                f.disk.failBefore = true; f.coordinator.endFileHoldOwner(p.claim)
                check(FileManager.default.fileExists(atPath: p.url(rejected).path), "Failed candidate retirement keeps discarded history bytes")
                f.disk.failBefore = false
            }
            _ = try f.coordinator.releaseAttachments()
            check(!FileManager.default.fileExists(atPath: p.url(rejected).path), "Ended owner cleanup removes discarded bytes after original finish")
            check(p.op.files.prefix(2).allSatisfy { FileManager.default.fileExists(atPath: p.url($0).path) }, "Cleanup retains adopted canonical file and PNG")
            let before = try Data(contentsOf: f.disk.root.appendingPathComponent("t3-code.json"))
            let fresh = T3MobileQueuedEdit(root: f.disk.root); _ = try fresh.releaseAttachments()
            check(try fresh.performPicker(p.request("status"), claim: nil)["status"] as? String == "absent", "Fresh coordinator has no persisted terminal")
            refuses("Restart cannot infer successful finish from absence") { _ = try fresh.performPicker(p.request("finish"), claim: nil) }
            check(try Data(contentsOf: f.disk.root.appendingPathComponent("t3-code.json")) == before
                  && p.op.files.prefix(2).allSatisfy { FileManager.default.fileExists(atPath: p.url($0).path) }, "Restart keeps adopted bytes and complete latest preferences")
        }
    }
    private func unobservedFinalUndo() async throws {
        let f = try fixture(newTask: true); defer { cleanup(f) }
        let identity = f.port.composerIdentity!, restored = "[note.txt](t3-context://v1/file/file-context)"
        var control = identity.json
        control.merge(["acknowledgedEventCount": f.port.composerEventCount, "ackCommandId": "", "active": true, "editable": true, "readOnly": false,
            "focusIntent": ["serial": "", "attempt": 0, "operation": "none"], "command": NSNull(),
            "document": ["value": restored, "selection": ["start": restored.utf16.count, "end": restored.utf16.count], "tokensJson": "[]", "isNativeEcho": false]]) { _, next in next }
        try f.port.setProps(["configuration": String(decoding: json(control), as: UTF8.self)])
        check(await perform(f)["ok"] as? Bool == true, "Final Undo acquires its actual saved file hold")
        func editor(_ root: UIView) -> T3MobileOwnedComposerView? {
            if let found = root as? T3MobileOwnedComposerView { return found }
            return root.subviews.lazy.compactMap(editor).first
        }
        guard let view = editor(f.port.view) else { throw CocoaError(.coderValueNotFound) }
        view.textView.becomeFirstResponder()
        view.applyReplacement(value: "", selection: .init(start: 0, end: 0), tokensJson: "[]")
        while let undo = view.textView.undoManager, undo.groupingLevel > 0 { undo.endUndoGrouping() }
        try saved(f, [])
        view.textView.undoManager?.undo()
        check(f.port.composerSnapshot?.value == restored, "Actual UIKit Undo restores the unobserved file reference")
        f.port.destroy(); await idle(f)
        let reply = f.operations.performFinalizations(["op": "composerEditorFinalizations", "generation": 1, "action": "read"])
        let retained = (reply["value"] as? [String: Any])?["receipts"] as? [[String: Any]] ?? []
        check(retained.count == 1 && (retained.first?["event"] as? [String: Any])?["value"] as? String == restored,
              "Retired source receipt retains the actual final Undo")
        _ = try f.coordinator.releaseAttachments()
        check(FileManager.default.fileExists(atPath: f.url.path), "Final source receipt protects Undo bytes before its durable ACK")
        guard let id = retained.first?["receiptId"] as? String else { throw CocoaError(.coderValueNotFound) }
        check(!acknowledge(f, UUID().uuidString), "A foreign final receipt cannot release Undo bytes")
        check(await perform(f)["ok"] as? Bool == false, "Detached source custody cannot acquire through its former native port")
        var restoredPreferences = try preferences(f)
        restoredPreferences["composerFiles"] = [f.file]; restoredPreferences["drafts"] = [f.file["draftKey"] as! String: restored]
        f.disk.failPreferences = true
        refuses("Failed final-source persistence cannot publish restored inventory") { try save(f, restoredPreferences) }
        f.disk.failPreferences = false
        _ = try f.coordinator.releaseAttachments()
        check((try preferences(f)["composerFiles"] as? [[String: Any]])?.isEmpty == true && FileManager.default.fileExists(atPath: f.url.path)
              && finals(f).first?["receiptId"] as? String == id, "Failed save leaves the exact final source and its canonical bytes retryable")
        try save(f, restoredPreferences)
        check(acknowledge(f, id), "The exact durable final source is acknowledged")
        await idle(f); _ = try f.coordinator.releaseAttachments()
        check(finals(f).isEmpty && (try? Data(contentsOf: f.url)) == Data("hello".utf8), "Restored saved inventory keeps canonical bytes after final acknowledgment")
        check(f.disk.pickerAnnouncements == 1 && f.disk.pickerAnnouncementsOnMain, "Final-source release announces settled coordinator cleanup on main")
        check(!acknowledge(f, id), "Duplicate final acknowledgment has no custody to release")
        await idle(f)
        check(f.disk.pickerAnnouncements == 1 && f.disk.sourceAnnouncements == 2, "Duplicate final acknowledgment announces neither another release nor another source change")
        try saved(f, []); _ = try f.coordinator.releaseAttachments()
        check(!FileManager.default.fileExists(atPath: f.url.path), "Final acknowledgment ends the claim after restored inventory later releases the file")
    }

    private func frozenSourceCustody(savedBeforeTeardown: Bool) async throws {
        let f = try fixture(newTask: true); defer { cleanup(f) }
        try ready(f)
        check(await perform(f)["ok"] as? Bool == true, "Frozen custody acquires its saved file")
        try saved(f, [])
        _ = try f.port.handoff(T3ComposerHandoffRequest(["op": "composerEditorHandoff", "generation": 1, "identity": f.port.composerIdentity!.json,
            "action": "freeze", "operationId": UUID().uuidString, "operationRevision": 1, "retireUndo": false]))
        guard let id = finals(f).first?["receiptId"] as? String else { throw CocoaError(.coderValueNotFound) }
        if savedBeforeTeardown {
            check(acknowledge(f, id), "Mounted frozen source acknowledges its exact receipt")
            check(await perform(f)["ok"] as? Bool == true, "Mounted frozen acknowledgment preserves live hold admission")
            _ = try f.coordinator.releaseAttachments()
            check(FileManager.default.fileExists(atPath: f.url.path), "Mounted frozen acknowledgment preserves Undo bytes")
        }
        f.port.destroy(); await idle(f)
        if savedBeforeTeardown {
            check(finals(f).isEmpty && !FileManager.default.fileExists(atPath: f.url.path), "Already-saved frozen teardown retires normally without new source custody")
            check(!acknowledge(f, id), "Old saved frozen acknowledgment has no pending source to release")
        } else {
            _ = try f.coordinator.releaseAttachments()
            check(finals(f).first?["receiptId"] as? String == id && finals(f).first?["retired"] as? Bool == true
                  && FileManager.default.fileExists(atPath: f.url.path), "Unsaved frozen teardown keeps the same receipt and canonical bytes")
            check(acknowledge(f, id), "Retired frozen source accepts its exact acknowledgment")
            await idle(f)
            check(!FileManager.default.fileExists(atPath: f.url.path), "Unreferenced frozen bytes clean up after final acknowledgment")
        }
    }

    private func detachedWorkers() async throws {
        let f = try fixture(newTask: true); defer { cleanup(f) }
        try ready(f)
        check(await perform(f)["ok"] as? Bool == true, "Detached worker baseline acquires saved bytes")
        try saved(f, [])
        f.disk.pause = true
        async let active = perform(f)
        let entered = await Task.detached { f.disk.entered.wait(timeout: .now() + 3) == .success }.value
        check(entered, "Acquire replay reaches its held durable save before detachment")
        let request = f.request
        let pick: [String: Any] = ["op": "composerPickerIntake", "action": "pick", "operationId": UUID().uuidString.lowercased(),
            "generation": 1, "identity": f.port.composerIdentity!.json, "target": f.request["target"]!,
            "document": ["incarnation": (f.request["target"] as! [String: String])["createdAt"]!, "revision": 2], "source": "files", "remaining": 1, "fileLimit": 100]
        let attachments = T3MobileAttachments(dataRoot: f.disk.root, agent: true)
        let pending: [[String: Any]] = await withCheckedContinuation { continuation in
            var answers: [[String: Any]] = []
            func receive(_ reply: [String: Any]) { answers.append(reply); if answers.count == 2 { continuation.resume(returning: answers) } }
            f.registry.perform(request, reply: receive)
            f.registry.performPicker(pick, attachments: attachments, reply: receive)
            f.port.destroy()
            check(f.disk.leftPause.wait(timeout: .now()) == .timedOut, "Detachment closes admission without waiting for the disk worker")
            f.disk.resume.signal()
        }
        check(await active["ok"] as? Bool == false, "An in-flight acquire cannot publish success after detachment")
        check(pending.count == 2 && pending.allSatisfy { $0["ok"] as? Bool == false }, "Queued acquire and pick refuse when their claim detaches before execution")
        let status = try f.coordinator.performPicker(T3PickerRequest(["op": "composerPickerIntake", "action": "status",
            "operationId": pick["operationId"]!, "generation": 1, "identity": pick["identity"]!]), claim: nil)
        check(status["status"] as? String == "absent", "Queued pick refusal creates no issued operation")
        await idle(f); _ = try f.coordinator.releaseAttachments()
        check(FileManager.default.fileExists(atPath: f.url.path), "Worker refusal cannot discard the detached final source's bytes")
        guard let id = finals(f).first?["receiptId"] as? String else { throw CocoaError(.coderValueNotFound) }
        check(acknowledge(f, id), "Detached worker source can be acknowledged after its failed admissions")
        await idle(f)
        check(!FileManager.default.fileExists(atPath: f.url.path), "Acknowledgment cleans unreferenced bytes after queued workers finish")
    }

    private func detachedCleanup(shutdown: Bool) async throws {
        let f = try fixture(newTask: true); defer { cleanup(f) }
        try ready(f); f.disk.failBefore = true
        check(await perform(f)["ok"] as? Bool == false, "Uncertain acquire retains its claim without a durable cleanup candidate")
        try saved(f, []); f.port.destroy(); await idle(f)
        guard let id = finals(f).first?["receiptId"] as? String else { throw CocoaError(.coderValueNotFound) }
        check(try remove(f)["retained"] as? Bool == true, "Detached uncertain acquire protects final-source bytes")
        if shutdown { f.registry.destroy(); f.operations.destroy() }
        else { check(acknowledge(f, id), "Final source acknowledgment releases uncertain byte custody") }
        await idle(f)
        check(try remove(f)["retained"] as? Bool == true, "Failed retirement candidate save retains ended byte ownership")
        f.disk.failBefore = false; _ = try f.coordinator.releaseAttachments()
        check(!FileManager.default.fileExists(atPath: f.url.path), "Explicit drain repairs retirement durability and removes unreferenced bytes")
        if shutdown {
            let reply = f.operations.performFinalizations(["op": "composerEditorFinalizations", "generation": 1, "action": "read"])
            check(reply["ok"] as? Bool == false && f.disk.pickerAnnouncements == 0, "Module closure refuses source reads and suppresses cleanup announcements")
        } else { check(!acknowledge(f, id), "A completed source acknowledgment remains idempotent after cleanup retry") }
    }

    private func resumedReceiptIsolation() async throws {
        let f = try fixture(newTask: true); defer { cleanup(f) }
        try ready(f); check(await perform(f)["ok"] as? Bool == true, "Resumed source acquires its saved file")
        try saved(f, [])
        let operationId = UUID().uuidString, identity = f.port.composerIdentity!
        _ = try f.port.handoff(T3ComposerHandoffRequest(["op": "composerEditorHandoff", "generation": 1, "identity": identity.json,
            "action": "freeze", "operationId": operationId, "operationRevision": 1, "retireUndo": false]))
        guard let old = finals(f).first?["receiptId"] as? String else { throw CocoaError(.coderValueNotFound) }
        check(acknowledge(f, old), "The first frozen source is saved before resume")
        _ = try f.port.handoff(T3ComposerHandoffRequest(["op": "composerEditorHandoff", "generation": 1, "identity": identity.json,
            "action": "resume", "operationId": operationId, "operationRevision": 1, "receiptId": old,
            "document": ["value": "after resume", "selection": ["start": 12, "end": 12], "tokensJson": "[]", "isNativeEcho": false]]))
        f.port.destroy(); await idle(f)
        guard let current = finals(f).first?["receiptId"] as? String else { throw CocoaError(.coderValueNotFound) }
        check(current != old && (finals(f).first?["event"] as? [String: Any])?["value"] as? String == "after resume", "Resumed teardown retains a new exact source receipt")
        check(!acknowledge(f, old), "An old frozen acknowledgment cannot end the resumed source's custody")
        _ = try f.coordinator.releaseAttachments()
        check(FileManager.default.fileExists(atPath: f.url.path), "Current receipt protects bytes after an older acknowledgment")
        check(acknowledge(f, current), "Only the current retired receipt ends resumed byte custody")
        await idle(f)
        check(!FileManager.default.fileExists(atPath: f.url.path), "Current acknowledgment cleans unreferenced resumed bytes")
    }

    func run() async -> [String: Any] {
        await attempt("unobserved final Undo") { try await unobservedFinalUndo() }
        for saved in [false, true] { await attempt("frozen source custody \(saved)") { try await frozenSourceCustody(savedBeforeTeardown: saved) } }
        await attempt("detached workers") { try await detachedWorkers() }
        for shutdown in [false, true] { await attempt("detached cleanup \(shutdown)") { try await detachedCleanup(shutdown: shutdown) } }
        await attempt("resumed receipt isolation") { try await resumedReceiptIsolation() }
        await attempt("independent seed retirement") {
            let f = try fixture(newTask: true); defer { cleanup(f) }
            check(await perform(f)["ok"] as? Bool == true, "Unacknowledged seed acquires its saved file")
            try saved(f, []); f.port.destroy(); await idle(f)
            check(finals(f).isEmpty && !FileManager.default.fileExists(atPath: f.url.path), "Seed teardown without a final receipt retires byte custody normally")
        }
        await attempt("picker retirement announcement") {
            let f = try fixture(); defer { cleanup(f) }
            f.port.destroy()
            check(f.disk.pickerAnnouncements == 0, "Retirement cannot announce before its queued coordinator cleanup")
            await idle(f)
            check(f.disk.pickerAnnouncements == 1 && f.disk.pickerAnnouncementsOnMain, "Settled retirement announces once on main without an original reply")
            f.port.destroy(); await idle(f)
            check(f.disk.pickerAnnouncements == 1, "Duplicate retirement does not announce again")
        }
        await attempt("closed picker silence") {
            let f = try fixture(); defer { cleanup(f) }
            f.registry.destroy(); await idle(f)
            check(f.disk.pickerAnnouncements == 0, "A closed controller suppresses its queued retirement announcement")
        }
        await attempt("picker abandonment completion") {
            let f = try fixture(); defer { cleanup(f) }
            let identity = f.port.composerIdentity!, operationId = UUID().uuidString.lowercased()
            let request = try T3PickerRequest(["op": "composerPickerIntake", "action": "pick", "operationId": operationId,
                "generation": 1, "identity": identity.json, "target": f.request["target"]!,
                "document": ["incarnation": "thread-incarnation", "revision": 0], "source": "files", "remaining": 1, "fileLimit": 100])
            let (operation, _, _) = try f.coordinator.beginPicker(request, claim: T3ComposerFileClaim(identity: identity))
            await withCheckedContinuation { continuation in f.coordinator.abandonPicker(operation) { continuation.resume() } }
            let status = try T3PickerRequest(["op": "composerPickerIntake", "action": "status", "operationId": operationId,
                "generation": 1, "identity": identity.json])
            check(try f.coordinator.performPicker(status, claim: nil)["status"] as? String == "cancelled", "Abandonment completion runs after cancellation has settled")
        }
        await attempt("basic") {
            let f = try fixture(); defer { cleanup(f) }
            let first = await perform(f), again = await perform(f)
            check(first["ok"] as? Bool == true, "real registered pre-edit native owner acquires saved file")
            check(NSDictionary(dictionary: receipt(first)).isEqual(to: receipt(again)), "lost reply identical retry returns exact receipt")
            check(f.disk.saves >= 2, "identical retry reconfirms queue durability")
            try saved(f, []); check(try remove(f)["retained"] as? Bool == true, "hold protects direct removal after metadata removal")
            _ = try f.coordinator.releaseAttachments(); check(FileManager.default.fileExists(atPath: f.url.path), "hold protects guarded drain")
            let retryAfterRemoval = await perform(f); check(retryAfterRemoval["ok"] as? Bool == true, "exact replay survives live inventory removal")
            var changed = f.request, file = f.file; file["name"] = "different"; changed["file"] = file
            check(await perform(f, changed)["ok"] as? Bool == false, "changed reuse refuses")
            check(await release(f)["ok"] as? Bool == true, "release succeeds")
            check(FileManager.default.fileExists(atPath: f.url.path), "release does not eagerly unlink")
            check(await release(f)["ok"] as? Bool == true, "duplicate release idempotent")
            check(await perform(f)["ok"] as? Bool == false, "released request never resurrects")
            _ = try f.coordinator.releaseAttachments(); check(!FileManager.default.fileExists(atPath: f.url.path), "later guarded drain cleans unowned file")
        }
        await attempt("two owners") {
            let f = try fixture(); defer { cleanup(f) }
            let second = try port(f.registry, owner: f.port.composerIdentity!.owner); defer { second.destroy(); second.view.removeFromSuperview() }
            var request = f.request; request["identity"] = second.composerIdentity!.json; request["requestId"] = UUID().uuidString.lowercased()
            let one = await perform(f), two = await perform(f, request)
            check(one["ok"] as? Bool == true && two["ok"] as? Bool == true, "two real native owners acquire independent holds")
            try saved(f, []); _ = await release(f); check(try remove(f)["retained"] as? Bool == true, "other native owner still protects")
            var stale = request; stale["action"] = "release"; stale["holdId"] = receipt(one)["holdId"]
            check(await perform(f, stale)["ok"] as? Bool == false, "foreign receipt cannot release second owner")
            _ = await release(f, request); check(FileManager.default.fileExists(atPath: f.url.path), "last explicit release also does not unlink")
        }
        for mode in ["foreign", "missing", "bad-list", "bad-sibling", "pasted", "image", "size", "symlink", "directory"] {
            await attempt("admission " + mode) {
                let f = try fixture(); defer { cleanup(f) }; var request = f.request
                switch mode {
                case "foreign": var target = request["target"] as! [String: String]; target["draftKey"] = "env:other"; request["target"] = target
                case "missing": try saved(f, [])
                case "bad-list": try saved(f, "invalid")
                case "bad-sibling": try saved(f, [f.file, ["id": "invalid"]])
                case "pasted": var file = f.file; file["source"] = "pasted-text"; request["file"] = file; try saved(f, [file])
                case "image": request["file"] = ["id": f.file["id"]!, "name": "image.png", "mimeType": "image/png", "sizeBytes": 5]
                case "size": try Data("shorter".utf8).write(to: f.url)
                case "symlink": try FileManager.default.removeItem(at: f.url); try FileManager.default.createSymbolicLink(at: f.url, withDestinationURL: f.disk.root.appendingPathComponent("t3-code.json"))
                default: try FileManager.default.removeItem(at: f.url); try FileManager.default.createDirectory(at: f.url, withIntermediateDirectories: true)
                }
                check(await perform(f, request)["ok"] as? Bool == false, mode + " initial admission refuses")
            }
        }
        for (name, raw) in [("boolean", true as Any), ("fraction", 1.5 as Any), ("negative", -1 as Any), ("unsafe", 9_007_199_254_740_992 as Any), ("nan", Double.nan as Any), ("infinity", Double.infinity as Any)] {
            await attempt("numeric " + name) {
                let f = try fixture(); defer { cleanup(f) }; var request = f.request; request["generation"] = raw
                check(await perform(f, request)["ok"] as? Bool == false, name + " generation refuses")
                var file = f.file; file["sizeBytes"] = raw; request = f.request; request["file"] = file
                if JSONSerialization.isValidJSONObject(file) { try saved(f, [file]) }
                check(await perform(f, request)["ok"] as? Bool == false, name + " saved/request size refuses")
            }
        }
        for raw in [true as Any, Double.nan as Any, Double.infinity as Any] {
            await attempt("video dimension") {
                let f = try fixture(); defer { cleanup(f) }; var request = f.request, file = f.file
                file["videoWidth"] = raw; file["videoHeight"] = 20; request["file"] = file
                if JSONSerialization.isValidJSONObject(file) { try saved(f, [file]) }
                check(await perform(f, request)["ok"] as? Bool == false, "boolean or nonfinite video dimension refuses")
            }
        }
        await attempt("inactive admission") {
            let f = try fixture(); defer { cleanup(f) }
            var control = f.port.composerIdentity!.json
            control.merge(["acknowledgedEventCount": f.port.composerEventCount, "ackCommandId": "", "active": false, "editable": false, "readOnly": false,
                "focusIntent": ["serial": "", "attempt": 0, "operation": "none"], "command": NSNull(),
                "document": ["value": "", "selection": ["start": 0, "end": 0], "tokensJson": "[]", "isNativeEcho": true]]) { _, new in new }
            try f.port.setProps(["configuration": String(decoding: json(control), as: UTF8.self)])
            check(await perform(f)["ok"] as? Bool == false, "inactive native owner cannot acquire")
            control["active"] = true; control["readOnly"] = true
            try f.port.setProps(["configuration": String(decoding: json(control), as: UTF8.self)])
            check(await perform(f)["ok"] as? Bool == false, "read-only native owner cannot acquire")
        }
        await attempt("replacement") {
            let f = try fixture(); defer { cleanup(f) }; check(await perform(f)["ok"] as? Bool == true, "replacement baseline acquired")
            try FileManager.default.removeItem(at: f.url); try Data("hello".utf8).write(to: f.url)
            check(await perform(f)["ok"] as? Bool == false, "same name and bytes with replaced inode cannot inherit receipt")
        }
        for after in [false, true] {
            await attempt("uncertain durability") {
                let f = try fixture(); defer { cleanup(f) }; f.disk.failBefore = !after; f.disk.failAfter = after
                check(await perform(f)["ok"] as? Bool == false, "queue failure \(after ? "after durable replace returned" : "before rename") refuses success")
                try saved(f, []); check(try remove(f)["retained"] as? Bool == true, "uncertain hold still protects canonical bytes")
                check(await release(f)["ok"] as? Bool == false, "release cannot drop unresolved cleanup candidate")
                f.disk.failBefore = false; f.disk.failAfter = false; let savesBeforeReplay = f.disk.saves
                check(await perform(f)["ok"] as? Bool == true, "identical uncertain acquire repairs queue durability without live inventory")
                check(f.disk.saves > savesBeforeReplay, "uncertain replay performs a fresh durable save")
                _ = await release(f); _ = try f.coordinator.releaseAttachments()
                check(!FileManager.default.fileExists(atPath: f.url.path), "repaired candidate drains after release")
            }
        }
        await attempt("owner end during worker") {
            let f = try fixture(); defer { cleanup(f) }; f.disk.pause = true
            async let result = perform(f)
            let entered = await Task.detached { f.disk.entered.wait(timeout: .now() + 3) == .success }.value
            check(entered, "worker reached held queue save")
            let start = Date(); f.port.destroy(); check(Date().timeIntervalSince(start) < 0.1, "native owner end does not wait behind disk worker")
            f.disk.resume.signal(); check(await result["ok"] as? Bool == false, "already-dispatched work refuses success after owner end")
            check(await perform(f)["ok"] as? Bool == false, "ended native registration cannot be forged back by same strings")
            await idle(f); try saved(f, []); _ = try f.coordinator.releaseAttachments()
            check(!FileManager.default.fileExists(atPath: f.url.path), "ended owner cleanup uses guarded candidate drain")
        }
        await attempt("owner ends before delayed worker starts") {
            let f = try fixture(); defer { cleanup(f) }; f.disk.pause = true
            let second = try port(f.registry, owner: f.port.composerIdentity!.owner); defer { second.destroy(); second.view.removeFromSuperview() }
            var request = f.request; request["identity"] = second.composerIdentity!.json; request["requestId"] = UUID().uuidString.lowercased()
            async let first = perform(f)
            let entered = await Task.detached { f.disk.entered.wait(timeout: .now() + 3) == .success }.value
            check(entered, "first worker blocks serial queue")
            let pending: [String: Any] = await withCheckedContinuation { continuation in
                f.registry.perform(request) { continuation.resume(returning: $0) }
                second.destroy(); f.disk.resume.signal()
            }
            _ = await first
            check(pending["ok"] as? Bool == false, "claimed work queued before retirement refuses when it starts later")
        }
        await attempt("ended uncertainty") {
            let f = try fixture(); defer { cleanup(f) }; f.disk.failBefore = true
            _ = await perform(f); try saved(f, []); f.port.destroy(); await idle(f)
            check(try remove(f)["retained"] as? Bool == true, "ended unresolved candidate retains bytes")
            f.disk.failBefore = false; _ = try f.coordinator.releaseAttachments()
            check(!FileManager.default.fileExists(atPath: f.url.path), "explicit drain repairs ended candidate then cleans bytes")
        }
        await attempt("restart") {
            let f = try fixture(); defer { cleanup(f) }; _ = await perform(f); try saved(f, [])
            // A fresh coordinator models a fresh process; no in-memory Undo owner survives.
            let fresh = T3MobileQueuedEdit(root: f.disk.root); _ = try fresh.releaseAttachments()
            check(!FileManager.default.fileExists(atPath: f.url.path), "persisted release candidate enables cleanup after process-local history loss")
        }
        await attempt("new task native holds") { try await newTaskHolds() }
        for newTask in [false, true] { await attempt("picker publication \(newTask)") { try pickerPublication(newTask: newTask) } }
        for retryFirst in [false, true] { await attempt("delayed first finish \(retryFirst)") { try await delayedNewTaskFinish(retryFirst: retryFirst) } }
        await attempt("choice bindings") { try newTaskChoiceBindings() }
        await attempt("invalid choices") { try invalidNewTaskChoices() }
        await attempt("protected bindings") { try protectedNewTaskBindings() }
        await attempt("finish write failures") { try newTaskFinishFailures() }
        await attempt("lost finish reply") { try newTaskLostReply() }
        await attempt("canonical byte binding") { try newTaskByteBinding() }
        await attempt("finish retirement and restart") { try newTaskRetirement() }
        voice.destroy()
        return ["passed": checks.filter { $0["pass"] as? Bool == true }.count, "failed": checks.filter { $0["pass"] as? Bool == false }.count, "checks": checks]
    }
}
#endif
