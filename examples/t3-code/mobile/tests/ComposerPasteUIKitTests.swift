#if os(iOS)
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
// Isolated actual UIKit/file owner fixture. Uses production bodies and private fixture roots only.
import UIKit

final class ComposerPasteUIKitTests {
    typealias Object = [String: Any]
    private var checks: [Object] = []
    private var roots: [URL] = [], temporaries: [URL] = []
    private let host: UIView
    private var events: [Object] = []
    init(host: UIView) { self.host = host }
    private func check(_ condition: Bool, _ label: String) { checks.append(["name": label, "pass": condition]) }
    private func refuses(_ label: String, _ action: () throws -> Void) {
        do { try action(); check(false, label) } catch { check(true, label) }
    }
    private func json(_ object: Any) throws -> String { String(decoding: try JSONSerialization.data(withJSONObject: object, options: [.sortedKeys]), as: UTF8.self) }
    private func root() throws -> URL {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("composer-paste-test-" + UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true); roots.append(url); return url
    }
    private func image() throws -> Data { try Data(contentsOf: Bundle.main.resourceURL!.appendingPathComponent("assets/file-icons/pierre_typescript.png")) }
    private func input(_ bytes: Data? = nil) throws -> T3ComposerPasteInput {
        let file = T3MobileComposerPasteStore.temporaryRoot.appendingPathComponent(UUID().uuidString + ".png")
        try FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
        try (bytes ?? image()).write(to: file, options: .atomic); temporaries.append(file)
        return .init(leaseId: UUID().uuidString.lowercased(), file: file)
    }
    private func request(_ inputs: [T3ComposerPasteInput], remaining: Int = 100) throws -> T3ComposerPasteRequest {
        try T3ComposerPasteRequest(["op": "composerEditorPasteFiles", "action": "stage", "generation": 1,
            "operationId": UUID().uuidString.lowercased(), "identity": ["owner": "owned", "editorId": "editor", "routeVisit": "route", "renderEpoch": "epoch", "mountId": "mount"],
            "richEventId": UUID().uuidString, "target": ["origin": "https://fixture.test", "environmentId": "env", "draftKey": "env:thread", "incarnation": UUID().uuidString, "capturedRevision": 0],
            "sources": inputs.map { ["kind": "editorImage", "leaseId": $0.leaseId] }, "remaining": remaining])
    }
    private func transition(_ action: String, _ staged: Object) throws -> T3ComposerPasteRequest {
        let proof = staged["proof"] as! Object
        return try T3ComposerPasteRequest(["op": "composerEditorPasteFiles", "action": action, "generation": 1, "operationId": proof["operationId"]!, "proof": proof])
    }
    private func basePreferences() -> Object { ["version": 1, "drafts": ["env:thread": ""], "snapshotDrafts": [String: Any](), "composerFiles": [Object](), "pending": [String: Any]()] }
    private func save(_ value: Object, _ owner: T3MobileQueuedEdit) throws { try owner.writePreferences(json(value)) }
    private func publish(_ staged: Object) throws -> Object {
        let proof = staged["proof"] as! Object, target = proof["target"] as! Object, key = target["draftKey"] as! String
        var value = basePreferences(), document = target
        document.removeValue(forKey: "capturedRevision"); document["revision"] = 1; document["threadId"] = "thread"; document["selection"] = NSNull()
        let rowKey = try json([target["origin"]!, target["environmentId"]!, key])
        value["drafts"] = [key: "image"]
        value["snapshotDrafts"] = [key: (proof["files"] as! [Object]).map { ["id": $0["id"]!, "name": $0["name"]!, "mimeType": $0["mimeType"]!, "sizeBytes": $0["sizeBytes"]!] }]
        value["mobileComposerEditor"] = ["version": 1, "documents": [rowKey: document], "sends": [String: Any]()]
        value["composerPasteAdoptions"] = [proof["operationId"] as! String: ["proof": proof, "publication": ["origin": target["origin"]!, "environmentId": target["environmentId"]!, "draftKey": key, "incarnation": target["incarnation"]!, "revision": 1]]]
        return value
    }
    private func file(_ staged: Object, root: URL, index: Int = 0) -> URL {
        root.appendingPathComponent("snapshots/drafts").appendingPathComponent(((staged["proof"] as! Object)["files"] as! [Object])[index]["id"] as! String)
    }
    private func adoption() throws {
        let folder = try root(), owner = T3MobileQueuedEdit(root: folder), original = try input()
        try save(basePreferences(), owner)
        let req = try request([original]), staged = try owner.composerPasteFiles(req, inputs: [original])
        check(staged["status"] as? String == "staged" && FileManager.default.fileExists(atPath: file(staged, root: folder).path), "Stage returns real canonical UUID PNG")
        check(!FileManager.default.fileExists(atPath: original.file.path), "Stage cleans its original owned temporary PNG")
        check(try json(owner.composerPasteFiles(req, inputs: nil)) == json(staged), "Lost stage reply replays exact proof and UUID")
        let id = file(staged, root: folder).lastPathComponent
        check(try owner.removeAttachment(["op": "snapshotDraftRemove", "id": id])["retained"] as? Bool == true, "Generic remover respects unadopted paste hold")
        var prefs = try publish(staged)
        var invalid = prefs, registry = prefs["mobileComposerEditor"] as! Object, rows = registry["documents"] as! [String: Object]
        rows[rows.keys.first!]?["blocked"] = true; registry["documents"] = rows; invalid["mobileComposerEditor"] = registry
        refuses("Blocked document cannot publish new paste marker") { try save(invalid, owner) }
        try save(prefs, owner)
        registry = prefs["mobileComposerEditor"] as! Object; rows = registry["documents"] as! [String: Object]
        rows[rows.keys.first!]?["revision"] = 2; registry["documents"] = rows; prefs["mobileComposerEditor"] = registry; prefs["drafts"] = ["env:thread": "image and typing"]
        try save(prefs, owner)
        check(true, "Historical marker survives later typing revision without old text replay")
        let reopened = T3MobileQueuedEdit(root: folder)
        let adopted = try reopened.composerPasteFiles(transition("adopt", staged), inputs: nil)
        check(adopted["status"] as? String == "adopted", "Cold owner proves and adopts durable historical publication")
        refuses("Retire requires durable adoption marker removal") { _ = try reopened.composerPasteFiles(transition("retire", staged), inputs: nil) }
        prefs.removeValue(forKey: "composerPasteAdoptions"); try save(prefs, reopened)
        check(try reopened.composerPasteFiles(transition("adopt", staged), inputs: nil)["status"] as? String == "adopted", "Adopt reply replay remains idempotent after marker removal")
        check(try reopened.composerPasteFiles(transition("retire", staged), inputs: nil)["status"] as? String == "retired", "Acknowledged terminal retires without canonical deletion")
        check(FileManager.default.fileExists(atPath: file(staged, root: folder).path), "Retiring adopted proof leaves actual draft bytes intact")
        refuses("Old delayed marker cannot return after receipt retirement") { try save(publish(staged), reopened) }
        refuses("Retired operation cannot mint stage authority without issued leases") { _ = try reopened.composerPasteFiles(req, inputs: nil) }
        check(try reopened.composerPasteFiles(transition("retire", staged), inputs: nil)["status"] as? String == "retired", "Unknown retire is a harmless no-op")
        check(try reopened.removeAttachment(["op": "snapshotDraftRemove", "id": id])["retained"] as? Bool == true, "Ordinary draft still protects retired paste bytes")
    }
    private func partialAndDiscard() throws {
        let folder = try root(), owner = T3MobileQueuedEdit(root: folder)
        try save(basePreferences(), owner)
        let inputs = try [input(), input(Data("not png".utf8)), input(Data(repeating: 0, count: T3MobileAttachments.maxImageBytes + 1)), input(), input()]
        let staged = try owner.composerPasteFiles(request(inputs, remaining: 4), inputs: inputs)
        let files = (staged["proof"] as! Object)["files"] as! [Object], skipped = staged["skipped"] as! [Object]
        check(files.count == 2 && skipped.map { $0["reason"] as! String } == ["unreadable", "too-large", "excess"], "Source loop keeps valid images and skips bad/oversize/excess by original index")
        check(inputs.allSatisfy { !FileManager.default.fileExists(atPath: $0.file.path) }, "Partial success cleans every owned original including skipped images")
        let discarded = try owner.composerPasteFiles(transition("discard", staged), inputs: nil)
        check(discarded["status"] as? String == "discarded" && files.allSatisfy { !FileManager.default.fileExists(atPath: folder.appendingPathComponent("snapshots/drafts/" + ($0["id"] as! String)).path) }, "Own discard exemption removes only its unadopted canonical files")
        check(try owner.composerPasteFiles(transition("discard", staged), inputs: nil)["status"] as? String == "discarded", "Lost discard reply is idempotent")
        refuses("Discarded operation rejects delayed preference publication") { try save(publish(staged), owner) }
        _ = try owner.composerPasteFiles(transition("retire", staged), inputs: nil)
        let emptyInputs = try [input(), input()]
        let empty = try owner.composerPasteFiles(request(emptyInputs, remaining: 0), inputs: emptyInputs)
        check(((empty["proof"] as! Object)["files"] as! [Object]).isEmpty, "No remaining source slots produces an empty successful stage")
        check(emptyInputs.allSatisfy { !FileManager.default.fileExists(atPath: $0.file.path) }, "Empty source stage cleans all excess originals")
        check(T3MobileComposerPasteStore.maxUnresolvedBytes >= 100 * T3MobileAttachments.maxImageBytes, "Unresolved budget admits a complete100-image source request")
    }
    private func failuresAndRestart() throws {
        for boundary in ["reserve-before", "reserve-after", "first-copy-after", "staged-before", "staged-after"] {
            let folder = try root(), inputs = try [input(), input()]
            var armed = false, fired = false, journalWrites = 0, sawReservation = false
            let owner = T3MobileQueuedEdit(root: folder, replace: { bytes, destination in
                if armed, destination.lastPathComponent == "mobile-composer-paste.json" { journalWrites += 1 }
                let copying = destination.deletingLastPathComponent().lastPathComponent == "drafts"
                if copying {
                    let journal = try JSONSerialization.jsonObject(with: Data(contentsOf: folder.appendingPathComponent("mobile-composer-paste.json"))) as! Object
                    let records = journal["records"] as! [String: Object]
                    sawReservation = records.values.contains { record in
                        record["state"] as? String == "reserved" && ((record["proof"] as? Object)?["files"] as? [Object])?.contains { $0["id"] as? String == destination.lastPathComponent } == true
                    }
                }
                let trigger = armed && !fired && ((boundary.hasPrefix("reserve") && journalWrites == 1 && destination.lastPathComponent == "mobile-composer-paste.json")
                    || (boundary == "first-copy-after" && copying)
                    || (boundary.hasPrefix("staged") && journalWrites == 2 && destination.lastPathComponent == "mobile-composer-paste.json"))
                if trigger && boundary.hasSuffix("before") { fired = true; throw CocoaError(.fileWriteOutOfSpace) }
                try T3MobileQueuedEdit.durableReplace(bytes, destination)
                if trigger { fired = true; throw CocoaError(.fileWriteUnknown) }
            })
            try save(basePreferences(), owner); armed = true
            let req = try request(inputs)
            refuses("Injected crash boundary \(boundary) is reported") { _ = try owner.composerPasteFiles(req, inputs: inputs) }
            check(fired, "Failure injection reached exact boundary \(boundary)")
            let reopened = T3MobileQueuedEdit(root: folder)
            let recovered = try reopened.composerPasteFiles(req, inputs: boundary == "reserve-before" ? inputs : nil)
            check(recovered["status"] as? String == "staged", "Restart safely resumes same import at \(boundary)")
            if boundary != "reserve-before" && boundary != "reserve-after" { check(sawReservation, "Canonical write had durable destination reservation at \(boundary)") }
            for f in (recovered["proof"] as! Object)["files"] as! [Object] {
                let bytes = try Data(contentsOf: folder.appendingPathComponent("snapshots/drafts/" + (f["id"] as! String)))
                check(bytes == (try image()), "Recovered exact original PNG at \(boundary)")
            }
        }
    }
    private func failedCanonicalSyncReplay() throws {
        let folder = try root(), original = try input(), req = try request([original])
        let journalURL = folder.appendingPathComponent("mobile-composer-paste.json")
        let drafts = folder.appendingPathComponent("snapshots/drafts")
        var renamed: URL?
        let first = T3MobileComposerPasteStore(root: folder, replace: { bytes, destination in
            if destination.deletingLastPathComponent().standardizedFileURL.path == drafts.standardizedFileURL.path {
                // Model the durableReplace boundary after rename/file sync but before directory sync.
                try bytes.write(to: destination, options: .atomic)
                try T3MobileIncomingShares.sync(destination)
                renamed = destination
                throw CocoaError(.fileWriteUnknown)
            }
            try T3MobileQueuedEdit.durableReplace(bytes, destination)
        })
        refuses("Canonical directory-sync failure after visible rename is reported") { _ = try first.stage(req, inputs: [original]) }
        guard let canonical = renamed else { throw T3ComposerPasteError.recovery }
        func record() throws -> T3MobileComposerPasteStore.Record {
            let journal = try JSONDecoder().decode(T3MobileComposerPasteStore.Journal.self, from: Data(contentsOf: journalURL))
            guard let record = journal.records[req.operationId] else { throw T3ComposerPasteError.recovery }
            return record
        }
        let reserved = try record()
        check(reserved.state == "reserved" && reserved.proof.files.first?.id == canonical.lastPathComponent,
              "Failed canonical sync retains the predetermined reservation")
        check(FileManager.default.fileExists(atPath: original.file.path), "Failed canonical sync retains its source PNG")
        var attempted: [String] = []
        let retry = T3MobileComposerPasteStore(root: folder, synchronize: { url in
            attempted.append(url.path)
            if url.standardizedFileURL.path == drafts.standardizedFileURL.path { throw CocoaError(.fileWriteUnknown) }
            try T3MobileIncomingShares.sync(url)
        })
        refuses("Visible verified canonical replay still refuses a failed directory sync") { _ = try retry.stage(req, inputs: nil) }
        check(attempted == [canonical.path, drafts.path], "Replay synchronizes canonical bytes before its directory")
        check(try record().state == "reserved" && FileManager.default.fileExists(atPath: original.file.path),
              "Failed replay sync cannot publish staged or delete the original")
        var synchronized: [String] = []
        let recovered = T3MobileComposerPasteStore(root: folder, synchronize: { url in
            try T3MobileIncomingShares.sync(url); synchronized.append(url.path)
        })
        let staged = try recovered.stage(req, inputs: nil)
        check(synchronized == [canonical.path, drafts.path, folder.appendingPathComponent("snapshots").path, folder.path],
              "Successful replay syncs canonical file and every directory ancestor before success")
        let finalRecord = try record()
        check(staged["status"] as? String == "staged" && finalRecord.proof == reserved.proof,
              "Durable replay publishes the exact reserved proof without a new UUID")
        check(try Data(contentsOf: canonical) == image(), "Durable replay retains exact canonical PNG bytes")
        check(!FileManager.default.fileExists(atPath: original.file.path), "Only fully synchronized replay removes the source PNG")
    }
    private func corruptAndForeign() throws {
        let folder = try root(), owner = T3MobileQueuedEdit(root: folder), original = try input()
        try save(basePreferences(), owner)
        let req = try request([original]), staged = try owner.composerPasteFiles(req, inputs: [original]), url = file(staged, root: folder)
        let journalURL = folder.appendingPathComponent("mobile-composer-paste.json"), originalJournal = try Data(contentsOf: journalURL)
        var bad = try JSONSerialization.jsonObject(with: originalJournal) as! Object
        var records = bad["records"] as! [String: Object]; records[req.operationId]?["state"] = "adopted"; bad["records"] = records
        try T3MobileQueuedEdit.durableReplace(JSONSerialization.data(withJSONObject: bad), journalURL)
        refuses("Malformed adopted journal without publication fails closed on generic removal") { _ = try owner.removeAttachment(["op": "snapshotDraftRemove", "id": url.lastPathComponent]) }
        check(FileManager.default.fileExists(atPath: url.path), "Corrupt journal retains canonical bytes")
        try T3MobileQueuedEdit.durableReplace(originalJournal, journalURL)
        var prefs = basePreferences(); prefs["snapshotDrafts"] = "corrupt"
        try T3MobileQueuedEdit.durableReplace(JSONSerialization.data(withJSONObject: prefs), folder.appendingPathComponent("t3-code.json"))
        refuses("Malformed inventory refuses paste discard") { _ = try owner.composerPasteFiles(transition("discard", staged), inputs: nil) }
        check(FileManager.default.fileExists(atPath: url.path), "Malformed inventory retains file and journal")
        try save(basePreferences(), owner)
        var proof = staged["proof"] as! Object, wrong = staged; proof["richEventId"] = UUID().uuidString; wrong["proof"] = proof
        refuses("Foreign proof cannot discard captured operation") { _ = try owner.composerPasteFiles(transition("discard", wrong), inputs: nil) }
        let link = try input(); try FileManager.default.removeItem(at: link.file); try FileManager.default.createSymbolicLink(at: link.file, withDestinationURL: url)
        refuses("Symlink source is an ownership failure rather than imported bytes") { _ = try owner.composerPasteFiles(request([link]), inputs: [link]) }
        check(FileManager.default.fileExists(atPath: url.path), "Foreign canonical target survives symlink refusal")
        let held = try input(), heldStage = try owner.composerPasteFiles(request([held]), inputs: [held])
        var pending = basePreferences()
        pending["snapshotDrafts"] = ["another-draft": [["id": file(heldStage, root: folder).lastPathComponent]]]
        try save(pending, owner)
        refuses("Discard cannot bypass another ordinary draft hold") { _ = try owner.composerPasteFiles(transition("discard", heldStage), inputs: nil) }
        check(FileManager.default.fileExists(atPath: file(heldStage, root: folder).path), "Other draft bytes survive own journal exclusion")
    }
    private func lostCommitReplies() throws {
        for boundary in ["preferences", "adopted", "discarded"] {
            let folder = try root(), original = try input()
            var armed = false, fired = false
            let owner = T3MobileQueuedEdit(root: folder, replace: { bytes, destination in
                try T3MobileQueuedEdit.durableReplace(bytes, destination)
                guard armed && !fired else { return }
                let matching: Bool
                if boundary == "preferences" { matching = destination.lastPathComponent == "t3-code.json" }
                else if destination.lastPathComponent == "mobile-composer-paste.json",
                        let object = try JSONSerialization.jsonObject(with: bytes) as? Object,
                        let records = object["records"] as? [String: Object] { matching = records.values.contains { $0["state"] as? String == boundary } }
                else { matching = false }
                if matching { fired = true; throw CocoaError(.fileWriteUnknown) }
            })
            try save(basePreferences(), owner)
            let staged = try owner.composerPasteFiles(request([original]), inputs: [original])
            if boundary == "preferences" {
                armed = true
                refuses("Lost preference rename reply is reported") { try save(publish(staged), owner) }
            } else {
                if boundary == "adopted" { try save(publish(staged), owner) }
                armed = true
                refuses("Lost terminal \(boundary) reply is reported") { _ = try owner.composerPasteFiles(transition(boundary == "adopted" ? "adopt" : "discard", staged), inputs: nil) }
            }
            check(fired, "Injected lost reply reached \(boundary)")
            let reopened = T3MobileQueuedEdit(root: folder)
            let response = try reopened.composerPasteFiles(transition(boundary == "discarded" ? "discard" : "adopt", staged), inputs: nil)
            check(response["status"] as? String == (boundary == "discarded" ? "discarded" : "adopted"), "Cold replay establishes terminal after \(boundary) reply loss")
            check(FileManager.default.fileExists(atPath: file(staged, root: folder).path) == (boundary != "discarded"), "Canonical ownership is correct after \(boundary) reply loss")
        }
    }
    private func writerDiscardRace() throws {
        for _ in 0..<4 {
            let folder = try root(), original = try input(), owner = T3MobileQueuedEdit(root: folder)
            try save(basePreferences(), owner)
            let staged = try owner.composerPasteFiles(request([original]), inputs: [original]), prefs = try json(publish(staged)), discard = try transition("discard", staged)
            let resultLock = NSLock(); var saved = false, result: String?
            DispatchQueue.concurrentPerform(iterations: 2) { index in
                if index == 0 {
                    do { try owner.writePreferences(prefs); resultLock.lock(); saved = true; resultLock.unlock() } catch {}
                } else {
                    let answer = try? owner.composerPasteFiles(discard, inputs: nil)
                    resultLock.lock(); result = answer?["status"] as? String; resultLock.unlock()
                }
            }
            let exists = FileManager.default.fileExists(atPath: file(staged, root: folder).path)
            check((saved && result == "adopted" && exists) || (!saved && result == "discarded" && !exists), "Concurrent writer/discard shares one mutex and cannot publish missing bytes")
        }
    }
    private func adoptedCorruptInventories() throws {
        let folder = try root(), original = try input(), owner = T3MobileQueuedEdit(root: folder)
        try save(basePreferences(), owner)
        let staged = try owner.composerPasteFiles(request([original]), inputs: [original]), valid = try publish(staged)
        try save(valid, owner)
        _ = try owner.composerPasteFiles(transition("adopt", staged), inputs: nil)
        let canonical = file(staged, root: folder), preferenceFile = folder.appendingPathComponent("t3-code.json")
        let live = (valid["snapshotDrafts"] as! Object)["env:thread"]!
        let originalMarkers = valid["composerPasteAdoptions"] as! [String: Object], operationId = originalMarkers.keys.first!
        var wrongPublication = originalMarkers, marker = originalMarkers[operationId]!, publication = marker["publication"] as! Object
        publication["revision"] = 9; marker["publication"] = publication; wrongPublication[operationId] = marker
        var wrongProof = originalMarkers; marker = originalMarkers[operationId]!
        var changedProof = marker["proof"] as! Object; changedProof["richEventId"] = UUID().uuidString; marker["proof"] = changedProof; wrongProof[operationId] = marker
        var wrongIntrinsic = originalMarkers; marker = originalMarkers[operationId]!; publication = marker["publication"] as! Object
        publication["incarnation"] = "another-incarnation"; marker["publication"] = publication; wrongIntrinsic[operationId] = marker
        let corruptions: [(String, String, Any)] = [
            ("substituted marker publication", "composerPasteAdoptions", wrongPublication),
            ("substituted marker proof", "composerPasteAdoptions", wrongProof),
            ("wrong marker intrinsic binding", "composerPasteAdoptions", wrongIntrinsic),
            ("whole snapshot dictionary", "snapshotDrafts", "bad"),
            ("sibling poisoned dictionary", "snapshotDrafts", ["env:thread": live, "sibling": "bad"] as Object),
            ("primitive snapshot slot", "snapshotDrafts", ["env:thread": 7]),
            ("null snapshot slot", "snapshotDrafts", ["env:thread": NSNull()]),
            ("nonarray snapshot slot", "snapshotDrafts", ["env:thread": ["id": canonical.lastPathComponent]]),
            ("primitive image row", "snapshotDrafts", ["env:thread": [7]]),
            ("missing image id", "snapshotDrafts", ["env:thread": [["name": "image"]]]),
            ("invalid image id", "snapshotDrafts", ["env:thread": [["id": "invalid"]]]),
            ("nonarray composer files", "composerFiles", "bad"),
            ("mixed composer file row", "composerFiles", [["id": canonical.lastPathComponent], 7] as [Any]),
            ("missing composer file id", "composerFiles", [["name": "file"]]),
            ("invalid composer file id", "composerFiles", [["id": "invalid"]]),
            ("nonobject pending", "pending", "bad"),
            ("nested pending entry", "pending", ["env": "bad"]),
            ("nested pending payload", "pending", ["env": ["method": "send", "payload": "bad"]]),
            ("pending attachment container", "pending", ["env": ["method": "send", "payload": ["attachments": "bad"]]]),
            ("pending primitive attachment", "pending", ["env": ["method": "send", "payload": ["attachments": [7]]]]),
            ("pending attachment missing id", "pending", ["env": ["method": "send", "payload": ["attachments": [["name": "x"]]]]]),
            ("pending attachment empty id", "pending", ["env": ["method": "send", "payload": ["attachments": [["id": ""]]]]]),
            ("initial message container", "pending", ["env": ["method": "launch", "payload": ["initialMessage": "bad"]]]),
            ("initial attachment array", "pending", ["env": ["method": "launch", "payload": ["initialMessage": ["attachments": "bad"]]]]),
            ("initial attachment id", "pending", ["env": ["method": "launch", "payload": ["initialMessage": ["attachments": [["id": 7]]]]]])
        ]
        for (name, field, malformed) in corruptions {
            var broken = valid; broken[field] = malformed
            try T3MobileQueuedEdit.durableReplace(JSONSerialization.data(withJSONObject: broken), preferenceFile)
            refuses("Adopted paste generic removal refuses \(name)") { _ = try owner.removeAttachment(["op": "snapshotDraftRemove", "id": canonical.lastPathComponent]) }
            check(FileManager.default.fileExists(atPath: canonical.path), "Adopted canonical bytes retained for \(name)")
        }
        var wire = valid
        wire["pending"] = ["env": ["method": "any.valid.method", "payload": ["attachments": [["id": "remote-upload-id"]], "initialMessage": ["attachments": [["id": "another-wire-id"]]]]]]
        try T3MobileQueuedEdit.durableReplace(JSONSerialization.data(withJSONObject: wire), preferenceFile)
        check(try owner.removeAttachment(["op": "snapshotDraftRemove", "id": canonical.lastPathComponent])["retained"] as? Bool == true, "Valid nonUUID wire attachment IDs remain supported and ordinary draft holds bytes")
        wire["snapshotDrafts"] = [String: Any](); wire["composerFiles"] = [Object]()
        try T3MobileQueuedEdit.durableReplace(JSONSerialization.data(withJSONObject: wire), preferenceFile)
        check(try owner.removeAttachment(["op": "snapshotDraftRemove", "id": canonical.lastPathComponent])["removed"] as? Bool == true, "Valid pending envelope alone does not invent canonical attachment ownership")
        let unrelated = UUID().uuidString.lowercased(), unrelatedURL = folder.appendingPathComponent("snapshots/drafts/" + unrelated)
        try image().write(to: unrelatedURL)
        var unknown = basePreferences(); unknown["snapshotDrafts"] = "bad"
        try T3MobileQueuedEdit.durableReplace(JSONSerialization.data(withJSONObject: unknown), preferenceFile)
        check(try owner.removeAttachment(["op": "snapshotDraftRemove", "id": unrelated])["removed"] as? Bool == true, "New paste guard leaves unknown UUID legacy behavior unchanged")
    }
    @MainActor private func issuedUIKitLease() async throws {
        let pasteboard = UIPasteboard.general, saved = UIPasteboard.general.items
        defer { pasteboard.items = saved }
        let voice = T3MobileVoiceEditor(), leases = T3MobileComposerPaste()
        let emitter = ExactNativeEvents(fn: { ctx, _, _, bytes, length in
            guard let ctx, let bytes else { return }
            let selfValue = Unmanaged<ComposerPasteUIKitTests>.fromOpaque(ctx).takeUnretainedValue()
            if let value = try? JSONSerialization.jsonObject(with: Data(bytes: bytes, count: Int(length))) as? Object { selfValue.events.append(value) }
        }, ctx: Unmanaged.passUnretained(self).toOpaque(), nonce: 1)
        let port = T3MobileComposerEditor(voice: voice, paste: leases, events: emitter)
        defer { port.destroy(); port.view.removeFromSuperview(); leases.destroy() }
        port.view.frame = CGRect(x: 20, y: 180, width: 600, height: 200); host.addSubview(port.view)
        var control: Object = ["owner": "paste-fixture", "editorId": "paste-editor", "routeVisit": "paste-route", "renderEpoch": "paste-epoch", "mountId": "",
            "acknowledgedEventCount": 0, "ackCommandId": "", "active": true, "editable": true, "readOnly": false,
            "focusIntent": ["serial": "", "attempt": 0, "operation": "none"], "command": NSNull(),
            "document": ["value": "", "selection": ["start": 0, "end": 0], "tokensJson": "[]", "isNativeEcho": false]]
        try port.setProps(["configuration": json(control)])
        control["mountId"] = port.composerIdentity!.mountId; control["acknowledgedEventCount"] = port.composerEventCount
        try port.setProps(["configuration": json(control)])
        let view = port.view.subviews.compactMap { $0 as? T3MobileOwnedComposerView }.first!
        view.textView.becomeFirstResponder(); pasteboard.items = []; pasteboard.image = UIImage(data: try image())
        view.textView.paste(nil)
        for _ in 0..<50 where !events.contains(where: { $0["kind"] as? String == "pasteImages" }) { try await Task.sleep(nanoseconds: 10_000_000) }
        guard let event = events.last(where: { $0["kind"] as? String == "pasteImages" }), let payload = event["payload"] as? Object,
              let files = payload["files"] as? [Object], let first = files.first else { throw T3ComposerPasteError.recovery }
        check(payload["uris"] == nil && first["leaseId"] is String, "Actual UIKit image paste issues opaque lease IDs with diagnostic URI only")
        check((event["editorEvent"] as? Object)?["eventCount"] as? Int == event["eventCount"] as? Int, "Lease message preserves exact foundation observation")
        var requestObject: Object = ["op": "composerEditorPasteFiles", "action": "stage", "generation": 1, "operationId": UUID().uuidString.lowercased(),
            "identity": port.composerIdentity!.json, "richEventId": event["richEventId"]!,
            "target": ["origin": "https://fixture.test", "environmentId": "env", "draftKey": "env:thread", "incarnation": UUID().uuidString, "capturedRevision": 0],
            "sources": files.map { ["kind": "editorImage", "leaseId": $0["leaseId"]!] }, "remaining": 100]
        var foreign = requestObject; foreign["richEventId"] = UUID().uuidString
        refuses("Actual issued lease refuses a different rich event") { _ = try leases.claim(T3ComposerPasteRequest(foreign)) }
        let request = try T3ComposerPasteRequest(requestObject), claimed = try leases.claim(request)
        port.destroy(); port.view.removeFromSuperview()
        check(FileManager.default.fileExists(atPath: URL(string: first["uri"] as! String)!.path), "Synchronous claim survives editor teardown before worker copy")
        let folder = try root(), store = T3MobileQueuedEdit(root: folder); try save(basePreferences(), store)
        let staged = try store.composerPasteFiles(request, inputs: claimed); leases.completed(request)
        check(staged["status"] as? String == "staged", "Claimed real UIKit bytes stage after unmount")
        requestObject["operationId"] = UUID().uuidString.lowercased()
        let stale = try T3ComposerPasteRequest(requestObject)
        check(try leases.claim(stale) == nil, "Consumed lease grants no new stage claim")
        refuses("Unknown disk operation with consumed lease cannot restage") { _ = try store.composerPasteFiles(stale, inputs: nil) }
        check(try store.composerPasteFiles(request, inputs: nil)["status"] as? String == "staged", "Original operation still replays after native lease consumption")
    }
    @MainActor func run() async -> Object {
        for (name, test) in [("adoption", adoption), ("partial/discard", partialAndDiscard), ("crash restart", failuresAndRestart), ("canonical sync replay", failedCanonicalSyncReplay), ("corrupt/foreign", corruptAndForeign), ("lost commit replies", lostCommitReplies), ("writer discard race", writerDiscardRace), ("adopted inventory", adoptedCorruptInventories)] {
            do { try test() } catch { check(false, "\(name) threw \(error)") }
        }
        do { try await issuedUIKitLease() } catch { check(false, "UIKit lease threw \(error)") }
        for url in temporaries + roots { try? FileManager.default.removeItem(at: url) }
        return ["checks": checks, "passed": checks.filter { $0["pass"] as? Bool == true }.count,
            "failed": checks.filter { $0["pass"] as? Bool == false }.count, "pid": ProcessInfo.processInfo.processIdentifier,
            "method": "Actual UIKit and production paste/attachment/QueuedEdit bodies; isolated owned file roots only."]
    }
}
#endif
