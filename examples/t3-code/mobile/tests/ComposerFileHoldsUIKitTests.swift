#if os(iOS)
// Actual editor registration, coordinator lock, canonical bytes and durable release queue.
// @ref examples/t3-code/mobile/docs/1130.005-composer-and-transcript.decision.md#composer-command-foundation
import UIKit

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
        var pickerAnnouncements = 0, pickerAnnouncementsOnMain = true
        let entered = DispatchSemaphore(value: 0), resume = DispatchSemaphore(value: 0)
        var pause = false
        func replace(_ bytes: Data, _ url: URL) throws {
            if failPreferences && url.lastPathComponent == "t3-code.json" { throw CocoaError(.fileWriteUnknown) }
            if url.lastPathComponent == "mobile-queued-edit.json" {
                saves += 1
                if pause { pause = false; entered.signal(); _ = resume.wait(timeout: .now() + 5) }
                if failBefore { throw CocoaError(.fileWriteUnknown) }
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
        let owner = String(decoding: try json(fields), as: UTF8.self), operations = T3MobileComposerOperations()
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
    private func cleanup(_ f: Fixture) { f.registry.destroy(); f.port.destroy(); f.port.view.removeFromSuperview() }
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
        f.coordinator.endFileHoldOwner(claim)
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
    }
    func run() async -> [String: Any] {
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
        voice.destroy()
        return ["passed": checks.filter { $0["pass"] as? Bool == true }.count, "failed": checks.filter { $0["pass"] as? Bool == false }.count, "checks": checks]
    }
}
#endif
