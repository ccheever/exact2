#if os(iOS)
// Actual editor registration, coordinator lock, canonical bytes and durable release queue.
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
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
        var failBefore = false, failAfter = false, saves = 0
        let entered = DispatchSemaphore(value: 0), resume = DispatchSemaphore(value: 0)
        var pause = false
        func replace(_ bytes: Data, _ url: URL) throws {
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
    }
    private func port(_ registry: T3MobileComposerFileHolds, owner: String) throws -> T3MobileComposerEditor {
        let port = T3MobileComposerEditor(voice: voice.editor, fileHolds: registry,
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
    private func fixture() throws -> Fixture {
        let disk = Disk(), coordinator = T3MobileQueuedEdit(root: disk.root, replace: disk.replace)
        let registry = T3MobileComposerFileHolds(coordinator: coordinator), id = UUID().uuidString.lowercased()
        let file: [String: Any] = ["id": id, "contextId": "file-context", "draftKey": "env:thread", "environmentId": "env",
            "name": "note.txt", "mimeType": "text/plain", "sizeBytes": 5, "source": "attached", "status": "staged", "attachmentId": ""]
        try FileManager.default.createDirectory(at: disk.root.appendingPathComponent("composer-files"), withIntermediateDirectories: true)
        let url = disk.root.appendingPathComponent("composer-files").appendingPathComponent(id)
        try Data("hello".utf8).write(to: url)
        try json(["composerFiles": [file], "drafts": ["env:thread": ""], "pending": [:]]).write(to: disk.root.appendingPathComponent("t3-code.json"))
        let owner = String(decoding: try json(["ordinary", "https://file.test", "env", 1, "env:thread"]), as: UTF8.self)
        let port = try self.port(registry, owner: owner)
        let request: [String: Any] = ["op": "composerFileHold", "action": "acquire", "generation": 1,
            "identity": port.composerIdentity!.json, "requestId": UUID().uuidString.lowercased(),
            "target": ["origin": "https://file.test", "environmentId": "env", "threadId": "thread", "draftKey": "env:thread"], "file": file]
        return Fixture(disk: disk, coordinator: coordinator, registry: registry, file: file, url: url, port: port, request: request)
    }
    private func perform(_ f: Fixture, _ request: [String: Any]? = nil) async -> [String: Any] {
        await withCheckedContinuation { continuation in f.registry.perform(request ?? f.request) { continuation.resume(returning: $0) } }
    }
    private func release(_ f: Fixture, _ request: [String: Any]? = nil) async -> [String: Any] {
        var request = request ?? f.request; request["action"] = "release"; return await perform(f, request)
    }
    private func remove(_ f: Fixture) throws -> [String: Any] { try f.coordinator.removeAttachment(["op": "composerAttachRemove", "id": f.file["id"]!]) }
    private func saved(_ f: Fixture, _ files: Any) throws {
        try f.coordinator.writePreferences(String(decoding: json(["composerFiles": files, "drafts": ["env:thread": ""], "pending": [:]]), as: UTF8.self))
    }
    private func receipt(_ reply: [String: Any]) -> [String: Any] { (reply["value"] as? [String: Any])?["receipt"] as? [String: Any] ?? [:] }
    private func cleanup(_ f: Fixture) { f.registry.destroy(); f.port.destroy(); f.port.view.removeFromSuperview() }
    private func settle() async { try? await Task.sleep(nanoseconds: 100_000_000) }
    func run() async -> [String: Any] {
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
            await settle(); try saved(f, []); _ = try f.coordinator.releaseAttachments()
            check(!FileManager.default.fileExists(atPath: f.url.path), "ended owner cleanup uses guarded candidate drain")
        }
        await attempt("owner ends before delayed worker starts") {
            let f = try fixture(); defer { cleanup(f) }; f.disk.pause = true
            let second = try port(f.registry, owner: f.port.composerIdentity!.owner); defer { second.destroy(); second.view.removeFromSuperview() }
            var request = f.request; request["identity"] = second.composerIdentity!.json; request["requestId"] = UUID().uuidString.lowercased()
            async let first = perform(f)
            let entered = await Task.detached { f.disk.entered.wait(timeout: .now() + 3) == .success }.value
            check(entered, "first worker blocks serial queue")
            var pending: [String: Any]?
            f.registry.perform(request) { pending = $0 }
            second.destroy(); f.disk.resume.signal(); _ = await first
            for _ in 0..<30 where pending == nil { await settle() }
            check(pending?["ok"] as? Bool == false, "claimed work queued before retirement refuses when it starts later")
        }
        await attempt("ended uncertainty") {
            let f = try fixture(); defer { cleanup(f) }; f.disk.failBefore = true
            _ = await perform(f); try saved(f, []); f.port.destroy(); await settle()
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
        voice.destroy()
        return ["passed": checks.filter { $0["pass"] as? Bool == true }.count, "failed": checks.filter { $0["pass"] as? Bool == false }.count, "checks": checks]
    }
}
#endif
